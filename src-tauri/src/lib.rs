// Pantheon — multi-session PTY engine.
//
// Each live agent runs on its own pseudo-terminal (ConPTY). Sessions are keyed
// by a client-supplied id so the frontend can address a pane (write/resize/kill)
// the moment it asks to spawn it, while the streaming command keeps running in
// the background. In later milestones this grows a TOML registry + git-worktree
// isolation; here it's the flat, working core.

pub mod headless;
mod mcp;
mod worktree;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager, State};

/// One live PTY's non-reader handles (the reader is moved into its own thread).
struct SessionHandle {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    /// Present when this session runs isolated in its own git worktree.
    worktree: Option<worktree::Worktree>,
    /// When this session last produced output, on the `mono_ms` clock. Written by
    /// the reader thread, read by `submit_to` to tell when a target has finished
    /// absorbing a pasted prompt. See `SUBMIT_QUIET_MS`.
    last_output: Arc<AtomicU64>,
    /// The program this session launched, so `submit_to` can tell which CLI it is
    /// typing into. Only Codex gets bracketed-paste framing; see `PASTE_START`.
    program: String,
    /// The resolved launch inputs retained for headless work in this pane's
    /// checkout and against this pane's MCP endpoint.
    launch: headless::LaunchSpec,
    /// This session's dedicated MCP listener. Present unless the endpoint failed
    /// and the session fell back to the shared one. Aborted on teardown; without
    /// this, one listener leaks per session for the life of the app.
    server: Option<mcp::SessionServer>,
}

#[derive(Clone)]
struct ActiveHeadless {
    pane_id: String,
    tree: Arc<dyn headless::ProcessTree>,
    cancellation: Arc<HeadlessCancellation>,
}

#[derive(Default)]
struct HeadlessCancellation {
    requested: AtomicBool,
    finished: Mutex<bool>,
    done: std::sync::Condvar,
}

impl HeadlessCancellation {
    fn wait(&self) {
        let mut finished = self.finished.lock().unwrap();
        while !*finished {
            finished = self.done.wait(finished).unwrap();
        }
    }

    fn complete(&self) {
        *self.finished.lock().unwrap() = true;
        self.done.notify_all();
    }
}

/// Process-monotonic millisecond clock, used to time the gap between a prompt
/// and the Enter that submits it.
///
/// Deliberately not `SystemTime`: that can step backwards (NTP correction, a DST
/// change), which would make a target's last-output timestamp look arbitrarily
/// far in the past. Quiet would then read as satisfied instantly and Enter would
/// fire early — precisely the failure this module exists to prevent.
static CLOCK: LazyLock<Instant> = LazyLock::new(Instant::now);

fn mono_ms() -> u64 {
    CLOCK.elapsed().as_millis() as u64
}

/// How long a target's output must be quiet before we accept that it has
/// finished receiving the prompt.
///
/// Sized against documented behaviour in the Codex TUI. Codex infers a paste
/// from keystroke burst timing, and suppresses Enter for 120 ms afterwards —
/// but it re-anchors that window on *every* buffered character, so the deadline
/// keeps moving for as long as bytes are still arriving. An Enter that lands
/// inside the window is absorbed into the composer as a newline, and the agent
/// never sees the task: no error, no timeout signal, just a pane sitting idle
/// with the instruction visible but unsent.
///
/// The previous fixed sleep measured from our own `write_all` returning, which
/// is the wrong anchor — that only means the bytes reached the PTY buffer, not
/// that the target consumed them. For a large payload ConPTY is still delivering
/// well after the call returns, so a 1946-character dispatch reliably lost its
/// Enter. Waiting for the target's *output* to stop instead measures the thing
/// that actually matters, and adapts to payload size and machine load for free.
/// 200 ms clears Codex's 120 ms window with room for delivery lag.
const SUBMIT_QUIET_MS: u64 = 200;

/// Never send Enter sooner than this after the write, however quiet the target
/// looks. A CLI that renders nothing in response to input is "quiet" the instant
/// we finish writing, which tells us nothing at all — this floor is what covers
/// that case. It is also the delay the previous implementation used, so short
/// prompts wait exactly as long as they did before and cannot regress.
///
/// This is the floor for a *small* payload; see `submit_floor_ms`, which raises
/// it in proportion to how much there is to deliver.
const SUBMIT_FLOOR_MS: u64 = 300;

/// Give up waiting for quiet and send Enter regardless. An agent that is already
/// mid-task streams output continuously and would never look quiet, so without a
/// ceiling its dispatch would wait forever. This bounds the wait instead.
///
/// Like the floor, this is the allowance for a small payload; `submit_ceiling_ms`
/// extends it by the same delivery estimate so a big paste is not cut off by a
/// bound that was only ever sized for a small one.
const SUBMIT_CEILING_MS: u64 = 4000;

/// Assumed worst-case rate at which a payload reaches the target, in bytes per
/// millisecond, used to size both bounds against the actual payload.
///
/// This is deliberately far below raw pipe throughput, because the quantity that
/// matters is not how fast ConPTY moves bytes but how fast the target *consumes*
/// them — a TUI that re-renders its composer on every chunk ingests far slower
/// than the pipe delivers. The value is anchored to the live repro: a 1946-byte
/// dispatch was still arriving after 300 ms, so the effective rate is below
/// 1946/300 ≈ 6.5 B/ms. Rounding down to 4 B/ms gives that payload a 486 ms
/// delivery allowance — comfortably past the point it was observed to fail —
/// and keeps the estimate conservative for slower machines.
const SUBMIT_BYTES_PER_MS: u64 = 4;

/// Ceiling on the payload-derived delivery allowance. Without it a pathologically
/// large prompt would scale its own timeout without limit; with it the total wait
/// can never exceed `SUBMIT_CEILING_MS + SUBMIT_DELIVERY_CAP_MS`.
const SUBMIT_DELIVERY_CAP_MS: u64 = 10_000;

/// How often to re-check for quiet while waiting.
const SUBMIT_POLL_MS: u64 = 20;

/// How long `payload_len` bytes could plausibly still be in flight to the target.
///
/// Both bounds are built from this, which is what ties the timing policy to the
/// thing that actually varies. The previous constants were fixed, so a 50-byte
/// prompt and a 50 KB one were given identical windows.
fn delivery_allowance_ms(payload_len: usize) -> u64 {
    ((payload_len as u64) / SUBMIT_BYTES_PER_MS).min(SUBMIT_DELIVERY_CAP_MS)
}

/// Earliest an Enter may be sent for a payload of `payload_len` bytes.
///
/// Raising the floor with payload size is what closes the noisy-pane hole. The
/// "has the target reacted" rule infers reaction from the output clock moving,
/// but a pane that was *already* streaming when the dispatch arrived satisfies
/// that instantly with output that has nothing to do with our paste; a lull in
/// that unrelated stream then reads as "done receiving". Holding until the
/// payload could plausibly have been delivered means such a lull can no longer
/// release an Enter into a paste that is still arriving.
fn submit_floor_ms(payload_len: usize) -> u64 {
    SUBMIT_FLOOR_MS.max(delivery_allowance_ms(payload_len))
}

/// Latest an Enter may be withheld for a payload of `payload_len` bytes.
///
/// Extending the ceiling by the same delivery estimate keeps the backstop from
/// expiring *during* delivery of a large paste, which would fire Enter into the
/// middle of it — the exact failure the ceiling is supposed to be a safe fallback
/// from. Adding the allowance to a base that already exceeds `SUBMIT_FLOOR_MS`
/// also keeps this strictly above `submit_floor_ms` for every input, so the
/// backstop can never preempt the floor.
fn submit_ceiling_ms(payload_len: usize) -> u64 {
    SUBMIT_CEILING_MS + delivery_allowance_ms(payload_len)
}

/// Bracketed paste markers (DECSET 2004). Wrapping a payload in these makes it
/// one explicit paste event with a defined end, instead of something the target
/// has to infer from keystroke burst timing — which removes the guesswork the
/// timing policy above can only approximate.
///
/// Applied to Codex alone, deliberately. Codex is the CLI whose burst inference
/// loses the Enter, and it recommends this framing for itself. Claude Code
/// already submits reliably, so there is nothing to gain there and a real regression
/// to risk if it turned out not to honour the markers: an unsupported sequence
/// does not vanish, it lands in the composer as literal junk. opencode is
/// verified to support them and can be added once it has been exercised.
const PASTE_START: &str = "\x1b[200~";
const PASTE_END: &str = "\x1b[201~";

fn is_codex(program: &str) -> bool {
    let p = program.to_ascii_lowercase();
    p.trim_end_matches(".exe").trim_end_matches(".cmd") == "codex"
}

/// Whether `program` is the opencode CLI, normalised the same way as
/// `is_codex`: case-folded and stripped of a Windows `.exe` or `.cmd` suffix.
/// The free-model guard keys on this, so a differently spelled program name
/// must not slip past it.
fn is_opencode(program: &str) -> bool {
    let p = program.to_ascii_lowercase();
    p.trim_end_matches(".exe").trim_end_matches(".cmd") == "opencode"
}

/// Whether a model id is a paid OpenRouter model that must be refused for opencode.
///
/// OpenRouter free-tier models have ids ending in `:free`, or exactly
/// `openrouter/free` / `openrouter/openrouter/free`. Anything else under
/// `openrouter/` is paid. Local providers (ollama, llama.cpp, lmstudio, etc.)
/// never match `openrouter/` and are therefore always allowed.
///
/// The id is trimmed first so a leading or trailing space — the kind of thing a
/// paste or a quoted shell argument leaves behind — cannot silently downgrade a
/// paid id to a free one. Empty model is allowed (the CLI picks its own default).
fn is_paid_openrouter_model(model: &str) -> bool {
    // Case-folded so a re-cased id ("OpenRouter/...") cannot slip past the
    // prefix check; OpenRouter resolves ids case-insensitively.
    let trimmed = model.trim().to_ascii_lowercase();
    let trimmed = trimmed.as_str();
    if trimmed.is_empty() {
        return false;
    }
    if !trimmed.starts_with("openrouter/") {
        return false;
    }
    // Strip the leading "openrouter/" prefix, then check the remainder.
    let stripped = &trimmed["openrouter/".len()..];
    if stripped == "free" {
        return false;
    }
    // Accept "openrouter/free" (stripped == "free") and "openrouter/openrouter/free" (stripped == "openrouter/free")
    if stripped == "openrouter/free" {
        return false;
    }
    if stripped.ends_with(":free") {
        return false;
    }
    true
}

/// The free-model guard for an opencode launch. Returns the refusal message
/// when the launch must not proceed.
///
/// A missing model is refused, not waved through: without `-m`, opencode falls
/// back to its config's `model`, then to whatever model it used last, and this
/// guard cannot see either. Requiring an explicit id is what makes the paid
/// check cover every launch path rather than only the typed one.
fn opencode_model_guard(model: Option<&str>) -> Result<(), String> {
    let model = model.map(str::trim).unwrap_or("");
    if model.is_empty() {
        return Err(
            "Refused: opencode needs an explicit model. Without one it falls back to its \
             config or its last-used model, which the free-model guard cannot check. Enter a \
             free OpenRouter id (openrouter/free, or one ending in ':free') or a local \
             provider's model in the launcher."
                .to_string(),
        );
    }
    if is_paid_openrouter_model(model) {
        return Err(format!(
            "Refused: model '{model}' is a paid OpenRouter model. opencode will only launch \
             with openrouter/* models that end in ':free' or equal 'openrouter/free' (or \
             'openrouter/openrouter/free')."
        ));
    }
    Ok(())
}

/// How long `list_models` waits for `opencode models` before giving up on it.
/// Measured locally at ~2s for 379 ids, so 15s leaves ample room for a slower
/// machine or a cold model cache without leaving the launcher hung indefinitely
/// on a CLI that never returns.
const LIST_MODELS_TIMEOUT: Duration = Duration::from_secs(15);

/// Parse `opencode models` output (one id per line) and drop every id the
/// free-model guard would refuse, so the launcher only ever offers models that
/// can actually be picked. Pulled out of `list_models` so the parsing and
/// filtering is testable without spawning a process.
fn parse_and_filter_models(raw: &str) -> Vec<String> {
    let mut models: Vec<String> = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !is_paid_openrouter_model(line))
        .map(str::to_string)
        .collect();
    models.sort();
    models.dedup();
    models
}

/// How often `run_with_timeout` polls for the child having exited.
const RUN_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Run `cmd` to completion, killing it if it has not exited within `timeout`.
/// Returns its exit status and captured stdout/stderr.
///
/// Reads stdout and stderr on their own threads rather than one after the
/// other: a child that fills one pipe while it is still blocked writing to
/// the other would otherwise deadlock the sequential read waiting on the
/// pipe that is not being drained.
///
/// The deadline is enforced by polling `try_wait` rather than by racing the
/// output reads, so it bounds the child's whole run, not just how long its
/// output takes to arrive. A child that closes both pipes early but keeps
/// running is exactly the case a read-only race would miss: the reads would
/// finish immediately, and a follow-up blocking `wait()` would then have no
/// deadline of its own and could hang forever.
fn run_with_timeout(
    mut cmd: Command,
    timeout: Duration,
) -> Result<(std::process::ExitStatus, String, String), String> {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn: {e}"))?;

    let mut stdout = child.stdout.take().expect("stdout was piped");
    let mut stderr = child.stderr.take().expect("stderr was piped");
    let (out_tx, out_rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        let _ = out_tx.send(buf);
    });
    let (err_tx, err_rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        let _ = err_tx.send(buf);
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(RUN_POLL_INTERVAL),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("timed out after {}s", timeout.as_secs()));
            }
            Err(e) => return Err(format!("failed to wait on child: {e}")),
        }
    };

    // The child itself has exited, but a descendant it spawned (a
    // backgrounded shell job, for example) can inherit the pipe and keep its
    // write end open long after nothing more will ever arrive on it:
    // `read_to_string` then blocks for that descendant's remaining lifetime,
    // not the exited child's. Collecting through a channel with the *same*
    // deadline the exit wait above used, rather than an unbounded thread
    // join, is what keeps this call bounded by `timeout` no matter which
    // process is still holding a pipe open.
    let remaining = deadline.saturating_duration_since(Instant::now());
    let out = out_rx
        .recv_timeout(remaining)
        .map_err(|_| format!("timed out after {}s waiting for output", timeout.as_secs()))?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    let err = err_rx
        .recv_timeout(remaining)
        .map_err(|_| format!("timed out after {}s waiting for output", timeout.as_secs()))?;
    Ok((status, out, err))
}

/// List the models a CLI offers, for the launcher's model picker.
///
/// Only opencode has a list command; every other program returns an empty
/// list so the frontend falls back to its static options plus Custom. The
/// child is given `LIST_MODELS_TIMEOUT` to finish and is killed if it
/// overruns, so a hung `opencode models` cannot hang the launcher with it.
#[tauri::command]
fn list_models(program: String) -> Result<Vec<String>, String> {
    if !is_opencode(&program) {
        return Ok(Vec::new());
    }

    let mut cmd = Command::new(&program);
    cmd.arg("models");
    let (status, out, err) = run_with_timeout(cmd, LIST_MODELS_TIMEOUT)
        .map_err(|e| format!("'{program} models' {e}"))?;
    if !status.success() {
        return Err(format!("'{program} models' failed: {}", err.trim()));
    }
    Ok(parse_and_filter_models(&out))
}

/// Whether the Enter that submits a prompt can be sent yet.
///
/// Split out from the waiting loop so the policy is testable without a live PTY.
///
/// `baseline` is the target's output clock sampled *before* the prompt was
/// written, and comparing against it is what makes this correct. The previous
/// version asked only "has output been quiet for a while", which a target that
/// stays silent while it buffers a paste satisfies trivially — its last output
/// predates the write, so the gap is already enormous and Enter fires the moment
/// the floor elapses. That is exactly the original bug wearing a new hat, and it
/// is what a large dispatch to Codex hit: it echoes nothing until its burst
/// flushes, so "quiet" meant "has not started yet" rather than "has finished".
///
/// Silence before the target has produced anything therefore counts as still
/// receiving. Only once it has actually said something does going quiet mean it
/// is done, and the ceiling still bounds the wait if it never speaks at all.
///
/// `payload_len` is the size of what was written, in bytes, and scales both
/// bounds — see `submit_floor_ms` and `submit_ceiling_ms`. Reacting to output
/// alone is not sufficient when the target was already talking before we wrote,
/// so the floor carries the part of the decision that output cannot.
fn ready_to_submit(
    now: u64,
    started: u64,
    last_output: u64,
    baseline: u64,
    payload_len: usize,
) -> bool {
    let waited = now.saturating_sub(started);
    // Checked first so a silent target is always released eventually.
    if waited >= submit_ceiling_ms(payload_len) {
        return true;
    }
    if waited < submit_floor_ms(payload_len) {
        return false;
    }
    if last_output == baseline {
        return false;
    }
    now.saturating_sub(last_output) >= SUBMIT_QUIET_MS
}

#[derive(Default)]
pub struct SessionManager {
    sessions: Mutex<HashMap<String, SessionHandle>>,
    headless: Mutex<HashMap<String, ActiveHeadless>>,
    /// Only the first window-close request starts graceful cleanup. Later close
    /// events wait for that cleanup's exit callback instead of starting an empty
    /// second pass that could exit the app before the first pass kills its trees.
    shutting_down: AtomicBool,
    /// Serialize ConPTY openpty+spawn: concurrent spawns can stall a PTY pipe on
    /// Windows, so only one session is created at a time.
    spawn_lock: Mutex<()>,
}

fn report_worktree_cleanup(worktree: &worktree::Worktree) {
    match worktree::remove(worktree) {
        Ok(worktree::RemoveOutcome::RefusedDirty) => eprintln!(
            "[pantheon] preserved dirty worktree at {}",
            worktree.path.display()
        ),
        Ok(_) => {}
        Err(error) => eprintln!(
            "[pantheon] worktree cleanup failed for {}: {error}",
            worktree.path.display()
        ),
    }
}

/// Release everything a live session owns. Shared by explicit kill and by the
/// natural-exit path so the two cannot drift apart — they previously cleaned up
/// slightly different sets of resources.
fn release_session(session_id: &str, handle: SessionHandle) {
    if let Some(server) = handle.server {
        server.shutdown();
    }
    if let Some(worktree) = &handle.worktree {
        report_worktree_cleanup(worktree);
    }
    let _ = std::fs::remove_dir_all(session_config_dir(session_id));
}

/// Owns what a spawn creates before a `SessionHandle` exists to own it: the
/// worktree, the per-session config dir, and the dedicated MCP listener.
/// `spawn_session` can still return early after those are in place (opening the
/// PTY or spawning the child can fail), and dropping this guard tears them back
/// down. Without it a failed spawn strands a worktree, a branch, a config
/// directory, and a live loopback listener with no session attached to any of it.
///
/// Disarmed by `into_session` at the moment the `SessionHandle` takes over.
struct SpawnRollback {
    session_id: String,
    worktree: Option<worktree::Worktree>,
    server: Option<mcp::SessionServer>,
    armed: bool,
}

impl SpawnRollback {
    fn new(session_id: String) -> Self {
        Self {
            session_id,
            worktree: None,
            server: None,
            armed: true,
        }
    }

    /// Hand the resources to the session that now owns them, cancelling cleanup.
    fn into_session(mut self) -> (Option<worktree::Worktree>, Option<mcp::SessionServer>) {
        self.armed = false;
        (self.worktree.take(), self.server.take())
    }
}

impl Drop for SpawnRollback {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        eprintln!(
            "[pantheon] spawn failed for {}, rolling back",
            self.session_id
        );
        if let Some(server) = self.server.take() {
            server.shutdown();
        }
        if let Some(worktree) = &self.worktree {
            report_worktree_cleanup(worktree);
        }
        let _ = std::fs::remove_dir_all(session_config_dir(&self.session_id));
    }
}

impl SessionManager {
    fn kill(&self, id: &str) {
        let handle = self.sessions.lock().unwrap().remove(id);
        let cleanup = self.kill_headless_for_pane(id);
        if let Some(mut h) = handle {
            let _ = h.child.kill();
            release_after_headless(id.to_string(), h, cleanup);
        }
    }

    /// Type text into a session's terminal. This is how the conductor dispatches
    /// work: the task lands visibly in the target agent's pane.
    pub fn write_to(&self, id: &str, data: &str) -> bool {
        let mut map = self.sessions.lock().unwrap();
        if let Some(h) = map.get_mut(id) {
            if h.writer.write_all(data.as_bytes()).is_ok() {
                let _ = h.writer.flush();
                return true;
            }
        }
        false
    }

    /// The session's output-activity clock and launched program, if it is live.
    fn session_meta(&self, id: &str) -> Option<(Arc<AtomicU64>, String)> {
        self.sessions
            .lock()
            .unwrap()
            .get(id)
            .map(|h| (h.last_output.clone(), h.program.clone()))
    }

    /// Submit a prompt to a terminal UI as typing followed by a distinct Enter
    /// keypress. Codex and Claude Code deliberately distinguish pasted text
    /// containing a carriage return from an interactive Enter event; writing
    /// both in one PTY operation can leave the prompt sitting in the editor.
    ///
    /// The prompt is written synchronously, so an unreachable session is still
    /// reported to the caller. The Enter is not: it waits for the target to stop
    /// producing output (see `SUBMIT_QUIET_MS`) on a thread of its own, because
    /// that wait is open-ended and dispatch is specifically documented to return
    /// immediately so a conductor can fan work out. Blocking here would serialize
    /// a fan-out into one round trip per target, which is the property dispatch
    /// exists to provide.
    ///
    /// A `true` return therefore means "delivered to the terminal", not "already
    /// submitted".
    pub fn submit_to(self: &Arc<Self>, id: &str, prompt: &str) -> bool {
        // Resolved before the write so the output clock can be sampled first:
        // the baseline is only meaningful if it predates the prompt.
        let Some((clock, program)) = self.session_meta(id) else {
            return false;
        };
        let baseline = clock.load(Ordering::Relaxed);

        let payload: Cow<'_, str> = if is_codex(&program) {
            Cow::Owned(format!("{PASTE_START}{prompt}{PASTE_END}"))
        } else {
            Cow::Borrowed(prompt)
        };
        // Measured on the framed payload, not the bare prompt: the markers are
        // bytes the target has to ingest too, and it is the full write that has
        // to have landed before an Enter can mean "submit".
        let payload_len = payload.len();
        if !self.write_to(id, &payload) {
            return false;
        }

        let engine = self.clone();
        let id = id.to_string();
        thread::spawn(move || {
            let started = mono_ms();
            while !ready_to_submit(
                mono_ms(),
                started,
                clock.load(Ordering::Relaxed),
                baseline,
                payload_len,
            ) {
                thread::sleep(Duration::from_millis(SUBMIT_POLL_MS));
            }
            engine.write_to(&id, "\r");
        });
        true
    }

    /// Ids of every session the manager still holds.
    ///
    /// Membership is not liveness. A session leaves this map when its reader
    /// loop sees the PTY close, and on Windows that can lag the agent process
    /// exiting, so an id here may name a pane whose child is already gone. Ask
    /// `liveness` when the answer has to be true rather than merely current.
    pub fn ids(&self) -> Vec<String> {
        self.sessions.lock().unwrap().keys().cloned().collect()
    }

    /// Every held session paired with whether its child process is still
    /// running.
    ///
    /// This exists because presence in the map was the only signal the MCP
    /// server had, and presence answers the wrong question. A conductor asking
    /// "who can take this work" was told about panes whose agent had already
    /// died, dispatched into them, and then waited on a result that could never
    /// arrive.
    ///
    /// `try_wait` reaps without blocking: `Ok(Some(_))` means the child has
    /// exited, `Ok(None)` means it is still running. A probe that errors is
    /// reported as alive on purpose. Declaring a pane dead is what abandons its
    /// task, and doing that on the strength of a failed syscall would throw away
    /// work that is still being done. Being slow to notice a death costs a wait;
    /// being wrong about one costs the result.
    pub fn liveness(&self) -> Vec<(String, bool)> {
        let mut map = self.sessions.lock().unwrap();
        map.iter_mut()
            .map(|(id, handle)| {
                let alive = !matches!(handle.child.try_wait(), Ok(Some(_)));
                (id.clone(), alive)
            })
            .collect()
    }

    /// The program a live session launched, if it's still live — lets a
    /// caller (here, `human_dispatch`) tell an agent CLI target from a plain
    /// shell without going through the MCP-side session list, which may not
    /// have an entry yet if the session's dedicated endpoint failed to bind
    /// (see the fallback path in `spawn_session`).
    pub fn program_of(&self, id: &str) -> Option<String> {
        self.sessions
            .lock()
            .unwrap()
            .get(id)
            .map(|h| h.program.clone())
    }

    /// The launch inputs for a live session, if it is still held.
    pub fn launch_of(&self, id: &str) -> Option<headless::LaunchSpec> {
        self.sessions
            .lock()
            .unwrap()
            .get(id)
            .map(|handle| handle.launch.clone())
    }

    /// Run one Claude print-mode child with the live pane's cwd, model, MCP
    /// endpoint, and identity. Dispatch lifecycle and quiet-gate policy remain
    /// with the caller.
    pub fn run_headless(
        &self,
        id: &str,
        brief: &str,
        budget_usd: f64,
        timeout: Duration,
    ) -> Result<headless::HeadlessOutcome, headless::HeadlessError> {
        self.run_headless_registered(id, brief, budget_usd, timeout, |_| {})
    }

    pub(crate) fn headless_activity_age_ms(&self, id: &str) -> Option<u64> {
        self.sessions
            .lock()
            .unwrap()
            .get(id)
            .map(|h| mono_ms().saturating_sub(h.last_output.load(Ordering::Acquire)))
    }

    pub(crate) fn run_headless_registered(
        &self,
        id: &str,
        brief: &str,
        budget_usd: f64,
        timeout: Duration,
        registered: impl FnOnce(&str),
    ) -> Result<headless::HeadlessOutcome, headless::HeadlessError> {
        let (child, cli_session, cancellation) = {
            // Hold the session map through registration so pane close cannot
            // remove the worktree between lookup and child tracking.
            let sessions = self.sessions.lock().unwrap();
            let launch = sessions
                .get(id)
                .map(|handle| handle.launch.clone())
                .ok_or_else(|| headless::HeadlessError::SessionNotFound(id.to_string()))?;
            let child = headless::HeadlessChild::spawn(id, &launch, brief, budget_usd)?;
            let cli_session = child.cli_session().to_string();
            let cancellation = Arc::new(HeadlessCancellation::default());
            self.headless.lock().unwrap().insert(
                cli_session.clone(),
                ActiveHeadless {
                    pane_id: id.to_string(),
                    tree: child.process_tree(),
                    cancellation: cancellation.clone(),
                },
            );
            (child, cli_session, cancellation)
        };
        registered(&cli_session);
        let result = child.wait(timeout);
        // Decide exit versus cancellation under the registry lock: cancellation
        // marks its latch under the same lock. Retain cancelled entries until
        // cleanup completes so a concurrent pane close also waits before
        // removing its worktree and endpoint.
        let cancelled = {
            let mut active = self.headless.lock().unwrap();
            let cancelled = cancellation.requested.load(Ordering::Acquire);
            if !cancelled {
                active.remove(&cli_session);
            }
            cancelled
        };
        if cancelled {
            cancellation.wait();
            self.headless.lock().unwrap().remove(&cli_session);
        }
        if cancelled || self.shutting_down.load(Ordering::Acquire) {
            return cancelled_headless_result(result);
        }
        result
    }

    pub(crate) fn kill_headless_for_pane(&self, pane_id: &str) -> Option<thread::JoinHandle<()>> {
        let children = {
            let active = self.headless.lock().unwrap();
            active
                .values()
                .filter(|child| child.pane_id == pane_id)
                .map(|child| {
                    let newly_cancelled =
                        !child.cancellation.requested.swap(true, Ordering::AcqRel);
                    (child.clone(), newly_cancelled)
                })
                .collect::<Vec<_>>()
        };
        if children.is_empty() {
            return None;
        }
        for (child, newly_cancelled) in &children {
            if *newly_cancelled {
                let _ = child.tree.terminate();
            }
        }
        Some(thread::spawn(move || {
            if children.iter().any(|(_, newly_cancelled)| *newly_cancelled) {
                thread::sleep(headless::HEADLESS_TERMINATION_GRACE);
                for (child, newly_cancelled) in &children {
                    if *newly_cancelled {
                        let _ = child.tree.kill();
                        child.cancellation.complete();
                    }
                }
            }
            for (child, _) in children {
                child.cancellation.wait();
            }
        }))
    }

    /// Start graceful cleanup away from the event-loop thread. Returns the one
    /// cleanup thread to the caller so it can exit the app only after the final
    /// kill pass finishes. A repeated close request returns `None`; the first
    /// request's thread and exit callback remain authoritative.
    fn kill_all(&self) -> Option<thread::JoinHandle<()>> {
        if self.shutting_down.swap(true, Ordering::AcqRel) {
            return None;
        }
        let mut handles = self.sessions.lock().unwrap().drain().collect::<Vec<_>>();
        let trees = self
            .headless
            .lock()
            .unwrap()
            .values()
            .map(|child| child.tree.clone())
            .collect::<Vec<_>>();
        // These calls are non-blocking syscalls. Send both kinds of child their
        // first shutdown signal before returning, so even an unusually prompt OS
        // exit does not depend on the cleanup thread having been scheduled yet.
        for tree in &trees {
            let _ = tree.terminate();
        }
        for (_, handle) in &mut handles {
            let _ = handle.child.kill();
        }
        Some(thread::spawn(move || {
            thread::sleep(headless::HEADLESS_CLOSE_GRACE);
            for tree in trees {
                let _ = tree.kill();
            }
            kill_session_handles(handles);
        }))
    }

    /// App exit cannot wait for a grace period: kill every retained process tree
    /// immediately. The Windows Job Object's kill-on-close setting remains the
    /// final backstop if the process exits before this callback completes.
    fn kill_all_immediate(&self) {
        self.shutting_down.store(true, Ordering::Release);
        let handles = self.sessions.lock().unwrap().drain().collect::<Vec<_>>();
        let trees = self
            .headless
            .lock()
            .unwrap()
            .drain()
            .map(|(_, child)| child.tree)
            .collect::<Vec<_>>();
        for tree in trees {
            let _ = tree.kill();
        }
        kill_session_handles(handles);
    }
}

fn kill_session_handles(handles: Vec<(String, SessionHandle)>) {
    for (id, mut handle) in handles {
        let _ = handle.child.kill();
        release_session(&id, handle);
    }
}

fn cancelled_headless_result(
    result: Result<headless::HeadlessOutcome, headless::HeadlessError>,
) -> Result<headless::HeadlessOutcome, headless::HeadlessError> {
    let (exit_code, usage) = match result {
        Ok(o) => (o.exit_code, o.usage.map(Box::new)),
        Err(headless::HeadlessError::Exited {
            exit_code, usage, ..
        }) => (exit_code, usage),
        Err(headless::HeadlessError::InvalidJson { exit_code, .. }) => (exit_code, None),
        _ => (None, None),
    };
    Err(headless::HeadlessError::Cancelled { exit_code, usage })
}

// A pane's worktree and endpoint outlive the child termination grace. Cleanup
// is off the event-loop thread, but never races the child still using them.
fn release_after_headless(
    id: String,
    handle: SessionHandle,
    cleanup: Option<thread::JoinHandle<()>>,
) {
    if let Some(cleanup) = cleanup {
        thread::spawn(move || {
            let _ = cleanup.join();
            release_session(&id, handle);
        });
    } else {
        release_session(&id, handle);
    }
}

/// Decide how to launch `program` (see the M1 note). On Windows real shells
/// launch directly, and everything else (claude/codex/opencode `.cmd` shims) is
/// routed through `cmd.exe /c <bare-name>` so PATH — not our quoting — resolves
/// it. On Unix there are no shims: the agent CLIs are real executables that
/// `execvp` resolves on PATH, so everything launches directly and a wrapper
/// would only add a shell to misquote through.
fn build_command(
    program: &str,
    args: &[String],
    cwd: &Path,
    extra_env: &[(String, String)],
) -> CommandBuilder {
    let is_native_shell = matches!(
        program.to_ascii_lowercase().trim_end_matches(".exe"),
        "powershell" | "pwsh" | "cmd" | "bash" | "sh" | "zsh" | "fish" | "wsl"
    );

    let mut cmd = if is_native_shell || !cfg!(windows) {
        let mut c = CommandBuilder::new(program);
        for a in args {
            c.arg(a);
        }
        c
    } else {
        let mut c = CommandBuilder::new("cmd.exe");
        c.arg("/c");
        c.arg(program);
        for a in args {
            c.arg(a);
        }
        c
    };

    // Launch in the session's directory — the project repo, or its own worktree
    // when isolated. This also decides which git root the agent resolves to,
    // which is what Claude keys its local-scope MCP registration by.
    cmd.cwd(cwd);

    for (k, v) in std::env::vars() {
        cmd.env(k, v);
    }

    // Pantheon is the terminal emulator here, so Pantheon declares the terminal
    // type. portable-pty sets none of its own, and the inherited value is
    // whatever launched the app: nothing at all from the desktop entry (which
    // runs `bash -lc`, a non-interactive shell that never sets TERM), or the
    // host terminal's own type when launched from a shell. Both are wrong.
    //
    // An absent TERM is the damaging case. A CLI on a PTY sees isTTY, then
    // reads TERM to decide how much color it may emit, and with TERM unset the
    // answer is none: Claude Code renders entirely in the default foreground,
    // its status icon included. `xterm-256color` plus COLORTERM is what
    // xterm.js actually implements, so it is what the pane advertises.
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    // Applied after the inherited environment so per-session wiring wins.
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    cmd
}

/// Root for everything Pantheon stores per machine: worktrees, per-session agent
/// config, and the fallback context dir.
///
/// Keyed by bundle identifier rather than product name on purpose. A per-user
/// install puts the app itself in `%LOCALAPPDATA%\Pantheon`, and Windows paths are
/// case-insensitive — so a plain "pantheon" here would mix runtime data in with
/// the installed binaries, and an uninstall could take an agent's worktree with it.
/// The identifier is kept on Unix too, so a repo cloned on both platforms keeps
/// one layout rather than two that drift.
pub fn app_data_dir() -> PathBuf {
    let root = if cfg!(windows) {
        std::env::var("LOCALAPPDATA").ok()
    } else {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .filter(|p| !p.is_empty())
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .filter(|h| !h.is_empty())
                    .map(|h| format!("{h}/.local/share"))
            })
    };
    let root = root
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| std::env::temp_dir().to_string_lossy().to_string());
    compatible_app_data_dir(PathBuf::from(root))
}

/// Keep the pre-rename machine directory when it exists. Worktree registrations
/// and saved pane records contain absolute paths beneath this root, so moving it
/// would make preserved work unreachable even if every byte moved successfully.
fn compatible_app_data_dir(root: PathBuf) -> PathBuf {
    let current = root.join("com.gavinhensley.pantheon");
    let legacy = root.join("com.gavinhensley.mosaic");
    if legacy.exists() {
        if current.exists() {
            eprintln!(
                "[pantheon] both {} and {} exist; using the legacy directory to preserve worktree paths",
                legacy.display(),
                current.display()
            );
        }
        legacy
    } else {
        current
    }
}

/// Move a pre-rename project context directory when possible, and keep using
/// the legacy path when the filesystem will not permit the move. If both
/// identities exist, prefer the legacy one so existing decisions never silently
/// disappear: preserved sessions and context matter more than making the new
/// path appear immediately.
fn migrate_legacy_dir(current: PathBuf, legacy: PathBuf) -> PathBuf {
    if !legacy.exists() {
        return current;
    }
    if current.exists() {
        eprintln!(
            "[pantheon] both {} and {} exist; using the legacy directory so existing context stays visible",
            legacy.display(),
            current.display()
        );
        return legacy;
    }

    if let Some(parent) = current.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::rename(&legacy, &current) {
        Ok(()) => current,
        Err(error) => {
            eprintln!(
                "[pantheon] could not migrate {} to {}: {error}; using the legacy path",
                legacy.display(),
                current.display()
            );
            legacy
        }
    }
}

/// Per-session scratch dir for the config files we hand an agent CLI at launch.
fn session_config_dir(session_id: &str) -> PathBuf {
    app_data_dir().join("sessions").join(session_id)
}

/// Whether a session's program is an agent CLI that gets wired to the shared
/// brain. Kept in step with the match in `agent_mcp_wiring`.
///
/// A Shell pane can be promoted to conductor like any other, but it has no MCP
/// connection and so can never dispatch. Typing a briefing into it would just
/// hand PowerShell a paragraph of prose to run.
pub fn is_agent_cli(program: &str) -> bool {
    let p = program.to_ascii_lowercase();
    let p = p.trim_end_matches(".exe").trim_end_matches(".cmd");
    matches!(p, "claude" | "codex" | "opencode")
}

/// Trim and validate the `human_dispatch` inputs, so the rule is the same
/// whether the emptiness comes from the UI or a caller invoking the command
/// directly. Pure and separate from the Tauri command so it's testable
/// without constructing app state.
fn validate_human_dispatch(target: &str, task: &str) -> Result<(String, String), String> {
    let target = target.trim();
    let task = task.trim();
    if target.is_empty() {
        return Err("target is required.".to_string());
    }
    if task.is_empty() {
        return Err("task text is required.".to_string());
    }
    Ok((target.to_string(), task.to_string()))
}

/// Point ONE agent CLI at ONE dedicated MCP endpoint, entirely through launch
/// arguments and environment — we never touch the user's global config.
///
/// That matters twice over. It keeps identity honest (a session's port is only
/// ever registered to that session, so Pantheon knows the caller from the
/// connection alone), and it leaves no stale `pantheon` server behind pointing at
/// a random port that died with the app.
///
/// Config is written to a file rather than passed inline because these commands
/// are routed through `cmd.exe /c` — a JSON blob on that command line would be
/// at the mercy of Windows quoting.
///
/// Returns `(extra_args, extra_env)` to fold into the launch.
/// Env var carrying the bearer token to Codex. Codex takes the NAME of the
/// variable in its config, not the value, which is what we want: the secret
/// travels in the environment while only its label appears in `-c` on the
/// command line. Command lines are readable by any local process; environments
/// are not, and putting the token in an argument would hand it to exactly the
/// observer the token exists to stop.
const CODEX_TOKEN_ENV: &str = "PANTHEON_MCP_TOKEN";

/// Write a token-bearing session config. Unix permissions are forced to 0600.
/// Windows still uses the containing directory's default ACL; explicit ACL
/// hardening is pending operator policy and is not implemented here.
fn write_private_file(path: &Path, body: &str) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(body.as_bytes())
}

fn agent_mcp_wiring(
    program: &str,
    session_id: &str,
    url: &str,
    token: Option<&str>,
) -> (Vec<String>, Vec<(String, String)>) {
    let prog = program.to_ascii_lowercase();
    let prog = prog.trim_end_matches(".exe").trim_end_matches(".cmd");
    let dir = session_config_dir(session_id);
    let _ = std::fs::create_dir_all(&dir);

    match prog {
        // Additive on purpose: `--strict-mcp-config` would suppress every other
        // MCP server the user has configured, so a Pantheon pane would silently
        // lose the rest of their toolkit.
        //
        // `--mcp-config` is VARIADIC — it keeps eating following arguments as
        // further config paths. These are appended last for that reason. Any
        // argument added after this one would be swallowed as a config file.
        "claude" => {
            let path = dir.join("claude-mcp.json");
            let headers = match token {
                Some(t) => format!(r#","headers":{{"Authorization":"Bearer {t}"}}"#),
                None => String::new(),
            };
            let body = format!(
                r#"{{"mcpServers":{{"pantheon":{{"type":"http","url":"{url}"{headers}}}}}}}"#
            );
            match write_private_file(&path, &body) {
                Ok(_) => (
                    vec![
                        "--mcp-config".to_string(),
                        path.to_string_lossy().to_string(),
                    ],
                    vec![],
                ),
                Err(e) => {
                    eprintln!("[pantheon] claude mcp config write failed: {e}");
                    (vec![], vec![])
                }
            }
        }
        // A bare value fails TOML parsing, at which point Codex documents that it
        // falls back to the raw string — which sidesteps nested quoting.
        "codex" => {
            let mut args = vec!["-c".to_string(), format!("mcp_servers.pantheon.url={url}")];
            let mut env = vec![];
            if let Some(t) = token {
                args.push("-c".to_string());
                args.push(format!(
                    "mcp_servers.pantheon.bearer_token_env_var={CODEX_TOKEN_ENV}"
                ));
                env.push((CODEX_TOKEN_ENV.to_string(), t.to_string()));
            }
            (args, env)
        }
        // OPENCODE_CONFIG is merged over the global config, not swapped for it.
        "opencode" => {
            let path = dir.join("opencode.json");
            let headers = match token {
                Some(t) => format!(r#","headers":{{"Authorization":"Bearer {t}"}}"#),
                None => String::new(),
            };
            let body = format!(
                r#"{{"$schema":"https://opencode.ai/config.json","mcp":{{"pantheon":{{"type":"remote","url":"{url}"{headers}}}}}}}"#
            );
            match write_private_file(&path, &body) {
                Ok(_) => (
                    vec![],
                    vec![(
                        "OPENCODE_CONFIG".to_string(),
                        path.to_string_lossy().to_string(),
                    )],
                ),
                Err(e) => {
                    eprintln!("[pantheon] opencode config write failed: {e}");
                    (vec![], vec![])
                }
            }
        }
        // Plain shells get no wiring — nothing to connect.
        _ => (vec![], vec![]),
    }
}

/// Why isolation could not be provided. Separate from the message so the
/// frontend can branch on the cause rather than parse prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IsolationReason {
    /// The directory the session would run in is not inside a git repository, so
    /// there is nothing to cut a worktree from. Previously this path did not even
    /// log — the `if let Some(root)` simply fell through and the session ran
    /// shared.
    NotARepository,
    /// The repository was found but `git worktree add` failed.
    WorktreeCreateFailed,
}

/// The detail behind a refused isolated spawn. Carries what a user needs to
/// decide between retrying and deliberately continuing without isolation, which
/// is the choice this error exists to put in front of them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IsolationFailure {
    pub reason: IsolationReason,
    /// The session that was refused, so the frontend can match this to the pane
    /// that asked.
    pub session_id: String,
    /// The directory isolation was requested for.
    pub project: String,
    /// The underlying git/IO error, verbatim. Empty for `NotARepository`, which
    /// has no underlying failure — the precondition simply is not met.
    pub detail: String,
    /// Whether retrying could plausibly succeed. A worktree creation failure is
    /// often transient (a stale lock, a full disk); a non-repository will not
    /// become one by retrying, so offering "retry" there would be a dead end.
    pub retryable: bool,
}

/// Discriminates the one error the frontend must treat specially from every
/// other way a spawn can fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpawnErrorKind {
    /// An ordinary spawn failure.
    Failed,
    /// Isolation was requested and could not be provided, so nothing was
    /// spawned. Distinct because the remedy is a user decision, not a retry.
    IsolationUnavailable,
}

/// A spawn failure as the frontend receives it.
///
/// `message` is always populated, so any generic error path still has something
/// to show; `kind` and `isolation` are what let the launcher offer retry or
/// continue-unisolated instead of a dead end.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnError {
    pub kind: SpawnErrorKind,
    pub message: String,
    /// Present only when `kind` is `IsolationUnavailable`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolation: Option<IsolationFailure>,
}

impl SpawnError {
    fn failed(message: impl Into<String>) -> Self {
        Self {
            kind: SpawnErrorKind::Failed,
            message: message.into(),
            isolation: None,
        }
    }

    fn isolation(
        reason: IsolationReason,
        session_id: &str,
        project: &Path,
        detail: impl Into<String>,
    ) -> Self {
        let detail = detail.into();
        let project = project.to_string_lossy().to_string();
        let message = match reason {
            IsolationReason::NotARepository => format!(
                "Isolation was requested for session {session_id}, but {project} is not inside a \
                 git repository, so no worktree could be created. The session was not started."
            ),
            IsolationReason::WorktreeCreateFailed => format!(
                "Isolation was requested for session {session_id}, but creating a git worktree in \
                 {project} failed: {detail}. The session was not started."
            ),
        };
        Self {
            kind: SpawnErrorKind::IsolationUnavailable,
            message,
            isolation: Some(IsolationFailure {
                reason,
                session_id: session_id.to_string(),
                project,
                detail,
                retryable: matches!(reason, IsolationReason::WorktreeCreateFailed),
            }),
        }
    }
}

/// Lets the existing `.map_err(|e| e.to_string())?` sites keep working, so the
/// structured type is additive rather than a rewrite of every failure path.
impl From<String> for SpawnError {
    fn from(message: String) -> Self {
        Self::failed(message)
    }
}

/// Decide where a session runs, refusing to start at all if isolation was asked
/// for and cannot be delivered.
///
/// This used to fall back to the shared project directory and note it with an
/// `eprintln!` that never reaches the app, so a user who asked for isolation got
/// a session running directly in their project while the UI showed it as
/// isolated. Isolation is a safety property the user explicitly requested; the
/// one thing it must not do is quietly become nothing. Downgrading it is now the
/// user's decision to make, which means it has to reach them as an error.
///
/// Split from `spawn_session` so the policy is testable without a Tauri command:
/// `create` is injected so a test can force the failure branch and keep real
/// worktrees out of the user's app-data directory.
fn resolve_isolation<F>(
    isolate: bool,
    session_id: &str,
    project: &Path,
    create: F,
) -> Result<(PathBuf, Option<worktree::Worktree>), SpawnError>
where
    F: FnOnce(&Path, &str) -> Result<worktree::Worktree, String>,
{
    if !isolate {
        return Ok((project.to_path_buf(), None));
    }
    let Some(root) = worktree::repo_root(project) else {
        return Err(SpawnError::isolation(
            IsolationReason::NotARepository,
            session_id,
            project,
            "",
        ));
    };
    match create(&root, session_id) {
        Ok(w) => Ok((w.path.clone(), Some(w))),
        Err(e) => Err(SpawnError::isolation(
            IsolationReason::WorktreeCreateFailed,
            session_id,
            project,
            e,
        )),
    }
}

/// Which worktree an isolated session should run in: the one it was already
/// working in, or a new one.
///
/// Restoring a pane must not quietly duplicate its worktree. The one from the
/// last run can hold uncommitted agent work — `worktree::remove` refuses to
/// delete a dirty worktree for exactly that reason — and a worktree no pane
/// points at is work the app can no longer lead the user back to. So a fresh one
/// is cut only when the old one is genuinely gone (or belonged to a different
/// project); a directory that is still on disk but unusable refuses the spawn
/// instead, naming the path so the user can recover it.
///
/// `create` is injected so a test can assert it was *not* called.
fn choose_worktree<F>(
    root: &Path,
    session_id: &str,
    saved: Option<&worktree::Saved>,
    create: F,
) -> Result<worktree::Worktree, String>
where
    F: FnOnce(&Path, &str) -> Result<worktree::Worktree, String>,
{
    if let Some(saved) = saved {
        match worktree::reattach(root, saved) {
            worktree::Reattach::Reused(w) => return Ok(w),
            worktree::Reattach::Unusable(why) => {
                return Err(format!(
                    "the worktree {session_id} was working in is still on disk at {} but {why}. \
                     Pantheon will not create a second one, because the first may hold uncommitted \
                     work — recover or delete it, then start the session again",
                    saved.path
                ))
            }
            // Nothing left to strand: cut a fresh one below.
            worktree::Reattach::Missing | worktree::Reattach::Foreign => {}
        }
    }
    create(root, session_id)
}

/// Which worktree a session ended up in, pushed to the frontend so it can
/// remember it and put the session back there after a restart.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionWorktree {
    session_id: String,
    repo: String,
    path: String,
    branch: String,
    base: String,
}

/// Spawn a session under `session_id` and stream its output over `channel` until
/// the child exits. Returns as soon as streaming ends; the frontend fires it
/// without awaiting and addresses the session by the id it supplied.
///
/// Fails closed on isolation: if `isolate` was requested and no worktree could be
/// created, this returns `SpawnErrorKind::IsolationUnavailable` and starts
/// nothing, rather than silently running the session in the shared project dir.
#[tauri::command]
async fn spawn_session(
    app: AppHandle,
    state: State<'_, Arc<SessionManager>>,
    mcp: State<'_, McpInfo>,
    shared: State<'_, Arc<mcp::Shared>>,
    session_id: String,
    channel: Channel<&[u8]>,
    program: String,
    args: Vec<String>,
    rows: u16,
    cols: u16,
    cwd: Option<String>,
    isolate: Option<bool>,
    // The worktree this session used before the app was last closed, when the
    // frontend is restoring a remembered pane. See `choose_worktree`.
    reuse_worktree: Option<worktree::Saved>,
    // Optional model override (e.g., "openrouter/free"). If provided, the
    // corresponding flag is prepended to args.
    model: Option<String>,
    // The model flag to use (e.g., "--model" for claude, "-m" for codex/opencode).
    // Comes from the frontend's SessionType definition so there's a single
    // source of truth for which CLI uses which flag.
    model_flag: Option<String>,
) -> Result<(), SpawnError> {
    // Refuse a session id that is already live. Inserting over one would replace
    // the handle without killing the process or removing its worktree, orphaning
    // both. This early check only saves work — it cannot be authoritative,
    // because nothing stops a second spawn passing it before the first inserts.
    // The binding check is inside the spawn lock below.
    if state.sessions.lock().unwrap().contains_key(&session_id) {
        return Err(SpawnError::failed(format!(
            "session {session_id} is already running"
        )));
    }

    // Free-model guard for opencode: an explicit model is required, and only
    // free-tier openrouter/* ids or local providers (ollama, lmstudio, etc.)
    // pass. See `opencode_model_guard` for why a missing model is refused.
    if is_opencode(&program) {
        if let Err(reason) = opencode_model_guard(model.as_deref()) {
            return Err(SpawnError::failed(reason));
        }
    }

    // Decide where this session runs: the project dir, or its own git worktree
    // when isolated. Done before taking the spawn lock — creating a worktree and
    // registering MCP takes seconds and shouldn't serialize other spawns.
    let project = cwd
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    // Everything created from here until the SessionHandle exists is owned by this
    // guard, so any early return below unwinds it instead of stranding it. That
    // includes the isolation refusal below, which is why it is armed first.
    let mut rollback = SpawnRollback::new(session_id.clone());
    let (session_cwd, worktree) = resolve_isolation(
        isolate.unwrap_or(false),
        &session_id,
        &project,
        |root, id| choose_worktree(root, id, reuse_worktree.as_ref(), worktree::create),
    )?;
    // Captured before the worktree is handed to the rollback guard, and emitted
    // only once the session is actually live — see below.
    let session_worktree = worktree.as_ref().map(|w| SessionWorktree {
        session_id: session_id.clone(),
        repo: w.repo.to_string_lossy().to_string(),
        path: w.path.to_string_lossy().to_string(),
        branch: w.branch.clone(),
        base: w.base.clone(),
    });
    rollback.worktree = worktree;

    // EVERY session gets its own endpoint, isolated or not. Because that port is
    // only ever handed to this one session, any request arriving on it is
    // provably from it — so identity needs no handshake and can't be spoofed or
    // forgotten. Sessions sharing one endpoint would all authenticate as
    // "unknown", which silently breaks brain assignment and the conductor.
    let (mcp_args, mcp_env, endpoint) =
        match mcp::start_session_server(shared.inner().clone(), session_id.clone()) {
            Ok(server) => {
                let url = format!("http://127.0.0.1:{}/mcp", server.port);
                let token = server.token.clone();
                rollback.server = Some(server);
                let model_str = model.as_deref().unwrap_or("");
                shared.note_session(&session_id, &program, model_str);
                let (args, env) = agent_mcp_wiring(&program, &session_id, &url, Some(&token));
                (
                    args,
                    env,
                    headless::LaunchEndpoint::Dedicated { url, token },
                )
            }
            // Endpoint failed: fall back to the shared one. The agent can still reach
            // the brain, it just has to declare who it is.
            //
            // No token here: the shared endpoint is not per-session, so there is
            // nothing session-specific to prove. That endpoint is exactly the
            // self-declared-identity path audit finding #6 also covers, and it is
            // still open.
            Err(e) => {
                eprintln!("[pantheon] session endpoint failed, using shared: {e}");
                let model_str = model.as_deref().unwrap_or("");
                shared.note_session(&session_id, &program, model_str);
                let (args, env) = agent_mcp_wiring(&program, &session_id, &mcp.url, None);
                (args, env, headless::LaunchEndpoint::Shared)
            }
        };
    let launch = headless::LaunchSpec::for_session(
        session_cwd.clone(),
        program.clone(),
        args,
        model.as_deref(),
        model_flag.as_deref(),
        mcp_args,
        mcp_env,
        &session_id,
        endpoint,
    );

    // Starts "active now" so a session that never writes anything is governed by
    // SUBMIT_FLOOR_MS rather than looking quiet since the epoch.
    let activity = Arc::new(AtomicU64::new(mono_ms()));

    // Create the PTY + child under the spawn lock (serialize ConPTY spawns). The
    // lock guard is confined to this block so it never crosses the .await below.
    let mut reader = {
        let _guard = state.spawn_lock.lock().unwrap();
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;

        // Take the reader and writer BEFORE spawning. Both can fail, and spawning
        // is the one step here that is not undoable — doing it last means a
        // failure never leaves a live child process behind for the guard to
        // reap, so the rollback only ever has inert resources to clean up.
        let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

        // The authoritative duplicate check. `spawn_lock` serializes this whole
        // block, so no other spawn can insert between here and the insertion
        // below — which makes check-then-insert atomic against them. It sits
        // before the spawn deliberately: losing this race must not leave a live
        // child behind, and the rollback guard only handles inert resources.
        if state.sessions.lock().unwrap().contains_key(&session_id) {
            return Err(SpawnError::failed(format!(
                "session {session_id} is already running"
            )));
        }

        let cmd = build_command(&launch.program, &launch.args, &launch.cwd, &launch.env);
        let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        drop(pair.slave); // so the reader hits EOF when the child exits

        // The session owns the worktree and listener from here on, so the guard
        // stands down. Nothing below this point can fail before insertion.
        let (worktree, server) = rollback.into_session();
        state.sessions.lock().unwrap().insert(
            session_id.clone(),
            SessionHandle {
                master: pair.master,
                writer,
                child,
                worktree,
                last_output: activity.clone(),
                program: program.clone(),
                launch,
                server,
            },
        );
        reader
    };

    // Now that the session is live, tell the frontend which worktree it got, so a
    // restart can put this pane back into it. Deliberately after insertion: a
    // spawn that failed earlier has had its worktree rolled back, and the
    // frontend must not be left remembering one that no longer exists.
    if let Some(event) = session_worktree {
        let _ = app.emit("session-worktree", event);
    }

    // Blocking reads on their own thread → async forward loop via mpsc.
    // Bound each session's pending output to roughly 512 KiB. Backpressure here
    // is preferable to six unbounded queues consuming memory when WebView2 is
    // busy or minimized; ConPTY naturally blocks the child until we catch up.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    // Stamped on arrival rather than after the send, which can
                    // block on a full channel and would misreport the target as
                    // quiet while it is in fact still talking.
                    activity.store(mono_ms(), Ordering::Relaxed);
                    if tx.blocking_send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    while let Some(mut bytes) = rx.recv().await {
        // Drain queued bursts into fewer IPC messages without adding latency to
        // interactive output. Keep batches modest so one noisy agent cannot
        // monopolize the WebView event loop.
        while bytes.len() < 64 * 1024 {
            match rx.try_recv() {
                Ok(next) => bytes.extend_from_slice(&next),
                Err(_) => break,
            }
        }
        if channel.send(&bytes[..]).is_err() {
            break;
        }
    }

    // Session ended on its own (agent quit or crashed). Tear down exactly what
    // an explicit kill would: dropping the handle alone would discard the
    // Worktree without removing it and leave the session's MCP listener serving,
    // stranding a directory, a branch, and a port for every session that wasn't
    // closed by hand.
    let handle = state.sessions.lock().unwrap().remove(&session_id);
    let cleanup = state.kill_headless_for_pane(&session_id);
    if let Some(h) = handle {
        release_after_headless(session_id.clone(), h, cleanup);
    }
    // If an explicit kill removed the handle, it owns cleanup, possibly after
    // a headless child's termination grace. Do not remove its config early.
    let _ = app.emit("session-exited", &session_id);
    Ok(())
}

#[tauri::command]
fn write_session(
    state: State<'_, Arc<SessionManager>>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    let mut map = state.sessions.lock().unwrap();
    if let Some(h) = map.get_mut(&session_id) {
        h.writer
            .write_all(data.as_bytes())
            .map_err(|e| e.to_string())?;
        h.writer.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn resize_session(
    state: State<'_, Arc<SessionManager>>,
    session_id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let map = state.sessions.lock().unwrap();
    if let Some(h) = map.get(&session_id) {
        h.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn kill_session(state: State<'_, Arc<SessionManager>>, session_id: String) {
    state.kill(&session_id);
}

/// Where the shared brain writes its markdown before a project is chosen.
/// App-local, so it is never at the mercy of the launch directory.
fn default_context_dir() -> PathBuf {
    app_data_dir().join("context")
}

/// Point the shared brain's markdown at the project the user picked, so a
/// project's decisions live with that project instead of in a global pile.
/// Called by the frontend on startup (from the remembered project) and on pick.
#[tauri::command]
fn set_project(shared: State<'_, Arc<mcp::Shared>>, path: Option<String>) {
    let dir = match path {
        Some(p) if !p.is_empty() => {
            let root = PathBuf::from(p);
            migrate_legacy_dir(root.join(".pantheon"), root.join(".mosaic")).join("context")
        }
        _ => default_context_dir(),
    };
    shared.set_dir(dir);
}

#[tauri::command]
fn project_is_repo(dir: String) -> bool {
    worktree::repo_root(Path::new(&dir)).is_some()
}

#[tauri::command]
fn init_project_repo(dir: String) -> Result<(), String> {
    let output = Command::new("git")
        .arg("init")
        .current_dir(&dir)
        .output()
        .map_err(|e| format!("failed to run git init in {dir}: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("git init failed in {dir}: {detail}"))
    }
}

/// Loopback URL + port of the in-process MCP server, for per-session registration.
#[derive(Clone, Serialize)]
struct McpInfo {
    url: String,
    port: u16,
}

#[tauri::command]
fn mcp_info(info: State<'_, McpInfo>) -> McpInfo {
    info.inner().clone()
}

/// A snapshot of the shared brain for the sidebar.
#[derive(Serialize)]
struct ContextSnapshot {
    entries: Vec<mcp::Entry>,
    sessions: Vec<mcp::AgentSession>,
}

#[tauri::command]
fn get_context(shared: State<'_, Arc<mcp::Shared>>) -> ContextSnapshot {
    ContextSnapshot {
        entries: shared.entries_snapshot(),
        sessions: shared.sessions_snapshot(),
    }
}

/// Assign an agent (by the name it declared) to a brain. The frontend calls this
/// when a pane is created and whenever you drag it into a different brain.
#[tauri::command]
fn set_agent_brain(shared: State<'_, Arc<mcp::Shared>>, name: String, brain: String) {
    shared.set_room(&name, &brain);
}

/// Conductor role + halt state + the task feed, for the ConductorBar.
#[derive(Serialize)]
struct ConductorState {
    conductor: Option<String>,
    halted: bool,
    tasks: Vec<mcp::Task>,
}

#[tauri::command]
fn conductor_state(shared: State<'_, Arc<mcp::Shared>>) -> ConductorState {
    ConductorState {
        conductor: shared.conductor(),
        halted: shared.is_halted(),
        tasks: shared.tasks_snapshot(),
    }
}

/// Promote a pane to conductor (or pass null to clear). The app owns this role —
/// an agent can never claim it for itself.
#[tauri::command]
fn set_conductor(shared: State<'_, Arc<mcp::Shared>>, name: Option<String>) {
    shared.set_conductor(name);
}

/// The global Stop: halts all dispatch and cancels pending tasks.
#[tauri::command]
fn halt_conductor(shared: State<'_, Arc<mcp::Shared>>, halted: bool) {
    shared.set_halted(halted);
}

/// Let a human dispatch a task to a live agent session directly from the UI,
/// instead of only through an agent calling the MCP `dispatch` tool. Reuses
/// the exact submit machinery and task store that tool uses — see
/// `mcp::Shared::dispatch_task` — so the ConductorBar and get_task_result see
/// one ledger regardless of which path started the task.
///
/// Attributed to the current conductor rather than to some "user" identity:
/// the app is what decides who holds that role, and a task the human triggers
/// through the UI is still work the conductor is orchestrating, so it belongs
/// on the same ledger the conductor's own dispatches land on.
#[tauri::command]
fn human_dispatch(
    state: State<'_, Arc<SessionManager>>,
    shared: State<'_, Arc<mcp::Shared>>,
    target: String,
    task: String,
) -> Result<mcp::DispatchOutcome, String> {
    let (target, task) = validate_human_dispatch(&target, &task)?;

    let from = shared
        .conductor()
        .ok_or_else(|| "no conductor is set — promote a pane first.".to_string())?;

    // Only a live agent CLI can act on a dispatch: it needs the MCP
    // connection to read the task and call complete_task back. A Shell pane
    // has neither, so a dispatch to one would just sit pending until it
    // timed out.
    match state.program_of(&target) {
        Some(p) if is_agent_cli(&p) => {}
        Some(_) => {
            return Err(format!(
                "'{target}' is a shell session and cannot receive a dispatch."
            ))
        }
        None => return Err(format!("no live session '{target}'.")),
    }

    // No reviewer: this is a human typing into the UI, and a human dispatching
    // work directly is already the second pair of eyes the gate exists to
    // provide. Give this a reviewer field when the UI grows somewhere to pick
    // one, rather than guessing a session on the user's behalf.
    shared.dispatch_task(&from, &target, &task, "")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // The session engine is shared with the MCP server so the conductor's
            // dispatch can write straight into a target session's terminal.
            let sessions = Arc::new(SessionManager::default());
            app.manage(sessions.clone());

            // Start the in-process MCP "shared brain" on a random loopback port.
            // Sessions each get their own endpoint; this one is the fallback and
            // the address shown in the sidebar.
            //
            // No agent CLI is registered here. Wiring happens per session, at
            // launch, through arguments and environment only — see
            // agent_mcp_wiring.
            let handle = app.handle().clone();
            // Placeholder until the frontend reports its project. Deriving this
            // from the working directory made the brain's files land wherever
            // the app happened to be started from, which for a packaged build is
            // wherever Explorer felt like.
            let dir = default_context_dir();
            let (port, shared) = mcp::start(handle, dir, sessions)?;
            let url = format!("http://127.0.0.1:{port}/mcp");
            app.manage(McpInfo { url, port });
            app.manage(shared);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            spawn_session,
            write_session,
            resize_session,
            kill_session,
            mcp_info,
            get_context,
            set_agent_brain,
            conductor_state,
            set_conductor,
            halt_conductor,
            set_project,
            project_is_repo,
            init_project_repo,
            list_models,
            human_dispatch
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let manager = window.state::<Arc<SessionManager>>().inner().clone();
                if let Some(cleanup) = manager.kill_all() {
                    let app = window.app_handle().clone();
                    thread::spawn(move || {
                        let _ = cleanup.join();
                        app.exit(0);
                    });
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            app.state::<Arc<SessionManager>>().kill_all_immediate();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{
        agent_mcp_wiring, build_command, choose_worktree, compatible_app_data_dir,
        delivery_allowance_ms, is_agent_cli, is_codex, is_opencode, is_paid_openrouter_model,
        list_models, migrate_legacy_dir, opencode_model_guard, parse_and_filter_models,
        ready_to_submit, resolve_isolation, run_with_timeout, submit_ceiling_ms, submit_floor_ms,
        validate_human_dispatch, worktree, ActiveHeadless, IsolationReason, SessionManager,
        SpawnErrorKind, SpawnRollback, CODEX_TOKEN_ENV, SUBMIT_BYTES_PER_MS, SUBMIT_CEILING_MS,
        SUBMIT_DELIVERY_CAP_MS, SUBMIT_FLOOR_MS, SUBMIT_QUIET_MS,
    };
    use std::fs;
    use std::process::Command;
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    #[test]
    fn externally_killed_headless_result_keeps_exit_and_usage() {
        use crate::headless::{HeadlessError, HeadlessOutcome, HeadlessUsage};
        let result = super::cancelled_headless_result(Ok(HeadlessOutcome {
            result: "partial".into(),
            cli_session: "uuid".into(),
            stderr: String::new(),
            exit_code: Some(143),
            usage: Some(HeadlessUsage {
                input_tokens: Some(9),
                ..Default::default()
            }),
        }));
        match result {
            Err(HeadlessError::Cancelled { exit_code, usage }) => {
                assert_eq!(exit_code, Some(143));
                assert_eq!(usage.unwrap().input_tokens, Some(9));
            }
            other => panic!("unexpected result: {other:?}"),
        }
        // The live SessionHandle/PTY registration path cannot be built by the
        // shared-store fixture. This is the exact conversion used after its
        // cancellation latch fires; process-tree termination is tested separately.
    }

    #[test]
    fn run_headless_registered_drops_an_uninvoked_callbacks_captured_guard_on_session_not_found() {
        use crate::headless::HeadlessError;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::RwLock;

        // mcp.rs's run_headless_task moves a store_gate/delivery/pane-lock
        // guard into the `registered` callback and relies on that callback
        // being the thing that drops it. A pre-review flagged the path where
        // the callback never runs at all (the session is gone before
        // `run_headless_registered` reaches its own lookup) as a possible
        // guard leak into `headless_exited_at`, which re-acquires one of the
        // same guards. This reproduces that shape directly: no live session
        // exists here at all, so the callback below can never run, yet the
        // guard it would have dropped must still be free the moment this
        // call returns, because dropping an un-invoked `FnOnce` drops
        // whatever it captured by move; there is no code path in
        // `run_headless_registered` that holds `registered` open past its
        // own early return. Nothing needed changing to make this pass; it
        // pins the guarantee mcp.rs's comment at that call site now relies on.
        let manager = SessionManager::default();
        let held = std::sync::Arc::new(RwLock::new(()));
        let guard = held.read().unwrap();
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_from_closure = called.clone();
        let result = manager.run_headless_registered(
            "no-such-session",
            "brief",
            5.0,
            Duration::from_secs(1),
            move |_session| {
                called_from_closure.store(true, Ordering::SeqCst);
                drop(guard);
            },
        );
        assert!(
            matches!(&result, Err(HeadlessError::SessionNotFound(id)) if id == "no-such-session"),
            "unexpected result: {result:?}"
        );
        assert!(
            !called.load(Ordering::SeqCst),
            "the callback must not run when the session is gone"
        );
        assert!(
            held.try_write().is_ok(),
            "the callback's captured guard leaked past a call that never invoked it"
        );
    }

    #[cfg(unix)]
    #[test]
    fn kill_all_returns_quickly_then_kills_an_active_headless_tree() {
        let manager = SessionManager::default();
        let mut command = std::process::Command::new("sh");
        command.args(["-c", "sleep 60 & sleep 60"]);
        let mut process = crate::headless::spawn_process_tree(&mut command).unwrap();
        let process_group = process.child.id() as libc::pid_t;
        manager.headless.lock().unwrap().insert(
            "shutdown-test".to_string(),
            ActiveHeadless {
                pane_id: "sess-test".to_string(),
                tree: process.tree.clone(),
                cancellation: std::sync::Arc::new(super::HeadlessCancellation::default()),
            },
        );

        let started = std::time::Instant::now();
        let cleanup = manager.kill_all().expect("first close starts cleanup");
        // This is well below HEADLESS_CLOSE_GRACE (500 ms), so moving that
        // sleep back onto the caller thread makes this assertion fail.
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "kill_all blocked the caller for {:?}",
            started.elapsed()
        );
        cleanup.join().unwrap();
        process.child.wait().unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let result = unsafe { libc::kill(-process_group, 0) };
            if result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "process group {process_group} survived app cleanup"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn legacy_data_directory_moves_to_the_pantheon_identity() {
        let tmp = TempDir::new().unwrap();
        let legacy = tmp.path().join(".mosaic");
        let current = tmp.path().join(".pantheon");
        fs::create_dir_all(legacy.join("context")).unwrap();
        fs::write(legacy.join("context/brain.jsonl"), "remember me").unwrap();

        let chosen = migrate_legacy_dir(current.clone(), legacy.clone());

        assert_eq!(chosen, current);
        assert!(!legacy.exists());
        assert_eq!(
            fs::read_to_string(chosen.join("context/brain.jsonl")).unwrap(),
            "remember me"
        );
    }

    #[test]
    fn coexisting_project_directories_keep_legacy_context_visible() {
        let tmp = TempDir::new().unwrap();
        let legacy = tmp.path().join(".mosaic");
        let current = tmp.path().join(".pantheon");
        fs::create_dir_all(legacy.join("context")).unwrap();
        fs::create_dir_all(current.join("context")).unwrap();
        fs::write(legacy.join("context/brain.jsonl"), "legacy work").unwrap();

        let chosen = migrate_legacy_dir(current, legacy.clone());

        assert_eq!(chosen, legacy);
        assert_eq!(
            fs::read_to_string(chosen.join("context/brain.jsonl")).unwrap(),
            "legacy work"
        );
    }

    #[test]
    fn existing_machine_data_keeps_its_absolute_worktree_root() {
        let tmp = TempDir::new().unwrap();
        let legacy = tmp.path().join("com.gavinhensley.mosaic");
        fs::create_dir_all(legacy.join("worktrees/session-1")).unwrap();

        assert_eq!(compatible_app_data_dir(tmp.path().to_path_buf()), legacy);
    }

    #[test]
    fn a_machine_carrying_both_identities_still_reads_the_legacy_one() {
        // What a machine that ran Mosaic and then a Pantheon build actually
        // looks like. The Pantheon directory exists because Tauri creates it
        // from the bundle identifier on first launch, which says nothing about
        // where the worktrees and sessions went.
        let tmp = TempDir::new().unwrap();
        let legacy = tmp.path().join("com.gavinhensley.mosaic");
        fs::create_dir_all(legacy.join("sessions/sess-1")).unwrap();
        fs::create_dir_all(tmp.path().join("com.gavinhensley.pantheon")).unwrap();

        assert_eq!(compatible_app_data_dir(tmp.path().to_path_buf()), legacy);
    }

    #[test]
    fn a_machine_that_never_ran_mosaic_uses_the_pantheon_identity() {
        let tmp = TempDir::new().unwrap();

        assert_eq!(
            compatible_app_data_dir(tmp.path().to_path_buf()),
            tmp.path().join("com.gavinhensley.pantheon")
        );
    }
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    /// A temp repo plus one worktree created inside it, standing in for the state
    /// a spawn has already built when a later step fails.
    fn worktree_fixture(tmp: &TempDir, session_id: &str) -> worktree::Worktree {
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        worktree::init_test_repo(&repo).unwrap();
        worktree::create_with_base_dir(&repo, session_id, tmp.path()).unwrap()
    }

    #[test]
    fn an_armed_rollback_removes_the_worktree_a_failed_spawn_left_behind() {
        let tmp = TempDir::new().unwrap();
        let wt = worktree_fixture(&tmp, "sess-rollback-armed");
        let path = wt.path.clone();
        assert!(path.exists(), "fixture should start on disk");

        {
            let mut rollback = SpawnRollback::new("sess-rollback-armed".into());
            rollback.worktree = Some(wt);
            // Dropped here, standing in for an early return from spawn_session.
        }

        assert!(!path.exists(), "rollback should remove the worktree");
    }

    #[test]
    fn a_rollback_disarmed_by_into_session_leaves_the_worktree_alone() {
        let tmp = TempDir::new().unwrap();
        let wt = worktree_fixture(&tmp, "sess-rollback-disarmed");
        let path = wt.path.clone();

        let handed_over = {
            let mut rollback = SpawnRollback::new("sess-rollback-disarmed".into());
            rollback.worktree = Some(wt);
            let (worktree, _server) = rollback.into_session();
            worktree
        };

        assert!(
            path.exists(),
            "a session that spawned successfully must keep its worktree"
        );
        assert!(
            handed_over.is_some(),
            "the worktree should be handed to the SessionHandle, not dropped"
        );
    }

    #[test]
    fn rollback_preserves_a_worktree_that_already_has_uncommitted_work() {
        let tmp = TempDir::new().unwrap();
        let wt = worktree_fixture(&tmp, "sess-rollback-dirty");
        let path = wt.path.clone();
        fs::write(path.join("in-progress.txt"), "unsaved").unwrap();

        {
            let mut rollback = SpawnRollback::new("sess-rollback-dirty".into());
            rollback.worktree = Some(wt);
        }

        assert!(
            path.join("in-progress.txt").exists(),
            "rollback must not discard uncommitted work"
        );
    }

    // ---- Restoring an isolated pane ----
    //
    // The failure these guard against is silent: a restored session cuts itself a
    // second worktree, and the one holding the agent's uncommitted work is left
    // with nothing pointing at it.

    fn saved_from(wt: &worktree::Worktree) -> worktree::Saved {
        worktree::Saved {
            repo: wt.repo.to_string_lossy().to_string(),
            path: wt.path.to_string_lossy().to_string(),
            branch: wt.branch.clone(),
            base: wt.base.clone(),
        }
    }

    /// Stands in for `worktree::create` where creating one would be the bug.
    fn create_must_not_be_called(
        _repo: &Path,
        _session_id: &str,
    ) -> Result<worktree::Worktree, String> {
        panic!("a second worktree must not be created while the first is on disk");
    }

    #[test]
    fn a_restored_session_goes_back_into_its_own_worktree() {
        let tmp = TempDir::new().unwrap();
        let wt = worktree_fixture(&tmp, "sess-restore");
        let repo = wt.repo.clone();
        fs::write(wt.path.join("in-progress.txt"), "unsaved").unwrap();

        let chosen = choose_worktree(
            &repo,
            "sess-restore",
            Some(&saved_from(&wt)),
            create_must_not_be_called,
        )
        .expect("an intact worktree should be reused");

        assert_eq!(chosen.path, wt.path);
        assert!(
            chosen.path.join("in-progress.txt").exists(),
            "the restored session must land on the work it left behind"
        );
    }

    #[test]
    fn a_restored_session_gets_a_new_worktree_only_when_the_old_one_is_gone() {
        let tmp = TempDir::new().unwrap();
        let wt = worktree_fixture(&tmp, "sess-clean-exit");
        let repo = wt.repo.clone();
        let saved = saved_from(&wt);
        // Last run ended clean, so cleanup took the worktree with it.
        worktree::remove(&wt).unwrap();

        let base = tmp.path().join("fresh");
        let chosen = choose_worktree(&repo, "sess-clean-exit", Some(&saved), |r, s| {
            worktree::create_with_base_dir(r, s, &base)
        })
        .expect("a vanished worktree should be replaced");

        assert!(chosen.path.exists());
        assert_ne!(chosen.path, wt.path);
    }

    #[test]
    fn an_unusable_worktree_refuses_the_spawn_rather_than_stranding_it() {
        let tmp = TempDir::new().unwrap();
        let wt = worktree_fixture(&tmp, "sess-unusable");
        let repo = wt.repo.clone();

        // On disk, holding work, but not registered with this repo any more.
        let orphan = tmp.path().join("orphan");
        fs::create_dir_all(&orphan).unwrap();
        fs::write(orphan.join("in-progress.txt"), "unsaved").unwrap();
        let saved = worktree::Saved {
            repo: repo.to_string_lossy().to_string(),
            path: orphan.to_string_lossy().to_string(),
            branch: "pantheon/sess-unusable".to_string(),
            base: wt.base.clone(),
        };

        // `Worktree` has no Debug, so `expect_err` is unavailable here.
        let err = match choose_worktree(
            &repo,
            "sess-unusable",
            Some(&saved),
            create_must_not_be_called,
        ) {
            Err(e) => e,
            Ok(_) => panic!("a worktree still on disk must not be silently abandoned"),
        };

        assert!(
            err.contains(&orphan.to_string_lossy().to_string()),
            "the message must name the path so the work can be recovered: {err}"
        );
        assert!(orphan.join("in-progress.txt").exists());
    }

    #[test]
    fn switching_projects_leaves_the_old_project_worktree_alone() {
        let tmp = TempDir::new().unwrap();
        let wt = worktree_fixture(&tmp, "sess-moved");

        // A different repo is now the project; the saved worktree belongs to the
        // old one, which still tracks it.
        let other = tmp.path().join("other");
        fs::create_dir_all(&other).unwrap();
        worktree::init_test_repo(&other).unwrap();

        let base = tmp.path().join("fresh");
        let chosen = choose_worktree(&other, "sess-moved", Some(&saved_from(&wt)), |r, s| {
            worktree::create_with_base_dir(r, s, &base)
        })
        .expect("a session in a new project gets a worktree of that project");

        assert_ne!(chosen.path, wt.path);
        assert!(
            wt.path.exists(),
            "the other project's worktree is untouched"
        );
    }

    // Isolation must fail closed. It used to fall back to the shared project
    // directory and note it with an eprintln! that never reaches the app, so a
    // user who asked for isolation got a session running loose in their project
    // while the UI showed it as isolated — a safety property they explicitly
    // asked for, silently downgraded to nothing.

    /// Stands in for `worktree::create` when a test needs the failure branch.
    /// Injected rather than provoked so the case is deterministic and no real
    /// worktree is created in the user's app-data directory.
    fn create_fails(_repo: &Path, _session_id: &str) -> Result<worktree::Worktree, String> {
        Err("fatal: could not lock ref: File exists".into())
    }

    /// `worktree::Worktree` deliberately has no `Debug`, so `expect_err` cannot be
    /// used on what `resolve_isolation` returns.
    fn expect_refusal(
        result: Result<(PathBuf, Option<worktree::Worktree>), super::SpawnError>,
        context: &str,
    ) -> super::SpawnError {
        match result {
            Ok(_) => panic!("{context}"),
            Err(e) => e,
        }
    }

    #[test]
    fn isolation_requested_but_unavailable_refuses_to_spawn() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        worktree::init_test_repo(&repo).unwrap();

        let err = expect_refusal(
            resolve_isolation(true, "sess-iso-fail", &repo, create_fails),
            "a session that asked for isolation must not start without it",
        );

        assert_eq!(err.kind, SpawnErrorKind::IsolationUnavailable);
        let detail = err.isolation.expect("the frontend needs the cause");
        assert_eq!(detail.reason, IsolationReason::WorktreeCreateFailed);
        assert_eq!(detail.session_id, "sess-iso-fail");
        assert!(
            detail.retryable,
            "a lock failure is transient, so retry must be offered"
        );
        // The underlying git error has to survive to the user, otherwise the
        // dialog can only say that something went wrong.
        assert!(
            detail.detail.contains("could not lock ref"),
            "underlying error must reach the frontend, got {:?}",
            detail.detail
        );
        assert!(
            err.message.contains("not started"),
            "the message must say nothing was spawned, got {:?}",
            err.message
        );
    }

    // The path that did not even log: `repo_root` returning None skipped the
    // whole isolation block and the session ran shared in silence.
    #[test]
    fn isolation_requested_outside_a_repository_refuses_to_spawn() {
        let tmp = TempDir::new().unwrap();
        let plain = tmp.path().join("not-a-repo");
        fs::create_dir_all(&plain).unwrap();

        let err = expect_refusal(
            resolve_isolation(true, "sess-iso-norepo", &plain, |_, _| {
                panic!("must not attempt to create a worktree outside a repository")
            }),
            "no repository means no isolation, so no spawn",
        );

        assert_eq!(err.kind, SpawnErrorKind::IsolationUnavailable);
        let detail = err.isolation.expect("the frontend needs the cause");
        assert_eq!(detail.reason, IsolationReason::NotARepository);
        assert!(
            !detail.retryable,
            "a directory will not become a repository by retrying"
        );
    }

    #[test]
    fn project_repo_check_distinguishes_repo_from_plain_directory() {
        let tmp = TempDir::new().unwrap();
        let plain = tmp.path().join("plain");
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&plain).unwrap();
        fs::create_dir_all(&repo).unwrap();
        worktree::init_test_repo(&repo).unwrap();

        assert!(!super::project_is_repo(
            plain.to_string_lossy().into_owned()
        ));
        assert!(super::project_is_repo(repo.to_string_lossy().into_owned()));
    }

    #[test]
    fn isolation_requested_and_available_runs_in_the_worktree() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        worktree::init_test_repo(&repo).unwrap();
        let base = tmp.path().to_path_buf();

        let (cwd, worktree) = resolve_isolation(true, "sess-iso-ok", &repo, |r, s| {
            worktree::create_with_base_dir(r, s, &base)
        })
        .expect("isolation that can be provided must still spawn");

        let wt = worktree.expect("the session must own its worktree for cleanup");
        assert_eq!(cwd, wt.path, "the session must run inside its worktree");
        assert_ne!(cwd, repo, "an isolated session must not run in the project");
        assert!(cwd.exists());
    }

    #[test]
    fn a_session_that_did_not_ask_for_isolation_is_unaffected() {
        let tmp = TempDir::new().unwrap();
        let plain = tmp.path().join("plain");
        fs::create_dir_all(&plain).unwrap();

        // Not a repository, and no worktree may be attempted — an unisolated
        // session must not acquire a git requirement it never had.
        let (cwd, worktree) = resolve_isolation(false, "sess-plain", &plain, |_, _| {
            panic!("must not create a worktree when isolation was not requested")
        })
        .expect("an unisolated session must still spawn");

        assert_eq!(cwd, plain, "it runs in the project directory, as before");
        assert!(worktree.is_none());
    }

    // Pins the wire contract the launcher UI is being built against.
    #[test]
    fn the_isolation_error_serializes_with_the_fields_the_frontend_branches_on() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        worktree::init_test_repo(&repo).unwrap();

        let err = expect_refusal(
            resolve_isolation(true, "sess-wire", &repo, create_fails),
            "isolation must be refused here",
        );
        let json = serde_json::to_value(&err).unwrap();

        assert_eq!(json["kind"], "isolationUnavailable");
        assert!(json["message"].is_string());
        assert_eq!(json["isolation"]["reason"], "worktreeCreateFailed");
        assert_eq!(json["isolation"]["sessionId"], "sess-wire");
        assert_eq!(json["isolation"]["retryable"], true);
        assert!(json["isolation"]["project"].is_string());
        assert!(json["isolation"]["detail"].is_string());

        // An ordinary failure stays distinguishable, so the launcher only offers
        // the isolation choice when isolation is actually what went wrong.
        let ordinary = serde_json::to_value(super::SpawnError::failed("boom")).unwrap();
        assert_eq!(ordinary["kind"], "failed");
        assert_eq!(ordinary["message"], "boom");
        assert!(
            ordinary.get("isolation").is_none(),
            "a non-isolation failure must not carry an isolation payload"
        );
    }

    // A dispatch is only lost in one direction: an Enter sent too early is
    // swallowed silently, while one sent late costs nothing but a pause. Every
    // case below is therefore written from the "hold unless certain" side.
    //
    // `baseline` is the output clock as of the write. Cases where it still equals
    // `last_output` are the target not having reacted yet.

    // Stands in for the output clock as of the write. Kept below the `now` values
    // used here so the cases stay arithmetically honest.
    const BASE: u64 = 100;

    // A payload small enough to add no delivery allowance, so the cases below
    // exercise the quiet/reacted rules against the unscaled base bounds. Size
    // scaling is covered separately further down.
    const SMALL: usize = 0;

    // The payload from the live repro: a ~1900-character dispatch that sat in
    // Codex's composer as "[Pasted Content 1946 chars]", unsubmitted.
    const REPRO: usize = 1946;

    #[test]
    fn holds_below_the_floor() {
        assert!(!ready_to_submit(
            SUBMIT_FLOOR_MS - 1,
            0,
            BASE + 1,
            BASE,
            SMALL
        ));
    }

    // The regression that shipped: a target which stays silent while it buffers a
    // paste has a last-output timestamp predating the write, so "quiet for long
    // enough" was true the instant the floor elapsed and Enter fired straight into
    // the paste. Silence before the target has said anything must not count.
    #[test]
    fn holds_while_a_silent_target_has_not_reacted_to_the_write_yet() {
        // Inside the ceiling window, so this isolates the has-it-reacted rule
        // rather than the backstop that eventually overrides it.
        let now = SUBMIT_FLOOR_MS + 500;
        assert!(now < SUBMIT_CEILING_MS);
        // Stale by design: the target has produced nothing since before the write,
        // so an elapsed-quiet check alone would read this as "finished" and fire.
        assert!(now - BASE >= SUBMIT_QUIET_MS);
        assert!(!ready_to_submit(now, 0, BASE, BASE, SMALL));
    }

    #[test]
    fn submits_once_the_target_has_spoken_and_then_gone_quiet() {
        let now = SUBMIT_FLOOR_MS + SUBMIT_QUIET_MS;
        let last_output = now - SUBMIT_QUIET_MS;
        assert!(last_output != BASE);
        assert!(ready_to_submit(now, 0, last_output, BASE, SMALL));
    }

    #[test]
    fn holds_while_the_target_is_still_producing_output() {
        let now = SUBMIT_FLOOR_MS + 500;
        assert!(!ready_to_submit(now, 0, now - 10, BASE, SMALL));
    }

    #[test]
    fn submits_at_the_ceiling_even_if_the_target_never_speaks() {
        // Backstop for a CLI that echoes nothing at all: without this the
        // has-it-reacted rule would hold the Enter forever.
        assert!(ready_to_submit(SUBMIT_CEILING_MS, 0, BASE, BASE, SMALL));
    }

    #[test]
    fn quiet_is_measured_from_the_last_output_not_from_the_write() {
        let now = SUBMIT_FLOOR_MS + 1_000;
        assert!(ready_to_submit(now, 0, now - SUBMIT_QUIET_MS, BASE, SMALL));
        assert!(!ready_to_submit(
            now,
            0,
            now - (SUBMIT_QUIET_MS - 1),
            BASE,
            SMALL
        ));
    }

    // Size scaling. Both bounds were fixed constants, so a 50-byte prompt and a
    // 50 KB one were given identical windows — the payload is the one input that
    // actually varies between dispatches.

    #[test]
    fn a_small_payload_keeps_the_original_bounds() {
        // Nothing that fit comfortably before may start waiting longer now.
        assert_eq!(submit_floor_ms(SMALL), SUBMIT_FLOOR_MS);
        assert_eq!(submit_ceiling_ms(SMALL), SUBMIT_CEILING_MS);
        // Anything under the floor's worth of bytes is still governed by the floor.
        let below = (SUBMIT_FLOOR_MS * SUBMIT_BYTES_PER_MS) as usize;
        assert_eq!(submit_floor_ms(below), SUBMIT_FLOOR_MS);
    }

    #[test]
    fn the_repro_payload_is_held_past_the_delay_it_was_seen_to_lose_enter_at() {
        // The dispatch that failed live was still arriving after 300 ms, so a
        // payload that size must not be released at 300 ms.
        let floor = submit_floor_ms(REPRO);
        assert!(
            floor > SUBMIT_FLOOR_MS,
            "a {REPRO}-byte payload must outwait the small-payload floor, got {floor}"
        );
        assert!(!ready_to_submit(SUBMIT_FLOOR_MS, 0, BASE + 1, BASE, REPRO));
    }

    // The hole that survived the quiet-detection fix. A pane that was already
    // streaming when the dispatch arrived moves its output clock off `baseline`
    // immediately, with output that has nothing to do with our paste — so the
    // has-it-reacted rule is satisfied by noise. A lull in that unrelated stream
    // then looks exactly like "finished receiving", and before the floor scaled
    // with size it would have fired an Enter into a paste still in delivery.
    #[test]
    fn a_lull_in_an_already_noisy_pane_cannot_release_a_large_paste_early() {
        // Sampled between the two floors, where the old fixed bound would have
        // released and the scaled one still holds. Derived rather than hardcoded
        // so retuning the rate cannot quietly empty this window.
        let now = SUBMIT_FLOOR_MS + (submit_floor_ms(REPRO) - SUBMIT_FLOOR_MS) / 2;
        assert!(now >= SUBMIT_FLOOR_MS && now < submit_floor_ms(REPRO));
        let last_output = now - SUBMIT_QUIET_MS;
        // The two rules that are supposed to protect us both read as satisfied:
        // the target has "reacted", and it has been quiet long enough.
        assert!(last_output != BASE);
        assert!(now - last_output >= SUBMIT_QUIET_MS);
        // Same instant, same clocks: a small payload goes, a large one is held.
        assert!(ready_to_submit(now, 0, last_output, BASE, SMALL));
        assert!(!ready_to_submit(now, 0, last_output, BASE, REPRO));
        // And it does go once the payload could actually have been delivered.
        let after = submit_floor_ms(REPRO) + SUBMIT_QUIET_MS;
        assert!(ready_to_submit(
            after,
            0,
            after - SUBMIT_QUIET_MS,
            BASE,
            REPRO
        ));
    }

    #[test]
    fn the_ceiling_never_expires_while_a_large_payload_is_still_arriving() {
        // The backstop is a fallback from the bug, so it must not reproduce it by
        // firing mid-delivery. It has to outlast the floor for every payload.
        for len in [SMALL, 1, REPRO, 64 * 1024, 8 * 1024 * 1024] {
            assert!(
                submit_ceiling_ms(len) > submit_floor_ms(len),
                "ceiling must outlast the floor at {len} bytes"
            );
        }
    }

    #[test]
    fn the_wait_stays_bounded_however_large_the_payload() {
        // Scaling must not become an unbounded timeout: a pathological prompt
        // still has to release Enter, and dispatch must not hang on it.
        let huge = usize::MAX;
        assert_eq!(delivery_allowance_ms(huge), SUBMIT_DELIVERY_CAP_MS);
        assert_eq!(
            submit_ceiling_ms(huge),
            SUBMIT_CEILING_MS + SUBMIT_DELIVERY_CAP_MS
        );
        assert!(ready_to_submit(
            SUBMIT_CEILING_MS + SUBMIT_DELIVERY_CAP_MS,
            0,
            BASE,
            BASE,
            huge
        ));
    }

    #[test]
    fn only_codex_is_framed_as_a_bracketed_paste() {
        assert!(is_codex("codex"));
        assert!(is_codex("Codex.cmd"));
        assert!(is_codex("CODEX.EXE"));
        // Claude Code already submits reliably and must keep its plain path.
        assert!(!is_codex("claude"));
        assert!(!is_codex("opencode"));
        assert!(!is_codex("powershell.exe"));
    }

    #[test]
    fn is_opencode_normalizes_case_and_windows_suffixes() {
        // The free-model guard keys on this, so every spelling a PATH lookup or
        // a custom command could produce has to be recognised.
        assert!(is_opencode("opencode"));
        assert!(is_opencode("OpenCode"));
        assert!(is_opencode("opencode.cmd"));
        assert!(is_opencode("OPENCODE.EXE"));
        assert!(!is_opencode("opencode-cli"));
        assert!(!is_opencode("codex"));
        assert!(!is_opencode("claude"));
    }

    #[test]
    fn is_agent_cli_recognizes_the_wired_clis_and_excludes_shells() {
        assert!(is_agent_cli("claude"));
        assert!(is_agent_cli("Codex.cmd"));
        assert!(is_agent_cli("OPENCODE.EXE"));
        assert!(!is_agent_cli("powershell.exe"));
        assert!(!is_agent_cli("cmd"));
        assert!(!is_agent_cli("bash"));
    }

    #[test]
    fn validate_human_dispatch_trims_and_requires_both_fields() {
        assert_eq!(
            validate_human_dispatch("  sess-2  ", "  do the thing  ").unwrap(),
            ("sess-2".to_string(), "do the thing".to_string())
        );
        assert!(validate_human_dispatch("", "task").is_err());
        assert!(validate_human_dispatch("sess-2", "   ").is_err());
        assert!(validate_human_dispatch("   ", "").is_err());
    }

    // The token is useless if it never reaches the agent, and each CLI takes it
    // a different way. These pin the three shapes so a config-format change
    // fails here rather than silently at runtime, where the symptom is a
    // session that connects and then 401s on every tool call.

    #[test]
    fn claude_carries_the_token_as_an_authorization_header() {
        let (args, env) = agent_mcp_wiring(
            "claude",
            "sess-tok-claude",
            "http://127.0.0.1:1/mcp",
            Some("secret123"),
        );
        let path = args
            .iter()
            .position(|a| a == "--mcp-config")
            .map(|i| args[i + 1].clone())
            .expect("claude gets a --mcp-config path");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(
            body.contains(r#""Authorization":"Bearer secret123""#),
            "{body}"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file_mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(
                file_mode & 0o777,
                0o600,
                "token file mode was {file_mode:o}"
            );
        }
        assert!(
            env.is_empty(),
            "claude reads the token from its config file"
        );
    }

    #[test]
    fn codex_passes_the_token_by_env_var_name_not_by_value() {
        let (args, env) = agent_mcp_wiring(
            "codex",
            "sess-tok-codex",
            "http://127.0.0.1:1/mcp",
            Some("secret123"),
        );
        let joined = args.join(" ");
        assert!(
            joined.contains(&format!("bearer_token_env_var={CODEX_TOKEN_ENV}")),
            "{joined}"
        );
        // The whole point: the secret is in the environment, never on the
        // command line, which any local process can read.
        assert!(
            !joined.contains("secret123"),
            "token leaked into argv: {joined}"
        );
        assert!(env.contains(&(CODEX_TOKEN_ENV.to_string(), "secret123".to_string())));
    }

    #[test]
    fn opencode_carries_the_token_as_an_authorization_header() {
        let (_, env) = agent_mcp_wiring(
            "opencode",
            "sess-tok-oc",
            "http://127.0.0.1:1/mcp",
            Some("secret123"),
        );
        let path = env
            .iter()
            .find(|(k, _)| k == "OPENCODE_CONFIG")
            .map(|(_, v)| v.clone())
            .expect("opencode gets OPENCODE_CONFIG");
        let body = std::fs::read_to_string(path).unwrap();
        assert!(
            body.contains(r#""Authorization":"Bearer secret123""#),
            "{body}"
        );
    }

    #[test]
    fn no_token_means_no_auth_stanza_anywhere() {
        // The shared-endpoint fallback has no per-session secret to present.
        // It must still produce a config the agent can load, not one with an
        // empty or malformed Authorization header.
        let (args, _) = agent_mcp_wiring("claude", "sess-tok-none", "http://127.0.0.1:1/mcp", None);
        let path = args
            .iter()
            .position(|a| a == "--mcp-config")
            .map(|i| args[i + 1].clone())
            .unwrap();
        let body = std::fs::read_to_string(path).unwrap();
        assert!(!body.contains("Authorization"), "{body}");
        serde_json::from_str::<serde_json::Value>(&body).expect("still valid JSON without a token");

        let (args, env) =
            agent_mcp_wiring("codex", "sess-tok-none2", "http://127.0.0.1:1/mcp", None);
        assert!(!args.join(" ").contains("bearer_token_env_var"));
        assert!(env.is_empty());
    }

    #[test]
    fn panes_advertise_a_color_capable_terminal() {
        // Without this the desktop entry's environment reaches the agent with
        // no TERM at all, and a CLI that checks TERM before emitting color
        // emits none: monochrome output in a pane that renders 24-bit fine.
        let cmd = build_command("bash", &[], Path::new("."), &[]);
        assert_eq!(cmd.get_env("TERM").unwrap(), "xterm-256color");
        assert_eq!(cmd.get_env("COLORTERM").unwrap(), "truecolor");
    }

    #[test]
    fn per_session_env_still_overrides_the_terminal_type() {
        // Per-session wiring is applied last on purpose, so a caller that has a
        // reason to declare a different terminal is not silently overruled.
        let extra = vec![("TERM".to_string(), "xterm-kitty".to_string())];
        let cmd = build_command("bash", &[], Path::new("."), &extra);
        assert_eq!(cmd.get_env("TERM").unwrap(), "xterm-kitty");
    }

    // Free-model guard tests: paid id, :free id, both router spellings,
    // local id, empty model.
    #[test]
    fn free_model_guard_refuses_paid_openrouter_id() {
        assert!(is_paid_openrouter_model("openrouter/claude-sonnet-4"));
        assert!(is_paid_openrouter_model("openrouter/gpt-4o"));
        assert!(is_paid_openrouter_model(
            "openrouter/anthropic/claude-3-5-sonnet"
        ));
    }

    #[test]
    fn free_model_guard_allows_free_openrouter_ids() {
        assert!(!is_paid_openrouter_model("openrouter/free"));
        assert!(!is_paid_openrouter_model("openrouter/openrouter/free"));
        assert!(!is_paid_openrouter_model(
            "openrouter/anthropic/claude-3-5-sonnet:free"
        ));
        assert!(!is_paid_openrouter_model("openrouter/any-model:free"));
    }

    #[test]
    fn free_model_guard_allows_local_and_empty_models() {
        assert!(!is_paid_openrouter_model("ollama/llama3"));
        assert!(!is_paid_openrouter_model("llama"));
        assert!(!is_paid_openrouter_model("lmstudio/mistral"));
        assert!(!is_paid_openrouter_model(""));
    }

    #[test]
    fn free_model_guard_ignores_case_in_the_prefix_and_suffix() {
        // OpenRouter resolves ids case-insensitively, so a re-cased id must be
        // judged the same as its lower-case form.
        assert!(is_paid_openrouter_model(
            "OpenRouter/anthropic/claude-3-5-sonnet"
        ));
        assert!(is_paid_openrouter_model("OPENROUTER/GPT-4O"));
        assert!(!is_paid_openrouter_model("OpenRouter/Free"));
        assert!(!is_paid_openrouter_model("OpenRouter/any-model:FREE"));
    }

    #[test]
    fn opencode_guard_requires_an_explicit_model() {
        // Without -m opencode falls back to config or its last-used model,
        // neither of which the guard can see, so the launch is refused.
        for missing in [None, Some(""), Some("   "), Some("\t\n")] {
            let err = opencode_model_guard(missing).unwrap_err();
            assert!(err.contains("needs an explicit model"), "{err}");
        }
    }

    #[test]
    fn opencode_guard_judges_the_supplied_model() {
        let err = opencode_model_guard(Some("openrouter/gpt-4o")).unwrap_err();
        assert!(err.contains("paid OpenRouter model"), "{err}");
        assert!(opencode_model_guard(Some("openrouter/free")).is_ok());
        assert!(opencode_model_guard(Some("openrouter/x:free")).is_ok());
        assert!(opencode_model_guard(Some("ollama/llama3")).is_ok());
        assert!(opencode_model_guard(Some(" lmstudio/mistral ")).is_ok());
    }

    #[test]
    fn free_model_guard_trims_whitespace_before_check() {
        // A pasted or quoted model id with surrounding whitespace must not be
        // silently downgraded from paid to free.
        assert!(is_paid_openrouter_model(" openrouter/claude-sonnet-4"));
        assert!(is_paid_openrouter_model("openrouter/claude-sonnet-4 "));
        assert!(is_paid_openrouter_model("\topenrouter/gpt-4o\n"));
        // Free-tier with whitespace is still free.
        assert!(!is_paid_openrouter_model(" openrouter/free "));
    }

    #[test]
    fn parse_and_filter_models_drops_paid_openrouter_ids() {
        let raw = "openrouter/free\n\
                    openrouter/gpt-4o\n\
                    openrouter/anthropic/claude-3-5-sonnet:free\n\
                    ollama/llama3\n";
        assert_eq!(
            parse_and_filter_models(raw),
            vec![
                "ollama/llama3",
                "openrouter/anthropic/claude-3-5-sonnet:free",
                "openrouter/free"
            ]
        );
    }

    #[test]
    fn parse_and_filter_models_trims_dedupes_and_sorts() {
        // Blank lines, surrounding whitespace, a duplicate id, and out-of-order
        // input are all things real CLI output can contain; none of them should
        // survive into the launcher's list.
        let raw = "\n  opencode/gpt-oss-120b-free  \nllama-3.1-8b\nllama-3.1-8b\n\topencode/gpt-oss-120b-free\t\n";
        assert_eq!(
            parse_and_filter_models(raw),
            vec!["llama-3.1-8b", "opencode/gpt-oss-120b-free"]
        );
    }

    #[test]
    fn parse_and_filter_models_of_empty_input_is_empty() {
        assert!(parse_and_filter_models("").is_empty());
        assert!(parse_and_filter_models("\n\n   \n").is_empty());
    }

    #[test]
    fn list_models_skips_programs_other_than_opencode() {
        // These must not spawn anything: an empty Ok is returned immediately so
        // the frontend falls back to Custom for CLIs with no list command.
        assert_eq!(
            list_models("claude".to_string()).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            list_models("codex".to_string()).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            list_models("bash".to_string()).unwrap(),
            Vec::<String>::new()
        );
    }

    // `run_with_timeout` is exercised directly with real child processes
    // (rather than only through `list_models`, which is pinned to the
    // literal program name "opencode") so the timeout and concurrent-read
    // behaviour is covered without needing an `opencode` binary on PATH.
    // `sh` makes these Unix-only.

    #[test]
    #[cfg(unix)]
    fn run_with_timeout_kills_a_child_that_closes_its_pipes_but_keeps_running() {
        // Closes stdout and stderr immediately, then sleeps far longer than
        // the timeout below. A timeout that only bounded reading the (now
        // closed, immediately-EOF) pipes would let this sleep run to
        // completion instead of killing it.
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "exec >/dev/null 2>&1; sleep 5"]);
        let started = Instant::now();
        let err = run_with_timeout(cmd, Duration::from_millis(200)).unwrap_err();
        assert!(err.contains("timed out"), "{err}");
        // Killed at the deadline, not left to run out its 5s sleep.
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "took {:?}, should have been killed well before the child's sleep finished",
            started.elapsed()
        );
    }

    #[test]
    #[cfg(unix)]
    fn run_with_timeout_drains_a_full_stderr_without_stalling_stdout() {
        // Writes 200000 bytes to stderr (comfortably past a pipe's default
        // buffer size) before writing to stdout and exiting. Reading stdout
        // to EOF first (the sequential-read bug this replaces) would block
        // forever here: the child cannot finish (and so cannot close stdout)
        // until its stderr write is drained, which a sequential reader never
        // gets to until stdout already closed.
        let mut cmd = Command::new("sh");
        cmd.args([
            "-c",
            "head -c 200000 /dev/zero | tr '\\0' 'e' 1>&2; printf out",
        ]);
        let started = Instant::now();
        let (status, out, err) = run_with_timeout(cmd, Duration::from_secs(10)).unwrap();
        assert!(status.success());
        assert_eq!(out, "out");
        assert_eq!(err.len(), 200_000);
        assert!(err.chars().all(|c| c == 'e'));
        // Finished on its own long before the generous timeout above; this
        // guards against a regression back to a stall that only the timeout
        // would eventually break.
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "took {:?}, should have completed almost immediately",
            started.elapsed()
        );
    }

    #[test]
    #[cfg(unix)]
    fn run_with_timeout_is_bounded_even_when_a_descendant_outlives_the_child() {
        // The shell backgrounds `sleep 2` (which inherits the piped stdout
        // and stderr) and then exits immediately itself. try_wait sees the
        // shell exit almost instantly, well inside the timeout, but the
        // sleep process is still holding the write end of both pipes open:
        // a plain `read_to_string`/thread join would then block for the
        // sleep's remaining 2s, not the shell's near-zero runtime. The
        // deadline on collecting output is what has to catch this, since the
        // process-exit deadline above already passed.
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 2 & exit 0"]);
        let started = Instant::now();
        let err = run_with_timeout(cmd, Duration::from_millis(200)).unwrap_err();
        assert!(err.contains("timed out"), "{err}");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "took {:?}, should have returned around the 200ms deadline, \
             not waited out the backgrounded sleep",
            started.elapsed()
        );
    }
}
