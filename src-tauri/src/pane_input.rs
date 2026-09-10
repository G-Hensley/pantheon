//! Pane input policy shared by production delivery and the real-CLI probe.

use std::borrow::Cow;
use std::io::{self, Write};

pub(crate) const LEGACY_MAX_BYTES: usize = 1023;
pub(crate) const CODEX_LINUX_MAX_BYTES: usize = 8192;

pub(crate) fn is_codex(program: &str) -> bool {
    let p = program.to_ascii_lowercase();
    p.trim_end_matches(".exe").trim_end_matches(".cmd") == "codex"
}

/// The existing dispatch contract, shared with the composer probe unchanged.
pub(crate) fn dispatch_prompt(conductor: &str, task_id: &str, task: &str) -> String {
    format!(
        "[pantheon] Task from conductor '{conductor}' (task_id {task_id}): {task} \
         When done, call the pantheon complete_task tool with task_id \"{task_id}\" \
         and your result."
    )
}

/// Inclusive prompt limit, before the existing paste delimiters are added.
/// Only Linux Codex has an exact composer-export proof above the old limit.
pub(crate) fn max_prompt_bytes(program: &str) -> usize {
    limit_on(program, cfg!(target_os = "linux"))
}

fn limit_on(program: &str, linux: bool) -> usize {
    if linux && program.eq_ignore_ascii_case("codex") {
        CODEX_LINUX_MAX_BYTES
    } else {
        LEGACY_MAX_BYTES
    }
}

/// Existing Codex framing; other targets retain their current input behavior.
pub(crate) fn frame<'a>(program: &str, prompt: &'a str) -> Cow<'a, str> {
    if is_codex(program) {
        Cow::Owned(format!("\x1b[200~{prompt}\x1b[201~"))
    } else {
        Cow::Borrowed(prompt)
    }
}

/// The final size guard also protects queued or retargeted tasks. No bytes are
/// written on refusal. Submission remains a separate operation owned by lib.rs.
pub(crate) fn write_prompt(
    writer: &mut (impl Write + ?Sized),
    program: &str,
    prompt: &str,
) -> io::Result<usize> {
    if prompt.len() > max_prompt_bytes(program) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "prompt exceeds the verified pane input limit",
        ));
    }
    let payload = frame(program, prompt);
    writer.write_all(payload.as_bytes())?;
    // Preserve production's existing behavior: a successful write establishes
    // delivery, while a flush error has never meant that no bytes were sent.
    let _ = writer.flush();
    Ok(payload.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_measured_platform_and_cli_get_the_larger_limit() {
        assert_eq!(limit_on("codex", true), 8192);
        for program in ["codex", "opencode", "claude", "bash", "unknown"] {
            assert_eq!(limit_on(program, false), 1023);
        }
        for program in ["opencode", "claude", "bash", "codex.exe", "/tmp/codex"] {
            assert_eq!(limit_on(program, true), 1023);
        }
    }

    #[test]
    fn the_boundary_is_byte_based_and_refusal_writes_nothing() {
        for program in ["codex", "opencode", "claude", "unknown"] {
            let cap = max_prompt_bytes(program);
            let prompt = "é".repeat(cap / 2) + &"x".repeat(cap % 2);
            let mut received = Vec::new();
            assert!(write_prompt(&mut received, program, &prompt).is_ok());
            assert_eq!(received, frame(program, &prompt).as_bytes());
            received.clear();
            assert_eq!(
                write_prompt(&mut received, program, &(prompt + "x"))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidInput
            );
            assert!(received.is_empty());
        }
    }
}
