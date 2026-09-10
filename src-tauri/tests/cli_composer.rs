#![cfg(target_os = "linux")]
//! Opt-in real Codex composer proof. No Enter or model request is sent.
//! See docs/dispatch-composer-evidence.md for prerequisites and invocation.

#[path = "../src/pane_input.rs"]
mod pane_input;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

struct ProcessGroup(u32);

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        // portable-pty creates the child in a new session. Only this fixture's
        // process group is terminated, including its local CLI subprocesses.
        unsafe { libc::kill(-(self.0 as i32), libc::SIGKILL) };
    }
}

fn collect(rx: &mpsc::Receiver<Vec<u8>>, writer: &mut dyn Write, output: &mut Vec<u8>) {
    if let Ok(bytes) = rx.recv_timeout(Duration::from_millis(100)) {
        output.extend_from_slice(&bytes);
        // A real terminal responds to cursor/device queries during startup.
        if bytes.windows(4).any(|b| b == b"\x1b[6n") {
            writer.write_all(b"\x1b[1;1R").unwrap();
        }
        if bytes.windows(3).any(|b| b == b"\x1b[c") {
            writer.write_all(b"\x1b[?1;2c").unwrap();
        }
    }
}

fn capture_composer(cwd: &Path, prompt: &str) -> Vec<u8> {
    let scratch = tempfile::tempdir().unwrap();
    let capture = scratch.path().join("composer.txt");
    let editor = scratch.path().join("capture-editor");
    fs::write(&editor, "#!/bin/sh\ncp -- \"$1\" \"$PANTHEON_CAPTURE.tmp\" && mv -- \"$PANTHEON_CAPTURE.tmp\" \"$PANTHEON_CAPTURE\"\n").unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o700)).unwrap();
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut writer = pair.master.take_writer().unwrap();
    let mut command = CommandBuilder::new("codex");
    command.args(["--no-alt-screen", "-s", "read-only", "-a", "never"]);
    command.cwd(cwd);
    command.env("EDITOR", &editor);
    command.env("VISUAL", &editor);
    command.env("PANTHEON_CAPTURE", &capture);
    command.env("TERM", "xterm-256color");
    let mut child = pair.slave.spawn_command(command).unwrap();
    let pid = child.process_id().unwrap();
    let group = ProcessGroup(pid);
    drop(pair.slave);

    // Even a blocked PTY write is bounded. Do not signal a reused PID after
    // cleanup: the child stays unreaped until this watchdog has been joined.
    let (cancel_tx, cancel_rx) = mpsc::channel();
    let watchdog = thread::spawn(move || {
        if matches!(
            cancel_rx.recv_timeout(Duration::from_secs(60)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
        }
    });
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = [0; 65536];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut output = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(45);
        while !output.windows(4).any(|b| b == b"Tip:") {
            assert!(
                Instant::now() < deadline,
                "Codex composer not ready; no trust/config change is permitted by this probe"
            );
            assert!(
                child.try_wait().unwrap().is_none(),
                "Codex exited before composer readiness"
            );
            collect(&rx, writer.as_mut(), &mut output);
        }
        // Let initial screen rendering settle, without interpreting it as
        // evidence of payload delivery. Only the editor export proves that.
        let settle = Instant::now() + Duration::from_secs(1);
        while Instant::now() < settle {
            collect(&rx, writer.as_mut(), &mut output);
        }
        let wire_bytes = pane_input::write_prompt(writer.as_mut(), "codex", prompt).unwrap();
        assert_eq!(wire_bytes, prompt.len() + 12);
        let settle = Instant::now() + Duration::from_secs(1);
        while Instant::now() < settle {
            collect(&rx, writer.as_mut(), &mut output);
        }
        writer.write_all(b"\x07").unwrap(); // Codex's external editor, never Enter.
        let deadline = Instant::now() + Duration::from_secs(5);
        while !capture.exists() {
            assert!(Instant::now() < deadline, "composer export unavailable");
            collect(&rx, writer.as_mut(), &mut output);
        }
        fs::read(&capture).unwrap()
    }));
    let _ = cancel_tx.send(());
    watchdog.join().unwrap();
    drop(group);
    let _ = child.wait();
    result.unwrap()
}

fn fixture(total_bytes: usize, unicode: bool) -> String {
    let id = "0123456789abcdef0123456789abcdef";
    let wrapper = pane_input::dispatch_prompt("fixture", id, "");
    let room = total_bytes - wrapper.len();
    let mut brief = "BEGIN-FIDELITY|".to_string();
    let token = if unicode { "é界🙂|" } else { "abcdefg|" };
    for offset in 0..total_bytes {
        let next = format!("{offset:08}|{token}");
        if brief.len() + next.len() > room {
            break;
        }
        brief.push_str(&next);
    }
    brief.push_str(&"x".repeat(room - brief.len()));
    pane_input::dispatch_prompt("fixture", id, &brief)
}

#[test]
#[ignore = "launches the installed Codex CLI; requires an already trusted cwd and normal local session storage"]
fn real_codex_composer_preserves_the_complete_production_payload() {
    let cwd = std::env::var_os("PANTHEON_CLI_TEST_CWD").expect(
        "set PANTHEON_CLI_TEST_CWD to an existing trusted checkout; this probe never grants trust",
    );
    let version = std::process::Command::new("codex")
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success());
    println!(
        "{}linux PTY; editor export before submission",
        String::from_utf8_lossy(&version.stdout)
    );
    println!("encoding requested wire received leading_loss complete_equal");
    for unicode in [false, true] {
        for size in [
            1023,
            1024,
            2048,
            2645,
            4096,
            pane_input::CODEX_LINUX_MAX_BYTES,
        ] {
            let prompt = fixture(size, unicode);
            assert_eq!(prompt.len(), size);
            let got = capture_composer(Path::new(&cwd), &prompt);
            if let Some(directory) = std::env::var_os("PANTHEON_CLI_EVIDENCE_DIR") {
                let directory = Path::new(&directory);
                fs::create_dir_all(directory).unwrap();
                let name = format!("{}-{size}", if unicode { "utf8" } else { "ascii" });
                fs::write(directory.join(format!("{name}.expected")), &prompt).unwrap();
                fs::write(directory.join(format!("{name}.received")), &got).unwrap();
            }
            let leading_loss = got
                .first()
                .and_then(|_| prompt.as_bytes().windows(got.len()).position(|w| w == got))
                .map(|n| n.to_string())
                .unwrap_or_else(|| "not-an-exact-suffix".into());
            println!(
                "{} {size} {} {} {leading_loss} {}",
                if unicode { "utf8" } else { "ascii" },
                size + 12,
                got.len(),
                got == prompt.as_bytes()
            );
            assert_eq!(
                got,
                prompt.as_bytes(),
                "composer differed at {size} bytes (unicode={unicode})"
            );
        }
    }
}
