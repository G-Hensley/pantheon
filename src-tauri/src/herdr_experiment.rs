//! Opt-in Linux spike: a verified framed socket attaches to Herdr-owned programs.
//! No Pantheon MCP server, task engine, worktree cleanup or credential transfer.
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{
    ipc::{Channel, Response},
    Emitter, Manager,
};

type Result<T> = std::result::Result<T, String>;
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

struct Attachment {
    generation: String,
    channel: u32,
    writer: Arc<Mutex<UnixStream>>,
}
pub struct Engine {
    root: PathBuf,
    codex: PathBuf,
    runtime_pid: u32,
    runtime_core: Value,
    attachments: Mutex<HashMap<String, Attachment>>,
}

impl Engine {
    fn new(root: PathBuf, binary: PathBuf, codex: PathBuf) -> Result<Arc<Self>> {
        for ancestor in root.ancestors().skip(1) {
            let meta = fs::metadata(ancestor).map_err(error)?;
            let sticky_system_directory =
                meta.uid() == 0 && meta.permissions().mode() & 0o1000 != 0;
            if (meta.uid() != 0 && meta.uid() != unsafe { libc::geteuid() })
                || (meta.permissions().mode() & 0o022 != 0 && !sticky_system_directory)
            {
                return Err("Experiment path has an untrusted writable ancestor".into());
            }
        }
        let metadata = fs::symlink_metadata(&root).map_err(error)?;
        if !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o077 != 0
            || fs::canonicalize(&root).map_err(error)? != root
        {
            return Err("Experiment root must be a canonical, owner-only directory".into());
        }
        for name in ["home", "state", "config", "runtime", "codex", "work"] {
            let path = root.join(name);
            let metadata = fs::symlink_metadata(&path).map_err(error)?;
            if !metadata.is_dir()
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.permissions().mode() & 0o077 != 0
                || fs::canonicalize(&path).map_err(error)? != path
            {
                return Err("Unsafe experiment subdirectory".into());
            }
        }
        let receipt: Value =
            serde_json::from_slice(&fs::read(root.join("runtime.json")).map_err(error)?)
                .map_err(error)?;
        if receipt["environment"] != "empty-profile-v1"
            || receipt["binary"]
                != fs::canonicalize(&binary)
                    .map_err(error)?
                    .to_string_lossy()
                    .as_ref()
        {
            return Err("Runtime must be created by the clean fixture launcher".into());
        }
        let runtime_pid = receipt["pid"]
            .as_u64()
            .and_then(|p| u32::try_from(p).ok())
            .ok_or("Invalid runtime PID")?;
        let engine = Arc::new(Self {
            root,
            codex,
            runtime_pid,
            runtime_core: receipt["core"].clone(),
            attachments: Mutex::new(HashMap::new()),
        });
        let hello = engine.rpc("hello", json!({"generation": 1}))?;
        if hello["generation"] != 1
            || hello["version"] != "0.1.21"
            || hello["core"] != receipt["core"]
        {
            return Err("This experiment requires Herdr Core 0.1.21, generation 1".into());
        }
        Ok(engine)
    }

    fn connect_verified(&self) -> Result<UnixStream> {
        // Every control and data connection verifies the peer. Never auto-start.
        let socket = self.root.join("core.sock");
        let metadata = fs::symlink_metadata(&socket).map_err(error)?;
        if !metadata.file_type().is_socket() || metadata.uid() != unsafe { libc::geteuid() } {
            return Err("Refusing foreign or symlinked runtime socket".into());
        }
        let stream = UnixStream::connect(socket).map_err(error)?;
        let mut peer = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // Linux peer credentials bind the connection to the fixture launcher receipt.
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut peer as *mut libc::ucred).cast(),
                &mut length,
            )
        };
        if result != 0
            || peer.uid != unsafe { libc::geteuid() }
            || peer.pid as u32 != self.runtime_pid
        {
            return Err("Refusing a replaced or foreign runtime process".into());
        }
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .map_err(error)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(error)?;
        Ok(stream)
    }

    fn rpc(&self, method: &str, params: Value) -> Result<Value> {
        let mut stream = self.connect_verified()?;
        serde_json::to_writer(
            &mut stream,
            &json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}),
        )
        .map_err(error)?;
        stream.write_all(b"\n").map_err(error)?;
        let mut line = String::new();
        BufReader::new(stream)
            .take(16 * 1024 * 1024)
            .read_line(&mut line)
            .map_err(error)?;
        let reply: Value = serde_json::from_str(&line).map_err(error)?;
        if let Some(e) = reply.get("error") {
            return Err(e.to_string());
        }
        reply
            .get("result")
            .cloned()
            .ok_or_else(|| "Missing RPC result".into())
    }

    fn binding_path(&self, id: &str) -> Result<PathBuf> {
        if !matches!(id, "shell" | "codex") {
            return Err("Unknown experiment pane".into());
        }
        Ok(self.root.join(format!("{id}.json")))
    }

    fn argv(&self, id: &str) -> Result<Vec<String>> {
        match id {
            "shell" => Ok(vec![
                "/bin/bash".into(),
                "--noprofile".into(),
                "--norc".into(),
            ]),
            "codex" => Ok(vec![
                self.codex.to_string_lossy().into_owned(),
                "--no-alt-screen".into(),
            ]),
            _ => Err("Unknown experiment pane".into()),
        }
    }

    fn verify(&self, id: &str, saved: &Value, actual: &Value) -> Result<()> {
        for key in ["address", "created_ms", "pid", "argv", "cwd", "labels"] {
            if actual.get(key).is_none() || saved[key] != actual[key] {
                return Err(format!("Refusing reattachment: {key} changed"));
            }
        }
        if actual["state"] != "live"
            || actual["pid"].as_u64().is_none()
            || actual["argv"] != json!(self.argv(id)?)
            || actual["cwd"] != self.root.join("work").to_string_lossy().as_ref()
            || actual["labels"]["pantheon-experiment"] != self.root.to_string_lossy().as_ref()
            || actual["keep"] != false
        {
            return Err("Refusing changed, finished or unowned fixture process".into());
        }
        Ok(())
    }

    fn terminal(&self, id: &str, rows: u16, cols: u16) -> Result<Value> {
        let path = self.binding_path(id)?;
        if path.exists() {
            let saved: Value =
                serde_json::from_slice(&fs::read(path).map_err(error)?).map_err(error)?;
            let actual = self.rpc("terminal.get", json!({"terminal": saved["address"]}))?;
            self.verify(id, &saved, &actual)?;
            return Ok(actual);
        }
        // A name collision refuses an orphaned create instead of adopting it.
        // No `keep`: a replacement shell must never masquerade as the same agent.
        let record = self.rpc(
            "terminal.run",
            json!({
                "argv": self.argv(id)?, "name": format!("pantheon-{id}"),
                "cwd": self.root.join("work"), "no_parent": true, "no_notify": true,
                "keep": false, "size": {"rows": rows, "cols": cols},
                "labels": {"pantheon-experiment": self.root}
            }),
        )?;
        self.verify(id, &record, &record)?;
        let temporary = path.with_extension("tmp");
        fs::write(
            &temporary,
            serde_json::to_vec_pretty(&record).map_err(error)?,
        )
        .map_err(error)?;
        fs::rename(temporary, path).map_err(error)?;
        Ok(record)
    }

    fn attach(
        self: &Arc<Self>,
        id: &str,
        rows: u16,
        cols: u16,
        output: impl Fn(Vec<u8>) -> bool + Send + 'static,
        ended: impl Fn(bool) + Send + 'static,
    ) -> Result<Value> {
        if rows == 0 || cols == 0 {
            return Err("Empty terminal size".into());
        }
        let mut map = self.attachments.lock().map_err(error)?;
        if map.contains_key(id) {
            return Err("Detach the old viewer first".into());
        }
        let record = self.terminal(id, rows, cols)?;
        let mut stream = self.connect_verified()?;
        stream.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"hello\",\"params\":{\"generation\":1,\"framed\":true}}\n").map_err(error)?;
        let mut reader = BufReader::new(stream.try_clone().map_err(error)?);
        let mut line = String::new();
        (&mut reader)
            .take(64 * 1024)
            .read_line(&mut line)
            .map_err(error)?;
        let hello: Value = serde_json::from_str(&line).map_err(error)?;
        if hello["result"]["generation"] != 1
            || hello["result"]["version"] != "0.1.21"
            || hello["result"]["core"] != self.runtime_core
        {
            return Err("Data connection did not match the verified runtime".into());
        }
        frame(
            &mut stream,
            0,
            0,
            &serde_json::to_vec(&json!({
                "jsonrpc": "2.0", "id": 1, "method": "terminal.attach",
                "params": {"terminal": record["address"], "history": 5000,
                           "rows": rows, "cols": cols, "observe": false}
            }))
            .map_err(error)?,
        )?;
        // Core queues the attach response before stream data. Refuse unexpected control.
        let (control, kind, payload) = read_frame(&mut reader)?;
        if control != 0 || kind != 0 {
            return Err("Unexpected attach response".into());
        }
        let reply: Value = serde_json::from_slice(&payload).map_err(error)?;
        if let Some(error) = reply.get("error") {
            return Err(error.to_string());
        }
        let channel = reply["result"]["channel"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n != 0)
            .ok_or("Missing attach channel")?;
        stream.set_read_timeout(None).map_err(error)?;
        let writer = Arc::new(Mutex::new(stream));
        let generation = uuid::Uuid::new_v4().to_string();
        map.insert(
            id.into(),
            Attachment {
                generation: generation.clone(),
                channel,
                writer: writer.clone(),
            },
        );
        let (engine, id, address) = (Arc::clone(self), id.to_string(), record["address"].clone());
        std::thread::spawn(move || {
            while let Ok((incoming, kind, payload)) = read_frame(&mut reader) {
                if incoming != channel {
                    break;
                }
                match kind {
                    0 => {
                        let consumed = payload.len() as u32;
                        if !output(payload) {
                            break;
                        }
                        let Ok(mut stream) = writer.lock() else { break };
                        if frame(&mut *stream, channel, 1, &consumed.to_le_bytes()).is_err() {
                            break;
                        }
                    }
                    2 => break,
                    _ => break,
                }
            }
            // An old viewer cannot remove or report the state of a new one.
            let removed = {
                let mut map = engine.attachments.lock().unwrap();
                if map.get(&id).is_some_and(|a| a.generation == generation) {
                    map.remove(&id)
                } else {
                    None
                }
            };
            if let Some(attachment) = removed {
                let _ = attachment
                    .writer
                    .lock()
                    .unwrap()
                    .shutdown(std::net::Shutdown::Both);
                let finished = engine
                    .rpc("terminal.get", json!({"terminal": address}))
                    .is_ok_and(|t| t["state"] == "finished");
                ended(finished);
            }
        });
        Ok(record)
    }

    fn detach(&self, id: &str) -> Result<()> {
        self.binding_path(id)?;
        let attachment = self.attachments.lock().map_err(error)?.remove(id);
        if let Some(a) = attachment {
            let _ = a
                .writer
                .lock()
                .map_err(error)?
                .shutdown(std::net::Shutdown::Both);
        }
        Ok(())
    }
    fn write(&self, id: &str, data: &[u8]) -> Result<()> {
        let mut map = self.attachments.lock().map_err(error)?;
        let a = map
            .get_mut(id)
            .ok_or("Viewer disconnected; reattach explicitly")?;
        let mut writer = a.writer.lock().map_err(error)?;
        for chunk in data.chunks(64 * 1024) {
            frame(&mut *writer, a.channel, 0, chunk)?;
        }
        Ok(())
    }
    fn resize(&self, id: &str, rows: u16, cols: u16) -> Result<()> {
        if rows == 0 || cols == 0 {
            return Err("Empty terminal size".into());
        }
        let map = self.attachments.lock().map_err(error)?;
        let a = map.get(id).ok_or("Viewer disconnected")?;
        let mut writer = a.writer.lock().map_err(error)?;
        let mut size = cols.to_le_bytes().to_vec();
        size.extend(rows.to_le_bytes());
        frame(&mut *writer, a.channel, 3, &size)
    }

    fn terminate(&self, id: &str) -> Result<()> {
        let saved: Value =
            serde_json::from_slice(&fs::read(self.binding_path(id)?).map_err(error)?)
                .map_err(error)?;
        let actual = self.rpc("terminal.get", json!({"terminal": saved["address"]}))?;
        self.verify(id, &saved, &actual)?;
        self.rpc("terminal.kill", json!({"terminal": saved["address"]}))?;
        Ok(())
    }
}

const MAX_FRAME: usize = 16 * 1024 * 1024;
fn frame(writer: &mut impl Write, channel: u32, kind: u8, payload: &[u8]) -> Result<()> {
    if payload.len() > MAX_FRAME {
        return Err("Frame exceeds protocol limit".into());
    }
    let mut header = channel.to_le_bytes().to_vec();
    header.push(kind);
    header.extend((payload.len() as u32).to_le_bytes());
    writer.write_all(&header).map_err(error)?;
    writer.write_all(payload).map_err(error)
}
fn read_frame(reader: &mut impl Read) -> Result<(u32, u8, Vec<u8>)> {
    let mut header = [0u8; 9];
    reader.read_exact(&mut header).map_err(error)?;
    let channel = u32::from_le_bytes(header[..4].try_into().unwrap());
    let length = u32::from_le_bytes(header[5..].try_into().unwrap()) as usize;
    if length > MAX_FRAME {
        return Err("Frame exceeds protocol limit".into());
    }
    let mut payload = vec![0; length];
    reader.read_exact(&mut payload).map_err(error)?;
    Ok((channel, header[4], payload))
}

#[tauri::command]
fn spawn_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Engine>>,
    session_id: String,
    channel: Channel<Response>,
    rows: u16,
    cols: u16,
) -> Result<()> {
    let end_app = app.clone();
    let end_id = session_id.clone();
    let record = state.inner().attach(
        &session_id,
        rows,
        cols,
        move |bytes| channel.send(Response::new(bytes)).is_ok(),
        move |finished| {
            let _ = end_app.emit(
                "herdr-connection",
                if finished {
                    "Runtime process ended"
                } else {
                    "Viewer disconnected; process state may still be live. Reattach explicitly."
                },
            );
            if finished {
                let _ = end_app.emit("session-exited", &end_id);
            }
        },
    )?;
    let _ = app.emit(
        "herdr-connection",
        format!(
            "Connected to {} | PID {} | MCP unavailable",
            record["address"], record["pid"]
        ),
    );
    Ok(())
}
#[tauri::command]
fn write_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Engine>>,
    session_id: String,
    data: String,
) -> Result<()> {
    let result = state.write(&session_id, data.as_bytes());
    if let Err(ref message) = result {
        let _ = app.emit("herdr-connection", message);
    }
    result
}
#[tauri::command]
fn resize_session(
    state: tauri::State<'_, Arc<Engine>>,
    session_id: String,
    rows: u16,
    cols: u16,
) -> Result<()> {
    state.resize(&session_id, rows, cols)
}
#[tauri::command]
fn detach_session(state: tauri::State<'_, Arc<Engine>>, session_id: String) -> Result<()> {
    state.detach(&session_id)
}
#[tauri::command]
fn kill_session(state: tauri::State<'_, Arc<Engine>>, session_id: String) -> Result<()> {
    state.terminate(&session_id)
}

pub fn run() {
    let get = |key| PathBuf::from(std::env::var_os(key).expect("Missing experiment environment"));
    let engine = Engine::new(
        get("PANTHEON_HERDR_ROOT"),
        get("PANTHEON_HERDR_BIN"),
        get("PANTHEON_HERDR_CODEX"),
    )
    .expect("Private Herdr runtime unavailable");
    tauri::Builder::default()
        .manage(engine)
        .on_page_load(|webview, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                // Fixed synthetic UI checks only. Never load arbitrary script paths.
                if let Ok(mode) = std::env::var("PANTHEON_HERDR_PROBE") {
                    if matches!(mode.as_str(), "shell-start" | "shell-resume" | "codex") {
                        let script = format!(
                            "{}\nrunHerdrProbe({});",
                            include_str!("../../experiments/herdr/dom_probe.js"),
                            json!(mode)
                        );
                        let _ = webview.eval(&script);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            spawn_session,
            write_session,
            resize_session,
            detach_session,
            kill_session
        ])
        .build(tauri::generate_context!())
        .expect("Experiment app build failed")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                let state = app.state::<Arc<Engine>>();
                for id in ["shell", "codex"] {
                    let _ = state.detach(id);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_rejects_oversize_and_truncated_payloads() {
        let mut bytes = Vec::new();
        frame(&mut bytes, 7, 0, "λ界🙂".as_bytes()).unwrap();
        assert_eq!(
            read_frame(&mut bytes.as_slice()).unwrap(),
            (7, 0, "λ界🙂".as_bytes().to_vec())
        );
        bytes.pop();
        assert!(read_frame(&mut bytes.as_slice()).is_err());
        let mut header = vec![0; 5];
        header.extend(((MAX_FRAME + 1) as u32).to_le_bytes());
        assert!(read_frame(&mut header.as_slice())
            .unwrap_err()
            .contains("limit"));
    }
    #[test]
    fn rejects_public_or_symlinked_root_before_connecting() {
        let temp = tempfile::tempdir().unwrap();
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(Engine::new(temp.path().into(), "/hc".into(), "/codex".into()).is_err());
    }

    #[test]
    fn refuses_symlinked_socket_and_unknown_pane() {
        let temp = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/tmp/not-the-fixture", temp.path().join("core.sock")).unwrap();
        let engine = Engine {
            root: temp.path().into(),
            codex: "/codex".into(),
            runtime_pid: 1,
            runtime_core: Value::Null,
            attachments: Mutex::new(HashMap::new()),
        };
        assert!(engine
            .rpc("hello", json!({}))
            .unwrap_err()
            .contains("symlinked"));
        assert!(engine.binding_path("../shell").is_err());
    }

    #[test]
    #[ignore = "requires a fresh private runtime from experiments/herdr/runtime.py"]
    fn live_bridge_persists_orders_buffers_resizes_and_terminates() {
        use std::sync::mpsc;
        use std::time::Instant;
        let root = PathBuf::from(std::env::var("PANTHEON_HERDR_TEST_ROOT").unwrap());
        let new = || {
            Engine::new(
                root.clone(),
                "/tmp/pantheon-herdr-dist/bin/herdr-core".into(),
                "/unused-codex".into(),
            )
            .unwrap()
        };
        let engine = new();
        let (tx, rx) = mpsc::channel();
        let (lost_tx, lost_rx) = mpsc::channel();
        let (end_tx, end_rx) = mpsc::channel();
        let first = engine
            .attach(
                "shell",
                24,
                80,
                move |b| tx.send(b).is_ok(),
                move |ended| {
                    let _ = end_tx.send(ended);
                },
            )
            .unwrap();
        let wait = |rx: &mpsc::Receiver<Vec<u8>>, needle: &[u8]| {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut bytes = Vec::new();
            while Instant::now() < deadline {
                if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(100)) {
                    let search_from = bytes.len().saturating_sub(needle.len());
                    bytes.extend(chunk);
                    if bytes[search_from..]
                        .windows(needle.len())
                        .any(|w| w == needle)
                    {
                        return;
                    }
                    assert!(bytes.len() < 16 * 1024 * 1024, "bounded fixture output");
                }
            }
            panic!(
                "output not observed: {}; received {} bytes, tail {:?}",
                String::from_utf8_lossy(needle),
                bytes.len(),
                String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(200)..])
            );
        };
        wait(&rx, b"bash-");
        engine
            .write(
                "shell",
                b"export SPIKE_VALUE=original; printf 'READY_%s\\n' bridge\n",
            )
            .unwrap();
        wait(&rx, b"READY_bridge");
        engine.detach("shell").unwrap();
        drop(engine);
        let engine = new();
        let (tx, rx) = mpsc::channel();
        let second = engine
            .attach(
                "shell",
                24,
                80,
                move |b| tx.send(b).is_ok(),
                move |ended| {
                    let _ = lost_tx.send(ended);
                },
            )
            .unwrap();
        assert_eq!(first["pid"], second["pid"]);
        assert_eq!(first["created_ms"], second["created_ms"]);
        let mut wrong = first.clone();
        wrong["created_ms"] = json!(0);
        assert!(engine.verify("shell", &wrong, &second).is_err());
        wait(&rx, b"READY_bridge");
        engine
            .write("shell", b"printf 'VALUE=%s\\n' \"$SPIKE_VALUE\"\n")
            .unwrap();
        wait(&rx, b"VALUE=original");
        engine.resize("shell", 35, 100).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        engine.write("shell", b"stty size\n").unwrap();
        wait(&rx, b"35 100");
        let fixture = root.join("work/raw.py");
        fs::write(&fixture, "import os,tty,termios\nold=termios.tcgetattr(0)\ntty.setraw(0)\ntry:\n os.write(1,b'RAW_READY\\r\\n')\n expected=b'0123456789abcdef'*65536\n got=bytearray()\n while len(got)<len(expected): got.extend(os.read(0,len(expected)-len(got)))\n assert got==expected\n os.write(1,'\\x1b[32mUNICODE=λ界🙂\\x1b[0m\\r\\n'.encode())\n os.write(1,b'B'*(5*1024*1024)+b'\\r\\nBUFFER_COMPLETE\\r\\n')\nfinally: termios.tcsetattr(0,termios.TCSANOW,old)\n").unwrap();
        engine
            .write(
                "shell",
                format!("python3 {}\n", fixture.display()).as_bytes(),
            )
            .unwrap();
        wait(&rx, b"RAW_READY");
        let paste = b"0123456789abcdef".repeat(65536);
        for chunk in paste.chunks(4093) {
            engine.write("shell", chunk).unwrap();
        }
        wait(&rx, "\x1b[32mUNICODE=λ界🙂\x1b[0m".as_bytes());
        wait(&rx, b"BUFFER_COMPLETE");
        // Unexpected viewer death is not runtime process death.
        engine
            .attachments
            .lock()
            .unwrap()
            .get_mut("shell")
            .unwrap()
            .writer
            .lock()
            .unwrap()
            .shutdown(std::net::Shutdown::Both)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !engine.attachments.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let actual = engine
            .rpc("terminal.get", json!({"terminal": first["address"]}))
            .unwrap();
        engine.verify("shell", &first, &actual).unwrap();
        assert!(!lost_rx.recv_timeout(Duration::from_secs(5)).unwrap());
        assert!(
            end_rx.try_recv().is_err(),
            "explicit detach must not report process exit"
        );
        let (exit_tx, exit_rx) = mpsc::channel();
        engine
            .attach(
                "shell",
                35,
                100,
                |_| true,
                move |ended| {
                    let _ = exit_tx.send(ended);
                },
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(600));
        engine.terminate("shell").unwrap();
        assert!(exit_rx.recv_timeout(Duration::from_secs(5)).unwrap());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let actual = engine
                .rpc("terminal.get", json!({"terminal": first["address"]}))
                .unwrap();
            if actual["state"] == "finished" {
                assert_eq!(actual["reason"], "killed");
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            engine.terminal("shell", 24, 80).is_err(),
            "never silently replace a finished process"
        );
    }
}
