// Pantheon "shared brain": an in-process MCP server on loopback that every agent
// CLI connects to. Agents publish decisions/facts/broadcasts and read the shared
// context, so one agent's decision instantly becomes another's knowledge. The
// tool handlers touch app state directly (same process), and each write emits a
// `context-changed` event so the sidebar updates live.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::response::IntoResponse;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::tower::StreamableHttpService;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// The largest injection that cannot lose a byte on the way to a pane.
///
/// Delivery drops every *complete* leading 1 KiB chunk and lands only the
/// trailing partial one (measured; see BACKLOG.md). An injection strictly
/// under one chunk has no complete leading chunk, so there is nothing for the
/// mechanism to take. That is what makes this a guarantee rather than a
/// mitigation, and it holds whatever turns out to be doing the chunking.
///
/// The real ceiling a conductor feels is lower: the wrapper costs 82 bytes of
/// header (matching the measured prefix) and 111 of completion contract, so
/// about **830 bytes of brief** get through. That is tight, and deliberately
/// so: a brief that does not fit is one that should have been split or put in
/// a file. Once the chunking is found and fixed this constant is the single
/// place to raise.
const MAX_INJECTION_BYTES: usize = 1024;

/// How far an injection exceeds what can be delivered intact, or `None` if it
/// fits. Bytes, not chars: the truncation is a byte-buffer effect.
fn injection_overage(injection: &str) -> Option<usize> {
    let len = injection.len();
    // `>=`, not `>`: at exactly 1024 there is one complete leading chunk, and
    // it is the one that gets dropped.
    (len >= MAX_INJECTION_BYTES).then(|| len - MAX_INJECTION_BYTES + 1)
}

/// The refusal for a brief too long to deliver whole, or `None` to proceed.
///
/// A free function for the same reason `dispatch_precheck` is one: `Shared`
/// needs a Tauri `AppHandle` and cannot be built in a unit test, so policy
/// that lives inside a method is policy nothing can assert on.
///
/// The message has to give the conductor the move, not just the verdict. It is
/// read by a model deciding what to do next, and "too long" alone invites a
/// retry of the same brief.
fn oversize_refusal(injection: &str) -> Option<String> {
    injection_overage(injection).map(|over| {
        format!(
            // "a {MAX} byte limit" read as though 1024 were allowed. It is the
            // refusal threshold, so the largest that goes through is one less.
            "brief is {over} bytes too long to deliver intact ({} bytes; the most that \
             can be delivered is {}). Longer briefs reach the agent with the beginning \
             silently missing. Split this into smaller tasks, or shorten it and point \
             the agent at a file for the detail.",
            injection.len(),
            MAX_INJECTION_BYTES - 1
        )
    })
}

/// Collapse CR/LF into spaces so an injected message stays on one terminal
/// line. Embedded CR/LF characters can become unintended submit events in a
/// target CLI. Preserves all other whitespace, because quoted commands and
/// code fragments may depend on it. Shared by every message Pantheon types
/// into a pane: a dispatch brief, a retarget, a review request, a rework
/// notice.
fn single_line(s: &str) -> String {
    s.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

fn dispatch_prompt(conductor: &str, task_id: &str, task: &str) -> String {
    let task = single_line(task);

    // Deliberately lean. Every byte spent here is a byte the brief cannot
    // have before `MAX_INJECTION_BYTES` refuses the dispatch, so anything the
    // agent can learn from an MCP tool call does not belong in the terminal.
    format!(
        "[pantheon] Task from conductor '{conductor}' (task_id {task_id}): {task} \
         When done, call the pantheon complete_task tool with task_id \"{task_id}\" \
         and your result."
    )
}

/// Truncate `s` to at most `max_bytes`, on a char boundary, marking the cut.
/// Byte-based to match `MAX_INJECTION_BYTES`, which measures the same way.
fn truncate_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes.saturating_sub(3);
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

/// Build an injection from a fixed wrapper and one variable part, shrinking
/// the variable part to fit `MAX_INJECTION_BYTES` rather than refusing.
///
/// This is the other half of size handling from `oversize_refusal`: a
/// dispatch has a caller to hand a refusal back to, but a review request or
/// rework notice is typed in on its own, by `drain_pane`, with nobody on
/// the line to refuse to. Truncating instead is safe here specifically
/// because the full text stays reachable by id through `get_task_result`:
/// nothing is actually lost, only what reaches the terminal is shortened.
fn fit_injection(build: impl Fn(&str) -> String, variable: &str) -> String {
    let full = build(variable);
    match injection_overage(&full) {
        None => full,
        Some(over) => {
            let trimmed = truncate_bytes(variable, variable.len().saturating_sub(over));
            build(&trimmed)
        }
    }
}

/// What a reviewer's terminal receives when a task reaches them for review.
fn review_request_notice(id: &str, target: &str, brief: &str) -> String {
    let brief = single_line(&truncate_chars(brief, TASK_ECHO_CHARS));
    fit_injection(
        |v| {
            format!(
                "[pantheon] Review task {id} from {target}: {v} Call the pantheon \
                 get_task_result tool with that id for the result, then review_task with \
                 verdict approve or rework."
            )
        },
        &brief,
    )
}

/// What a target's terminal receives when its reviewer sends a task back.
fn rework_notice(id: &str, reviewer: &str, findings: &str) -> String {
    let findings = single_line(&truncate_chars(findings, TASK_ECHO_CHARS));
    fit_injection(
        |v| {
            format!(
                "[pantheon] Task {id} sent back by {reviewer}: {v} Fix, then call the \
                 pantheon complete_task tool again with task_id \"{id}\"."
            )
        },
        &findings,
    )
}

/// What a pane is told, in its own terminal, at the moment it becomes conductor.
///
/// This is *typed into the composer and left there unsent*: see `set_conductor`
/// for why. That shapes the text: it has to be short enough to read at a glance
/// and to type after, because the user is expected to append their actual first
/// instruction to it and send both together.
///
/// So this carries only what MCP cannot: the role is live *now*, and who is
/// actually running. The playbook for writing a task, recording decisions, and
/// choosing subagents versus sessions is already in BRAIN_INSTRUCTIONS, which
/// every agent receives on connect. Repeating it here just buried the two facts
/// that were new.
fn conductor_briefing(peers: &[String]) -> String {
    // Single line: a newline lands in most composers as a submit, which would
    // fire this off half-written: exactly what we are avoiding.
    let roster = if peers.is_empty() {
        "No other sessions are open yet (Ctrl+K opens one).".to_string()
    } else {
        format!(
            "Live sessions you can dispatch to: {}.",
            // Just id, kind, and model. `roster_lines` also carries brain= and the
            // conductor marker, which are useful in list_sessions output but are
            // noise in a line the user has to read and type around; the agent can
            // call list_sessions for the full picture.
            peers
                .iter()
                .map(|l| {
                    let l = l.trim_start_matches("- ").replace('\n', " ");
                    match l.find(" brain=") {
                        Some(i) => l[..i].to_string(),
                        None => l,
                    }
                })
                .collect::<Vec<_>>()
                .join("; ")
        )
    };
    format!(
        "[pantheon] You are now the conductor of this workspace. \
{roster} \
Each is a live agent idling until you give it work. Use the pantheon dispatch tool rather than doing separable work yourself; it returns immediately, so fan out every independent piece and collect with get_task_result. "
    )
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Entry {
    pub kind: String, // "decision" | "fact" | "broadcast"
    pub author: String,
    pub topic: String,
    pub body: String,
    pub ts_ms: u64,
    pub room: String, // which brain this belongs to
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct AgentSession {
    pub name: String,
    pub kind: String,
    /// The model id the user selected at launch, or empty when none was given.
    /// Empty is a deliberate distinction from "unknown": an empty string means
    /// the user chose not to pass one, and the CLI picked its own default.
    #[serde(default)]
    pub model: String,
}

/// One dispatched unit of work, from the conductor to another session.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Task {
    pub id: String,
    pub from: String,
    pub target: String,
    pub task: String,
    /// "pending" | "overdue" | "in_review" | "rework" | "done" | "cancelled"
    /// | "error" | "abandoned".
    /// "overdue" is non-terminal: still running, result still accepted.
    /// "in_review" and "rework" are also non-terminal: the work exists but has
    /// not been signed off, which is the whole point of them being distinct
    /// from "done".
    /// "abandoned" is terminal and means the pane holding this work is gone. It
    /// is deliberately distinct from "cancelled", which a human chose, and from
    /// "overdue", which is still working. Collapsing the three loses the only
    /// question a conductor actually has: is anyone still doing this.
    pub status: String,
    pub result: String,
    pub ts_ms: u64,
    /// Session that must sign off before this counts as done. Empty means the
    /// conductor waived review, which is the honest way to express "a typo fix
    /// does not need a round trip" without leaving it to whoever remembers.
    ///
    /// `serde(default)` is load-bearing: every task already written to
    /// `brain.jsonl` predates this field, and a workspace must not lose its
    /// history to a schema change.
    #[serde(default)]
    pub reviewer: String,
    /// What the reviewer said, kept whether they approved or rejected. A
    /// rejection whose reasons are discarded makes the next attempt guesswork.
    #[serde(default)]
    pub findings: String,
    /// When this task reached a terminal state, if it has. Distinct from
    /// `ts_ms`, which is stamped once at dispatch and never moves: reading
    /// that as a completion time says a task "finished" the moment it was
    /// handed out.
    ///
    /// `None` while the task is still live, including `in_review` and
    /// `rework`: neither is finished, so neither may claim a finish time.
    ///
    /// This is what lets the UI tell a result that just arrived from one that
    /// was already sitting in the store at startup. Without it, a restart
    /// cannot distinguish them and announces the whole history at once.
    ///
    /// `serde(default)` for the same reason as the fields above: tasks already
    /// written to `brain.jsonl` predate it, and they load as `None`, which is
    /// the honest answer for a task whose finish time was never recorded.
    #[serde(default)]
    pub done_ms: Option<u64>,
    /// Questions this task's agent put to the conductor, with their answers.
    ///
    /// Kept on the task rather than thrown away after delivery because a
    /// question answered and then lost is one the next agent asks again. This
    /// is a decision made mid-task, and the reasoning behind a brief is exactly
    /// what the brief had to compress out.
    ///
    /// The last entry with an empty `answer` is the open question; there is at
    /// most one, because asking blocks. `serde(default)` for the same reason as
    /// the fields above: older records predate it and load as empty.
    #[serde(default)]
    pub exchanges: Vec<Exchange>,
    /// True while this task is `in_review` or `rework` and its named reviewer
    /// is not live. Not abandonment: the submitted work already exists, so a
    /// dead reviewer means nobody will sign it off, not that nothing is there
    /// to sign off on. Set and cleared by the same sweep that abandons a dead
    /// target, so it is never stale by more than one reconciliation.
    ///
    /// `serde(default)` for the same reason as the fields above: tasks already
    /// written to `brain.jsonl` predate it and load as `false`, which is the
    /// honest answer before the first sweep has looked.
    #[serde(default)]
    pub reviewer_gone: bool,
    /// Whether the message this task owes its own pane has reached it: the
    /// review request for an `in_review` task (to `reviewer`), or the rework
    /// notice for a `rework` task (to `target`). Meaningless, and left
    /// `true`, for every other status: there is nothing to notify about.
    ///
    /// `serde(default)` loads every task written before this field as
    /// `false`: "not yet delivered". That is the honest answer for an
    /// `in_review` or `rework` task from before Pantheon had any delivery
    /// mechanism at all, and it is also the useful one, closing what
    /// BACKLOG.md's "review request was never delivered" entry named: a
    /// workspace upgrading mid-run retries exactly the notifications that
    /// never had anywhere to go.
    #[serde(default)]
    pub notice_delivered: bool,
}

/// One question from a working agent and the conductor's answer to it.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Exchange {
    pub question: String,
    /// Empty while the question is still open.
    pub answer: String,
    pub asked_ms: u64,
}

/// Terminal states: the ones that stamp `done_ms` and never accept a result
/// afterwards. `in_review` and `rework` are deliberately absent: the work
/// exists but has not been signed off, which is the whole point of them.
///
/// `abandoned` is terminal because the pane that held the work no longer
/// exists. Its per-session MCP endpoint was shut down with it (see
/// `release_session`), so there is no longer any caller that could report a
/// result for the task. This is the one terminal state nobody chose.
fn is_terminal(status: &str) -> bool {
    matches!(status, "done" | "cancelled" | "error" | "abandoned")
}

/// The status a task reaches when the pane doing it is gone.
///
/// A named constant rather than a bare string because this value is compared in
/// five places and written in three, and a typo in any of them produces a task
/// that is silently neither open nor terminal.
pub const STATUS_ABANDONED: &str = "abandoned";

/// The status a task holds while its agent waits on an answer from the
/// conductor. Open, not terminal: the agent is alive and the work is unfinished.
pub const STATUS_BLOCKED: &str = "blocked";

/// The status a freshly dispatched (or redirected) task holds while its
/// target pane is occupied: recorded, but not yet typed into any terminal.
/// Open, not terminal, and deliberately not counted as "busy" the way
/// `pending` is (see `attribute_open_tasks`), because nothing is running
/// yet for it to be slow at. `drain_pane` is what moves a task off this
/// status once its pane is free.
pub const STATUS_QUEUED: &str = "queued";

/// How many tasks may sit queued behind one pane at once. A ceiling for the
/// same reason `MAX_DISPATCHES` and `MAX_QUESTIONS_PER_TASK` are: a
/// conductor that keeps dispatching to a pane that never catches up should
/// be told to stop and reconsider, not let the queue grow without limit.
const QUEUE_CAP: usize = 3;

/// How many questions one task may put to the conductor.
///
/// A pane that can interrupt the conductor can do so in a loop, and the
/// conductor's context is the scarce resource in a long session. Same reasoning
/// as `MAX_DISPATCHES`, and the same shape of answer: a hard ceiling that
/// refuses rather than a heuristic that hopes.
const MAX_QUESTIONS_PER_TASK: usize = 5;

/// How long a blocked agent waits for its answer before giving up and deciding
/// for itself.
///
/// Bounded for the same reason `wait_for_tasks` is: a conductor that never
/// answers must not strand a pane forever. On expiry the agent is told to use
/// its own judgement and say what it assumed, which is strictly better than
/// stalling, and the question stays on the task so the record is not lost.
const ASK_TIMEOUT_SECS: u64 = 900;

/// How often a blocked agent re-checks for its answer.
const ASK_POLL_MS: u64 = 1500;

const STORE_FILE: &str = "brain.jsonl";

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum StoreRecord {
    Entry(Entry),
    Session(AgentSession),
    Task(Task),
}

#[derive(Default)]
struct StoredBrain {
    entries: Vec<Entry>,
    sessions: Vec<AgentSession>,
    tasks: Vec<Task>,
}

fn load_brain(dir: &Path) -> StoredBrain {
    let Ok(contents) = fs::read_to_string(dir.join(STORE_FILE)) else {
        return StoredBrain::default();
    };
    let mut brain = StoredBrain::default();
    for record in contents
        .lines()
        .filter_map(|line| serde_json::from_str::<StoreRecord>(line).ok())
    {
        match record {
            StoreRecord::Entry(entry) => brain.entries.push(entry),
            StoreRecord::Session(session) => {
                if let Some(existing) = brain.sessions.iter_mut().find(|s| s.name == session.name) {
                    *existing = session;
                } else {
                    brain.sessions.push(session);
                }
            }
            StoreRecord::Task(task) => {
                if let Some(existing) = brain.tasks.iter_mut().find(|t| t.id == task.id) {
                    *existing = task;
                } else {
                    brain.tasks.push(task);
                }
            }
        }
    }
    brain
}

fn append_record(dir: &Path, record: &StoreRecord) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let mut line = serde_json::to_string(record).map_err(std::io::Error::other)?;
    line.push('\n');
    append_line(&dir.join(STORE_FILE), &line)
}

/// Guardrails. Note that depth is bounded structurally: only the conductor may
/// dispatch, so a dispatched agent cannot dispatch onward.
const MAX_DISPATCHES: u32 = 40;

/// How long before a still-running task is reported as "overdue".
///
/// This is a reporting threshold, not a deadline. Nothing here cancels an
/// agent: the dispatched CLI keeps working, and its `complete_task` call is
/// still accepted afterwards. Crossing this line only changes what the
/// conductor is told, so that a slow task is visible without its result being
/// thrown away.
///
/// Twenty minutes because real dispatched work routinely runs past ten. A
/// threshold most tasks trip is noise, and noise gets ignored.
const TASK_OVERDUE_MS: u64 = 20 * 60 * 1000;

/// How long `wait_for_tasks` waits when the caller does not say.
///
/// Measured 2026-09-03: a Claude Code pane's MCP transport kills the call
/// somewhere between 45 and 110 seconds ("The operation timed out"), while a
/// 45 second call reliably returns. The previous ten-minute default never
/// fired on that host at all: the transport gave up first, so the caller got
/// a transport error rather than the honest "still running" this tool exists
/// to report, and had no way to tell the two apart. 45 sits inside the
/// measured working range rather than at its edge, and the tool description
/// says outright that a wait returns within this window and should simply be
/// called again: a conductor that treats one call as the whole wait was the
/// failure mode a longer default invited.
const WAIT_DEFAULT_SECS: u64 = 45;

/// The longest any single wait may last, whatever the caller asks for.
///
/// A wait that could be unbounded is a wait that can hang a conductor forever
/// on a pane that will never answer. Dead panes are settled by
/// `reconcile_abandoned`, which ends a wait properly, but a live pane that has
/// simply stopped is not detectable, and this is the backstop for that case.
/// Capped at 55, not 1800: the same transport measurement that set
/// `WAIT_DEFAULT_SECS` puts failure somewhere between 45 and 110 seconds, so a
/// cap anywhere near the old 30 minutes was never reachable from a Claude Code
/// pane: the call would die en route and look like a hang rather than the
/// timeout report this tool is supposed to hand back.
const WAIT_MAX_SECS: u64 = 55;

/// How often a wait re-reads task state.
///
/// Polling rather than a condvar because the wait has to re-run liveness
/// reconciliation on each pass, not merely observe a status write: a pane can
/// die without anything writing a task record, and that death is exactly what
/// has to end the wait. Two seconds is far below any realistic task duration
/// and costs one non-blocking probe per pane per pass.
const WAIT_POLL_MS: u64 = 2000;

/// A task record was created, and either typed in or queued. `delivered` is
/// false both when the target was occupied (`queued` is `true`) and when the
/// prompt could not be written to a free target's terminal (`queued` is
/// `false` and the task exists in "error" status), so check `queued` to tell
/// the two apart.
#[derive(Clone, Debug, Serialize)]
pub struct DispatchOutcome {
    pub task_id: String,
    pub delivered: bool,
    /// Who will review it, empty when nobody will. Reported back because the
    /// conductor usually did not choose this: omitting a reviewer picks one,
    /// and a conductor that is not told which cannot follow up.
    pub reviewer: String,
    /// The id of the task this one is queued behind, set exactly when
    /// `queued` is `true`. Before the per-pane queue existed, a dispatch to
    /// an occupied target still typed the new brief in on top of the old
    /// one; this field named the older task so the conductor could act on
    /// it. Queueing closed that gap, and this now names what the new task
    /// is actually waiting on.
    pub already_busy: Option<String>,
    /// True when this dispatch was held rather than typed in immediately,
    /// because the target pane was occupied.
    pub queued: bool,
    /// 1-based position in the target's queue, set exactly when `queued` is
    /// `true`.
    pub queue_position: Option<usize>,
}

/// The checks a dispatch must pass before any task record is created, in the
/// order they're applied: that order is what decides which message wins when
/// more than one would refuse. Kept pure (no lock, no I/O) so it's testable
/// without a live `Shared`, which needs a real `AppHandle` to construct.
///
/// The dispatch budget is deliberately NOT one of these checks: consuming it
/// has to be atomic with checking it (or two concurrent dispatches could both
/// pass), so `dispatch_task` calls `take_dispatch_budget` itself, last, under
/// its own lock.
fn dispatch_precheck(
    halted: bool,
    from: &str,
    target: &str,
    target_is_live: bool,
    injection: &str,
) -> Result<(), String> {
    if halted {
        return Err("dispatch is halted by the user (Stop). Do not retry.".to_string());
    }
    if target == from {
        return Err("cannot dispatch to yourself.".to_string());
    }
    if !target_is_live {
        return Err(format!(
            "no live session '{target}'. Call list_sessions for valid targets."
        ));
    }
    // Last, so a conductor that got the target wrong hears about the target.
    // Both errors are actionable, but only one of them is about the thing the
    // conductor just typed.
    if let Some(refusal) = oversize_refusal(injection) {
        return Err(refusal);
    }
    Ok(())
}

/// How `Shared` tells the UI something changed.
///
/// A seam, for one reason: `Shared` used to hold an `AppHandle` directly, and
/// an `AppHandle` cannot be built outside a running app. That made every policy
/// living on a `Shared` method unreachable from a test, so it could only be
/// verified by reading the code, which is exactly how the dispatch gate's
/// ordering went untested until a reviewer asked.
///
/// `tauri::test::mock_app()` is the other way to solve this, but its handle is
/// `AppHandle<MockRuntime>`, so taking it means making `Shared` generic over the
/// runtime and threading that parameter through every caller. This is smaller
/// and the emissions are fire-and-forget anyway.
/// A closure rather than an `AppHandle` variant, and that detail is the whole
/// point. Holding an `AppHandle` anywhere in `Shared`'s type graph makes the
/// compiler instantiate `AppHandle<Wry>`'s drop glue for any test that builds a
/// `Shared`, which drags Wry's runtime into the test binary and fails it at
/// *load* time with `STATUS_ENTRYPOINT_NOT_FOUND`: before a single test runs,
/// with no hint as to the cause. Erasing the handle behind `dyn Fn` keeps it
/// out of `Shared` entirely.
/// Named so the `Notifier` field stays readable; clippy flags the raw form as
/// a very complex type.
type EmitFn = Box<dyn Fn(&str) + Send + Sync>;

pub struct Notifier(Option<EmitFn>);

impl Notifier {
    pub fn to_app(app: AppHandle) -> Self {
        Notifier(Some(Box::new(move |event| {
            let _ = app.emit(event, ());
        })))
    }

    /// Drops events. Tests assert on state, not on notifications.
    #[cfg(test)]
    pub fn silent() -> Self {
        Notifier(None)
    }

    fn emit(&self, event: &str) {
        if let Some(send) = &self.0 {
            send(event);
        }
    }
}

/// The shared store: one instance, cloned by Arc into every agent's handler.
pub struct Shared {
    app: Notifier,
    /// Where entries are mirrored as markdown. Follows the picked project, so it
    /// changes at runtime rather than being fixed at startup.
    dir: Mutex<PathBuf>,
    entries: Mutex<Vec<Entry>>,
    sessions: Mutex<Vec<AgentSession>>,
    /// agent name -> brain (room). The app owns this; drag reassigns it live.
    name_to_room: Mutex<HashMap<String, String>>,
    /// The live session engine, which lets dispatch type into a target's
    /// terminal.
    engine: Arc<crate::SessionManager>,
    /// Which agent (if any) is the conductor. Set by the app, never self-claimed.
    conductor: Mutex<Option<String>>,
    /// Panes whose MCP endpoint has actually received a request since their
    /// most recent spawn: the CLI is up, reading its terminal, and this is
    /// not merely a live PTY that could still be mid-boot. `note_session`
    /// runs before the PTY and its engine handle even exist, and a live
    /// process still starting up cannot receive typed input either, so
    /// neither is proof a pane can be delivered to; a connection is.
    /// `drain_pane` refuses to attempt anything against a pane not in this
    /// set. Cleared on respawn (`note_session`) so a stale connection from a
    /// prior incarnation of the same pane id cannot mark the new one ready.
    connected: Mutex<HashSet<String>>,
    /// Global kill-switch for all dispatch.
    halted: Mutex<bool>,
    tasks: Mutex<Vec<Task>>,
    dispatches: Mutex<u32>,
    /// Gates every terminal write, separately from `tasks`. `set_halted`
    /// takes this exclusively while it flips the flag and cancels, so a
    /// delivery path holding it as a reader cannot have Stop land between
    /// its eligibility check and the write; readers hold it only across
    /// that check and the write itself, never across the task mutex, so a
    /// blocked PTY write stalls other deliveries but never a completion,
    /// review, or cancellation.
    delivery: RwLock<()>,
    /// One mutex per pane, handed out by `pane_delivery_lock`. `delivery`
    /// above is a single `RwLock` shared by every delivery path, and
    /// `RwLock::read` lets any number of readers run at once by design, so
    /// it only ever excludes Stop and cancel (the writers) from a delivery
    /// path, never one delivery path from another: two concurrent drains of
    /// the same pane could both select the same candidate before either
    /// promoted it, and a fresh dispatch could see a pane as free in the
    /// same window, landing concurrently with, and out of order with, a
    /// drain that had already picked its queue head but not yet delivered
    /// it. `drain_pane`, `dispatch_task`, and `reassign_task`'s own delivery
    /// all hold the lock for their pane across the whole "decide what this
    /// pane gets next, then send it" section, so only one of them can ever
    /// be inside that section for a given pane at a time. Entries are never
    /// removed: a project has few, long-lived pane names, so the map does
    /// not grow without bound.
    pane_delivery: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    /// A test-only pause point, absent from every non-test build (the field
    /// does not exist at all outside `#[cfg(test)]`, so this costs nothing
    /// in production). `hit_test_seam` calls the hook, if one is set, at a
    /// handful of internal boundaries a real race would need to land on:
    /// after `drain_pane` selects a candidate, after `enqueue_within_cap`'s
    /// cap check passes, and after `drain_pane` acquires its pane lock. A
    /// test sets a hook with `set_test_seam` that blocks the calling thread
    /// (a channel receive, not a sleep), giving the test explicit control
    /// over when each thread is allowed past that boundary instead of
    /// hoping the scheduler happens to interleave two threads there. See
    /// the sixth-round tests below for why: a barrier at a function's
    /// entrance, or a sleep assumed long enough for another thread to reach
    /// a lock, both let the unfixed code pass by accident when the OS
    /// schedules the two threads sequentially instead of concurrently.
    #[cfg(test)]
    test_seam: Mutex<Option<Arc<dyn Fn(&str) + Send + Sync>>>,
}

impl Shared {
    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Re-point storage (the user picked a different project) and rehydrate it.
    ///
    /// A live pane's identity must survive the switch. `set_project` restores
    /// panes and records their real kind (`note_session`) before the frontend
    /// calls this, so the sessions held in memory at this instant are correct
    /// for whichever panes are actually running; the new dir's own journal may
    /// still say whatever it last recorded for those names. `merge_live_identity`
    /// keeps this call's in-memory truth for every name that is live and lets
    /// the journal stand for everything else, so a pane spawned into the old
    /// dir does not read back the previous project's CLI once the new one
    /// loads. See `merge_live_identity` for the rule itself and why it is a
    /// pure helper rather than inline here.
    pub fn set_dir(&self, dir: PathBuf) {
        let brain = load_brain(&dir);
        let live = self.live_ids();
        let (sessions, changed) =
            merge_live_identity(&self.sessions_snapshot(), &brain.sessions, &live);
        *self.dir.lock().unwrap() = dir.clone();
        *self.entries.lock().unwrap() = brain.entries;
        *self.sessions.lock().unwrap() = sessions;
        *self.tasks.lock().unwrap() = brain.tasks;
        // Same semantics as `note_session`: append one record per session that
        // actually changed, to the dir just loaded, so the correction is not
        // lost the next time this project's journal is read.
        for session in &changed {
            let _ = append_record(&dir, &StoreRecord::Session(session.clone()));
        }
        self.app.emit("context-changed");
        self.app.emit("conductor-changed");
        // The tasks just loaded may hold a queued brief on a pane that is
        // already connected, or an undelivered review request or rework
        // notice for one, left over from before the project switched.
        // Nothing else sweeps for that, so this is the moment to. Swept by
        // `connected`, not `live`: a live pane can still be mid-boot with no
        // engine handle yet, which `drain_pane` would refuse anyway (see
        // `is_connected`), so sweeping `live` here would mostly cost a
        // lock-and-no-op per pane for no benefit.
        let connected: Vec<String> = self.connected.lock().unwrap().iter().cloned().collect();
        for id in &connected {
            self.drain_pane(id);
        }
    }

    /// The brain a given agent name is currently in. Defaults to "main".
    pub fn room_for(&self, name: &str) -> String {
        self.name_to_room
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .unwrap_or_else(|| "main".to_string())
    }

    /// Assign an agent name to a brain. Called by the app on spawn and on drag,
    /// so re-homing a running agent takes effect on its next tool call.
    fn try_set_room(&self, name: &str, room: &str) -> Result<(), String> {
        validate_path_component("room", room)?;
        self.name_to_room
            .lock()
            .unwrap()
            .insert(name.to_string(), room.to_string());
        self.app.emit("context-changed");
        Ok(())
    }

    pub fn set_room(&self, name: &str, room: &str) {
        let _ = self.try_set_room(name, room);
    }

    /// Append an entry (tagged with the author's current brain) to memory + its
    /// human-readable markdown file, then notify the UI.
    fn add(&self, kind: &str, author: &str, topic: &str, body: &str) {
        let room = self.room_for(author);
        let entry = Entry {
            kind: kind.to_string(),
            author: author.to_string(),
            topic: topic.to_string(),
            body: body.to_string(),
            ts_ms: Self::now_ms(),
            room: room.clone(),
        };
        let dir = self.dir.lock().unwrap().clone();
        let _ = fs::create_dir_all(&dir);
        let file = dir.join(format!("{room}-{kind}s.md"));
        let _ = append_line(&file, &format!("- **{topic}** ({author}): {body}\n"));
        let _ = append_record(&dir, &StoreRecord::Entry(entry.clone()));
        self.entries.lock().unwrap().push(entry);
        self.app.emit("context-changed");
    }

    pub fn entries_snapshot(&self) -> Vec<Entry> {
        self.entries.lock().unwrap().clone()
    }

    pub fn sessions_snapshot(&self) -> Vec<AgentSession> {
        self.sessions.lock().unwrap().clone()
    }

    /// Record a session the app already knows about (dedicated endpoint), so it
    /// shows up in list_sessions without the agent announcing itself.
    ///
    /// Refreshes whichever of `kind` or `model` changed when a name
    /// reconnects, rather than recording it once and never looking again. A
    /// pane's id is stable across relaunches but the CLI in it is not, the
    /// same id can be closed as one agent and reopened as another, and its
    /// model can change on a relaunch with a different flag; a roster line
    /// that still named the old CLI or model was actively misleading a
    /// conductor about what it was dispatching to.
    ///
    /// Called at MCP-wiring time in `spawn_session`, before the PTY and its
    /// engine handle exist. An earlier version drained the pane from here;
    /// that meant every restart errored each restored pane's queue at spawn
    /// time, against a handle the engine did not have yet. This clears
    /// `connected` instead: whatever readiness the last incarnation of this
    /// pane earned is stale the moment a fresh one is about to start.
    pub fn note_session(&self, name: &str, kind: &str, model: &str) {
        let to_persist = {
            let mut s = self.sessions.lock().unwrap();
            match s.iter_mut().find(|a| a.name == name) {
                Some(existing) => {
                    let mut changed = false;
                    if existing.kind != kind {
                        existing.kind = kind.to_string();
                        changed = true;
                    }
                    if existing.model != model {
                        existing.model = model.to_string();
                        changed = true;
                    }
                    changed.then(|| existing.clone())
                }
                None => {
                    let session = AgentSession {
                        name: name.to_string(),
                        kind: kind.to_string(),
                        model: model.to_string(),
                    };
                    s.push(session.clone());
                    Some(session)
                }
            }
        };
        if let Some(session) = to_persist {
            let dir = self.dir.lock().unwrap().clone();
            let _ = append_record(&dir, &StoreRecord::Session(session));
            self.app.emit("context-changed");
        }
        // This runs before the PTY and its engine handle exist (see
        // spawn_session), so it is not proof the pane can be delivered to:
        // it is the opposite, a fresh incarnation about to start that must
        // not inherit whatever connected status the last one earned.
        // `mark_connected` is what actually notices the pane is back and
        // drains it, once its MCP endpoint hears from it again.
        self.connected.lock().unwrap().remove(name);
    }

    // ---- conductor ----

    /// One line per live session: id, kind, brain, and role. Shared by
    /// `list_sessions` and the conductor briefing so an agent sees the same
    /// picture of the workspace however it asks.
    ///
    /// Sorted by (length, text) rather than plain lexical order, which keeps
    /// `sess-9` ahead of `sess-10`. Session ids come out of a HashMap, so
    /// without this the roster reshuffles between calls and an agent reading it
    /// twice cannot tell a reordering from a membership change.
    pub fn roster_lines(&self) -> Vec<String> {
        let identified = self.sessions_snapshot();
        let conductor = self.conductor();
        let now = Self::now_ms();
        // Settle dead panes before reading the task list, not after. A roster
        // that still counted an abandoned task as open would tell the conductor
        // a pane is busy on work nobody is doing, which is the exact wrong
        // answer at the exact moment it is choosing a target.
        let live = self.reconcile_abandoned();
        // Presence is not readiness. A pane that took a task and went quiet is
        // listed exactly like an idle one, so a conductor picks it again and
        // waits on a result that is not coming. The open task and its age are
        // the cheapest honest signal available here.
        let tasks = self.tasks_snapshot();
        let (open_for, in_review_for) = attribute_open_tasks(&tasks, now);
        // Held sessions, not live ones: a pane whose process has died is still
        // worth a line, marked dead, because a conductor that simply stops
        // seeing it cannot tell a pane that died from one that was never there,
        // and will not understand why its task was abandoned.
        let mut ids = self.engine.ids();
        ids.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
        ids.iter()
            .map(|id| {
                let kind = identified
                    .iter()
                    .find(|a| &a.name == id)
                    .map(|a| a.kind.clone())
                    .unwrap_or_else(|| "unidentified".to_string());
                let room = self.room_for(id);
                let role = if conductor.as_deref() == Some(id.as_str()) {
                    " [conductor]"
                } else {
                    ""
                };
                // The oldest open task, because that is the one most likely to
                // be stuck and the one worth warning about.
                let busy = open_for
                    .iter()
                    .filter(|(target, _)| target == id)
                    .map(|(_, age_ms)| *age_ms)
                    .max();
                let reviewing = in_review_for
                    .iter()
                    .filter(|(reviewer, _)| reviewer == id)
                    .map(|(_, age_ms)| *age_ms)
                    .max();
                // A dead pane says so instead of reporting how busy it is. Its
                // tasks have just been abandoned, so any busy label would be
                // describing work that no longer exists.
                if !live.iter().any(|l| l == id) {
                    return format!(
                        "- {id} ({kind}) brain={room}{role} [DEAD, process exited, \
                         do not dispatch]"
                    );
                }
                let model_display = identified
                    .iter()
                    .find(|a| &a.name == id)
                    .map(|a| a.model.as_str())
                    .unwrap_or("model unknown");
                let depth = queued_ids_for(&tasks, id).len();
                // Folded into whichever label already exists (busy, since a
                // queue only ever holds tasks queued behind the pane's own
                // work, never behind a review it is doing); a standalone
                // bracket only when the pane has neither.
                let (busy_str, reviewing_str) = if busy.is_some() {
                    (
                        append_queued(busy_label(busy), depth),
                        reviewing_label(reviewing),
                    )
                } else {
                    (
                        busy_label(busy),
                        append_queued(reviewing_label(reviewing), depth),
                    )
                };
                format!(
                    "- {id} ({kind}, {model_display}) brain={room}{role}{busy_str}{reviewing_str}"
                )
            })
            .collect()
    }

    /// Promote (or, with `None`, demote) a pane.
    ///
    /// Promotion also briefs the agent in its own terminal. That injection is
    /// the whole point rather than a nicety: MCP hands a server's instructions
    /// to a client once, at connect time, but which pane is the conductor is
    /// decided by the user long afterwards and can change during a run. An agent
    /// promoted at minute ten has therefore never been told it now commands
    /// every other pane, and the observed result is that it just keeps working
    /// alone. The terminal is the only channel that reaches a *running* agent,
    /// so the role change is delivered the moment it becomes true.
    ///
    /// The briefing is typed into the composer but deliberately NOT submitted.
    /// Auto-sending it made promotion silently spend a turn on a prompt the user
    /// never wrote, and left them no way to say what they actually wanted done.
    /// The pane just started talking. Leaving it unsent turns a hijacked turn
    /// into a prefilled one: the user appends their real first instruction and
    /// sends both together, so the agent learns its role and its task at once.
    /// Dispatch still submits, because there no human is at the keyboard.
    pub fn set_conductor(&self, name: Option<String>) {
        *self.conductor.lock().unwrap() = name.clone();
        self.app.emit("conductor-changed");

        let Some(target) = name else { return };
        // A Shell pane has no MCP connection, so it cannot dispatch and the
        // briefing would land in PowerShell as a command.
        let is_agent = self
            .sessions_snapshot()
            .iter()
            .any(|s| s.name == target && crate::is_agent_cli(&s.kind));
        if !is_agent {
            return;
        }
        let peers: Vec<String> = self
            .roster_lines()
            .into_iter()
            .filter(|l| !l.starts_with(&format!("- {target} ")))
            .collect();
        // write_to rather than submit_to: no Enter is sent, which also means no
        // sleep, so this no longer needs a thread of its own.
        let _ = self.engine.write_to(&target, &conductor_briefing(&peers));
    }

    pub fn conductor(&self) -> Option<String> {
        self.conductor.lock().unwrap().clone()
    }

    /// Every write to a task's persisted state goes through here: `f` gets
    /// the locked task list to mutate in place, and the `tasks` lock stays
    /// held until every record `f` hands back has been journaled. Nothing
    /// else can observe this batch of tasks as mid-flight between the
    /// mutation and the append, because nothing else can even see the
    /// mutation until this whole call releases the lock. This is the only
    /// place in the file that constructs `StoreRecord::Task` for a write;
    /// every task mutator below routes through it (directly, or through
    /// `mutate_task_and_journal`) rather than locking `tasks` itself, so a
    /// future mutator that skips it is a review question, not a silent gap.
    fn mutate_and_journal<R>(&self, f: impl FnOnce(&mut Vec<Task>) -> (R, Vec<Task>)) -> R {
        let mut tasks = self.tasks.lock().unwrap();
        let (result, changed) = f(&mut tasks);
        if !changed.is_empty() {
            let dir = self.dir.lock().unwrap().clone();
            for task in &changed {
                let _ = append_record(&dir, &StoreRecord::Task(task.clone()));
            }
        }
        result
    }

    /// The shape `review_task`, `finish_task`, and `ask_task_question` all
    /// share: run a `..._pending`-style mutator, then journal the one task
    /// it touched. Written once here instead of three times with the same
    /// lock-mutate-refetch dance, which is exactly the shape that left the
    /// refetch reading through a second, separate lock acquisition in the
    /// finding this round.
    fn mutate_task_and_journal<E: From<TaskAccessError>>(
        &self,
        id: &str,
        f: impl FnOnce(&mut Vec<Task>) -> Result<(), E>,
    ) -> Result<Task, E> {
        self.mutate_and_journal(|tasks| match f(tasks) {
            Ok(()) => match tasks.iter().find(|t| t.id == id).cloned() {
                Some(t) => (Ok(t.clone()), vec![t]),
                None => (Err(TaskAccessError::NotFound.into()), Vec::new()),
            },
            Err(e) => (Err(e), Vec::new()),
        })
    }

    /// The mutex that serializes "decide what `pane` gets next, then send
    /// it" against every other delivery path for that same pane. See
    /// `pane_delivery` for why this exists alongside `delivery`. Entries
    /// are created on first use and never removed.
    fn pane_delivery_lock(&self, pane: &str) -> Arc<Mutex<()>> {
        self.pane_delivery
            .lock()
            .unwrap()
            .entry(pane.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Install a test-only pause hook; see `test_seam`. `hook` runs on
    /// whichever thread reaches the boundary, synchronously, so a hook that
    /// blocks (a channel receive) pauses that thread there until the test
    /// releases it.
    #[cfg(test)]
    fn set_test_seam(&self, hook: impl Fn(&str) + Send + Sync + 'static) {
        *self.test_seam.lock().unwrap() = Some(Arc::new(hook));
    }

    /// Call the test hook, if one is set, with `label` naming which
    /// boundary this is. A no-op with nothing installed, which is every
    /// call outside a test that opts in with `set_test_seam`.
    #[cfg(test)]
    fn hit_test_seam(&self, label: &str) {
        let hook = self.test_seam.lock().unwrap().clone();
        if let Some(hook) = hook {
            hook(label);
        }
    }

    /// The production twin of the `#[cfg(test)]` version above: the field
    /// it would read does not exist in this build, so this inlines to
    /// nothing.
    #[cfg(not(test))]
    #[inline(always)]
    fn hit_test_seam(&self, _label: &str) {}

    /// Check `target`'s queued depth against `QUEUE_CAP`, charge the
    /// dispatch budget, and push the new task, all in the same `tasks`-lock
    /// hold: a separate check-then-act let two concurrent dispatches each
    /// observe the same depth one below the cap, each pass, and together
    /// push it two over. The budget charge sits between the cap check and
    /// the push, in that order, so a refusal from either still costs
    /// nothing and journals nothing, matching every other dispatch
    /// precheck; the cap is checked first so a queue-full refusal never
    /// spends a unit of budget it is about to hand back unused. `build`
    /// gets whether `target` is occupied, since that decides the new
    /// task's status (`pending` vs queued) and can only be answered
    /// honestly from inside this same lock hold too, for the same reason.
    /// Returns the task as created, whether it landed occupied, and its
    /// target's queued depth afterward (position in line, when occupied).
    ///
    /// Called directly by tests as well as by `dispatch_task`: the method
    /// itself needs a live target, which `shared_for_test`'s engine can
    /// never report (see `a_refused_dispatch_charges_nothing_and_records_nothing`),
    /// so this is the only way the cap's atomicity is exercised here.
    fn enqueue_within_cap(
        &self,
        target: &str,
        build: impl FnOnce(bool) -> Task,
    ) -> Result<(Task, bool, usize), String> {
        self.mutate_and_journal(|tasks| {
            let queued_ids = queued_ids_for(tasks, target);
            if let Some(refusal) = queue_cap_refusal(&queued_ids) {
                return (Err(refusal), Vec::new());
            }
            // Test-only pause point, between the cap check just passed and
            // the push below, both still under this same `tasks` lock hold;
            // see `test_seam`.
            self.hit_test_seam("enqueue_within_cap");
            if !self.take_dispatch_budget() {
                return (
                    Err("dispatch budget exhausted for this run.".to_string()),
                    Vec::new(),
                );
            }
            let occupied = is_occupied(tasks, target);
            let t = build(occupied);
            tasks.push(t.clone());
            let queue_len = queued_ids_for(tasks, target).len();
            (Ok((t.clone(), occupied, queue_len)), vec![t])
        })
    }

    /// Stop halts all dispatch immediately; clearing it also refreshes the
    /// budget and resumes delivery, since a pane could have been freed
    /// while halted with nothing able to act on it (`drain_pane` refuses to
    /// type while halted).
    pub fn set_halted(&self, v: bool) {
        {
            // Exclusive: a delivery path holding `delivery` as a reader is
            // between its own eligibility check and its `submit_to` call, and
            // must not have this land in between. Taking this here, not
            // `tasks`, is what keeps that guarantee without ever holding
            // `tasks` across a PTY write. Scoped so the guard is dropped
            // before the resume sweep below, which takes its own read lock.
            let _delivery = self.delivery.write().unwrap();
            *self.halted.lock().unwrap() = v;
            if v {
                let now = Self::now_ms();
                self.mutate_and_journal(|tasks| {
                    let mut changed = Vec::new();
                    for t in tasks.iter_mut() {
                        // A queued task is exactly as stopped by Stop as a
                        // pending one: neither has been typed anywhere yet.
                        if t.status == "pending" || t.status == STATUS_QUEUED {
                            t.status = "cancelled".to_string();
                            t.done_ms = Some(now);
                            changed.push(t.clone());
                        }
                    }
                    ((), changed)
                });
            } else {
                *self.dispatches.lock().unwrap() = 0;
            }
        }
        self.app.emit("conductor-changed");
        if !v {
            for id in self.live_ids() {
                self.drain_pane(&id);
            }
        }
    }

    pub fn is_halted(&self) -> bool {
        *self.halted.lock().unwrap()
    }

    pub fn tasks_snapshot(&self) -> Vec<Task> {
        self.tasks.lock().unwrap().clone()
    }

    /// Consume one unit of dispatch budget; false when exhausted.
    fn take_dispatch_budget(&self) -> bool {
        let mut n = self.dispatches.lock().unwrap();
        if *n >= MAX_DISPATCHES {
            return false;
        }
        *n += 1;
        true
    }

    /// Ids of panes whose agent process is still running.
    ///
    /// Everything that has to decide "is anyone there" goes through this rather
    /// than `engine.ids()`, so the dead-pane answer is the same whether it is
    /// reached from the roster, a dispatch, or a task collection.
    fn live_ids(&self) -> Vec<String> {
        self.engine
            .liveness()
            .into_iter()
            .filter(|(_, alive)| *alive)
            .map(|(id, _)| id)
            .collect()
    }

    /// Settle any task whose pane has died, then report which panes are live.
    ///
    /// Called from the three places that ask about task state or pick a target:
    /// the roster, task collection, and dispatch. Reconciling at the read rather
    /// than on a timer means there is no background sweep to keep alive and no
    /// window in which a caller sees a task the roster has already written off.
    /// The cost is one non-blocking `try_wait` per pane per call, which is
    /// cheaper than the poll it replaces.
    fn reconcile_abandoned(&self) -> Vec<String> {
        let live = self.live_ids();
        let now = Self::now_ms();
        let changed = self.mutate_and_journal(|tasks| {
            let changed = abandon_lost(tasks, &live, now);
            (changed.clone(), changed)
        });
        if !changed.is_empty() {
            for task in &changed {
                if task.status == STATUS_ABANDONED {
                    eprintln!(
                        "[pantheon] task {} abandoned: target '{}' is gone",
                        task.id, task.target
                    );
                } else if task.reviewer_gone {
                    eprintln!(
                        "[pantheon] task {} flagged: reviewer '{}' is gone",
                        task.id, task.reviewer
                    );
                } else {
                    eprintln!(
                        "[pantheon] task {} unflagged: reviewer '{}' is live again",
                        task.id, task.reviewer
                    );
                }
            }
            self.app.emit("conductor-changed");
        }
        live
    }

    /// Approving or rejecting both end `caller`'s occupancy as this task's
    /// reviewer, so its pane is drained either way; a rejection also owes
    /// the target a rework notice, drained on its pane too.
    fn review_task(
        &self,
        caller: &str,
        id: &str,
        approved: bool,
        findings: &str,
    ) -> Result<Task, TaskAccessError> {
        let task = self.mutate_task_and_journal(id, |tasks| {
            review_pending(tasks, caller, id, approved, findings, Self::now_ms())
        })?;
        self.app.emit("context-changed");
        self.drain_pane(caller);
        if task.status == "rework" {
            self.drain_pane(&task.target);
        }
        Ok(task)
    }

    /// Block a task on a question for its conductor.
    fn ask_task_question(&self, caller: &str, id: &str, question: &str) -> Result<(), AskError> {
        self.mutate_task_and_journal(id, |tasks| {
            ask_pending(tasks, caller, id, question, Self::now_ms())
        })?;
        self.app.emit("conductor-changed");
        Ok(())
    }

    /// Answer a blocked task's open question and set it working again.
    fn answer_task_question(
        &self,
        caller: &str,
        id: &str,
        answer: &str,
    ) -> Result<String, AskError> {
        let conductor = self.conductor();
        let question = self.mutate_and_journal(|tasks| {
            match answer_pending(tasks, caller, conductor.as_deref(), id, answer) {
                Ok(question) => match tasks.iter().find(|t| t.id == id).cloned() {
                    Some(t) => (Ok(question), vec![t]),
                    None => (Err(AskError::Access(TaskAccessError::NotFound)), Vec::new()),
                },
                Err(e) => (Err(e), Vec::new()),
            }
        })?;
        self.app.emit("conductor-changed");
        Ok(question)
    }

    /// The open question on a task, for the agent that is waiting on it.
    fn task_answer(&self, id: &str) -> Option<Exchange> {
        self.tasks
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| t.exchanges.last().cloned())
    }

    /// Returns the task, so the caller can tell the agent what actually
    /// happened to it. "Result recorded" is the wrong thing to say when the
    /// work has gone to a reviewer and is not finished.
    ///
    /// Finishing always ends `caller`'s own occupancy as this task's target,
    /// so its pane is drained; when the work went to a reviewer, that
    /// reviewer's pane is drained too, to try delivering the review request
    /// immediately rather than leaving it for the next unrelated event.
    fn finish_task(&self, caller: &str, id: &str, result: &str) -> Result<Task, TaskAccessError> {
        let task = self.mutate_task_and_journal(id, |tasks| {
            finish_pending(tasks, caller, id, result, Self::now_ms())
        })?;
        self.app.emit("conductor-changed");
        self.drain_pane(caller);
        if task.status == "in_review" {
            self.drain_pane(&task.reviewer);
        }
        Ok(task)
    }

    /// Mark a recorded task as errored when terminal delivery fails. A task
    /// that completed or was cancelled concurrently is never overwritten.
    fn mark_delivery_failed(&self, id: &str) {
        let now = Self::now_ms();
        let changed = self.mutate_and_journal(|tasks| {
            if !mark_pending_error(tasks, id, now) {
                return (false, Vec::new());
            }
            match tasks.iter().find(|t| t.id == id).cloned() {
                Some(t) => (true, vec![t]),
                None => (false, Vec::new()),
            }
        });
        if changed {
            self.app.emit("conductor-changed");
        }
    }

    /// Look a task up, flipping it to "overdue" if it has aged out.
    fn task_status(&self, caller: &str, id: &str) -> Result<Task, TaskAccessError> {
        // Same reason as `tasks_from`: asking after one task must be able to
        // answer "its pane is gone", not just "still pending".
        self.reconcile_abandoned();
        let now = Self::now_ms();
        self.mutate_and_journal(|tasks| {
            let t = match task_for_reader(tasks, caller, id) {
                Ok(t) => t,
                Err(e) => return (Err(e), Vec::new()),
            };
            let previous = t.status.clone();
            age(t, now);
            let changed = if t.status != previous {
                vec![t.clone()]
            } else {
                Vec::new()
            };
            (Ok(t.clone()), changed)
        })
    }

    /// Every task a given agent dispatched, aged out the same way `task_status`
    /// ages a single one. This is what makes a parallel fan-out cheap to
    /// collect: without it a conductor holding six task ids has to make six
    /// round trips to find out that five are still running.
    fn tasks_from(&self, from: &str) -> Vec<Task> {
        // Settle dead panes first, so a collection reports "abandoned" on the
        // call where the pane is found gone rather than one call later. This is
        // the call a conductor makes while waiting, so it is the one that has to
        // stop it waiting. Done before the lock, because it takes the lock.
        self.reconcile_abandoned();
        let now = Self::now_ms();
        self.mutate_and_journal(|tasks| {
            let mut changed = Vec::new();
            let result = tasks
                .iter_mut()
                .filter(|t| t.from == from)
                .map(|t| {
                    let previous = t.status.clone();
                    age(t, now);
                    if t.status != previous {
                        changed.push(t.clone());
                    }
                    t.clone()
                })
                .collect();
            (result, changed)
        })
    }

    /// Validate and hand a task to a live session: the shared core of the
    /// agent-facing `dispatch` MCP tool and the human-facing `human_dispatch`
    /// Tauri command (lib.rs): one ledger and one set of rules regardless of
    /// who started the task. Conductor-only enforcement is deliberately NOT
    /// here: that's an MCP-specific policy the `dispatch` tool applies before
    /// calling this, and `human_dispatch` has no agent identity to check it
    /// to check it against: the human already decided who to promote.
    ///
    /// The task record is created in "pending" (or, when the target is
    /// occupied, "queued") status BEFORE `submit_to` is called. That closes
    /// the original race while still allowing an unusually fast target to
    /// complete the task as soon as it receives the prompt. The gap that
    /// left open, from before Phase 1, is between that creation and the
    /// write: the occupancy check, the task creation, and the delivery
    /// decision all happen while holding `delivery` as a reader, so
    /// `set_halted` cannot cancel this task in between without this call
    /// seeing it before it types anything.
    pub fn dispatch_task(
        &self,
        from: &str,
        target: &str,
        task: &str,
        reviewer: &str,
    ) -> Result<DispatchOutcome, String> {
        // Liveness, not membership, and it settles dead panes' tasks on the way
        // past. Dispatching into a pane whose process has exited was the way
        // this failure compounded: the work was never done, the task never
        // closed, and the conductor re-dispatched the same brief to the same
        // corpse. A reviewer is chosen from the same list for the same reason.
        let live = self.reconcile_abandoned();
        let target_is_live = live.iter().any(|i| i == target);

        // Before the budget and before the task is recorded, like every other
        // refusal here: a dispatch naming an impossible reviewer should cost
        // nothing and leave nothing to collect.
        let reviewer = choose_reviewer(reviewer, from, target, &live, &self.sessions_snapshot())?;

        // Built before the gate because the size rule needs the finished
        // injection, and the id is part of what it measures. Nothing here
        // mutates: every refusal below still costs no budget and records no
        // task, which `a_refused_dispatch_charges_nothing_and_records_nothing`
        // holds us to.
        let id = uuid::Uuid::new_v4().simple().to_string();
        let injection = dispatch_prompt(from, &id, task);
        dispatch_precheck(self.is_halted(), from, target, target_is_live, &injection)?;

        // Held from here through the write (when there is one): `set_halted`
        // and `cancel_tasks` both take `delivery` exclusively before they
        // touch a task, so once this is held, neither can act until it
        // releases. `pane_delivery_lock(target)` is the second, narrower
        // exclusion this needs on top of that: `delivery` only ever keeps
        // Stop and cancel out, since any number of readers (this dispatch,
        // a concurrent one, drain_pane) can hold it at once, so without the
        // per-pane lock too, a dispatch landing while drain_pane had
        // already picked this target's queue head but not yet promoted it
        // would see the pane as free and deliver at the same time, out of
        // order. See `pane_delivery`.
        let delivery_gate = self.delivery.read().unwrap();
        let pane_lock = self.pane_delivery_lock(target);
        let pane_gate = pane_lock.lock().unwrap();

        // The queue cap and the dispatch budget are both checked inside
        // `enqueue_within_cap`, atomically with the task's creation; see
        // its doc comment for why a separate check beforehand (this
        // function's previous shape) let two concurrent dispatches both
        // pass the same stale depth.
        let now = Self::now_ms();
        let (_, occupied, queue_len) = match self.enqueue_within_cap(target, |occupied| Task {
            id: id.clone(),
            from: from.to_string(),
            target: target.to_string(),
            task: task.to_string(),
            status: if occupied { STATUS_QUEUED } else { "pending" }.to_string(),
            result: String::new(),
            ts_ms: now,
            reviewer: reviewer.clone(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            done_ms: None,
            notice_delivered: true,
        }) {
            Err(refusal) => {
                drop(pane_gate);
                drop(delivery_gate);
                return Err(refusal);
            }
            Ok(v) => v,
        };
        self.app.emit("conductor-changed");

        if occupied {
            drop(pane_gate);
            drop(delivery_gate);
            return Ok(DispatchOutcome {
                task_id: id,
                delivered: false,
                reviewer,
                already_busy: queue_predecessor(&self.tasks.lock().unwrap(), target),
                queued: true,
                queue_position: Some(queue_len),
            });
        }

        // Typed into the target's terminal, so the human sees every
        // instruction. Submit Enter separately: Codex and Claude Code treat
        // text+CR in one PTY write as a paste and can leave it waiting in
        // the input editor; see `SessionManager::submit_to`. `is_halted` is
        // re-read here, inside the gate, because the precheck above ran
        // before the gate was taken and before the task existed at all.
        let delivered = if self.is_halted() {
            false
        } else {
            self.engine.submit_to(target, &injection)
        };
        drop(pane_gate);
        drop(delivery_gate);
        if !delivered {
            self.mark_delivery_failed(&id);
        }
        Ok(DispatchOutcome {
            task_id: id,
            delivered,
            reviewer,
            already_busy: None,
            queued: false,
            queue_position: None,
        })
    }

    /// Cancel every id in `ids` that is still open, recording `reason` on
    /// each. Conductor-only enforcement lives in the `cancel_task` tool, the
    /// same as `dispatch_task`.
    ///
    /// Takes several ids in one call because the failure this exists for is a
    /// pile of stale tasks, not one: 38 open tasks aged 1 to 113 hours,
    /// measured 2026-09-03, with no way to close them at all. A conductor
    /// clearing that pile one id per call is a conductor that gives up before
    /// finishing.
    ///
    /// Takes `delivery` exclusively, the same way `set_halted` does: without
    /// it, a cancel could land between a delivery path's status check and
    /// its `submit_to` call, cancelling a task that gets typed into a
    /// terminal anyway a moment later.
    ///
    /// Cancelling a task can end its target's or its reviewer's occupancy
    /// (a queued task cancels out of the queue without occupying anyone, so
    /// draining it is a harmless no-op), so every distinct pane touched by a
    /// cancelled task is drained afterwards, once `delivery` is released.
    fn cancel_tasks(&self, ids: &[String], reason: &str) -> Vec<(String, CancelOutcome)> {
        let delivery_gate = self.delivery.write().unwrap();
        let now = Self::now_ms();
        let (results, changed) = self.mutate_and_journal(|tasks| {
            let results: Vec<(String, CancelOutcome)> = ids
                .iter()
                .map(|id| (id.clone(), cancel_pending(tasks, id, reason, now)))
                .collect();
            let changed: Vec<Task> = results
                .iter()
                .filter(|(_, outcome)| *outcome == CancelOutcome::Cancelled)
                .filter_map(|(id, _)| tasks.iter().find(|t| &t.id == id).cloned())
                .collect();
            ((results, changed.clone()), changed)
        });
        if !changed.is_empty() {
            self.app.emit("conductor-changed");
        }
        drop(delivery_gate);
        let mut drained: Vec<&str> = Vec::new();
        for t in &changed {
            for pane in [t.target.as_str(), t.reviewer.as_str()] {
                if !pane.is_empty() && !drained.contains(&pane) {
                    drained.push(pane);
                    self.drain_pane(pane);
                }
            }
        }
        results
    }

    /// Retarget a pending/overdue/abandoned task to a new live session,
    /// redelivering the same brief, or hand an in_review/rework task to a new
    /// live reviewer. Conductor-only enforcement lives in the `reassign_task`
    /// tool, the same as `dispatch_task`; `caller` is that conductor, needed
    /// on retarget so the reassigned task's `from` follows the caller rather
    /// than staying pinned to whoever dispatched it first.
    ///
    /// Returns the task as it stands after the change, plus whether
    /// redelivery reached the new target's terminal. The second value is
    /// meaningless (`true`) when this call only changed the reviewer, because
    /// nothing is typed into a terminal for that: a reviewer's assignment is
    /// carried entirely by the `reviewer` field, the same way it is when
    /// `dispatch_task` first chooses one.
    ///
    /// Five rounds landed on this shape. Holding `tasks` across the
    /// mutation and its journal append is still what makes the record order
    /// agree with the mutation order (`mutate_and_journal`), but the third
    /// round found that holding `tasks` across `submit_to` too, on top of
    /// closing the halt race, stalled every other completion, review, and
    /// cancellation behind a PTY write. Delivery runs under `delivery`
    /// instead: a shared hold blocks `set_halted` and `cancel_tasks`'s
    /// exclusive one (and the reverse), so Stop or a cancel still cannot
    /// land mid-decision, without the task mutex ever being held across the
    /// write. The fourth round found the remaining gap: `delivery.read()`
    /// was taken only for the write itself, after the retarget mutation had
    /// already happened, so a Stop landing between the halted check above
    /// and the mutation could still turn an overdue or abandoned task
    /// pending after the halt, with nothing left to ever deliver it. The
    /// gate is now held from before the mutation through the write, so the
    /// halt check, the mutation, its journal, and the write are all one
    /// section nothing else can interleave with.
    ///
    /// A retarget that lands on an occupied pane is held the same way a
    /// fresh dispatch is: `reassign_pending` still moves it to `pending`
    /// (fresh delivery, fresh clock), but if the new target turns out to be
    /// occupied this flips it straight on to `queued` instead of typing over
    /// whatever that pane is doing, and `delivered` comes back `false`. The
    /// pane the task moved *off* of is drained afterwards, since retargeting
    /// away is one of the ways a pane stops being occupied.
    ///
    /// The fifth round found two more gaps in that occupied branch
    /// specifically. It used to drop `delivery_gate` before its own second
    /// mutation (the flip to `queued`), reopening the exact window the
    /// fourth round had just closed for the first one: a Stop or a cancel
    /// landing there could cancel the task, and the unconditional flip that
    /// followed would overwrite that, resurrecting it. The gate now spans
    /// both mutations, and the flip itself is conditional
    /// (`requeue_if_still_pending`) on nothing else having already decided
    /// the task's fate. And the occupancy read plus both branches now also
    /// hold `pane_delivery_lock(&task.target)`, the same lock `drain_pane`
    /// and `dispatch_task` hold for their own decide-then-send section:
    /// `delivery_gate` alone never excluded those from each other, since any
    /// number of readers can hold it at once. See `pane_delivery`.
    fn reassign_task(
        &self,
        caller: &str,
        id: &str,
        new_target: Option<&str>,
        new_reviewer: Option<&str>,
    ) -> Result<(Task, bool), ReassignError> {
        // Retargeting redelivers into the new target's terminal, the same as
        // dispatch, so it is gated by Stop the same way dispatch is. Handing
        // a task to a new reviewer types nothing into any terminal and is
        // not subject to it. This first check is cheap and early, but is not
        // itself what closes the race: the gate below is.
        if new_target.is_some() && self.is_halted() {
            return Err(ReassignError::Halted);
        }
        let live = self.reconcile_abandoned();
        let retargeting = new_target.is_some();
        let now = Self::now_ms();

        // Captured before the mutation, so there is something to drain
        // afterwards if this retarget moves work off it. A reviewer
        // reassignment types nothing into any terminal, so it never frees a
        // pane and this stays `None` for that branch.
        let previous_target = if retargeting {
            self.tasks
                .lock()
                .unwrap()
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.target.clone())
        } else {
            None
        };

        // Held from here through the write: `set_halted` and `cancel_tasks`
        // both take `delivery` exclusively before they touch a task, so
        // once this is held, neither can act on this task, or flip
        // `halted`, until it releases.
        let delivery_gate = self.delivery.read().unwrap();
        if retargeting && self.is_halted() {
            return Err(ReassignError::Halted);
        }

        let task = self.mutate_and_journal(|tasks| {
            match reassign_pending(tasks, id, caller, new_target, new_reviewer, &live, now) {
                Ok(t) => (Ok(t.clone()), vec![t]),
                Err(e) => (Err(e), Vec::new()),
            }
        })?;
        self.app.emit("conductor-changed");

        if !retargeting {
            return Ok((task, true));
        }

        // Held from here alongside `delivery_gate`, for the new target:
        // `delivery_gate` only ever keeps Stop and cancel out, since any
        // number of readers (this call, a concurrent dispatch, drain_pane)
        // can hold it at once, so without this too, the occupancy read
        // below and whichever branch acts on it could interleave with one
        // of those on the same pane. See `pane_delivery`.
        let pane_lock = self.pane_delivery_lock(&task.target);
        let pane_gate = pane_lock.lock().unwrap();

        // Occupancy of the NEW target, checked while still under both gates
        // so nothing else can change what "occupied" means between this
        // read and whichever branch below acts on it. `excluding` the
        // task's own id matters here: `reassign_pending` just set its
        // status to `pending` against this very target, which would
        // otherwise make `occupying_task` see the task as occupying the
        // pane it is about to be delivered to.
        let occupied = {
            let tasks = self.tasks.lock().unwrap();
            occupying_task(&tasks, &task.target, &task.id).is_some()
        };

        let (result, delivered) = if occupied {
            // `delivery_gate` stays held through this mutation, not dropped
            // before it: dropping early let Stop or cancel take the gate in
            // the gap and cancel this task, and the unconditional flip to
            // `queued` that used to run here would then overwrite that
            // cancellation, resurrecting the task after it was stopped. The
            // gate now spans both mutations, so nothing can land in that
            // gap at all; `requeue_if_still_pending`'s own condition is the
            // second half of the fix, for anything that could still have
            // changed the task's fate before this call ever reached here.
            let queued = self.mutate_and_journal(|tasks| {
                match requeue_if_still_pending(tasks, &task.id, &task.target) {
                    Some(t) => (t.clone(), vec![t.clone()]),
                    None => (
                        tasks
                            .iter()
                            .find(|t| t.id == task.id)
                            .cloned()
                            .unwrap_or_else(|| task.clone()),
                        Vec::new(),
                    ),
                }
            });
            drop(pane_gate);
            drop(delivery_gate);
            (queued, false)
        } else {
            // Same delivery path `dispatch_task` uses, so a reassigned brief
            // looks to the new target exactly like a fresh dispatch: typed
            // into its terminal, Enter sent separately. Still under both
            // gates, so nothing can have cancelled this task, flipped
            // halted, or raced it onto the same pane since the mutation
            // just journaled it pending.
            let injection = dispatch_prompt(&task.from, &task.id, &task.task);
            let delivered = self.engine.submit_to(&task.target, &injection);
            drop(pane_gate);
            drop(delivery_gate);
            let result = if delivered {
                task
            } else {
                self.mark_delivery_failed(&task.id);
                self.tasks
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|t| t.id == task.id)
                    .cloned()
                    .unwrap_or(task)
            };
            (result, delivered)
        };

        if let Some(old_target) = previous_target {
            self.drain_pane(&old_target);
        }
        Ok((result, delivered))
    }

    /// Deliver the next thing waiting for `pane`'s terminal, if the pane is
    /// free to receive it: an undelivered review request or rework notice,
    /// or the head of its dispatch queue (see `next_delivery_for`). A no-op
    /// when the pane is occupied, halted, or has nothing pending, which is
    /// the overwhelmingly common case, since almost every call to this
    /// frees nothing up.
    ///
    /// Called after every mutation that can end a pane's occupancy: a task
    /// finishing, being reviewed either way, cancelled, or moved off the
    /// pane by a retarget. `delivery` is held across the read that decides
    /// what to send and the `submit_to` that sends it, same as `dispatch_task`
    /// and `reassign_task`, so a Stop cannot land in between; the tasks lock
    /// is taken twice, briefly, for the read and for the write, and never
    /// held across the PTY write itself. `pane_delivery_lock(pane)` is held
    /// across that same span too, so a second, concurrent drain of this
    /// pane (or a dispatch or reassign landing on it) cannot select the
    /// same candidate this call already picked, or act on the pane while
    /// this call still has one reserved; see `pane_delivery`.
    fn drain_pane(&self, pane: &str) {
        let delivery_gate = self.delivery.read().unwrap();
        if self.is_halted() {
            return;
        }
        // A pane that has not connected since its most recent spawn has no
        // engine handle yet, or has one but the CLI has not finished
        // booting and would lose the bytes either way. Neither is a failed
        // delivery: there was no delivery to fail, so nothing here may
        // become `error` or count a notice as attempted. Leave it queued
        // (or undelivered) for the next trigger, which fires once the pane
        // actually connects; see `mark_connected`.
        if !self.is_connected(pane) {
            return;
        }
        let pane_lock = self.pane_delivery_lock(pane);
        let pane_gate = pane_lock.lock().unwrap();
        let candidate = {
            let tasks = self.tasks.lock().unwrap();
            next_delivery_for(&tasks, pane).cloned()
        };
        let Some(task) = candidate else {
            return;
        };
        // Test-only pause point: past selection, still holding `pane_gate`,
        // before submit_to or the post-submit journal below; see
        // `test_seam`.
        self.hit_test_seam("drain_pane");

        let is_dispatch = task.status == STATUS_QUEUED;
        let text = if is_dispatch {
            dispatch_prompt(&task.from, &task.id, &task.task)
        } else if task.status == "rework" {
            rework_notice(&task.id, &task.reviewer, &task.findings)
        } else {
            review_request_notice(&task.id, &task.target, &task.task)
        };
        let delivered = self.engine.submit_to(pane, &text);
        let now = Self::now_ms();

        self.mutate_and_journal(|tasks| {
            let Some(t) = tasks.iter_mut().find(|t| t.id == task.id) else {
                return ((), Vec::new());
            };
            if is_dispatch {
                if delivered {
                    t.status = "pending".to_string();
                    t.ts_ms = now;
                } else {
                    t.status = "error".to_string();
                    t.done_ms = Some(now);
                }
            } else if delivered {
                t.notice_delivered = true;
            }
            // A failed notice leaves `notice_delivered` false so the next
            // drain of this pane retries it; nothing else changes.
            ((), vec![t.clone()])
        });
        drop(pane_gate);
        drop(delivery_gate);
        self.app.emit("conductor-changed");
    }

    /// Whether `pane` has connected to its MCP endpoint since its most
    /// recent spawn. See the `connected` field doc for why this, and not
    /// liveness or a recorded identity, is what gates `drain_pane`.
    fn is_connected(&self, pane: &str) -> bool {
        self.connected.lock().unwrap().contains(pane)
    }

    /// Record that a pane's MCP endpoint just received a request from it,
    /// and drain whatever was waiting on that. Call this from the actual
    /// connection points: the dedicated endpoint's per-session handler
    /// factory (one call per new session on that port) and
    /// `set_session_identity` on the shared endpoint. Not from
    /// `note_session`, which runs at spawn time before the PTY or its
    /// engine handle exist and is not evidence of anything reachable.
    pub fn mark_connected(&self, pane: &str) {
        self.connected.lock().unwrap().insert(pane.to_string());
        self.drain_pane(pane);
    }
}

/// Carry a live pane's kind/model forward across a `set_dir` switch, letting
/// only a genuinely dead name fall back to whatever the new journal says.
///
/// `old` is the sessions held in memory right before the switch; `new` is
/// what `load_brain` just read from the dir being switched to. For every
/// name in `live`, `old` wins: it reflects `note_session` or
/// `set_session_identity` calls the app already made for the pane that is
/// actually running, while `new` may be stale (a different project's
/// journal, or this project's own journal from before the pane's current
/// CLI connected). A name not in `live` is left entirely to `new`, since
/// nothing contradicts the journal for a pane that is not there to be wrong
/// about.
///
/// Returns the merged roster plus exactly the entries that differ from
/// `new`, so the caller can persist only those, the same "refresh when
/// different" rule `note_session` already applies. Pure (no lock, no I/O),
/// so the merge itself is testable without a live `Shared` or a running
/// pane, which the test fixture's engine cannot provide (it reports nothing
/// live).
fn merge_live_identity(
    old: &[AgentSession],
    new: &[AgentSession],
    live: &[String],
) -> (Vec<AgentSession>, Vec<AgentSession>) {
    let mut merged = new.to_vec();
    let mut changed = Vec::new();
    for prior in old {
        if !live.iter().any(|id| id == &prior.name) {
            continue;
        }
        match merged.iter_mut().find(|s| s.name == prior.name) {
            Some(existing) => {
                if existing.kind != prior.kind || existing.model != prior.model {
                    existing.kind = prior.kind.clone();
                    existing.model = prior.model.clone();
                    changed.push(existing.clone());
                }
            }
            // Live per the engine, but the new journal has never heard of it
            // (a brand-new project). Carry the in-memory record forward
            // rather than dropping a pane the roster already knows.
            None => {
                merged.push(prior.clone());
                changed.push(prior.clone());
            }
        }
    }
    (merged, changed)
}

/// Age a single task in place: still-pending past the threshold becomes
/// "overdue". Shared by `task_status` (one id) and `tasks_from` (a whole
/// list), which used to duplicate this check inline.
///
/// "overdue" is deliberately not terminal. The agent is still running and its
/// result is still accepted, so this only marks the task as slow.
/// What to append to a roster line for a pane's oldest open task.
///
/// Past the overdue threshold the wording changes deliberately. "busy 45m" and
/// "busy 45m, OVERDUE, may be stuck" are the same fact, but only the second
/// tells a conductor to stop waiting and consider another target. A pane that
/// took a task and went silent was previously indistinguishable from an idle
/// one, and got picked again.
/// Every id queued for `pane`, oldest first. `ts_ms` is untouched while a
/// task sits queued (see `drain_pane`, which resets it only on delivery), so
/// it doubles as the position in line.
fn queued_ids_for(tasks: &[Task], pane: &str) -> Vec<String> {
    let mut queued: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.target == pane && t.status == STATUS_QUEUED)
        .collect();
    queued.sort_by_key(|t| t.ts_ms);
    queued.into_iter().map(|t| t.id.clone()).collect()
}

/// Whichever task makes `pane` occupied right now: its own open work
/// (`pending`, `overdue`, `rework`, or `blocked`), or the `in_review` task it
/// is reviewing. `excluding` skips one id, so a task cannot be judged
/// occupied by itself the moment its own status is what would occupy the
/// pane (see `drain_pane` and `Shared::reassign_task`, which both check
/// occupancy of a pane a task is *about* to notify or move to).
fn occupying_task<'a>(tasks: &'a [Task], pane: &str, excluding: &str) -> Option<&'a Task> {
    tasks.iter().find(|t| {
        t.id != excluding
            && ((t.target == pane
                && matches!(
                    t.status.as_str(),
                    "pending" | "overdue" | "rework" | STATUS_BLOCKED
                ))
                || (t.reviewer == pane && t.status == "in_review"))
    })
}

fn is_occupied(tasks: &[Task], pane: &str) -> bool {
    occupying_task(tasks, pane, "").is_some()
}

/// The id of the task a fresh dispatch to `target` would be queued behind:
/// the last task already in its queue, or whatever currently occupies the
/// pane when the queue is still empty. `None` means the pane is free.
fn queue_predecessor(tasks: &[Task], target: &str) -> Option<String> {
    queued_ids_for(tasks, target)
        .last()
        .cloned()
        .or_else(|| occupying_task(tasks, target, "").map(|t| t.id.clone()))
}

/// The refusal for a dispatch that would push `target`'s queue past
/// `QUEUE_CAP`, or `None` to proceed. Pure over the ids already queued, for
/// the same reason `dispatch_precheck` is pure: testable without a live
/// `Shared`, and free to run before the budget is taken and before any task
/// is recorded, since refusing here costs nothing, same as any other
/// refused dispatch.
fn queue_cap_refusal(queued_ids: &[String]) -> Option<String> {
    if queued_ids.len() < QUEUE_CAP {
        return None;
    }
    Some(format!(
        "target's queue is full ({QUEUE_CAP} already queued: {}). cancel_task one of \
         those first, or wait for the pane to catch up.",
        queued_ids.join(", ")
    ))
}

/// What `pane`'s terminal should receive next, if it is free to receive
/// anything: an undelivered review request or rework notice ahead of a
/// freshly queued dispatch, since that work already exists and someone is
/// waiting on it, while a queued dispatch is new work that can wait one more
/// turn. `None` when the pane is occupied or has nothing pending. Pure over
/// the task list, so the ordering rule is testable without a live `Shared`
/// or `drain_pane`'s PTY write.
fn next_delivery_for<'a>(tasks: &'a [Task], pane: &str) -> Option<&'a Task> {
    // A rework task occupies its target and an in_review task occupies its
    // reviewer (see `occupying_task`), but that is exactly the task whose
    // notice this function exists to deliver: it must not disqualify
    // itself. Pick the candidate first, then check for occupancy by
    // anything else.
    let notice = tasks.iter().find(|t| {
        !t.notice_delivered
            && ((t.status == "rework" && t.target == pane)
                || (t.status == "in_review" && t.reviewer == pane))
    });
    let candidate = notice.or_else(|| {
        queued_ids_for(tasks, pane)
            .first()
            .and_then(|id| tasks.iter().find(|t| &t.id == id))
    })?;
    if occupying_task(tasks, pane, &candidate.id).is_some() {
        return None;
    }
    Some(candidate)
}

/// Who holds an open task, and for how long: a session name paired with the
/// task's age. Named so `attribute_open_tasks`'s signature reads, rather than
/// repeating the pair inline twice.
type AgeBySession = Vec<(String, u64)>;

/// Who each open task counts against for the roster's busy/reviewing labels,
/// and for how long. Pure over a task list, so the attribution rule is
/// testable without a live `Shared` or a running pane.
///
/// Attribution differs by status: an `in_review` task is waiting on its
/// reviewer, not its target, who already submitted and is free to take more
/// work. Counting it against the target is the bug that left a pane reading
/// `[busy, OVERDUE]` forever after it had finished and handed the result off.
/// Every other open status still names the target, who is the one actually
/// holding the work.
///
/// `queued` is excluded from both buckets: nothing is running yet for it to
/// be slow at, so folding it into "busy" would misreport a fresh queue
/// entry's age as how long the pane has been working. `roster_lines` reports
/// queue depth separately, as a count rather than an age.
fn attribute_open_tasks(tasks: &[Task], now_ms: u64) -> (AgeBySession, AgeBySession) {
    let mut busy = Vec::new();
    let mut reviewing = Vec::new();
    for t in tasks
        .iter()
        .filter(|t| is_open(&t.status) && t.status != STATUS_QUEUED)
    {
        let age_ms = now_ms.saturating_sub(t.ts_ms);
        if t.status == "in_review" {
            reviewing.push((t.reviewer.clone(), age_ms));
        } else {
            busy.push((t.target.clone(), age_ms));
        }
    }
    (busy, reviewing)
}

fn busy_label(oldest_open_ms: Option<u64>) -> String {
    match oldest_open_ms {
        None => String::new(),
        Some(ms) if ms > TASK_OVERDUE_MS => {
            format!(" [busy {}, OVERDUE, may be stuck]", human_ms(ms))
        }
        Some(ms) => format!(" [busy {}]", human_ms(ms)),
    }
}

/// What to append to a roster line for the oldest `in_review` task a pane is
/// reviewing. A separate label from `busy_label`, not a shared one with
/// different wording, because the two describe different roles: a pane can be
/// both a target holding its own work and a reviewer holding someone else's,
/// and a conductor needs to be able to tell which is which at a glance.
fn reviewing_label(oldest_review_ms: Option<u64>) -> String {
    match oldest_review_ms {
        None => String::new(),
        Some(ms) => format!(" [reviewing {}]", human_ms(ms)),
    }
}

/// Fold a queue-depth note into an existing bracketed label (", N queued"
/// before its closing `]`), or give it its own bracket when there is no
/// other label to fold into. `depth` of 0 leaves `label` untouched. Used to
/// turn "[busy 4m]" into "[busy 4m, 2 queued]" rather than adding a second,
/// separate bracket, which reads as two unrelated facts instead of one.
fn append_queued(label: String, depth: usize) -> String {
    if depth == 0 {
        return label;
    }
    let note = format!("{depth} queued");
    if label.is_empty() {
        return format!(" [{note}]");
    }
    let mut label = label;
    label.truncate(label.len() - 1); // drop the trailing ']'
    label.push_str(&format!(", {note}]"));
    label
}

/// A duration a human reads at a glance. Roster lines are scanned, not parsed.
fn human_ms(ms: u64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        return format!("{secs}s");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m");
    }
    format!("{}h{:02}m", mins / 60, mins % 60)
}

fn age(t: &mut Task, now: u64) {
    if t.status == "pending" && now.saturating_sub(t.ts_ms) > TASK_OVERDUE_MS {
        t.status = "overdue".to_string();
    }
}

/// Flip every open task whose *actor* is gone to `abandoned`, and flag (or
/// clear) `reviewer_gone` on an `in_review`/`rework` task whose reviewer is
/// not live. Returns every task that changed, so the caller can persist and
/// announce them.
///
/// Pure over a task list and a roster, with no lock, no clock and no I/O, so
/// the transition can be tested directly. `live` is the set of panes whose
/// child process is still running, from `SessionManager::liveness`.
///
/// The rule is deliberately narrow. Only `is_open` tasks are touched, so a
/// finished or cancelled task is never rewritten by a pane closing later. And
/// only a target that is genuinely absent counts: a pane that is merely slow is
/// still in `live`, and slowness is what `overdue` is for.
///
/// This is the fix for the failure that made "overdue" permanent. Before it, a
/// task whose agent had died sat open forever, and the conductor could not tell
/// that from an agent still thinking. The absence of a result meant both, so it
/// meant nothing.
///
/// `in_review` is a second fix for a related bug: this used to check the
/// *target*'s liveness for every open status, including `in_review`, even
    /// though the target has already submitted and is no longer the actor: the
    /// reviewer is. That meant a target closing its pane after a clean submission
/// could erase a result the reviewer had not even looked at yet, while a task
/// stuck on a reviewer that no longer existed (the actual failure, see
/// BACKLOG.md) went completely undetected: the target was still live, so
/// nothing ever flagged it. `in_review` is therefore never abandoned by this
/// sweep, whatever the target is doing; a dead reviewer is surfaced instead,
/// because reassigning the reviewer is all it takes to recover, and abandoning
/// a result nobody asked to discard would be worse than leaving it stuck.
/// `rework` keeps the target-liveness check, because rework hands the
/// reviewer's findings back to the target to act on, so the target is the
/// actor again there.
fn abandon_lost(tasks: &mut [Task], live: &[String], now_ms: u64) -> Vec<Task> {
    let mut changed = Vec::new();
    for t in tasks.iter_mut() {
        if !is_open(&t.status) {
            continue;
        }

        // Tracks whether this task needs a record appended, so a rework task
        // that both flips reviewer_gone and then gets abandoned in the same
        // sweep (dead reviewer, dead target) appends once with its final
        // state, not once per check that touched it.
        let mut touched = false;

        if matches!(t.status.as_str(), "in_review" | "rework") {
            // An empty reviewer means review was waived, so there is no
            // reviewer to be gone; that combination should not occur for
            // these statuses in practice (see `finish_pending` and
            // `choose_reviewer`), but treating it as "not gone" is the safe
            // reading if it ever does.
            let reviewer_gone = !t.reviewer.is_empty() && !live.iter().any(|id| id == &t.reviewer);
            if t.reviewer_gone != reviewer_gone {
                t.reviewer_gone = reviewer_gone;
                touched = true;
            }
            if t.status == "in_review" {
                if touched {
                    changed.push(t.clone());
                }
                continue;
            }
            // rework still falls through to the target-liveness check below:
            // the reviewer flag above is informational, not a substitute for
            // settling a rework task whose implementer is actually gone.
        }

        if live.iter().any(|id| id == &t.target) {
            if touched {
                changed.push(t.clone());
            }
            continue;
        }
        t.status = STATUS_ABANDONED.to_string();
        // Terminal, so it takes a finish time like any other terminal state.
        // Without one the UI cannot tell an abandonment that just happened from
        // one already in the store at startup, and announces the whole history.
        t.done_ms = Some(now_ms);
        // A terminal task is not waiting on anyone, reviewer included: the
        // flag means something only for an open task.
        t.reviewer_gone = false;
        // The result field is where a conductor looks for what happened, and it
        // is empty precisely because nobody reported. Say so there rather than
        // leaving a blank that reads like a result of no content.
        if t.result.is_empty() {
            t.result = format!(
                "Abandoned: session '{}' is no longer running, so this task can \
                 never report a result. Re-dispatch it to a live pane if the work \
                 is still wanted.",
                t.target
            );
        }
        changed.push(t.clone());
    }
    changed
}

/// Record a question from a working agent and block its task on the answer.
///
/// Pure over a task list so the policy is testable without a live `Shared`.
/// Only the task's own target may ask, for the same reason only the target may
/// complete: a bystander posing questions against someone else's task would let
/// any pane spend the conductor's attention.
fn ask_pending(
    tasks: &mut [Task],
    caller: &str,
    id: &str,
    question: &str,
    now_ms: u64,
) -> Result<(), AskError> {
    match tasks.iter_mut().find(|t| t.id == id) {
        None => Err(AskError::Access(TaskAccessError::NotFound)),
        Some(t) if t.target != caller => Err(AskError::Access(TaskAccessError::Forbidden)),
        // Asking about work that is over is not a question, it is a leak. The
        // agent should have finished, and answering would imply the task is
        // live again when nothing will act on the answer.
        Some(t) if !is_open(&t.status) => Err(AskError::Access(TaskAccessError::NotPending)),
        Some(t) if t.status == STATUS_BLOCKED => Err(AskError::AlreadyAsking),
        Some(t) if t.exchanges.len() >= MAX_QUESTIONS_PER_TASK => Err(AskError::TooMany),
        Some(t) => {
            t.exchanges.push(Exchange {
                question: question.to_string(),
                answer: String::new(),
                asked_ms: now_ms,
            });
            t.status = STATUS_BLOCKED.to_string();
            Ok(())
        }
    }
}

/// Answer the open question on a task and set it working again.
///
/// Returns the question that was answered, so the conductor can be told what it
/// just resolved rather than only that something was resolved.
fn answer_pending(
    tasks: &mut [Task],
    caller: &str,
    conductor: Option<&str>,
    id: &str,
    answer: &str,
) -> Result<String, AskError> {
    // Only the conductor answers. The whole point is that questions stop going
    // to whoever happens to be nearest.
    if conductor != Some(caller) {
        return Err(AskError::Access(TaskAccessError::Forbidden));
    }
    match tasks.iter_mut().find(|t| t.id == id) {
        None => Err(AskError::Access(TaskAccessError::NotFound)),
        Some(t) if t.status != STATUS_BLOCKED => Err(AskError::NotAsking),
        Some(t) => match t.exchanges.iter_mut().rev().find(|e| e.answer.is_empty()) {
            None => Err(AskError::NotAsking),
            Some(open) => {
                open.answer = answer.to_string();
                let question = open.question.clone();
                // Back to pending, not overdue: the clock that matters for
                // "is this slow" is dispatch time, and `age` recomputes that on
                // the next read anyway.
                t.status = "pending".to_string();
                Ok(question)
            }
        },
    }
}

/// The open question on a task, if it has one.
fn open_question(t: &Task) -> Option<&Exchange> {
    t.exchanges.iter().rev().find(|e| e.answer.is_empty())
}

/// Whether `ask_conductor`'s wait loop should release the agent because its
/// task left `blocked` for some other reason (`cancel_task` is the common
/// case). Pure over the task's current status so the loop's early-return is
/// testable without a live handler. Without this, `answer_question` refuses
/// once the task is no longer blocked, and the agent would otherwise wait
/// out the full timeout for an answer that can never come.
fn blocked_wait_should_release(status: &str) -> bool {
    status != STATUS_BLOCKED
}

/// Why an ask or an answer was refused. Wraps `TaskAccessError` rather than
/// extending it, because the existing variants already carry meanings that
/// `complete_task` and `review_task` depend on.
#[derive(Debug, PartialEq)]
enum AskError {
    Access(TaskAccessError),
    /// A second question while the first is still open. Asking blocks, so this
    /// means the agent did not wait, and answering both would be ambiguous.
    AlreadyAsking,
    /// Answering a task that has no open question.
    NotAsking,
    /// The per-task ceiling. A pane in a question loop is bounded here.
    TooMany,
}

/// So `ask_task_question` can route through `mutate_task_and_journal`,
/// which needs to turn "the task this id names is gone" into whatever
/// error type the caller's mutator returns.
impl From<TaskAccessError> for AskError {
    fn from(e: TaskAccessError) -> Self {
        AskError::Access(e)
    }
}

/// States a dispatched agent may still report a result from.
///
/// "overdue" belongs here and its absence was a real bug: a task that aged out
/// could never be completed, so an agent that did the whole job came back to
/// report and was refused, and the work was discarded. Nothing ever cancelled
/// that agent, so the wall clock alone decided the result was worthless.
fn accepts_result(status: &str) -> bool {
    // "rework" is here because a rejected review has to be answerable. Without
    // it the implementer is told what is wrong and given no way to say it is
    // fixed, so the review round trip dead-ends on its first rejection.
    // "blocked" is here because an agent whose question timed out, or that
    // worked out the answer for itself while waiting, must be able to report
    // what it did. Refusing would discard finished work over an unanswered
    // question, which is the same mistake the overdue bug made.
    matches!(status, "pending" | "overdue" | "rework" | STATUS_BLOCKED)
}

/// The core of `finish_task`: flip a task to "done" if it is still awaiting a
/// result. Pure over a task list, with no lock or emit, so the state
/// transition can be tested directly.
#[derive(Debug, PartialEq)]
enum TaskAccessError {
    NotFound,
    Forbidden,
    NotPending,
}

fn finish_pending(
    tasks: &mut [Task],
    caller: &str,
    id: &str,
    result: &str,
    now_ms: u64,
) -> Result<(), TaskAccessError> {
    match tasks.iter_mut().find(|t| t.id == id) {
        Some(t) if t.target != caller => Err(TaskAccessError::Forbidden),
        // Genuinely terminal states still refuse: a cancelled task should not
        // resurrect, and a completed one should not be silently rewritten.
        Some(t) if !accepts_result(&t.status) => Err(TaskAccessError::NotPending),
        Some(t) => {
            t.result = result.to_string();
            // The gate. Work with a named reviewer is not finished when the
            // agent that did it says so, which was the previous contract and
            // the reason "done" meant "self-certified".
            t.status = if t.reviewer.is_empty() {
                "done".to_string()
            } else {
                "in_review".to_string()
            };
            // Only the ungated path finished here. Work that went to a reviewer
            // finishes when they sign off, so stamping it now would date it to
            // the submission instead.
            if is_terminal(&t.status) {
                t.done_ms = Some(now_ms);
                t.reviewer_gone = false;
            }
            // Reviewed work now owes its reviewer a notice; `Shared::finish_task`
            // drains the reviewer's pane afterwards to try delivering it.
            if t.status == "in_review" {
                t.notice_delivered = false;
            }
            Ok(())
        }
        None => Err(TaskAccessError::NotFound),
    }
}

/// Sign off on a task, or send it back. The other half of the gate.
///
/// Separate from `finish_pending` because the callers are different sessions
/// with different rights: the target may submit work, and only the named
/// reviewer may decide whether it counts.
fn review_pending(
    tasks: &mut [Task],
    caller: &str,
    id: &str,
    approved: bool,
    findings: &str,
    now_ms: u64,
) -> Result<(), TaskAccessError> {
    match tasks.iter_mut().find(|t| t.id == id) {
        // Only the named reviewer. Not the implementer, who would be
        // self-certifying by another route, and not a bystander.
        Some(t) if t.reviewer != caller => Err(TaskAccessError::Forbidden),
        Some(t) if t.status != "in_review" => Err(TaskAccessError::NotPending),
        Some(t) => {
            t.findings = findings.to_string();
            // Rejection returns it to the implementer rather than killing it.
            // The result stays: the reviewer is judging a specific submission
            // and discarding it would lose what they were judging.
            t.status = if approved { "done" } else { "rework" }.to_string();
            // Sign-off is where reviewed work actually finishes. A rejection
            // sends it back to "rework", which is live again, so it keeps its
            // empty finish time.
            if is_terminal(&t.status) {
                t.done_ms = Some(now_ms);
                t.reviewer_gone = false;
            }
            // A rejection now owes its target a rework notice;
            // `Shared::review_task` drains the target's pane afterwards to
            // try delivering it.
            if t.status == "rework" {
                t.notice_delivered = false;
            }
            Ok(())
        }
        None => Err(TaskAccessError::NotFound),
    }
}

/// What a conductor passes as `reviewer` to skip review deliberately.
///
/// A word rather than an empty string, because the two must not be the same
/// thing. Omitting the field is a conductor that did not think about review;
/// this is one that did and decided against it.
pub const REVIEW_WAIVED: &str = "none";

/// Who reviews this task: the empty string when nobody does.
///
/// **Omitting a reviewer requests one rather than skipping one.** That default
/// is the entire enforcement mechanism. An opt-in gate is skipped by the
/// conductor who forgets, which is precisely the failure being fixed, so
/// forgetting has to produce a review rather than the absence of one.
///
/// A third party is preferred over the dispatcher, on the grounds that whoever
/// wrote the brief is the reader least likely to notice it was misread. The
/// dispatcher is still a fine reviewer, and is the intended case for a
/// conductor checking work from a different model.
///
/// An unnamed reviewer also prefers a session whose CLI *kind* differs from
/// the target's, before falling back to any other live session. This is
/// `CONTRIBUTING.md`'s "Review before you commit" rule ("route to a
/// **different-model** reviewer") applied automatically: before this, a
/// conductor that omitted `reviewer` could get the target's own kind back,
/// which enforces the review gate without enforcing the reason it exists.
/// Naming a reviewer explicitly still bypasses this preference entirely,
/// same as it already bypasses the third-party-over-dispatcher tie-break:
/// an explicit choice is the conductor doing the job this defaults for.
fn choose_reviewer(
    requested: &str,
    from: &str,
    target: &str,
    live: &[String],
    sessions: &[AgentSession],
) -> Result<String, String> {
    if requested == REVIEW_WAIVED {
        return Ok(String::new());
    }

    if !requested.is_empty() {
        if requested == target {
            return Err(format!(
                "'{requested}' cannot review its own work. Name a different session, \
                 or pass reviewer '{REVIEW_WAIVED}' to skip review for this task."
            ));
        }
        if !live.iter().any(|s| s == requested) {
            return Err(format!(
                "no live session '{requested}' to review this. Call list_sessions for \
                 valid reviewers."
            ));
        }
        return Ok(requested.to_string());
    }

    // The existing third-party-over-dispatcher tie-break, applied first to
    // whichever candidates share the preference (a different kind than the
    // target), then, if that group is empty, to every live candidate. Kept
    // as one closure so the tie-break itself is written once rather than
    // twice.
    let pick = |candidates: &[String]| -> Option<String> {
        let third_party = candidates.iter().find(|s| *s != target && *s != from);
        let dispatcher = candidates.iter().find(|s| *s != target && *s == from);
        third_party.or(dispatcher).cloned()
    };

    let kind_of = |name: &str| -> &str {
        sessions
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.kind.as_str())
            .unwrap_or("")
    };
    // An empty kind means "unidentified", not "different". Treating it as a
    // real kind would make an unidentified session look preferable to a
    // same-kind one that has actually announced itself, which is backwards:
    // there is no evidence it differs at all.
    let target_kind = kind_of(target);
    let different_kind: Vec<String> = live
        .iter()
        .filter(|s| !target_kind.is_empty() && !kind_of(s).is_empty() && kind_of(s) != target_kind)
        .cloned()
        .collect();

    match pick(&different_kind).or_else(|| pick(live)) {
        Some(reviewer) => Ok(reviewer),
        // A workspace with nobody else in it. Refusing the dispatch would make
        // a solo pane unusable, so this waives and the caller says so out loud.
        None => Ok(String::new()),
    }
}

/// A single-id `get_task_result` lookup, open to the task's dispatcher,
/// target, and reviewer. The no-id listing (`tasks_from`) stays
/// dispatcher-only; this only widens who may read one task by its full id,
/// which Phase 2 needs so a review request or rework notice can point its
/// reader at `get_task_result` and have the call actually work.
fn task_for_reader<'a>(
    tasks: &'a mut [Task],
    caller: &str,
    id: &str,
) -> Result<&'a mut Task, TaskAccessError> {
    let task = tasks
        .iter_mut()
        .find(|task| task.id == id)
        .ok_or(TaskAccessError::NotFound)?;
    if task.from != caller && task.target != caller && task.reviewer != caller {
        return Err(TaskAccessError::Forbidden);
    }
    Ok(task)
}

/// Add a note to a task's result without losing what was already there. Used
/// by both `cancel_pending` and `reassign_pending`, whose refusals must never
/// look like the erasure of a real result: submitted work is exactly what a
/// conductor is trying to recover when it reaches for either tool.
fn append_note(t: &mut Task, note: &str) {
    t.result = if t.result.is_empty() {
        note.to_string()
    } else {
        format!("{}\n\n{note}", t.result)
    };
}

/// What happened to one id passed to `cancel_pending`.
#[derive(Debug, PartialEq)]
enum CancelOutcome {
    Cancelled,
    /// Already done, error, cancelled, or abandoned. Not a refusal: asking to
    /// cancel work that already finished is a stale request, not a mistake,
    /// and the 38-stale-task diagnosis this tool exists to clear out is
    /// exactly the situation where a conductor does not know which is which.
    AlreadyTerminal,
    NotFound,
}

/// Cancel one task if it is still open (pending, overdue, in_review, rework,
/// or blocked), recording why. Pure over a task list, so the transition is
/// testable without a live `Shared`.
///
/// Conductor-only enforcement lives in the `cancel_task` tool, the same way
/// it does for `dispatch`: this function has no notion of who may call it,
/// only of what cancelling means once the caller is already authorized.
fn cancel_pending(tasks: &mut [Task], id: &str, reason: &str, now_ms: u64) -> CancelOutcome {
    let Some(t) = tasks.iter_mut().find(|t| t.id == id) else {
        return CancelOutcome::NotFound;
    };
    if !is_open(&t.status) {
        return CancelOutcome::AlreadyTerminal;
    }
    let reason = reason.trim();
    append_note(
        t,
        &if reason.is_empty() {
            "Cancelled by the conductor.".to_string()
        } else {
            format!("Cancelled by the conductor: {reason}")
        },
    );
    t.status = "cancelled".to_string();
    t.done_ms = Some(now_ms);
    // Terminal, so nobody is waiting on a reviewer any more; the flag would
    // otherwise show "cancelled, reviewer gone" forever.
    t.reviewer_gone = false;
    CancelOutcome::Cancelled
}

/// Why a reassignment was refused.
#[derive(Debug, PartialEq)]
enum ReassignError {
    NotFound,
    /// `target` was given but the task is not pending or overdue, so there is
    /// nothing to redeliver.
    NotOpenForRetarget,
    /// `reviewer` was given but the task is not in_review or rework, so there
    /// is no reviewer to replace.
    NotOpenForReview,
    /// Neither field was given, or both were. A task's status decides which
    /// one field it accepts; naming both invites changing a status the caller
    /// did not ask for, and naming neither leaves nothing to do.
    AmbiguousChange,
    SameTarget,
    SameReviewer,
    /// The named reviewer is also the target: the same self-certification
    /// `choose_reviewer` already refuses at dispatch time.
    ReviewerIsTarget,
    NotLive(String),
    /// `target` was given while dispatch is halted (Stop). Retargeting types
    /// a brief into a terminal the same way dispatch does, so it is gated
    /// the same way.
    Halted,
    /// The named target is also the task's current reviewer: retargeting
    /// there would let the reviewer submit and then sign off on its own
    /// work, the same self-certification `ReviewerIsTarget` already refuses
    /// from the other direction.
    TargetIsReviewer,
}

/// Retarget a pending, overdue, abandoned, or queued task, or hand an
/// in_review/rework task to a new reviewer. Exactly one of `new_target` /
/// `new_reviewer` may be set, because which field a task accepts depends on
/// its status.
///
/// Pure over the task list and the live roster, so the branching is testable
/// without a live `Shared`. Redelivering the brief to a new target happens in
/// `Shared::reassign_task`, which is the only place that can reach the
/// engine; this function only decides whether the change is allowed and
/// leaves the task ready for it.
fn reassign_pending(
    tasks: &mut [Task],
    id: &str,
    caller: &str,
    new_target: Option<&str>,
    new_reviewer: Option<&str>,
    live: &[String],
    now_ms: u64,
) -> Result<Task, ReassignError> {
    let (new_target, new_reviewer) = match (new_target, new_reviewer) {
        (Some(target), None) if !target.is_empty() => (Some(target), None),
        (None, Some(reviewer)) if !reviewer.is_empty() => (None, Some(reviewer)),
        _ => return Err(ReassignError::AmbiguousChange),
    };

    let t = tasks
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or(ReassignError::NotFound)?;

    if let Some(target) = new_target {
        if !matches!(
            t.status.as_str(),
            "pending" | "overdue" | STATUS_ABANDONED | STATUS_QUEUED
        ) {
            return Err(ReassignError::NotOpenForRetarget);
        }
        if target == t.target {
            return Err(ReassignError::SameTarget);
        }
        if target == t.reviewer {
            return Err(ReassignError::TargetIsReviewer);
        }
        if !live.iter().any(|s| s == target) {
            return Err(ReassignError::NotLive(target.to_string()));
        }
        append_note(
            t,
            &format!("[reassigned by conductor: target {} -> {target}]", t.target),
        );
        if caller != t.from {
            append_note(
                t,
                &format!(
                    "[reassigned by conductor: dispatcher {} -> {caller}]",
                    t.from
                ),
            );
            t.from = caller.to_string();
        }
        t.target = target.to_string();
        // Fresh delivery, fresh clock: the old dispatch time would otherwise
        // read as though the new target had already been sitting on it.
        t.status = "pending".to_string();
        t.ts_ms = now_ms;
        // A non-terminal status must not carry a finish stamp; an abandoned
        // task being revived needs this cleared same as any other reopen.
        t.done_ms = None;
        return Ok(t.clone());
    }

    let reviewer = new_reviewer.expect("checked above: exactly one of the two is Some");
    if !matches!(t.status.as_str(), "in_review" | "rework") {
        return Err(ReassignError::NotOpenForReview);
    }
    if reviewer == t.reviewer {
        return Err(ReassignError::SameReviewer);
    }
    if reviewer == t.target {
        return Err(ReassignError::ReviewerIsTarget);
    }
    if !live.iter().any(|s| s == reviewer) {
        return Err(ReassignError::NotLive(reviewer.to_string()));
    }
    let previous = if t.reviewer.is_empty() {
        "(none)".to_string()
    } else {
        t.reviewer.clone()
    };
    append_note(
        t,
        &format!("[reassigned by conductor: reviewer {previous} -> {reviewer}]"),
    );
    t.reviewer = reviewer.to_string();
    // The reviewer named here was just checked live, by the call above.
    t.reviewer_gone = false;
    Ok(t.clone())
}

/// Flip a task from `pending` back to `queued`, but only if it is still
/// exactly what `Shared::reassign_task` just delivered: `pending`, and
/// still aimed at `expected_target`. `None` for anything else, including
/// an id that no longer exists, and leaves `tasks` untouched: something
/// else (a cancel, a Stop) already decided this task's fate first, and
/// this must not overwrite that decision, which is the bug this guards.
/// See `Shared::reassign_task`'s occupied branch, the only caller.
///
/// Pure so the condition is testable directly: `reassign_task` itself
/// needs a live new target, which `shared_for_test`'s engine can never
/// report (see `reassign_pending_refuses_a_target_that_is_not_live`), so
/// this is the only way this specific fix is exercised here.
fn requeue_if_still_pending<'a>(
    tasks: &'a mut [Task],
    id: &str,
    expected_target: &str,
) -> Option<&'a Task> {
    let t = tasks.iter_mut().find(|t| t.id == id)?;
    if t.status == "pending" && t.target == expected_target {
        t.status = STATUS_QUEUED.to_string();
        Some(t)
    } else {
        None
    }
}

fn validate_path_component(label: &str, value: &str) -> Result<(), String> {
    let safe = !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
    if safe {
        Ok(())
    } else {
        Err(format!(
            "invalid {label}: use only ASCII letters, digits, '-' and '_'"
        ))
    }
}

/// Mark delivery failure only while the task is still pending. Completion or
/// cancellation that wins the race remains authoritative.
fn mark_pending_error(tasks: &mut [Task], id: &str, now_ms: u64) -> bool {
    match tasks
        .iter_mut()
        .find(|t| t.id == id && t.status == "pending")
    {
        Some(t) => {
            t.status = "error".to_string();
            t.done_ms = Some(now_ms);
            true
        }
        None => false,
    }
}

/// The status shown to an agent, with a dead reviewer folded in so it cannot
/// be rendered without it. Shared by `render_task` and `render_task_summary`
/// so the full view and the abbreviated one never disagree about it.
fn status_tag(t: &Task) -> String {
    if t.reviewer_gone {
        format!("{}, reviewer gone", t.status)
    } else {
        t.status.clone()
    }
}

/// One task rendered for an agent. Kept in one place so a single-id lookup and
/// the collect-everything listing never drift apart.
fn render_task(t: &Task) -> String {
    // Who signed off is part of the answer, not a footnote. A conductor
    // reading "done" needs to know whether that means reviewed or waived,
    // because the two carry very different confidence.
    let sign_off = match (t.status.as_str(), t.reviewer.as_str()) {
        ("done", "") => " (no review was required)".to_string(),
        ("done", r) => format!(" (reviewed by {r})"),
        ("in_review", r) => format!(" (awaiting review by {r})"),
        ("rework", r) => format!(" (sent back by {r})"),
        _ => String::new(),
    };

    // Findings matter most on rejection and are still worth showing on
    // approval: an approval with caveats is not an unqualified one.
    let findings = if t.findings.is_empty() {
        String::new()
    } else {
        format!("\n--- review by {} ---\n{}", t.reviewer, t.findings)
    };

    // A blocked task is the one status where the reader is the one who has to
    // act, so the question goes in the line itself rather than behind another
    // call. A conductor that has to go looking for the question will not.
    let asked = match open_question(t) {
        Some(e) => format!(
            "\n--- waiting on you ---\n{}\nAnswer with answer_question(\"{}\", ...)",
            e.question, t.id
        ),
        None => String::new(),
    };

    // The other state where the reader is the one who has to act: nobody is
    // going to sign this off until the conductor does something about it.
    // Named explicitly rather than left to the status tag alone, because
    // "in_review, reviewer gone" reads as a fact and this is a call to act.
    let reviewer_gone = if t.reviewer_gone {
        format!(
            "\n--- reviewer gone ---\n'{}' is no longer live, so this cannot be signed off as \
             is. reassign_task to hand review to someone else, or review it yourself if that \
             makes sense here.",
            t.reviewer
        )
    } else {
        String::new()
    };

    let status = status_tag(t);
    if t.result.is_empty() {
        format!(
            "[{status}]{sign_off} {} â†’ {}{asked}{reviewer_gone}",
            t.target, t.task
        )
    } else {
        format!(
            "[{status}]{sign_off} {} â†’ {}\n{}{findings}{asked}{reviewer_gone}",
            t.target, t.task, t.result
        )
    }
}

/// How much of the original brief to echo back in a multi-task listing.
const TASK_ECHO_CHARS: usize = 160;

/// Terminal tasks kept in the default listing, newest first.
const RECENT_FINISHED: usize = 10;

fn is_open(status: &str) -> bool {
    // "overdue" is explicitly still running, so it belongs with pending.
    // "in_review" and "rework" are open for a different reason: the work is
    // submitted but unsigned. Counting either as finished would hide exactly
    // the debt this gate exists to surface.
    // "blocked" is open in a third way: the agent is alive and waiting on an
    // answer, so the work is neither progressing nor finished. A conductor
    // collecting results needs to see the difference, because this is the one
    // open state where it is the conductor that has to act.
    // "queued" is open in a fourth way: recorded and real, but not yet typed
    // into any terminal, so it must survive `wait_for_tasks` and a listing
    // the same as any other unfinished task. `attribute_open_tasks`
    // deliberately excludes it from the "busy" bucket despite this, since
    // nothing is running yet for it to be slow at.
    matches!(
        status,
        "pending" | "overdue" | "in_review" | "rework" | STATUS_BLOCKED | STATUS_QUEUED
    )
}

fn truncate_chars(s: &str, limit: usize) -> String {
    let mut out: String = s.chars().take(limit).collect();
    if s.chars().nth(limit).is_some() {
        out.push_str("...");
    }
    out
}

/// Task ids however a conductor happened to separate them.
///
/// Commas, spaces, tabs and newlines all work, because a model asked for "the
/// ids" produces any of them and refusing the wrong separator turns a wait into
/// a syntax puzzle.
fn parse_task_ids(raw: &str) -> Vec<String> {
    raw.split([',', ' ', '\t', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// How long to wait: the caller's request, clamped.
///
/// Zero means "did not say" and takes the default rather than returning
/// instantly, because a wait that returns at once is a poll, and polling is the
/// thing this call exists to replace.
fn wait_timeout(requested_secs: u64) -> Duration {
    let secs = if requested_secs == 0 {
        WAIT_DEFAULT_SECS
    } else {
        requested_secs.min(WAIT_MAX_SECS)
    };
    Duration::from_secs(secs)
}

/// Which of the wanted ids are still running.
///
/// Anything `is_open` counts as still running, so a wait covers `in_review` and
/// `rework` too: work submitted but unsigned is not finished, and returning
/// then would hand the conductor a result the gate has not passed.
///
/// An id that matches no task is treated as finished rather than held open. A
/// task that vanished cannot complete, and waiting on it forever is the failure
/// this whole call exists to prevent. `abandoned` is not open, so a dead pane's
/// task ends the wait as soon as `reconcile_abandoned` settles it.
fn still_open(tasks: &[Task], wanted: &[String]) -> Vec<String> {
    wanted
        .iter()
        .filter(|id| tasks.iter().any(|t| &&t.id == id && is_open(&t.status)))
        .cloned()
        .collect()
}

/// One task in a listing.
///
/// Deliberately does NOT echo the whole brief. The conductor wrote it and
/// still has it; replaying every prompt back is what grew this response to
/// 111k characters over 39 tasks, past the tool response limit, so the
/// documented way to collect a fan-out failed exactly when a workspace had
/// been used enough to need it. The result is kept whole, because that is the
/// part the caller does not already have.
/// One summary line per task in `mine` whose id is in `open`, in `open`'s
/// order. Used by `wait_for_tasks`'s timeout message, which otherwise prints
/// bare ids and so never shows a `reviewer_gone` task for what it is: a task
/// that is not going to finish on its own.
fn render_open_task_summaries(mine: &[Task], open: &[String]) -> Vec<String> {
    open.iter()
        .filter_map(|id| mine.iter().find(|t| &t.id == id))
        .map(render_task_summary)
        .collect()
}

fn render_task_summary(t: &Task) -> String {
    let brief = truncate_chars(&t.task, TASK_ECHO_CHARS);
    if t.status == "done" {
        format!("[done] {} â†’ {}\n{}", t.target, brief, t.result)
    } else {
        format!("[{}] {} â†’ {}", status_tag(t), t.target, brief)
    }
}

/// The tasks a listing should show, and how many were held back.
///
/// Open tasks are never dropped: "what am I still waiting on" is the question
/// this tool exists to answer, and truncating that would be worse than the
/// size problem it fixes.
fn select_tasks(mine: Vec<Task>, include_all: bool, status: &str) -> (Vec<Task>, usize) {
    if !status.is_empty() {
        let filtered: Vec<Task> = mine.into_iter().filter(|t| t.status == status).collect();
        return (filtered, 0);
    }
    if include_all {
        return (mine, 0);
    }
    let total = mine.len();
    let (open, finished): (Vec<Task>, Vec<Task>) =
        mine.into_iter().partition(|t| is_open(&t.status));
    let kept_finished = finished.len().min(RECENT_FINISHED);
    let dropped = finished.len() - kept_finished;
    // Newest finished first, then re-joined after the open ones.
    let mut recent: Vec<Task> = finished;
    recent.sort_by_key(|t| std::cmp::Reverse(t.ts_ms));
    recent.truncate(kept_finished);
    let mut out = open;
    out.extend(recent);
    debug_assert!(out.len() + dropped == total);
    (out, dropped)
}

fn append_line(path: &PathBuf, s: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    f.write_all(s.as_bytes())
}

/// One agent's MCP handler. Shares the global store; holds its own declared identity.
#[derive(Clone)]
pub struct BrainHandler {
    shared: Arc<Shared>,
    identity: Arc<Mutex<Option<AgentSession>>>,
    /// Set when this handler serves ONE specific session on its own endpoint.
    /// Identity then comes from the connection: it can't be forgotten or
    /// spoofed, so the agent never has to declare a name.
    bound: Option<String>,
    // Used by the #[tool_handler]-generated code, which dead-code analysis can't see.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl BrainHandler {
    pub fn new(shared: Arc<Shared>) -> Self {
        Self {
            shared,
            identity: Arc::new(Mutex::new(None)),
            bound: None,
            tool_router: Self::tool_router(),
        }
    }

    /// A handler dedicated to one session, used by that session's own endpoint.
    pub fn bound_to(shared: Arc<Shared>, session: String) -> Self {
        Self {
            shared,
            identity: Arc::new(Mutex::new(None)),
            bound: Some(session),
            tool_router: Self::tool_router(),
        }
    }

    fn author(&self) -> String {
        if let Some(b) = &self.bound {
            return b.clone();
        }
        self.identity
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// The gate `dispatch`, `cancel_task`, and `reassign_task` all share:
    /// conductor-only, checked here rather than on `Shared`, because it is
    /// MCP-specific policy: `human_dispatch` (lib.rs) has no agent identity
    /// to check it against, since the human already decided who to promote.
    fn require_conductor(&self) -> Result<(), String> {
        let me = self.author();
        match self.shared.conductor() {
            Some(c) if c == me => Ok(()),
            Some(_) => Err("Refused: you are not the conductor of this workspace.".to_string()),
            None => {
                Err("Refused: no conductor is set. Ask the user to promote a pane.".to_string())
            }
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct Identify {
    /// A short name you go by, e.g. "claude-frontend".
    pub name: String,
    /// Your tool/kind, e.g. "claude", "codex", "opencode".
    #[serde(default)]
    pub kind: String,
    /// Optional brain to join. Usually the app assigns this; leave empty to keep
    /// whatever the app set (defaults to "main").
    #[serde(default)]
    pub room: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct DecisionArgs {
    /// What the decision is about, e.g. "auth" or "db-schema".
    pub topic: String,
    /// The decision itself.
    pub decision: String,
    /// Optional reasoning other agents should know.
    #[serde(default)]
    pub rationale: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct FactArgs {
    pub category: String,
    pub fact: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct BroadcastArgs {
    pub message: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchArgs {
    pub query: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct DispatchArgs {
    /// The session to hand the task to: use an id from list_sessions.
    pub target: String,
    /// The task, written the way you'd say it to a teammate.
    pub task: String,
    /// Session that must approve the result before it counts as done. Use a
    /// different model from the target where you can: that is the whole value.
    /// You may name yourself. Leave empty only to waive review deliberately.
    #[serde(default)]
    pub reviewer: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CancelArgs {
    /// One or more task_ids to cancel, separated by commas or spaces.
    pub task_ids: String,
    /// Why you're cancelling. Kept on the task, so the history says what
    /// happened rather than just that it stopped.
    #[serde(default)]
    pub reason: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReassignArgs {
    /// The task_id to reassign.
    pub task_id: String,
    /// New target for a pending or overdue task: redelivers the same brief to
    /// this live session and resets its dispatch clock. Leave empty when you
    /// are reassigning a reviewer instead.
    #[serde(default)]
    pub target: String,
    /// New reviewer for an in_review or rework task. Leave empty when you are
    /// retargeting a pending or overdue task instead.
    #[serde(default)]
    pub reviewer: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReviewArgs {
    /// The task_id you were asked to review.
    pub task_id: String,
    /// True to sign it off, false to send it back for rework.
    pub approved: bool,
    /// What you found. Required on rejection and worth writing on approval:
    /// an approval with caveats is not an unqualified one.
    #[serde(default)]
    pub findings: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CompleteArgs {
    /// The task_id you were given when the work was dispatched to you.
    pub task_id: String,
    /// What you did / what you found.
    pub result: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct AskArgs {
    /// The task_id you were given when this work was dispatched to you.
    pub task_id: String,
    /// What you need to know. Ask the specific thing that is blocking you, and
    /// say what you would do by default, so the answer can be a correction
    /// rather than a decision made from scratch.
    pub question: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct AnswerArgs {
    /// The blocked task to answer.
    pub task_id: String,
    /// The answer. The agent acts on this, so decide rather than deliberate.
    pub answer: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct WaitArgs {
    /// Task ids to wait for, separated by commas or spaces. Leave this out to
    /// wait on every task you have dispatched that is still running.
    #[serde(default)]
    pub task_ids: String,
    /// Give up after this many seconds and report what is still running.
    /// Defaults to 45 and is capped at 55, sized to the host's own transport
    /// timeout rather than the task. A timeout cancels nothing: call this
    /// again with the same ids to keep waiting.
    #[serde(default)]
    pub timeout_seconds: u64,
}

#[derive(Deserialize, JsonSchema)]
pub struct TaskQuery {
    /// A single task_id to check. Leave this out to collect the fan-out you
    /// are waiting on, which is the efficient way to gather parallel work.
    #[serde(default)]
    pub task_id: String,
    /// Only tasks with this status: pending, overdue, done, error, cancelled
    /// or abandoned. Use "pending" or "overdue" to ask what is still running.
    #[serde(default)]
    pub status: String,
    /// Include the whole dispatch history rather than open tasks plus the
    /// most recent finished ones. Large workspaces can exceed the response
    /// limit; prefer `status` when you want something specific.
    #[serde(default)]
    pub include_all: bool,
}

#[tool_router]
impl BrainHandler {
    #[tool(
        description = "Declare who you are in this Pantheon workspace. Call once at startup before other tools."
    )]
    fn set_session_identity(&self, Parameters(p): Parameters<Identify>) -> String {
        // On a dedicated endpoint Pantheon already knows who you are.
        if let Some(b) = &self.bound {
            if !p.room.is_empty() {
                if let Err(e) = self.shared.try_set_room(b, &p.room) {
                    return format!("Refused: {e}.");
                }
            }
            return format!("Already identified as '{b}': Pantheon knows this session.");
        }
        // Preserve the model the session was launched with: an agent that
        // re-identifies must not lose the model that was set at spawn, because
        // the roster would drop it and the conductor would see a different
        // pane than the one that actually exists.
        let existing_model = self
            .shared
            .sessions
            .lock()
            .unwrap()
            .iter()
            .find(|a| a.name == p.name)
            .map(|a| a.model.clone())
            .unwrap_or_default();
        let session = AgentSession {
            name: p.name.clone(),
            kind: p.kind.clone(),
            model: existing_model,
        };
        *self.identity.lock().unwrap() = Some(session.clone());
        // Replace rather than append: an agent that identifies twice should not
        // show up twice in the session list.
        {
            let mut all = self.shared.sessions.lock().unwrap();
            all.retain(|a| a.name != session.name);
            all.push(session);
        }
        if !p.room.is_empty() {
            if let Err(e) = self.shared.try_set_room(&p.name, &p.room) {
                return format!("Refused: {e}.");
            }
        }
        self.shared.app.emit("context-changed");
        // The shared-endpoint counterpart to the dedicated endpoint's
        // per-session handler factory: this call is itself the connection,
        // proof the session is here and reading its terminal, which is
        // exactly when a queued brief or an undelivered notice from before
        // a restart or reconnect needs to reach it. See `mark_connected`.
        self.shared.mark_connected(&p.name);
        format!(
            "Identity set to '{}' in brain '{}'",
            p.name,
            self.shared.room_for(&p.name)
        )
    }

    #[tool(
        description = "Record a decision so every other agent instantly knows it. Use for choices that affect shared work."
    )]
    fn record_decision(&self, Parameters(p): Parameters<DecisionArgs>) -> String {
        let body = if p.rationale.is_empty() {
            p.decision
        } else {
            format!("{}: {}", p.decision, p.rationale)
        };
        self.shared.add("decision", &self.author(), &p.topic, &body);
        "Decision recorded to the shared brain.".to_string()
    }

    #[tool(
        description = "Record a durable fact other agents can rely on (e.g. an API shape, a path, a convention)."
    )]
    fn record_fact(&self, Parameters(p): Parameters<FactArgs>) -> String {
        self.shared
            .add("fact", &self.author(), &p.category, &p.fact);
        "Fact recorded to the shared brain.".to_string()
    }

    #[tool(description = "Broadcast a short message or blocker to all agents.")]
    fn broadcast(&self, Parameters(p): Parameters<BroadcastArgs>) -> String {
        self.shared
            .add("broadcast", &self.author(), "broadcast", &p.message);
        "Broadcast sent.".to_string()
    }

    #[tool(
        description = "Read the shared context (recent decisions, facts, broadcasts) from all agents. Read this before re-deriving something."
    )]
    fn get_shared_context(&self) -> String {
        let room = self.shared.room_for(&self.author());
        let entries = self.shared.entries_snapshot();
        let mine: Vec<&Entry> = entries.iter().filter(|e| e.room == room).collect();
        if mine.is_empty() {
            return format!("No shared context yet in brain '{room}'.");
        }
        let mut out = format!("# Shared context: brain '{room}' (most recent first)\n");
        for e in mine.iter().rev().take(50) {
            out.push_str(&format!(
                "- [{}] ({}) {}: {}\n",
                e.kind, e.author, e.topic, e.body
            ));
        }
        out
    }

    #[tool(description = "Search the shared context for entries containing a query string.")]
    fn search_context(&self, Parameters(p): Parameters<SearchArgs>) -> String {
        let q = p.query.to_lowercase();
        let room = self.shared.room_for(&self.author());
        let entries = self.shared.entries_snapshot();
        let hits: Vec<&Entry> = entries
            .iter()
            .filter(|e| {
                e.room == room
                    && (e.body.to_lowercase().contains(&q) || e.topic.to_lowercase().contains(&q))
            })
            .collect();
        if hits.is_empty() {
            return format!("No matches for '{}'.", p.query);
        }
        let mut out = String::new();
        for e in hits.iter().take(50) {
            out.push_str(&format!("- [{}] {}: {}\n", e.kind, e.topic, e.body));
        }
        out
    }

    #[tool(
        description = "List the other AI agents live in this workspace right now, with their CLI kind, model (if one was set at launch), brain, and whether each is already working. Call this when you are planning work: if you are the conductor these are real agents you can hand tasks to in parallel via dispatch. A pane marked [busy ...] already has an open task; one marked OVERDUE has held it past the expected window and may be stuck, so prefer a free pane over waiting on it. A pane marked DEAD has had its process exit: dispatch to it will be refused, and any task it was holding has already been marked abandoned, so re-dispatch that work to a live pane rather than waiting on it."
    )]
    fn list_sessions(&self) -> String {
        let lines = self.shared.roster_lines();
        if lines.is_empty() {
            return "No live sessions.".to_string();
        }
        let me = self.author();
        let mut out = String::from("# Live sessions\n");
        for l in &lines {
            out.push_str(l);
            out.push('\n');
        }
        // The roster alone reads as passive status. Close with what the caller
        // can actually do with it, which differs by role.
        let peers = lines.len().saturating_sub(1);
        if self.shared.conductor().as_deref() == Some(me.as_str()) {
            out.push_str(&format!(
                "\nYou are the conductor: you can dispatch work to any of the other {peers} session(s). \
                 Dispatch to several before polling, so their work overlaps rather than queues.\n"
            ));
        } else {
            out.push_str(
                "\nYou are not the conductor, so dispatch will refuse: that is expected. \
                 You can still reach these agents through record_decision, record_fact and broadcast.\n",
            );
        }
        out
    }

    #[tool(
        description = "Conductor only: hand a task to another live AI agent in this workspace. Returns immediately with a task_id (it does NOT block) so dispatch every independent piece of work first, then call wait_for_tasks once with all the ids, and the agents run in parallel while you wait in a single call. Needing the answer before you can continue is a reason to dispatch and wait, not a reason to do the work yourself. Reach for this before doing a separable chunk of work yourself: each target is a different model with its own context window. Write the task as you would brief a colleague who cannot see your screen: the goal, the paths involved, and what to report back. Work is reviewed by default: if you do not name a reviewer, one is picked for you, preferring a live session running a different CLI than the target, and the result goes to in_review before done. Name a reviewer to choose who, ideally a different model from the target, and you may name yourself. Pass reviewer 'none' only when you have decided the work does not need checking. If the target already has open work, this is queued rather than typed in on top of it: the response says what it is queued behind, and it is delivered automatically, in order, once the pane is free. A per-target queue holds at most 3; a fourth dispatch is refused with the ids already waiting."
    )]
    fn dispatch(&self, Parameters(p): Parameters<DispatchArgs>) -> String {
        let me = self.author();

        // Conductor-only is MCP-specific policy, so it's checked here rather
        // than in `dispatch_task`: see that method's doc comment.
        if self.shared.is_halted() {
            return "Refused: dispatch is halted by the user (Stop). Do not retry.".to_string();
        }
        if let Err(refusal) = self.require_conductor() {
            return refusal;
        }

        match self
            .shared
            .dispatch_task(&me, &p.target, &p.task, &p.reviewer)
        {
            Err(e) => format!("Refused: {e}"),
            Ok(o) if o.queued => {
                let position = o.queue_position.unwrap_or(0);
                let behind = o
                    .already_busy
                    .as_deref()
                    .map(|id| format!(" behind {id}"))
                    .unwrap_or_default();
                format!(
                    "{} is occupied, so task {} is queued{behind} at position {position}. It \
                     will be delivered automatically once the pane is free; no need to \
                     re-dispatch it.",
                    p.target, o.task_id
                )
            }
            Ok(o) if o.delivered && !o.reviewer.is_empty() => format!(
                "Dispatched to {} as task {}. {} will review it before it counts as done, so \
                 expect status 'in_review' before 'done'. Poll get_task_result with that id.",
                p.target, o.task_id, o.reviewer
            ),
            Ok(o) if o.delivered => format!(
                "Dispatched to {} as task {} with NO review: nobody else is live to check it. \
                 Poll get_task_result with that id.",
                p.target, o.task_id
            ),
            Ok(o) => format!(
                "Task {} recorded for {} but delivery failed (could not write to that session), \
                 so status is 'error'.",
                o.task_id, p.target
            ),
        }
    }

    #[tool(
        description = "Conductor only: cancel any open task (pending, overdue, in_review, rework, or blocked), permanently. Accepts several task_ids at once, comma or space separated, so a sweep of stale work is one call rather than one per id. A task that already finished, errored, was cancelled, or was abandoned is reported as already terminal rather than an error: cancelling it again is a no-op you can safely include in a sweep, not a mistake. Say why in reason; it is kept on the task, so the history says what happened rather than only that it stopped."
    )]
    fn cancel_task(&self, Parameters(p): Parameters<CancelArgs>) -> String {
        if let Err(refusal) = self.require_conductor() {
            return refusal;
        }
        let ids = parse_task_ids(&p.task_ids);
        if ids.is_empty() {
            return "Refused: name at least one task_id.".to_string();
        }
        let mut out = String::new();
        for (id, outcome) in self.shared.cancel_tasks(&ids, &p.reason) {
            let line = match outcome {
                CancelOutcome::Cancelled => format!("{id}: cancelled."),
                CancelOutcome::AlreadyTerminal => {
                    format!("{id}: already finished, so there was nothing to cancel.")
                }
                CancelOutcome::NotFound => format!("{id}: no such task."),
            };
            out.push_str(&line);
            out.push('\n');
        }
        out
    }

    #[tool(
        description = "Conductor only: move a task off a target or reviewer that is stuck or gone, rather than leaving it stranded. Set target to redeliver a pending, overdue, queued, or already-abandoned task to a different live session: this resets its dispatch clock, the same as a fresh dispatch, and either types the brief into the new target's terminal or, if that pane is occupied, queues it there instead. Refused while dispatch is halted (Stop) for the same reason dispatch itself is. Set reviewer to hand an in_review or rework task to a different live reviewer instead, which requires no redelivery since a reviewer's assignment lives entirely in the task record. Set exactly one of the two, never both: which field a task accepts depends on its current status, and get_task_result will say which. Refuses a target or reviewer that is not live, that is the same as what is already there, or that would let one session both do the work and sign off on it, naming why."
    )]
    fn reassign_task(&self, Parameters(p): Parameters<ReassignArgs>) -> String {
        let target = (!p.target.is_empty()).then_some(p.target.as_str());
        let reviewer = (!p.reviewer.is_empty()).then_some(p.reviewer.as_str());
        if let Err(refusal) = self.require_conductor() {
            return refusal;
        }
        match self.shared.reassign_task(&self.author(), &p.task_id, target, reviewer) {
            Ok((task, delivered)) if target.is_some() && delivered => format!(
                "Task '{}' reassigned to {}: redelivered and reset to pending.",
                task.id, task.target
            ),
            Ok((task, _)) if target.is_some() => format!(
                "Task '{}' reassigned to {} but delivery did not happen; current status is '{}'.",
                task.id, task.target, task.status
            ),
            Ok((task, _)) => format!(
                "Task '{}' reassigned to reviewer {}.",
                task.id, task.reviewer
            ),
            Err(ReassignError::NotFound) => format!("No task '{}'.", p.task_id),
            Err(ReassignError::NotOpenForRetarget) => "Refused: target only applies to a pending, overdue, abandoned, or queued task. Use reviewer for an in_review or rework task."
                .to_string(),
            Err(ReassignError::NotOpenForReview) => "Refused: reviewer only applies to an in_review or rework task. Use target for a pending or overdue task."
                .to_string(),
            Err(ReassignError::AmbiguousChange) => "Refused: set exactly one of target or reviewer, matching the task's current status."
                .to_string(),
            Err(ReassignError::SameTarget) => "Refused: that is already the target.".to_string(),
            Err(ReassignError::SameReviewer) => {
                "Refused: that is already the reviewer.".to_string()
            }
            Err(ReassignError::ReviewerIsTarget) => {
                "Refused: the reviewer cannot be the session that did the work.".to_string()
            }
            Err(ReassignError::TargetIsReviewer) => {
                "Refused: the target cannot be the task's reviewer, that would let it approve its own work.".to_string()
            }
            Err(ReassignError::NotLive(who)) => format!(
                "Refused: no live session '{who}'. Call list_sessions for valid names."
            ),
            Err(ReassignError::Halted) => {
                "Refused: dispatch is halted by the user (Stop). Do not retry.".to_string()
            }
        }
    }

    #[tool(
        description = "Report the result of a task the conductor dispatched to you. If the task has a reviewer, this submits your work for review rather than finishing it: expect status in_review, and if it comes back as rework, fix what the findings raise and call this again."
    )]
    fn complete_task(&self, Parameters(p): Parameters<CompleteArgs>) -> String {
        match self
            .shared
            .finish_task(&self.author(), &p.task_id, &p.result)
        {
            Ok(t) if t.status == "in_review" => format!(
                "Result recorded and sent to {} for review. It is NOT done yet: if they send                  it back the status becomes 'rework' and you should fix what they raise and                  call complete_task again.",
                t.reviewer
            ),
            Ok(_) => "Result recorded: the conductor can now read it.".to_string(),
            Err(TaskAccessError::Forbidden) => {
                "Refused: this task is assigned to a different session.".to_string()
            }
            Err(TaskAccessError::NotFound) => format!("No task '{}'.", p.task_id),
            Err(TaskAccessError::NotPending) => format!(
                "Task '{}' is already finished or was cancelled, so no result was recorded.",
                p.task_id
            ),
        }
    }

    #[tool(
        description = "Sign off on a task you were named to review, or send it back. Only the named reviewer can call this, and only while the task is in_review. Approving marks it done; rejecting sets its status to 'rework' and keeps your findings on the record, but changes nothing else: nothing is typed into the agent's terminal, and the agent cannot read the record itself (get_task_result is dispatcher-only), so the conductor has to tell it by hand that rework is waiting. Read the work before deciding: an approval you did not earn is worse than no review, because it looks like one."
    )]
    fn review_task(&self, Parameters(p): Parameters<ReviewArgs>) -> String {
        if !p.approved && p.findings.trim().is_empty() {
            return "Refused: a rejection needs findings. Say what is wrong, or the agent                     doing the rework is guessing."
                .to_string();
        }
        match self
            .shared
            .review_task(&self.author(), &p.task_id, p.approved, &p.findings)
        {
            Ok(t) if p.approved => format!(
                "Approved task '{}'. It is now done and {} can read your findings.",
                p.task_id, t.from
            ),
            Ok(t) => format!(
                "Sent task '{}' back to {} as 'rework' with your findings.",
                p.task_id, t.target
            ),
            Err(TaskAccessError::Forbidden) => {
                "Refused: you are not the reviewer named for this task.".to_string()
            }
            Err(TaskAccessError::NotFound) => format!("No task '{}'.", p.task_id),
            Err(TaskAccessError::NotPending) => format!(
                "Task '{}' is not awaiting review, so there is nothing to sign off.",
                p.task_id
            ),
        }
    }

    #[tool(
        description = "Collect the results of work. With a task_id, the dispatcher, the target, and the named reviewer may all read that one task in full: a review request or rework notice you receive points you back here. With no task_id, this returns every task YOU dispatched (open ones plus the most recently finished) in one call, so do that instead of polling ids one by one; that listing stays dispatcher-only. Briefs are abbreviated in that listing; pass a task_id for one task in full, status to filter (pending, overdue, done, error, cancelled, abandoned), or include_all for the whole history. Statuses: an overdue task is STILL RUNNING and its result is still accepted, it has just taken longer than expected, so keep waiting rather than treating it as failed or re-dispatching it. abandoned is the opposite and is final: the pane holding that work no longer exists, so no result is ever coming, and re-dispatching to a live session is the only way to get it done. queued means it is recorded but has not been typed into its target's terminal yet, because that pane was occupied when it was dispatched; it will be delivered automatically. in_review means the work is submitted and waiting on its reviewer; rework means the reviewer sent it back and the agent is fixing it. Neither is finished, and only done means a reviewer signed off (or that review was explicitly waived, which the output tells you)."
    )]
    fn get_task_result(&self, Parameters(p): Parameters<TaskQuery>) -> String {
        if !p.task_id.is_empty() {
            return match self.shared.task_status(&self.author(), &p.task_id) {
                Ok(t) => render_task(&t),
                Err(TaskAccessError::Forbidden) => {
                    "Refused: you are not the dispatcher, target, or reviewer of this task."
                        .to_string()
                }
                Err(TaskAccessError::NotFound) => format!("No task '{}'.", p.task_id),
                // `task_status` looks a task up without caring about its state,
                // so this arm is unreachable today. Answering instead of
                // `unreachable!()` keeps a future change to that lookup from
                // turning a lookup into a panic inside a live tool call.
                Err(TaskAccessError::NotPending) => format!("No task '{}'.", p.task_id),
            };
        }

        let mine = self.shared.tasks_from(&self.author());
        if mine.is_empty() {
            return "You have not dispatched any tasks.".to_string();
        }
        let total = mine.len();
        let running = mine.iter().filter(|t| is_open(&t.status)).count();
        let (shown, dropped) = select_tasks(mine, p.include_all, &p.status);

        if shown.is_empty() {
            return format!(
                "No tasks with status '{}'. {total} dispatched in all.",
                p.status
            );
        }

        let mut out = if p.status.is_empty() {
            format!("# Your dispatched tasks ({total} total, {running} still running)\n")
        } else {
            format!(
                "# Your dispatched tasks with status '{}' ({} of {total})\n",
                p.status,
                shown.len()
            )
        };
        for t in &shown {
            out.push_str(&format!(
                "\n## {} â†’ {}\n{}\n",
                t.id,
                t.target,
                render_task_summary(t)
            ));
        }
        if dropped > 0 {
            out.push_str(&format!(
                "\n{dropped} older finished task(s) not shown. Pass include_all for the \
                 whole history, or status to filter. Briefs are abbreviated here; pass a \
                 task_id for one task in full.\n"
            ));
        }
        out
    }

    #[tool(
        description = "Block until work you dispatched finishes, then return its results. This is the companion to dispatch: dispatch every independent slice first, then call this once with all their task_ids, and they run in parallel while you wait in a single call instead of polling. Reach for it whenever you would otherwise do a piece of work yourself because you needed the answer before continuing: waiting costs you nothing that doing it yourself would not have cost, and the other panes work at the same time. Leave task_ids empty to wait on everything you have dispatched that is still open. Returns as soon as they are all finished, at the timeout with a note of what is still running, or within about 45-55 seconds either way: this host's own MCP transport does not reliably survive a longer single call, so a wait this size is one call in a short series, not the whole wait. A timeout is not a failure and does NOT cancel anything: the agents keep working and their results are still accepted afterwards, so simply call this again with the same task_ids to keep waiting. A task whose pane dies while you wait comes back as abandoned rather than holding the wait open."
    )]
    async fn wait_for_tasks(&self, Parameters(p): Parameters<WaitArgs>) -> String {
        let me = self.author();
        let requested = parse_task_ids(&p.task_ids);

        // Existence and ownership are settled before any blocking. Waiting out
        // the whole timeout to be told an id was mistyped is the worst
        // available answer, and it is the answer a naive poll loop gives.
        for id in &requested {
            match self.shared.task_status(&me, id) {
                Ok(_) => {}
                Err(TaskAccessError::Forbidden) => {
                    return format!(
                        "Refused: you are not the dispatcher, target, or reviewer of task '{id}'."
                    );
                }
                Err(_) => {
                    return format!(
                        "No task '{id}'. Nothing was waited on: check the id against \
                         get_task_result before waiting again."
                    );
                }
            }
        }

        let wanted = if requested.is_empty() {
            let open: Vec<String> = self
                .shared
                .tasks_from(&me)
                .into_iter()
                .filter(|t| is_open(&t.status))
                .map(|t| t.id)
                .collect();
            if open.is_empty() {
                return "Nothing to wait for: none of your dispatched tasks are still running."
                    .to_string();
            }
            open
        } else {
            requested
        };

        let deadline = Instant::now() + wait_timeout(p.timeout_seconds);
        let waited_on = wanted.len();
        loop {
            // Stop is a user pressing a button, so it outranks the wait. A
            // conductor blocked here would otherwise hold the workspace for the
            // full timeout after the user had asked it to stop.
            if self.shared.is_halted() {
                return "Stopped: dispatch was halted by the user while waiting. \
                        Collect what landed with get_task_result."
                    .to_string();
            }

            // Goes through `tasks_from`, which reconciles pane liveness on the
            // way past. That is what stops a dead pane holding this open: the
            // task becomes "abandoned", which is not open, so the wait ends with
            // a truthful answer instead of running to the timeout.
            let mine = self.shared.tasks_from(&me);

            // A blocked task ends the wait immediately. It is the one open state
            // that needs the conductor rather than the agent, so continuing to
            // wait would be waiting on itself: the agent is stopped until this
            // caller answers, and this caller is stopped until the agent moves.
            let blocked: Vec<&Task> = mine
                .iter()
                .filter(|t| wanted.contains(&t.id) && t.status == STATUS_BLOCKED)
                .collect();
            if !blocked.is_empty() {
                let mut out = format!(
                    "# {} task(s) are BLOCKED and waiting on you\n\nAnswer with \
                     answer_question(task_id, answer). Each agent is stopped until you do, \
                     and the rest of your wait is still running.\n",
                    blocked.len()
                );
                for t in &blocked {
                    let question = open_question(t)
                        .map(|e| e.question.as_str())
                        .unwrap_or("(question missing)");
                    out.push_str(&format!("\n## {} from {}\n{question}\n", t.id, t.target));
                }
                return out;
            }

            let open = still_open(&mine, &wanted);
            if open.is_empty() {
                let finished: Vec<Task> = mine
                    .into_iter()
                    .filter(|t| wanted.contains(&t.id))
                    .collect();
                let mut out = format!("# {waited_on} task(s) finished\n");
                for t in &finished {
                    out.push_str(&format!(
                        "\n## {} to {}\n{}\n",
                        t.id,
                        t.target,
                        render_task(t)
                    ));
                }
                return out;
            }

            if Instant::now() >= deadline {
                // Timing out and finishing must not look alike. A conductor that
                // cannot tell them apart will report work as done that is still
                // running, which is worse than not waiting at all.
                let summaries = render_open_task_summaries(&mine, &open);
                let mut out = format!(
                    "Timed out with {} of {waited_on} task(s) still running:\n{}\n\
                     They have NOT been cancelled: the agents are still working and their \
                     results are still accepted. Wait again, or collect what landed with \
                     get_task_result.\n",
                    open.len(),
                    summaries.join("\n")
                );
                let done: Vec<Task> = mine
                    .into_iter()
                    .filter(|t| wanted.contains(&t.id) && !is_open(&t.status))
                    .collect();
                for t in &done {
                    out.push_str(&format!(
                        "\n## {} to {}\n{}\n",
                        t.id,
                        t.target,
                        render_task(t)
                    ));
                }
                return out;
            }

            tokio::time::sleep(Duration::from_millis(WAIT_POLL_MS)).await;
        }
    }

    #[tool(
        description = "Ask the conductor that dispatched your task a question, and wait for the answer. Use this the moment a brief turns out to be ambiguous, instead of guessing or asking the human in your terminal. The conductor wrote the brief, holds the reasoning it compressed away, and can see the other work in flight, so it is usually the better answerer as well as the right one. Your task shows as 'blocked' while you wait, which is what tells the conductor to answer rather than keep waiting on you. Returns the answer, or tells you to use your own judgement if nobody answers in time; either way say in your result what you assumed. Bounded: at most 5 questions per task."
    )]
    async fn ask_conductor(&self, Parameters(p): Parameters<AskArgs>) -> String {
        let me = self.author();
        let question = p.question.trim();
        if question.is_empty() {
            return "Refused: ask a question. An empty one costs the conductor a turn and \
                    tells it nothing."
                .to_string();
        }

        // No conductor, or a halted workspace, means nobody is going to answer.
        // Falling back to the human is right here, but it has to be a stated
        // fallback rather than the silent default it used to be.
        let Some(conductor) = self.shared.conductor() else {
            return "No conductor is set, so there is nobody to ask. Decide with your own \
                    judgement and state the assumption in your result, or ask the human in \
                    your terminal if the decision is theirs to make."
                .to_string();
        };
        if self.shared.is_halted() {
            return "Dispatch is halted, so the conductor will not answer. Stop and wait for \
                    the human."
                .to_string();
        }

        match self.shared.ask_task_question(&me, &p.task_id, question) {
            Ok(()) => {}
            Err(AskError::Access(TaskAccessError::NotFound)) => {
                return format!("No task '{}'.", p.task_id);
            }
            Err(AskError::Access(TaskAccessError::Forbidden)) => {
                return "Refused: you can only ask about a task dispatched to you.".to_string();
            }
            Err(AskError::Access(TaskAccessError::NotPending)) => {
                return "That task is already finished, so there is nothing to unblock. If you \
                        have more to say, say it in a new task or record it as a decision."
                    .to_string();
            }
            Err(AskError::AlreadyAsking) => {
                return "You already have a question open on that task. Wait for it rather than \
                        stacking another."
                    .to_string();
            }
            Err(AskError::NotAsking) => {
                return "That task has no open question.".to_string();
            }
            Err(AskError::TooMany) => {
                return format!(
                    "Refused: {MAX_QUESTIONS_PER_TASK} questions is the ceiling for one task, \
                     and it has been reached. Decide with your own judgement and state the \
                     assumption in your result."
                );
            }
        }

        let deadline = Instant::now() + Duration::from_secs(ASK_TIMEOUT_SECS);
        loop {
            if let Some(exchange) = self.shared.task_answer(&p.task_id) {
                if !exchange.answer.is_empty() {
                    return format!(
                        "{conductor} answered:\n\n{}\n\nYour task is running again. Carry on.",
                        exchange.answer
                    );
                }
            }

            // A task that leaves "blocked" for any other reason (cancel_task
            // is the common case) releases the agent too. Without this,
            // answer_question refuses once the task is no longer blocked, so
            // this call would otherwise hold the agent to the full 900s
            // ceiling asking a question nobody can ever answer any more.
            if let Some(t) = self
                .shared
                .tasks_snapshot()
                .into_iter()
                .find(|t| t.id == p.task_id)
            {
                if blocked_wait_should_release(&t.status) {
                    return format!(
                        "Your task left 'blocked' while you were waiting (status is now \
                         '{}'), most likely because it was cancelled. There is nothing left \
                         to wait for here; call get_task_result if you need the details.",
                        t.status
                    );
                }
            }

            // A stop while blocked releases the agent rather than holding it to
            // the ceiling, for the same reason it releases a wait.
            if self.shared.is_halted() {
                return "Dispatch was halted while you waited. Stop and wait for the human."
                    .to_string();
            }

            if Instant::now() >= deadline {
                return format!(
                    "No answer from {conductor} within {}s. The question stays recorded on the \
                     task. Decide with your own judgement and say in your result what you \
                     assumed, so the conductor can correct it if the assumption was wrong.",
                    ASK_TIMEOUT_SECS
                );
            }

            tokio::time::sleep(Duration::from_millis(ASK_POLL_MS)).await;
        }
    }

    #[tool(
        description = "Conductor only: answer a question a working agent asked you. The task is blocked until you do, so answering is what starts it moving again. Questions show up on any task listed as 'blocked' by get_task_result or wait_for_tasks, and a wait returns early when one arrives precisely so you can answer it. The exchange is kept on the task, so an answer given once is not asked again."
    )]
    fn answer_question(&self, Parameters(p): Parameters<AnswerArgs>) -> String {
        let me = self.author();
        let answer = p.answer.trim();
        if answer.is_empty() {
            return "Refused: an empty answer leaves the agent exactly as stuck, but tells it \
                    the question was considered and dismissed."
                .to_string();
        }
        match self.shared.answer_task_question(&me, &p.task_id, answer) {
            Ok(question) => format!(
                "Answered. '{}' is running again.\n\nThe question was: {question}",
                p.task_id
            ),
            Err(AskError::Access(TaskAccessError::NotFound)) => {
                format!("No task '{}'.", p.task_id)
            }
            Err(AskError::Access(TaskAccessError::Forbidden)) => {
                "Refused: only the conductor answers questions.".to_string()
            }
            Err(AskError::Access(TaskAccessError::NotPending)) | Err(AskError::NotAsking) => {
                format!(
                    "Task '{}' is not waiting on an answer. Call get_task_result to see what \
                     is actually blocked.",
                    p.task_id
                )
            }
            Err(AskError::AlreadyAsking) | Err(AskError::TooMany) => "Refused.".to_string(),
        }
    }
}

/// What every connecting agent is told about the workspace it just joined.
///
/// Exposing well-described tools is not enough on its own: an agent that is not
/// told to consult shared context simply won't, and the brain stays empty while
/// two agents build incompatible halves of the same thing. MCP carries these
/// instructions on the connection itself, which is why this lives here rather
/// than in each project's AGENTS.md: it reaches every session automatically,
/// in whichever repo it was launched against.
///
/// The workspace section exists for a second, distinct failure: an agent that
/// treats Pantheon as a nicer terminal and never notices the other panes are
/// usable capacity. Because MCP delivers this text once, at connect time, it can
/// only describe the role an agent *might* be given: the conductor briefing
/// injected by `set_conductor` is what covers the role it actually has.
const BRAIN_INSTRUCTIONS: &str = r#"You are one of several AI agents working in parallel inside Pantheon, each in its own terminal, on the same project at the same time. This server is your shared brain: it is how you learn what the others have already decided, how they learn what you decide, and how work is handed between you.

Pantheon already knows who you are from this connection. You do not need to call set_session_identity.

## The workspace

The other panes are not logs or history. They are live AI coding agents, often different models with their own separate context windows, sitting idle until given work. Call list_sessions to see who is here.

Pantheon gives exactly one session the conductor role, and the user assigns it; you cannot claim it. Call list_sessions to find out whether that is you, and expect the answer to change during a run.

If you ARE the conductor, the rest of the workspace is yours to direct, and using it is the point of this tool:
- Before doing a separable piece of work yourself, ask whether it should be dispatched instead. Independent slices, such as different files or subsystems, separate research questions, or a second opinion from a different model, are what the other sessions are for.
- dispatch returns immediately with a task_id rather than blocking. So dispatch every independent task first and collect afterwards; that is what makes the agents run in parallel instead of queueing behind each other.
- wait_for_tasks blocks until the ids you name are finished, so you do not have to guess an interval and poll. Dispatch the whole fan-out, then wait on it once. "I will report when it lands" is only true if you actually wait; otherwise nothing brings you back. A single call returns within about 45-55 seconds either way, finished or not: that is one call in a short series, not the whole wait, so a timeout just means call it again with the same ids.
- Call get_task_result with no task_id to collect every task you dispatched in one call, rather than polling ids one at a time.
- A task reported as blocked is waiting on YOU, not on the agent. Answer it with answer_question(task_id, answer) and the agent starts moving again. wait_for_tasks returns early when one appears, precisely so you can answer without watching for it.
- If a target already holds open work, dispatch tells you so rather than silently piling a second brief on top of the first. cancel_task closes any open task with a reason, in bulk if you give it several ids. reassign_task changes a pending or overdue task's target (redelivering the brief) or an in_review/rework task's reviewer, so a stuck or gone session does not leave the work stranded.
- A dispatched agent cannot see your screen or your context. State the goal, the concrete paths, and what you want reported back.
- This does not replace your own subagents. Prefer a Pantheon session when you want a different model or a genuinely separate context window; prefer your own subagents for work inside your own.

If you are NOT the conductor, dispatch will refuse: that is expected, not an error to work around. When a line starting with "[pantheon] Task from conductor" appears in your terminal, that is real work assigned to you: carry it out, then call complete_task with the task_id you were given and a summary of the result. The conductor is waiting on that call.

If that brief turns out to be ambiguous, call ask_conductor with your task_id and the specific question rather than guessing or asking the human in your terminal. The conductor wrote the brief and holds the reasoning it compressed away, so it is usually the better answerer as well as the right one; the human often lacks the context and did not ask to be the routing point for five panes. Your task shows as blocked while you wait, and you get the answer back in the same call. If nobody answers in time you are told to use your own judgement, and then you should say in your result what you assumed.

## Shared context

- BEFORE making a decision that affects shared work, such as architecture, dependencies, data models, API shapes, file layout, or naming conventions, call get_shared_context. Another agent may have already settled it. Do not re-derive or quietly contradict an existing decision; if you disagree with one, broadcast the disagreement instead of diverging in silence.
- Use search_context to check one specific topic before you spend effort researching it.
- AFTER making such a decision, call record_decision with the topic, the decision, and your reasoning. This is the single most important thing you do here: it is what stops two agents building halves that don't fit together. If you are dispatching work that depends on a convention, record it before you dispatch.
- Use record_fact for durable things others will need: an API shape, a path, a command, a convention you just established.
- Use broadcast for blockers, or anything the others need to know immediately."#;

#[tool_handler]
impl ServerHandler for BrainHandler {
    // Supplying get_info suppresses the macro's generated one, so both the tools
    // capability and our own name/version have to be restated here: otherwise
    // no tools are advertised, and the server introduces itself to agents as
    // "rmcp" (the default is resolved inside that crate, not ours).
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("pantheon", env!("CARGO_PKG_VERSION")))
            .with_instructions(BRAIN_INSTRUCTIONS)
    }
}

/// Bind the MCP server on a random loopback port and spawn it. Returns the port
/// and the shared store (also used by the frontend `get_context` command).
/// Bind a loopback endpoint dedicated to ONE session. Because only that session
/// is registered against this port, every request on it is provably from that
/// session: identity without a handshake.
/// The caller owns the returned handle: dropping it does NOT stop the server, so
/// it must be aborted explicitly when the session ends, or the listener outlives
/// the session it was bound to.
pub struct SessionServer {
    pub port: u16,
    /// Secret this session must present as `Authorization: Bearer <token>`.
    /// Handed to the caller so it can be written into that one session's agent
    /// config; it is never logged and never leaves the machine.
    pub token: String,
    task: tauri::async_runtime::JoinHandle<()>,
}

impl SessionServer {
    /// Stop serving. Called when the session is killed or exits on its own.
    pub fn shutdown(self) {
        self.task.abort();
    }
}

/// Mint a secret for one session's endpoint.
///
/// Two v4 UUIDs, whose randomness comes from the OS CSPRNG via `getrandom`,
/// give 244 bits with no new dependency. Far past brute force, which matters
/// because the endpoint sits on loopback where anything local can reach it and
/// retry without limit.
fn mint_session_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Whether an `Authorization` header carries exactly the expected bearer token.
///
/// Compared in constant time over the whole candidate so a caller cannot learn
/// the secret one byte at a time from response latency. Localhost makes that
/// attack awkward rather than impossible, and the cost of not leaking is a
/// single XOR per byte. Split out from the middleware so it is directly
/// testable without standing up a server.
fn bearer_matches(header: Option<&str>, expected: &str) -> bool {
    let Some(value) = header else {
        return false;
    };
    let Some(presented) = value.strip_prefix("Bearer ") else {
        return false;
    };
    if presented.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in presented.bytes().zip(expected.bytes()) {
        diff |= a ^ b;
    }
    diff == 0
}

pub fn start_session_server(
    shared: Arc<Shared>,
    session_id: String,
) -> std::io::Result<SessionServer> {
    validate_path_component("session_id", &session_id)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let std_listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    std_listener.set_nonblocking(true)?;
    let port = std_listener.local_addr()?.port();
    let token = mint_session_token();
    let expected = token.clone();

    let task = tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(std_listener) {
            Ok(l) => l,
            Err(_) => return,
        };
        let service = StreamableHttpService::new(
            move || {
                // This factory runs once per new MCP session on this port,
                // which for a dedicated endpoint is the actual proof the CLI
                // is up and reading its terminal; see `mark_connected`.
                shared.mark_connected(&session_id);
                Ok(BrainHandler::bound_to(shared.clone(), session_id.clone()))
            },
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default(),
        );
        // The port still identifies WHICH session this is; the token proves the
        // caller is that session rather than any other local process that
        // guessed the port. Both checks, not one instead of the other.
        let router =
            axum::Router::new()
                .nest_service("/mcp", service)
                .layer(axum::middleware::from_fn(
                    move |req: axum::extract::Request, next: axum::middleware::Next| {
                        let expected = expected.clone();
                        async move {
                            let header = req
                                .headers()
                                .get(axum::http::header::AUTHORIZATION)
                                .and_then(|v| v.to_str().ok());
                            if bearer_matches(header, &expected) {
                                next.run(req).await
                            } else {
                                axum::http::StatusCode::UNAUTHORIZED.into_response()
                            }
                        }
                    },
                ));
        let _ = axum::serve(listener, router).await;
    });

    Ok(SessionServer { port, token, task })
}

pub fn start(
    app: AppHandle,
    dir: PathBuf,
    engine: Arc<crate::SessionManager>,
) -> std::io::Result<(u16, Arc<Shared>)> {
    let brain = load_brain(&dir);
    let shared = Arc::new(Shared {
        app: Notifier::to_app(app),
        dir: Mutex::new(dir),
        entries: Mutex::new(brain.entries),
        sessions: Mutex::new(brain.sessions),
        name_to_room: Mutex::new(HashMap::new()),
        engine,
        conductor: Mutex::new(None),
        connected: Mutex::new(HashSet::new()),
        halted: Mutex::new(false),
        tasks: Mutex::new(brain.tasks),
        dispatches: Mutex::new(0),
        delivery: RwLock::new(()),
        pane_delivery: Mutex::new(HashMap::new()),
        #[cfg(test)]
        test_seam: Mutex::new(None),
    });

    // Bind synchronously so we can hand the port back before the server task runs.
    let std_listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    std_listener.set_nonblocking(true)?;
    let port = std_listener.local_addr()?.port();

    let shared_for_server = shared.clone();
    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(std_listener) {
            Ok(l) => l,
            Err(_) => return,
        };
        let service = StreamableHttpService::new(
            move || Ok(BrainHandler::new(shared_for_server.clone())),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default(),
        );
        let router = axum::Router::new().nest_service("/mcp", service);
        let _ = axum::serve(listener, router).await;
    });

    Ok((port, shared))
}

#[cfg(test)]
mod tests {
    use super::{
        abandon_lost, age, answer_pending, append_queued, append_record, ask_pending,
        attribute_open_tasks, bearer_matches, blocked_wait_should_release, busy_label,
        cancel_pending, choose_reviewer, conductor_briefing, dispatch_precheck, dispatch_prompt,
        finish_pending, fit_injection, human_ms, injection_overage, is_occupied, is_open,
        is_terminal, load_brain, mark_pending_error, merge_live_identity, mint_session_token,
        next_delivery_for, occupying_task, open_question, oversize_refusal, parse_task_ids,
        queue_cap_refusal, queue_predecessor, queued_ids_for, reassign_pending,
        render_open_task_summaries, render_task, render_task_summary, requeue_if_still_pending,
        review_pending, review_request_notice, reviewing_label, rework_notice, select_tasks,
        single_line, status_tag, still_open, task_for_reader, truncate_bytes, truncate_chars,
        validate_path_component, wait_timeout, AgentSession, AskError, BrainHandler, CancelOutcome,
        Entry, Exchange, Identify, Notifier, Parameters, ReassignError, Shared, StoreRecord, Task,
        TaskAccessError, MAX_QUESTIONS_PER_TASK, QUEUE_CAP, RECENT_FINISHED, REVIEW_WAIVED,
        STATUS_ABANDONED, STATUS_BLOCKED, STATUS_QUEUED, TASK_ECHO_CHARS, TASK_OVERDUE_MS,
        WAIT_DEFAULT_SECS, WAIT_MAX_SECS,
    };
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex, RwLock};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn a_free_pane_carries_no_busy_marker() {
        assert_eq!(busy_label(None), "");
    }

    // ---- done_ms: when a task actually finished ----
    //
    // These exist because the UI used `ts_ms` for this and `ts_ms` never moves
    // off the dispatch time. Reading it as a finish time made every task in the
    // store look like it had just completed, which is what made a restart
    // announce the entire history at once.

    /// Dispatch time and finish time must be able to disagree, because the
    /// whole bug was code that assumed they could not.
    const DISPATCHED_AT: u64 = 1_000;
    const FINISHED_AT: u64 = 9_000;

    fn dispatched_task(reviewer: &str) -> Vec<Task> {
        vec![Task {
            id: "abc123".into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "audit the parser".into(),
            status: "pending".into(),
            result: String::new(),
            ts_ms: DISPATCHED_AT,
            reviewer: reviewer.into(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            done_ms: None,
            notice_delivered: true,
        }]
    }

    #[test]
    fn a_live_task_has_no_finish_time() {
        let tasks = dispatched_task("");
        assert_eq!(
            tasks[0].done_ms, None,
            "a task that has not finished must not claim a finish time"
        );
    }

    #[test]
    fn an_unreviewed_task_is_stamped_when_its_result_lands() {
        let mut tasks = dispatched_task("");
        finish_pending(&mut tasks, "sess-2", "abc123", "the report", FINISHED_AT).unwrap();

        assert_eq!(tasks[0].status, "done");
        assert_eq!(tasks[0].done_ms, Some(FINISHED_AT));
        assert_eq!(
            tasks[0].ts_ms, DISPATCHED_AT,
            "stamping the finish must not disturb the dispatch time"
        );
    }

    #[test]
    fn submitting_for_review_is_not_finishing() {
        // The case that makes a separate field necessary rather than reusing
        // ts_ms: the work exists, but nobody has signed it off.
        let mut tasks = dispatched_task("sess-3");
        finish_pending(&mut tasks, "sess-2", "abc123", "the report", FINISHED_AT).unwrap();

        assert_eq!(tasks[0].status, "in_review");
        assert_eq!(
            tasks[0].done_ms, None,
            "in_review is not finished, so it must carry no finish time"
        );
    }

    #[test]
    fn sign_off_is_what_finishes_reviewed_work() {
        let mut tasks = dispatched_task("sess-3");
        finish_pending(&mut tasks, "sess-2", "abc123", "the report", FINISHED_AT).unwrap();
        review_pending(&mut tasks, "sess-3", "abc123", true, "correct", 12_000).unwrap();

        assert_eq!(tasks[0].status, "done");
        assert_eq!(
            tasks[0].done_ms,
            Some(12_000),
            "reviewed work finishes when the reviewer signs off, not when it was submitted"
        );
    }

    #[test]
    fn a_rejected_review_leaves_the_task_unfinished() {
        let mut tasks = dispatched_task("sess-3");
        finish_pending(&mut tasks, "sess-2", "abc123", "first attempt", FINISHED_AT).unwrap();
        review_pending(&mut tasks, "sess-3", "abc123", false, "untested", 12_000).unwrap();

        assert_eq!(tasks[0].status, "rework");
        assert_eq!(
            tasks[0].done_ms, None,
            "rework is live again, so it must not carry a finish time"
        );
    }

    #[test]
    fn a_delivery_failure_is_a_finish() {
        let mut tasks = dispatched_task("");
        assert!(mark_pending_error(&mut tasks, "abc123", FINISHED_AT));

        assert_eq!(tasks[0].status, "error");
        assert_eq!(
            tasks[0].done_ms,
            Some(FINISHED_AT),
            "an errored task is terminal, and the UI has to be able to date it"
        );
    }

    #[test]
    fn a_task_from_a_previous_run_loads_without_a_finish_time() {
        // Every task already in brain.jsonl predates this field. They must load
        // rather than break the store, and None is the honest answer for a
        // finish time that was never recorded.
        let stored = r#"{"id":"old1","from":"sess-1","target":"sess-2","task":"t","status":"done","result":"r","ts_ms":5}"#;
        let task: Task = serde_json::from_str(stored).expect("an older task must still load");

        assert_eq!(task.status, "done");
        assert_eq!(task.done_ms, None);
    }

    #[test]
    fn a_pane_past_the_overdue_window_is_called_out_not_just_timed() {
        // Both strings carry the same number. Only one tells a conductor to
        // stop waiting, which is the whole point: a silent pane used to look
        // exactly like an idle one and got dispatched to again.
        let working = busy_label(Some(TASK_OVERDUE_MS - 1));
        let stuck = busy_label(Some(TASK_OVERDUE_MS + 1));

        assert!(working.contains("busy"));
        assert!(!working.contains("OVERDUE"));
        assert!(stuck.contains("OVERDUE"));
        assert!(stuck.contains("may be stuck"));
    }

    #[test]
    fn durations_read_at_a_glance() {
        assert_eq!(human_ms(0), "0s");
        assert_eq!(human_ms(45 * 1000), "45s");
        assert_eq!(human_ms(90 * 1000), "1m");
        assert_eq!(human_ms(45 * 60 * 1000), "45m");
        assert_eq!(human_ms(3 * 60 * 60 * 1000 + 7 * 60 * 1000), "3h07m");
    }

    fn task_at(id: &str, status: &str, ts_ms: u64) -> Task {
        Task {
            id: id.into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "audit the parser".into(),
            status: status.into(),
            result: String::new(),
            ts_ms,
            done_ms: None,
            reviewer: String::new(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            notice_delivered: true,
        }
    }

    #[test]
    fn a_listing_never_drops_an_open_task() {
        // "What am I still waiting on" is the question this tool exists to
        // answer, so windowing must never cost an open task.
        let mut tasks: Vec<Task> = (0..RECENT_FINISHED as u64 * 3)
            .map(|i| task_at(&format!("done{i}"), "done", i))
            .collect();
        tasks.push(task_at("open1", "pending", 999));
        tasks.push(task_at("open2", "overdue", 1000));

        let (shown, dropped) = select_tasks(tasks, false, "");

        let ids: Vec<&str> = shown.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"open1"), "pending task was dropped");
        assert!(ids.contains(&"open2"), "overdue task was dropped");
        assert_eq!(shown.len(), 2 + RECENT_FINISHED);
        assert_eq!(dropped, RECENT_FINISHED * 3 - RECENT_FINISHED);
    }

    #[test]
    fn a_listing_keeps_the_newest_finished_tasks() {
        let tasks: Vec<Task> = (0..RECENT_FINISHED as u64 + 5)
            .map(|i| task_at(&format!("t{i}"), "done", i))
            .collect();

        let (shown, dropped) = select_tasks(tasks, false, "");

        assert_eq!(dropped, 5);
        let oldest_kept = shown.iter().map(|t| t.ts_ms).min().unwrap();
        assert_eq!(oldest_kept, 5, "kept the oldest instead of the newest");
    }

    #[test]
    fn include_all_and_status_bypass_the_window() {
        let tasks: Vec<Task> = (0..RECENT_FINISHED as u64 + 5)
            .map(|i| task_at(&format!("t{i}"), "done", i))
            .collect();
        let total = tasks.len();

        let (all, dropped) = select_tasks(tasks.clone(), true, "");
        assert_eq!(all.len(), total);
        assert_eq!(dropped, 0);

        let (none, _) = select_tasks(tasks, false, "pending");
        assert!(none.is_empty(), "status filter must not invent matches");
    }

    #[test]
    fn a_listing_abbreviates_the_brief_but_never_the_result() {
        // The conductor wrote the brief and still has it. Replaying every
        // prompt is what pushed this response past the tool's size limit.
        let long_brief = "b".repeat(TASK_ECHO_CHARS * 4);
        let long_result = "r".repeat(4000);
        let mut t = task_at("abc", "done", 0);
        t.task = long_brief.clone();
        t.result = long_result.clone();

        let summary = render_task_summary(&t);

        assert!(!summary.contains(&long_brief), "brief was echoed in full");
        assert!(summary.contains("..."), "truncation was not marked");
        assert!(summary.contains(&long_result), "result must survive whole");
        assert!(
            render_task(&t).contains(&long_brief),
            "single lookup stays full"
        );
    }

    #[test]
    fn dispatch_prompt_is_single_line_and_includes_completion_contract() {
        let prompt = dispatch_prompt("sess-1", "abc123", "audit this\r\nthen report");

        assert!(!prompt.contains(['\r', '\n']));
        assert!(prompt.contains("audit this then report"));
        assert!(prompt.contains("task_id \"abc123\""));
    }

    #[test]
    fn an_injection_within_the_limit_survives_the_observed_truncation() {
        // The property the limit buys, stated as the corruption itself:
        // replay the measured mechanism (every complete leading 1 KiB chunk is
        // dropped, only the trailing partial chunk lands) and require the whole
        // injection back. An earlier attempt put an integrity *footer* in the
        // prompt instead; this test is what killed it. The surviving tail is
        // `len % 1024` bytes, which can be a handful, so no footer of any
        // length is guaranteed to arrive. Only staying under a chunk is.
        let task = "x".repeat(800);
        let prompt = dispatch_prompt("sess-1", "abc123", &task);
        assert!(injection_overage(&prompt).is_none(), "fixture is too long");

        let bytes = prompt.as_bytes();
        let survived = &bytes[(bytes.len() / 1024) * 1024..];

        assert_eq!(
            String::from_utf8_lossy(survived),
            prompt,
            "an injection under one chunk must lose nothing"
        );
    }

    #[test]
    fn the_limit_bites_at_exactly_one_whole_chunk() {
        // At 1024 there is one complete leading chunk and it is the one that
        // gets dropped, so the boundary is `>=`. Asserted directly because an
        // off-by-one here is invisible until a brief silently loses its head.
        assert_eq!(injection_overage(&"x".repeat(1023)), None);
        assert_eq!(injection_overage(&"x".repeat(1024)), Some(1));
        assert_eq!(injection_overage(&"x".repeat(1030)), Some(7));
    }

    #[test]
    fn the_gate_refuses_an_oversized_brief_but_reports_the_target_first() {
        let long = dispatch_prompt("sess-1", "abc123", &"x".repeat(2000));

        let err = dispatch_precheck(false, "sess-1", "sess-2", true, &long).unwrap_err();
        assert!(err.contains("too long to deliver intact"), "{err}");

        // Both are wrong here. The target is the one the conductor can act on
        // without rewriting anything, so it wins.
        let err = dispatch_precheck(false, "sess-1", "sess-2", false, &long).unwrap_err();
        assert!(err.contains("no live session"), "{err}");
    }

    /// A `Shared` with no live sessions and a scratch store.
    ///
    /// Possible only because of the `Notifier` seam: `Shared` used to hold an
    /// `AppHandle`, which cannot exist outside a running app, so every policy
    /// on a `Shared` method could only be verified by reading it. The `TempDir`
    /// is returned because dropping it would delete the store mid-test.
    fn shared_for_test() -> (Arc<Shared>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let shared = Arc::new(Shared {
            app: Notifier::silent(),
            dir: Mutex::new(dir.path().to_path_buf()),
            entries: Mutex::new(Vec::new()),
            sessions: Mutex::new(Vec::new()),
            name_to_room: Mutex::new(HashMap::new()),
            engine: Arc::new(crate::SessionManager::default()),
            conductor: Mutex::new(Some("sess-1".to_string())),
            connected: Mutex::new(HashSet::new()),
            halted: Mutex::new(false),
            tasks: Mutex::new(Vec::new()),
            dispatches: Mutex::new(0),
            delivery: RwLock::new(()),
            pane_delivery: Mutex::new(HashMap::new()),
            test_seam: Mutex::new(None),
        });
        // `app` owns the runtime the handle points at, so it has to outlive the
        // handle. Leaking it is cheaper than threading it through every caller
        // and this is a test process that is about to exit.

        (shared, dir)
    }

    #[test]
    fn a_refused_dispatch_charges_nothing_and_records_nothing() {
        // The property the whole gate rests on, and the one a reviewer caught
        // as untested: refusal happens before *any* mutation. Move
        // `dispatch_precheck` below `take_dispatch_budget` or `add_task` and
        // this fails, while every pure test above stays green.
        let (shared, _dir) = shared_for_test();

        let err = shared
            .dispatch_task("sess-1", "sess-2", "audit the parser", "")
            .expect_err("no session is live in this fixture");
        assert!(err.contains("no live session"), "{err}");

        assert!(
            shared.tasks_from("sess-1").is_empty(),
            "a refused dispatch left a task the conductor can never collect"
        );
        assert_eq!(
            *shared.dispatches.lock().unwrap(),
            0,
            "a refused dispatch charged the budget"
        );
    }

    #[test]
    fn a_realistic_brief_is_refused_with_a_move_the_conductor_can_make() {
        // Sized from the measurement that started this: the 2645-byte dispatch
        // that reached an opencode pane missing its first 2048 bytes. That one
        // now refuses instead of arriving half-eaten.
        let prompt = dispatch_prompt("sess-1", "abc123", &"x".repeat(2645));
        let refusal = oversize_refusal(&prompt).expect("2645 bytes cannot arrive whole");

        assert!(refusal.contains("too long to deliver intact"), "{refusal}");
        // A verdict without a next move invites retrying the same brief.
        assert!(
            refusal.contains("Split this into smaller tasks"),
            "{refusal}"
        );
        // The numbers have to be actionable, not decorative.
        assert!(
            refusal.contains(&format!("{} bytes;", prompt.len())),
            "{refusal}"
        );
        // "a 1024 byte limit" read as though 1024 were allowed.
        assert!(
            refusal.contains("the most that can be delivered is 1023"),
            "{refusal}"
        );

        assert_eq!(
            oversize_refusal(&dispatch_prompt("sess-1", "abc123", "audit the parser")),
            None,
            "an ordinary brief must still go through"
        );
    }

    // Both terminal injections share the same hard constraint: an embedded
    // newline submits the message to the target CLI in fragments, so the agent
    // acts on half a briefing. It matters more for the briefing than for a
    // dispatch, because the briefing is left unsent on purpose: a stray newline
    // would fire it off before the user has added anything.
    #[test]
    fn conductor_briefing_is_single_line_and_names_the_peers() {
        let peers = vec![
            "- sess-2 (codex) brain=main".to_string(),
            "- sess-3 (opencode) brain=main".to_string(),
        ];
        let msg = conductor_briefing(&peers);

        assert!(!msg.contains(['\r', '\n']));
        assert!(msg.contains("sess-2 (codex)"));
        assert!(msg.contains("sess-3 (opencode)"));
        assert!(msg.contains("dispatch"));
    }

    #[test]
    fn conductor_briefing_survives_an_empty_workspace() {
        let msg = conductor_briefing(&[]);

        assert!(!msg.contains(['\r', '\n']));
        assert!(msg.contains("No other sessions are open yet"));
    }

    // The user has to be able to type their own instruction after it, so it has
    // to stay short enough to read at a glance. The detail it used to carry now
    // lives in BRAIN_INSTRUCTIONS, which every agent gets on connect.
    #[test]
    fn conductor_briefing_stays_short_enough_to_type_after() {
        let peers = vec!["- sess-2 (codex) brain=main".to_string()];
        let msg = conductor_briefing(&peers);

        assert!(
            msg.len() < 400,
            "briefing is {} chars; it prefills the composer, so it must stay skimmable",
            msg.len()
        );
        // Trailing space so the user's own text does not run into the last word.
        assert!(msg.ends_with(' '));
    }

    #[test]
    fn render_task_shows_the_result_only_once_done() {
        let base = Task {
            id: "abc123".into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "audit the parser".into(),
            status: "pending".into(),
            result: String::new(),
            ts_ms: 0,
            done_ms: None,
            reviewer: String::new(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            notice_delivered: true,
        };
        assert!(render_task(&base).starts_with("[pending]"));

        let done = Task {
            status: "done".into(),
            result: "found two bugs".into(),
            ..base
        };
        let rendered = render_task(&done);
        assert!(rendered.starts_with("[done]"));
        assert!(rendered.contains("found two bugs"));
    }

    #[test]
    fn render_task_handles_the_error_status() {
        let base = Task {
            id: "abc123".into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "audit the parser".into(),
            status: "error".into(),
            result: String::new(),
            reviewer: String::new(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            ts_ms: 0,
            done_ms: None,
            notice_delivered: true,
        };
        assert!(render_task(&base).starts_with("[error]"));
    }

    // dispatch_precheck: order matters because it decides which message wins
    // when more than one check would refuse, and that order is what the live
    // `dispatch` tool preserves by checking `is_halted` first, outside this
    // function, before ever calling it.

    #[test]
    fn dispatch_precheck_refuses_when_halted() {
        let err = dispatch_precheck(true, "sess-1", "sess-2", true, "short").unwrap_err();
        assert!(err.contains("halted"));
    }

    #[test]
    fn dispatch_precheck_refuses_self_dispatch() {
        let err = dispatch_precheck(false, "sess-1", "sess-1", true, "short").unwrap_err();
        assert!(err.contains("cannot dispatch to yourself"));
    }

    #[test]
    fn dispatch_precheck_refuses_a_target_that_is_not_live() {
        let err = dispatch_precheck(false, "sess-1", "sess-2", false, "short").unwrap_err();
        assert!(err.contains("no live session 'sess-2'"));
    }

    #[test]
    fn dispatch_precheck_passes_a_valid_dispatch() {
        assert!(dispatch_precheck(false, "sess-1", "sess-2", true, "short").is_ok());
    }

    #[test]
    fn finish_pending_completes_a_pending_task() {
        let mut tasks = vec![Task {
            id: "abc123".into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "audit the parser".into(),
            status: "pending".into(),
            result: String::new(),
            ts_ms: 0,
            done_ms: None,
            reviewer: String::new(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            notice_delivered: true,
        }];
        assert_eq!(
            finish_pending(&mut tasks, "sess-2", "abc123", "found two bugs", 0),
            Ok(())
        );
        assert_eq!(tasks[0].status, "done");
        assert_eq!(tasks[0].result, "found two bugs");
    }

    #[test]
    fn finish_pending_refuses_an_unknown_id() {
        let mut tasks: Vec<Task> = vec![];
        assert_eq!(
            finish_pending(&mut tasks, "sess-2", "nope", "result", 0),
            Err(TaskAccessError::NotFound)
        );
    }

    #[test]
    fn complete_task_rejects_a_caller_other_than_the_assigned_target() {
        let mut tasks = vec![Task {
            id: "abc123".into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "audit".into(),
            status: "pending".into(),
            result: String::new(),
            ts_ms: 0,
            done_ms: None,
            reviewer: String::new(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            notice_delivered: true,
        }];

        assert_eq!(
            finish_pending(&mut tasks, "sess-3", "abc123", "stolen", 0),
            Err(TaskAccessError::Forbidden)
        );
        assert_eq!(tasks[0].status, "pending");
        assert!(tasks[0].result.is_empty());
    }

    #[test]
    fn get_task_result_allows_the_dispatcher_target_and_reviewer_but_no_one_else() {
        // Phase 2: a task's target and reviewer can now read it by full id
        // (the shape a review request or rework notice points them at), but
        // a bystander still cannot.
        let mut tasks = vec![Task {
            id: "abc123".into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "audit".into(),
            status: "pending".into(),
            result: String::new(),
            ts_ms: 0,
            done_ms: None,
            reviewer: "sess-4".into(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            notice_delivered: true,
        }];

        assert!(
            task_for_reader(&mut tasks, "sess-1", "abc123").is_ok(),
            "dispatcher"
        );
        assert!(
            task_for_reader(&mut tasks, "sess-2", "abc123").is_ok(),
            "target"
        );
        assert!(
            task_for_reader(&mut tasks, "sess-4", "abc123").is_ok(),
            "reviewer"
        );
        assert_eq!(
            task_for_reader(&mut tasks, "sess-3", "abc123").unwrap_err(),
            TaskAccessError::Forbidden,
            "a bystander must still be refused"
        );
    }

    #[test]
    fn path_components_reject_room_and_session_traversal() {
        assert!(validate_path_component("room", "main").is_ok());
        assert!(validate_path_component("session_id", "sess-1").is_ok());
        assert!(validate_path_component("room", "../../outside").is_err());
        assert!(validate_path_component("session_id", "..\\outside").is_err());
    }

    #[test]
    fn a_session_endpoint_accepts_only_its_own_bearer_token() {
        let token = mint_session_token();
        assert!(bearer_matches(Some(&format!("Bearer {token}")), &token));

        // Every way a caller can get it wrong.
        assert!(!bearer_matches(None, &token), "missing header");
        assert!(!bearer_matches(Some(""), &token), "empty header");
        assert!(
            !bearer_matches(Some(&token), &token),
            "raw token, no scheme"
        );
        assert!(
            !bearer_matches(Some(&format!("Basic {token}")), &token),
            "wrong scheme"
        );
        assert!(
            !bearer_matches(Some(&format!("bearer {token}")), &token),
            "scheme is case-sensitive per RFC 6750 usage here"
        );
        assert!(
            !bearer_matches(Some(&format!("Bearer {}", mint_session_token())), &token),
            "another session's token"
        );
        assert!(
            !bearer_matches(Some(&format!("Bearer {token}x")), &token),
            "correct prefix, extra byte"
        );
        assert!(
            !bearer_matches(
                Some(&format!("Bearer {}", &token[..token.len() - 1])),
                &token
            ),
            "correct prefix, truncated"
        );
    }

    #[test]
    fn session_tokens_are_long_and_distinct() {
        // Guards against a refactor that returns a constant, an empty string,
        // or something short enough to grind against a loopback port that
        // imposes no rate limit.
        let a = mint_session_token();
        let b = mint_session_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 64, "two hyphen-free v4 UUIDs");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn mark_pending_error_records_a_delivery_failure() {
        let mut tasks = vec![Task {
            id: "abc123".into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "t".into(),
            status: "pending".into(),
            result: String::new(),
            ts_ms: 0,
            done_ms: None,
            reviewer: String::new(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            notice_delivered: true,
        }];
        assert!(mark_pending_error(&mut tasks, "abc123", 0));
        assert_eq!(tasks[0].status, "error");
    }

    #[test]
    fn mark_pending_error_does_not_overwrite_a_cancelled_task() {
        // Simulates set_halted landing between add_task and delivery failure:
        // by the time delivery resolves the task is already "cancelled", and
        // that must win over the delivery outcome.
        let mut tasks = vec![Task {
            id: "abc123".into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "t".into(),
            status: "cancelled".into(),
            result: String::new(),
            reviewer: String::new(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            ts_ms: 0,
            done_ms: None,
            notice_delivered: true,
        }];
        assert!(!mark_pending_error(&mut tasks, "abc123", 0));
        assert_eq!(tasks[0].status, "cancelled");
    }

    #[test]
    fn age_marks_a_stale_pending_task_overdue_but_leaves_other_statuses_alone() {
        let base = Task {
            id: "a".into(),
            from: "f".into(),
            target: "t".into(),
            task: "x".into(),
            status: "pending".into(),
            result: String::new(),
            ts_ms: 0,
            done_ms: None,
            reviewer: String::new(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            notice_delivered: true,
        };

        let mut pending = base.clone();
        age(&mut pending, TASK_OVERDUE_MS + 1);
        assert_eq!(pending.status, "overdue");

        let mut errored = Task {
            status: "error".into(),
            ..base
        };
        age(&mut errored, TASK_OVERDUE_MS + 1);
        assert_eq!(errored.status, "error");
    }

    #[test]
    fn an_overdue_task_still_accepts_its_result() {
        // The bug this fixes. Nothing cancels a dispatched agent, so one that
        // ran past the threshold kept working, finished the job, called
        // complete_task, and was refused. The work was done and thrown away
        // because a wall clock had moved.
        let mut tasks = vec![Task {
            id: "abc123".into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "long research task".into(),
            status: "pending".into(),
            result: String::new(),
            ts_ms: 0,
            done_ms: None,
            reviewer: String::new(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            notice_delivered: true,
        }];

        age(&mut tasks[0], TASK_OVERDUE_MS + 1);
        assert_eq!(tasks[0].status, "overdue");

        assert_eq!(
            finish_pending(&mut tasks, "sess-2", "abc123", "here is the report", 0),
            Ok(())
        );
        assert_eq!(tasks[0].status, "done");
        assert_eq!(tasks[0].result, "here is the report");
    }

    #[test]
    fn genuinely_terminal_states_still_refuse_a_result() {
        // The tolerance must not resurrect a cancelled task or silently
        // rewrite one that already reported.
        for status in ["cancelled", "done", "error"] {
            let mut tasks = vec![Task {
                id: "abc123".into(),
                from: "sess-1".into(),
                target: "sess-2".into(),
                task: "t".into(),
                status: status.into(),
                result: "original".into(),
                reviewer: String::new(),
                findings: String::new(),
                exchanges: Vec::new(),
                reviewer_gone: false,
                ts_ms: 0,
                done_ms: None,
                notice_delivered: true,
            }];
            assert_eq!(
                finish_pending(&mut tasks, "sess-2", "abc123", "late overwrite", 0),
                Err(TaskAccessError::NotPending),
                "{status} must not accept a result"
            );
            assert_eq!(tasks[0].result, "original");
        }
    }

    #[test]
    fn an_overdue_task_still_checks_the_caller() {
        // Accepting late results must not weaken authorization.
        let mut tasks = vec![Task {
            id: "abc123".into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "t".into(),
            status: "overdue".into(),
            result: String::new(),
            reviewer: String::new(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            ts_ms: 0,
            done_ms: None,
            notice_delivered: true,
        }];
        assert_eq!(
            finish_pending(&mut tasks, "sess-3", "abc123", "stolen", 0),
            Err(TaskAccessError::Forbidden)
        );
        assert_eq!(tasks[0].status, "overdue");
    }

    #[test]
    fn durable_brain_round_trips_and_applies_latest_task_state() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().to_path_buf();
        let entry = Entry {
            kind: "decision".into(),
            author: "sess-1".into(),
            topic: "storage".into(),
            body: "use jsonl".into(),
            ts_ms: 1,
            room: "main".into(),
        };
        let session = AgentSession {
            name: "sess-1".into(),
            kind: "codex".into(),
            model: String::new(),
        };
        let pending = Task {
            id: "task-1".into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "test persistence".into(),
            status: "pending".into(),
            result: String::new(),
            reviewer: String::new(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            ts_ms: 2,
            done_ms: None,
            notice_delivered: true,
        };
        let done = Task {
            status: "done".into(),
            result: "passed".into(),
            ..pending.clone()
        };

        append_record(&dir, &StoreRecord::Entry(entry.clone())).unwrap();
        append_record(&dir, &StoreRecord::Session(session.clone())).unwrap();
        append_record(&dir, &StoreRecord::Task(pending)).unwrap();
        append_record(&dir, &StoreRecord::Task(done.clone())).unwrap();

        let loaded = load_brain(&dir);
        assert_eq!(loaded.entries, vec![entry]);
        assert_eq!(loaded.sessions, vec![session]);
        assert_eq!(loaded.tasks, vec![done]);
    }

    // -----------------------------------------------------------------
    // The review gate: work is not done because the agent that did it says so
    // -----------------------------------------------------------------

    fn reviewed_task(status: &str) -> Task {
        Task {
            id: "abc123".into(),
            from: "sess-1".into(),
            target: "sess-2".into(),
            task: "audit the parser".into(),
            status: status.into(),
            result: String::new(),
            ts_ms: 0,
            done_ms: None,
            reviewer: "sess-3".into(),
            findings: String::new(),
            exchanges: Vec::new(),
            reviewer_gone: false,
            notice_delivered: true,
        }
    }

    #[test]
    fn submitting_work_with_a_reviewer_does_not_finish_it() {
        // The entire point. Before this, complete_task wrote "done" and the
        // task's life ended there, so "done" meant "self-certified".
        let mut tasks = vec![reviewed_task("pending")];

        assert_eq!(
            finish_pending(&mut tasks, "sess-2", "abc123", "found two bugs", 0),
            Ok(())
        );

        assert_eq!(tasks[0].status, "in_review");
        assert_eq!(tasks[0].result, "found two bugs", "the work itself is kept");
    }

    #[test]
    fn submitting_work_with_no_reviewer_still_finishes_it() {
        // The waiver, and it has to stay cheap. A gate that makes a typo fix
        // cost a round trip gets routed around rather than followed.
        let mut tasks = vec![Task {
            reviewer: String::new(),
            ..reviewed_task("pending")
        }];

        assert_eq!(
            finish_pending(&mut tasks, "sess-2", "abc123", "fixed", 0),
            Ok(())
        );

        assert_eq!(tasks[0].status, "done");
    }

    #[test]
    fn only_the_named_reviewer_may_sign_off() {
        // Including, and especially, the agent that did the work: approving
        // your own submission is the self-certification this replaces, just
        // reached by a different call.
        let mut tasks = vec![reviewed_task("in_review")];

        assert_eq!(
            review_pending(&mut tasks, "sess-2", "abc123", true, "looks fine to me", 0),
            Err(TaskAccessError::Forbidden),
            "the implementer must not be able to approve its own work"
        );
        assert_eq!(
            review_pending(&mut tasks, "sess-4", "abc123", true, "sure", 0),
            Err(TaskAccessError::Forbidden),
            "a bystander must not be able to approve it either"
        );

        assert_eq!(
            tasks[0].status, "in_review",
            "neither attempt changed anything"
        );
    }

    #[test]
    fn approval_finishes_the_task_and_keeps_what_the_reviewer_said() {
        let mut tasks = vec![reviewed_task("in_review")];

        assert_eq!(
            review_pending(
                &mut tasks,
                "sess-3",
                "abc123",
                true,
                "correct, one nit accepted",
                0
            ),
            Ok(())
        );

        assert_eq!(tasks[0].status, "done");
        // An approval with caveats is not an unqualified one, so the caveats
        // survive rather than being thrown away on the happy path.
        assert_eq!(tasks[0].findings, "correct, one nit accepted");
    }

    #[test]
    fn rejection_returns_the_work_and_the_implementer_can_answer_it() {
        // The round trip has to close. A rejection that leaves the task in a
        // state nobody can act on is a dead end wearing the word "review".
        let mut tasks = vec![reviewed_task("in_review")];
        tasks[0].result = "first attempt".into();

        assert_eq!(
            review_pending(
                &mut tasks,
                "sess-3",
                "abc123",
                false,
                "the ordering claim is untested",
                0
            ),
            Ok(())
        );
        assert_eq!(tasks[0].status, "rework");
        assert_eq!(tasks[0].findings, "the ordering claim is untested");
        assert_eq!(
            tasks[0].result, "first attempt",
            "the reviewer judged a specific submission; discarding it loses what was judged"
        );

        // And back round again.
        assert_eq!(
            finish_pending(
                &mut tasks,
                "sess-2",
                "abc123",
                "second attempt, test added",
                0
            ),
            Ok(())
        );
        assert_eq!(
            tasks[0].status, "in_review",
            "a fix goes back to the same reviewer"
        );
        assert_eq!(tasks[0].result, "second attempt, test added");
    }

    #[test]
    fn a_task_can_only_be_reviewed_once_and_only_when_submitted() {
        let mut tasks = vec![reviewed_task("pending")];
        assert_eq!(
            review_pending(&mut tasks, "sess-3", "abc123", true, "", 0),
            Err(TaskAccessError::NotPending),
            "nothing has been submitted yet, so there is nothing to sign off"
        );

        tasks[0].status = "in_review".into();
        assert_eq!(
            review_pending(&mut tasks, "sess-3", "abc123", true, "ok", 0),
            Ok(())
        );
        assert_eq!(
            review_pending(&mut tasks, "sess-3", "abc123", false, "changed my mind", 0),
            Err(TaskAccessError::NotPending),
            "a signed-off task must not be reopened by a second review"
        );
        assert_eq!(tasks[0].status, "done");
    }

    #[test]
    fn omitting_a_reviewer_requests_one_rather_than_skipping_one() {
        // The load-bearing default, and the reason this is a gate rather than
        // a suggestion. An opt-in review is skipped by the conductor who
        // forgets, which is the exact failure being fixed here.
        let live = vec!["sess-1".to_string(), "sess-2".into(), "sess-3".into()];

        assert_eq!(
            choose_reviewer("", "sess-1", "sess-2", &live, &[]),
            Ok("sess-3".to_string()),
            "a third party is preferred over the conductor who wrote the brief"
        );
    }

    #[test]
    fn the_dispatcher_reviews_when_it_is_the_only_one_left() {
        // Two panes and the conductor is one of them. This is the case the
        // feature was asked for: one model checking another model's work.
        let live = vec!["sess-1".to_string(), "sess-2".into()];

        assert_eq!(
            choose_reviewer("", "sess-1", "sess-2", &live, &[]),
            Ok("sess-1".to_string())
        );
    }

    #[test]
    fn a_solo_workspace_waives_rather_than_refusing_the_dispatch() {
        // Nobody else is here. Refusing would make a single-pane workspace
        // unusable, so this waives, and dispatch says so out loud rather than
        // letting the conductor assume a review happened.
        let live = vec!["sess-2".to_string()];

        assert_eq!(
            choose_reviewer("", "sess-2", "sess-2", &live, &[]),
            Ok(String::new())
        );
    }

    #[test]
    fn skipping_review_takes_a_word_not_an_omission() {
        let live = vec!["sess-1".to_string(), "sess-2".into(), "sess-3".into()];

        assert_eq!(
            choose_reviewer(REVIEW_WAIVED, "sess-1", "sess-2", &live, &[]),
            Ok(String::new()),
            "an explicit waiver is honoured"
        );
        assert_ne!(
            choose_reviewer("", "sess-1", "sess-2", &live, &[]),
            Ok(String::new()),
            "but silence is not a waiver"
        );
    }

    #[test]
    fn a_named_reviewer_must_be_someone_other_than_the_worker_and_must_exist() {
        let live = vec!["sess-1".to_string(), "sess-2".into(), "sess-3".into()];

        assert_eq!(
            choose_reviewer("sess-3", "sess-1", "sess-2", &live, &[]),
            Ok("sess-3".to_string())
        );
        // The conductor reviewing is explicitly fine.
        assert_eq!(
            choose_reviewer("sess-1", "sess-1", "sess-2", &live, &[]),
            Ok("sess-1".to_string())
        );

        let self_review = choose_reviewer("sess-2", "sess-1", "sess-2", &live, &[]).unwrap_err();
        assert!(
            self_review.contains("cannot review its own work"),
            "{self_review}"
        );
        // And it names the escape hatch, or the conductor is stuck guessing.
        assert!(self_review.contains(REVIEW_WAIVED), "{self_review}");

        let dead = choose_reviewer("sess-9", "sess-1", "sess-2", &live, &[]).unwrap_err();
        assert!(dead.contains("no live session 'sess-9'"), "{dead}");
    }

    #[test]
    fn an_unnamed_reviewer_prefers_a_different_cli_kind_than_the_target() {
        // sess-3 is the same kind as the target; sess-4 is not. The plain
        // third-party-over-dispatcher tie-break would have picked sess-3,
        // which enforces the review gate without enforcing the reason
        // CONTRIBUTING.md wants it: a different model actually looking.
        let live = vec!["sess-1".to_string(), "sess-3".into(), "sess-4".into()];
        let sessions = vec![
            agent("sess-2", "claude"), // the target
            agent("sess-3", "claude"),
            agent("sess-4", "codex"),
        ];

        assert_eq!(
            choose_reviewer("", "sess-1", "sess-2", &live, &sessions),
            Ok("sess-4".to_string())
        );
    }

    #[test]
    fn an_unnamed_reviewer_falls_back_to_the_same_kind_when_no_other_kind_is_live() {
        let live = vec!["sess-1".to_string(), "sess-3".into()];
        let sessions = vec![agent("sess-2", "claude"), agent("sess-3", "claude")];

        assert_eq!(
            choose_reviewer("", "sess-1", "sess-2", &live, &sessions),
            Ok("sess-3".to_string()),
            "no different-kind candidate exists, so the old rule still applies"
        );
    }

    #[test]
    fn a_named_reviewer_is_unaffected_by_the_kind_preference() {
        // Naming a reviewer is the conductor doing the job the preference is
        // a default for; it must not be second-guessed by kind.
        let live = vec!["sess-1".to_string(), "sess-3".into()];
        let sessions = vec![agent("sess-2", "claude"), agent("sess-3", "claude")];

        assert_eq!(
            choose_reviewer("sess-3", "sess-1", "sess-2", &live, &sessions),
            Ok("sess-3".to_string())
        );
    }

    #[test]
    fn unsigned_work_still_counts_as_open() {
        // Otherwise the listing that answers "what am I waiting on" hides
        // exactly the debt this gate exists to surface.
        assert!(is_open("in_review"));
        assert!(is_open("rework"));
        assert!(!is_open("done"));
    }

    #[test]
    fn a_reader_can_tell_reviewed_work_from_unreviewed_work() {
        // "done" alone cannot be trusted, so it never appears alone.
        let mut approved = reviewed_task("done");
        approved.result = "found two bugs".into();
        approved.findings = "verified both".into();
        let text = render_task(&approved);
        assert!(text.contains("reviewed by sess-3"), "{text}");
        assert!(text.contains("verified both"), "{text}");

        let waived = Task {
            reviewer: String::new(),
            result: "fixed".into(),
            ..reviewed_task("done")
        };
        assert!(
            render_task(&waived).contains("no review was required"),
            "a waived task must not read the same as a reviewed one"
        );

        assert!(render_task(&reviewed_task("in_review")).contains("awaiting review by sess-3"));
        assert!(render_task(&reviewed_task("rework")).contains("sent back by sess-3"));
    }

    #[test]
    fn tasks_written_before_the_gate_existed_still_load() {
        // brain.jsonl is append-only and predates these fields. A schema
        // change that silently drops a workspace's history is not a feature.
        use std::io::Write;

        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().to_path_buf();
        let mut file = std::fs::File::create(dir.join(super::STORE_FILE)).unwrap();
        let legacy = concat!(
            r#"{"type":"task","value":{"id":"old1","from":"sess-1","target":"sess-2","#,
            r#""task":"audit","status":"done","result":"did it","ts_ms":1}}"#,
            "\n"
        );
        file.write_all(legacy.as_bytes()).unwrap();

        let loaded = load_brain(&dir);

        assert_eq!(loaded.tasks.len(), 1, "an old task must not be discarded");
        assert_eq!(loaded.tasks[0].result, "did it");
        assert_eq!(
            loaded.tasks[0].reviewer, "",
            "a legacy task reads as review-waived, not as reviewed by nobody"
        );
    }

    #[test]
    fn durable_brain_ignores_corrupt_and_partial_lines() {
        use std::io::Write;

        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().to_path_buf();
        let entry = Entry {
            kind: "fact".into(),
            author: "sess-1".into(),
            topic: "safe".into(),
            body: "valid records survive".into(),
            ts_ms: 1,
            room: "main".into(),
        };
        append_record(&dir, &StoreRecord::Entry(entry.clone())).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(dir.join(super::STORE_FILE))
            .unwrap();
        file.write_all(b"not json\n{\"type\":\"entry\",\"value\":")
            .unwrap();

        let loaded = load_brain(&dir);
        assert_eq!(loaded.entries, vec![entry]);
        assert!(loaded.sessions.is_empty());
        assert!(loaded.tasks.is_empty());
        assert!(load_brain(&temp.path().join("missing")).entries.is_empty());
    }

    // ---- dead panes and abandoned tasks ----

    #[test]
    fn an_abandoned_task_is_terminal_and_no_longer_open() {
        // The whole point of the state. If it were open, a conductor would keep
        // waiting; if it were not terminal, it would never stamp a finish time.
        assert!(is_terminal(STATUS_ABANDONED));
        assert!(!is_open(STATUS_ABANDONED));
    }

    #[test]
    fn abandoned_is_distinct_from_cancelled_and_overdue() {
        // Three different facts: nobody is doing this, a human stopped it, and
        // it is taking a while. Collapsing any pair loses the question the
        // conductor is actually asking.
        assert!(is_terminal("cancelled"));
        assert!(!is_open("cancelled"));
        assert!(!is_terminal("overdue"));
        assert!(is_open("overdue"));
        assert_ne!(STATUS_ABANDONED, "cancelled");
    }

    #[test]
    fn abandon_lost_settles_an_open_task_whose_pane_is_gone() {
        let mut tasks = vec![task_at("t1", "pending", 0)];
        let changed = abandon_lost(&mut tasks, &[], 5_000);

        assert_eq!(changed.len(), 1);
        assert_eq!(tasks[0].status, STATUS_ABANDONED);
        // Terminal states carry a finish time, so the UI can tell an
        // abandonment that just happened from one already in the store.
        assert_eq!(tasks[0].done_ms, Some(5_000));
    }

    #[test]
    fn abandon_lost_leaves_a_slow_but_live_pane_alone() {
        // The failure this must not cause: an agent that is merely thinking is
        // not an agent that is gone. Slowness is what "overdue" is for, and
        // abandoning here would discard work still being done.
        let mut tasks = vec![task_at("t1", "overdue", 0)];
        let live = vec!["sess-2".to_string()];

        let changed = abandon_lost(&mut tasks, &live, TASK_OVERDUE_MS * 10);

        assert!(changed.is_empty());
        assert_eq!(tasks[0].status, "overdue");
        assert_eq!(tasks[0].done_ms, None);
    }

    #[test]
    fn abandon_lost_covers_every_status_whose_actor_is_the_target() {
        // A pane can die while a rework fix sits open, or while a task is
        // still waiting behind the pane's queue, just as easily as while
        // pending or overdue work is running: in all four the target is the
        // one who has to act, so the target's death is what ends them.
        let mut tasks = vec![
            task_at("t1", "pending", 0),
            task_at("t2", "overdue", 0),
            task_at("t3", "rework", 0),
            task_at("t4", STATUS_QUEUED, 0),
        ];

        let changed = abandon_lost(&mut tasks, &[], 1_000);

        assert_eq!(changed.len(), 4);
        assert!(tasks.iter().all(|t| t.status == STATUS_ABANDONED));
    }

    #[test]
    fn abandon_lost_never_abandons_in_review_even_when_its_target_is_gone() {
        // The submitted work already exists once it is in_review; the
        // reviewer is the actor now, not the target, so the target dying must
        // not erase a result the reviewer has not even looked at yet. This is
        // the bug the reviewer-liveness fix replaced: before it, a target
        // that closed its pane right after a clean submission could discard
        // work nobody had rejected.
        let mut tasks = vec![task_at("t1", "in_review", 0)];
        tasks[0].reviewer = "sess-9".into();
        let live = vec!["sess-9".to_string()]; // reviewer live, target ("sess-2") gone

        let changed = abandon_lost(&mut tasks, &live, 1_000);

        assert!(changed.is_empty());
        assert_eq!(tasks[0].status, "in_review");
    }

    #[test]
    fn abandon_lost_flags_a_dead_reviewer_instead_of_abandoning_the_task() {
        // The other half of the same fix, from the other direction: a
        // reviewer that no longer exists is the failure `abandon_lost` used
        // to miss entirely, because it only ever checked the target. The
        // work still exists, so this is a flag for the conductor to act on
        // (reassign_task), not an abandonment.
        let mut tasks = vec![task_at("t1", "in_review", 0)];
        tasks[0].reviewer = "sess-9".into();
        let live = vec!["sess-2".to_string()]; // target live, reviewer gone

        let changed = abandon_lost(&mut tasks, &live, 1_000);

        assert_eq!(changed.len(), 1);
        assert_eq!(
            tasks[0].status, "in_review",
            "not abandoned: the submitted work still exists"
        );
        assert!(tasks[0].reviewer_gone);
    }

    #[test]
    fn abandon_lost_clears_reviewer_gone_once_the_reviewer_is_live_again() {
        // A stale flag is its own kind of dishonest roster, the same failure
        // this whole mechanism exists to fix on the target side.
        let mut tasks = vec![task_at("t1", "in_review", 0)];
        tasks[0].reviewer = "sess-9".into();
        tasks[0].reviewer_gone = true;
        let live = vec!["sess-2".to_string(), "sess-9".to_string()];

        let changed = abandon_lost(&mut tasks, &live, 1_000);

        assert_eq!(changed.len(), 1);
        assert!(!tasks[0].reviewer_gone);
    }

    #[test]
    fn abandon_lost_flags_reviewer_gone_on_rework_without_abandoning_the_target() {
        // rework still keeps its target-liveness check (the implementer is
        // the actor), but a reviewer it will eventually resubmit to can be
        // gone too, and that is worth surfacing now rather than only after
        // the fix is resubmitted into a dead end.
        let mut tasks = vec![task_at("t1", "rework", 0)];
        tasks[0].reviewer = "sess-9".into();
        let live = vec!["sess-2".to_string()]; // target live, reviewer gone

        let changed = abandon_lost(&mut tasks, &live, 1_000);

        assert_eq!(changed.len(), 1);
        assert_eq!(tasks[0].status, "rework");
        assert!(tasks[0].reviewer_gone);
    }

    #[test]
    fn abandon_lost_appends_once_for_a_rework_task_whose_reviewer_and_target_are_both_gone() {
        // Finding 6: a rework task first has reviewer_gone flipped by the
        // in_review/rework check, then falls through to the target check and
        // gets abandoned in the same sweep. That must land as one record with
        // the final state, not one for the flip and a second for the
        // abandonment, and the flag has no meaning once the task is terminal.
        let mut tasks = vec![task_at("t1", "rework", 0)];
        tasks[0].reviewer = "sess-9".into();
        let live: Vec<String> = vec![]; // both target and reviewer gone

        let changed = abandon_lost(&mut tasks, &live, 1_000);

        assert_eq!(changed.len(), 1, "one record for one sweep, not two");
        assert_eq!(tasks[0].status, STATUS_ABANDONED);
        assert!(
            !tasks[0].reviewer_gone,
            "a terminal task is not waiting on a reviewer any more"
        );
    }

    #[test]
    fn abandon_lost_never_rewrites_work_that_already_finished() {
        // A pane closing after its work landed must not erase the result. This
        // is the regression that would quietly destroy history.
        let mut tasks = vec![
            task_at("t1", "done", 0),
            task_at("t2", "cancelled", 0),
            task_at("t3", "error", 0),
        ];
        tasks[0].result = "the parser is fine".into();

        let changed = abandon_lost(&mut tasks, &[], 1_000);

        assert!(changed.is_empty());
        assert_eq!(tasks[0].status, "done");
        assert_eq!(tasks[0].result, "the parser is fine");
        assert_eq!(tasks[1].status, "cancelled");
        assert_eq!(tasks[2].status, "error");
    }

    #[test]
    fn abandon_lost_touches_only_the_dead_panes_tasks() {
        let mut tasks = vec![task_at("t1", "pending", 0), task_at("t2", "pending", 0)];
        tasks[1].target = "sess-3".into();
        let live = vec!["sess-3".to_string()];

        let changed = abandon_lost(&mut tasks, &live, 1_000);

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id, "t1");
        assert_eq!(tasks[0].status, STATUS_ABANDONED);
        assert_eq!(tasks[1].status, "pending");
    }

    #[test]
    fn an_abandoned_task_says_why_rather_than_leaving_an_empty_result() {
        // An empty result reads like a result of no content. The conductor needs
        // to know nobody is coming, and that re-dispatching is its move.
        let mut tasks = vec![task_at("t1", "pending", 0)];

        abandon_lost(&mut tasks, &[], 1_000);

        assert!(tasks[0].result.contains("sess-2"));
        assert!(tasks[0].result.contains("no longer running"));
        assert!(tasks[0].result.contains("Re-dispatch"));
    }

    #[test]
    fn abandoning_keeps_a_result_the_agent_already_reported() {
        // Work sent back for rework, then the implementer's pane died. The
        // reviewer's findings and the agent's own report are the valuable
        // part, and overwriting them with the abandonment notice would throw
        // away the only record of what happened.
        let mut tasks = vec![task_at("t1", "rework", 0)];
        tasks[0].result = "found three bugs in the lexer".into();

        abandon_lost(&mut tasks, &[], 1_000);

        assert_eq!(tasks[0].status, STATUS_ABANDONED);
        assert_eq!(tasks[0].result, "found three bugs in the lexer");
    }

    #[test]
    fn an_abandoned_task_refuses_a_late_result() {
        // The pane is gone and its MCP endpoint went with it, so nothing should
        // be able to report here. If something does, it is not the target.
        let mut tasks = vec![task_at("t1", "pending", 0)];
        abandon_lost(&mut tasks, &[], 1_000);

        let outcome = finish_pending(&mut tasks, "sess-2", "t1", "done at last", 2_000);

        assert_eq!(outcome, Err(TaskAccessError::NotPending));
        assert_eq!(tasks[0].status, STATUS_ABANDONED);
    }

    #[test]
    fn reconcile_abandoned_settles_a_dead_panes_task_and_persists_it() {
        // End to end over a real `Shared`. Its engine holds no sessions, so
        // every target is gone by definition, which is exactly the dead-pane
        // case. Proves the transition reaches the store, not just memory.
        let (shared, dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", "pending", 0));

        let live = shared.reconcile_abandoned();

        assert!(live.is_empty());
        assert_eq!(
            shared.tasks.lock().unwrap()[0].status,
            STATUS_ABANDONED,
            "the in-memory task should be settled"
        );

        let reloaded = load_brain(dir.path());
        assert_eq!(reloaded.tasks.len(), 1, "the change must survive a restart");
        assert_eq!(reloaded.tasks[0].status, STATUS_ABANDONED);
    }

    #[test]
    fn reconcile_abandoned_is_idempotent() {
        // It runs on every roster read, dispatch and collection, so running it
        // twice must not append a second record or move the finish time.
        let (shared, dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", "pending", 0));

        shared.reconcile_abandoned();
        let first_done_ms = shared.tasks.lock().unwrap()[0].done_ms;
        shared.reconcile_abandoned();

        assert_eq!(shared.tasks.lock().unwrap()[0].done_ms, first_done_ms);
        assert_eq!(
            load_brain(dir.path()).tasks.len(),
            1,
            "a settled task should not be written again"
        );
    }

    #[test]
    fn collecting_results_reports_a_dead_panes_task_as_abandoned() {
        // The call a conductor makes while waiting is the one that has to stop
        // it waiting. Before this, the answer here was "pending" forever.
        let (shared, _dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", "pending", 0));

        let collected = shared.tasks_from("sess-1");

        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].status, STATUS_ABANDONED);
    }

    // ---- waiting for a dispatch ----

    fn task_with(id: &str, status: &str) -> Task {
        let mut t = task_at(id, status, 0);
        t.id = id.into();
        t
    }

    #[test]
    fn an_omitted_timeout_waits_rather_than_returning_at_once() {
        // Zero means "did not say". Treating it as zero seconds would turn the
        // one call that blocks back into the poll it exists to replace.
        assert_eq!(wait_timeout(0).as_secs(), WAIT_DEFAULT_SECS);
    }

    #[test]
    fn a_wait_cannot_be_asked_to_last_forever() {
        // The backstop for a live pane that has quietly stopped. Dead panes end
        // a wait properly; a pane that is up but silent is not detectable, so
        // the ceiling is the only thing that returns control to the conductor.
        assert_eq!(wait_timeout(u64::MAX).as_secs(), WAIT_MAX_SECS);
        assert_eq!(wait_timeout(30).as_secs(), 30);
    }

    #[test]
    fn task_ids_arrive_however_a_conductor_happens_to_separate_them() {
        let expected = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(parse_task_ids("a,b,c"), expected);
        assert_eq!(parse_task_ids("a b c"), expected);
        assert_eq!(parse_task_ids("a, b\tc\n"), expected);
        assert!(parse_task_ids("   ").is_empty());
    }

    #[test]
    fn an_overdue_task_is_still_waited_for_rather_than_given_up_on() {
        // Overdue is slow, not finished. Returning here would report work as
        // complete while the agent is still doing it.
        let tasks = vec![task_with("t1", "overdue")];
        assert_eq!(still_open(&tasks, &["t1".to_string()]), vec!["t1"]);
    }

    #[test]
    fn work_submitted_but_unsigned_is_still_worth_waiting_for() {
        // in_review and rework are open on purpose: the gate has not passed, so
        // handing the result back now would defeat the review it is waiting on.
        let tasks = vec![task_with("t1", "in_review"), task_with("t2", "rework")];
        let wanted = vec!["t1".to_string(), "t2".to_string()];
        assert_eq!(still_open(&tasks, &wanted), wanted);
    }

    #[test]
    fn a_wait_ends_when_every_task_reaches_a_terminal_state() {
        let tasks = vec![
            task_with("t1", "done"),
            task_with("t2", "cancelled"),
            task_with("t3", "error"),
        ];
        let wanted = vec!["t1".to_string(), "t2".to_string(), "t3".to_string()];
        assert!(still_open(&tasks, &wanted).is_empty());
    }

    #[test]
    fn a_dead_panes_task_releases_the_wait_instead_of_holding_it_to_the_timeout() {
        // The reason dq3s0j was blocked on e18r66. Without a terminal state for
        // a dead pane, this wait would run the full ceiling and then report the
        // task as still running, forever, on every retry.
        let tasks = vec![task_with("t1", STATUS_ABANDONED)];
        assert!(still_open(&tasks, &["t1".to_string()]).is_empty());
    }

    #[test]
    fn a_task_that_vanished_does_not_hold_the_wait_open_forever() {
        // An id matching nothing cannot ever complete. Treating it as open is
        // the exact hang this call exists to prevent.
        let tasks: Vec<Task> = Vec::new();
        assert!(still_open(&tasks, &["ghost".to_string()]).is_empty());
    }

    #[test]
    fn waiting_only_reports_the_tasks_that_were_asked_for() {
        // A conductor waiting on one slice must not be held by an unrelated
        // long-running task it dispatched earlier.
        let tasks = vec![task_with("t1", "pending"), task_with("t2", "pending")];
        assert_eq!(still_open(&tasks, &["t2".to_string()]), vec!["t2"]);
    }

    // ---- a blocked agent asking the conductor ----

    #[test]
    fn a_blocked_task_is_open_but_not_finished() {
        // The agent is alive and the work is unfinished, so a conductor
        // collecting results must still see it. If it were terminal, the
        // question would never be answered and the task would silently die.
        assert!(is_open(STATUS_BLOCKED));
        assert!(!is_terminal(STATUS_BLOCKED));
    }

    #[test]
    fn asking_blocks_the_task_and_records_the_question() {
        let mut tasks = vec![task_at("t1", "pending", 0)];

        let outcome = ask_pending(&mut tasks, "sess-2", "t1", "which parser?", 500);

        assert_eq!(outcome, Ok(()));
        assert_eq!(tasks[0].status, STATUS_BLOCKED);
        assert_eq!(tasks[0].exchanges.len(), 1);
        assert_eq!(tasks[0].exchanges[0].question, "which parser?");
        assert_eq!(tasks[0].exchanges[0].asked_ms, 500);
        assert!(tasks[0].exchanges[0].answer.is_empty());
    }

    #[test]
    fn only_the_agent_doing_the_work_may_ask_about_it() {
        // Otherwise any pane could spend the conductor's attention on a task it
        // has nothing to do with.
        let mut tasks = vec![task_at("t1", "pending", 0)];

        let outcome = ask_pending(&mut tasks, "sess-9", "t1", "which parser?", 0);

        assert_eq!(outcome, Err(AskError::Access(TaskAccessError::Forbidden)));
        assert_eq!(tasks[0].status, "pending");
    }

    #[test]
    fn a_finished_task_cannot_be_reopened_with_a_question() {
        let mut tasks = vec![task_at("t1", "done", 0)];

        let outcome = ask_pending(&mut tasks, "sess-2", "t1", "one more thing?", 0);

        assert_eq!(outcome, Err(AskError::Access(TaskAccessError::NotPending)));
        assert_eq!(tasks[0].status, "done");
    }

    #[test]
    fn a_second_question_while_the_first_is_open_is_refused() {
        // Asking blocks, so a second question means the agent did not wait, and
        // one answer could not be matched to one question.
        let mut tasks = vec![task_at("t1", "pending", 0)];
        ask_pending(&mut tasks, "sess-2", "t1", "first", 0).unwrap();

        let outcome = ask_pending(&mut tasks, "sess-2", "t1", "second", 0);

        assert_eq!(outcome, Err(AskError::AlreadyAsking));
        assert_eq!(tasks[0].exchanges.len(), 1);
    }

    #[test]
    fn a_pane_cannot_interrogate_the_conductor_in_a_loop() {
        // The MAX_DISPATCHES precedent: a hard ceiling, because the conductor's
        // context is the scarce resource in a long session.
        let mut tasks = vec![task_at("t1", "pending", 0)];
        for i in 0..MAX_QUESTIONS_PER_TASK {
            ask_pending(&mut tasks, "sess-2", "t1", &format!("q{i}"), 0).unwrap();
            answer_pending(&mut tasks, "sess-1", Some("sess-1"), "t1", &format!("a{i}")).unwrap();
        }

        let outcome = ask_pending(&mut tasks, "sess-2", "t1", "one more", 0);

        assert_eq!(outcome, Err(AskError::TooMany));
    }

    #[test]
    fn answering_unblocks_the_task_and_keeps_the_exchange() {
        // A question answered and then lost is one the next agent asks again.
        let mut tasks = vec![task_at("t1", "pending", 0)];
        ask_pending(&mut tasks, "sess-2", "t1", "which parser?", 0).unwrap();

        let question = answer_pending(&mut tasks, "sess-1", Some("sess-1"), "t1", "the new one");

        assert_eq!(question, Ok("which parser?".to_string()));
        assert_eq!(tasks[0].status, "pending");
        assert_eq!(tasks[0].exchanges[0].answer, "the new one");
    }

    #[test]
    fn only_the_conductor_answers() {
        // The whole point is that questions stop going to whoever is nearest.
        let mut tasks = vec![task_at("t1", "pending", 0)];
        ask_pending(&mut tasks, "sess-2", "t1", "which parser?", 0).unwrap();

        let outcome = answer_pending(&mut tasks, "sess-9", Some("sess-1"), "t1", "mine");

        assert_eq!(outcome, Err(AskError::Access(TaskAccessError::Forbidden)));
        assert_eq!(tasks[0].status, STATUS_BLOCKED);
    }

    #[test]
    fn answering_a_task_that_asked_nothing_is_refused() {
        let mut tasks = vec![task_at("t1", "pending", 0)];

        let outcome = answer_pending(&mut tasks, "sess-1", Some("sess-1"), "t1", "unprompted");

        assert_eq!(outcome, Err(AskError::NotAsking));
    }

    #[test]
    fn a_blocked_agent_can_still_report_what_it_managed_to_do() {
        // Its question may have timed out, or it may have worked the answer out
        // for itself. Refusing here would discard finished work over an
        // unanswered question, which is the overdue bug in another costume.
        let mut tasks = vec![task_at("t1", "pending", 0)];
        ask_pending(&mut tasks, "sess-2", "t1", "which parser?", 0).unwrap();

        let outcome = finish_pending(&mut tasks, "sess-2", "t1", "assumed the new one", 9_000);

        assert_eq!(outcome, Ok(()));
        assert_eq!(tasks[0].result, "assumed the new one");
    }

    #[test]
    fn a_dead_pane_abandons_its_blocked_task_too() {
        // Otherwise a pane that died while waiting on an answer would leave the
        // conductor holding a question nobody is left to receive the answer to.
        let mut tasks = vec![task_at("t1", "pending", 0)];
        ask_pending(&mut tasks, "sess-2", "t1", "which parser?", 0).unwrap();

        let changed = abandon_lost(&mut tasks, &[], 5_000);

        assert_eq!(changed.len(), 1);
        assert_eq!(tasks[0].status, STATUS_ABANDONED);
    }

    #[test]
    fn open_question_finds_the_unanswered_one_and_nothing_else() {
        let mut tasks = vec![task_at("t1", "pending", 0)];
        ask_pending(&mut tasks, "sess-2", "t1", "first", 0).unwrap();
        answer_pending(&mut tasks, "sess-1", Some("sess-1"), "t1", "answered").unwrap();
        assert!(open_question(&tasks[0]).is_none());

        ask_pending(&mut tasks, "sess-2", "t1", "second", 0).unwrap();

        assert_eq!(
            open_question(&tasks[0]).map(|e| e.question.as_str()),
            Some("second")
        );
    }

    #[test]
    fn blocked_wait_should_release_holds_while_still_blocked() {
        assert!(!blocked_wait_should_release(STATUS_BLOCKED));
    }

    #[test]
    fn blocked_wait_should_release_fires_once_cancelled() {
        // Finding 3, second round: cancel_task accepts a blocked task, but
        // without this the agent's ask_conductor call would sit until the
        // 900s ceiling asking a question nobody can ever answer.
        assert!(blocked_wait_should_release("cancelled"));
    }

    #[test]
    fn blocked_wait_should_release_fires_for_any_other_status_too() {
        for status in ["pending", "overdue", "done", "error", STATUS_ABANDONED] {
            assert!(blocked_wait_should_release(status), "{status}");
        }
    }

    #[test]
    fn a_listing_puts_the_question_where_the_conductor_will_see_it() {
        // Behind another call is behind a call the conductor will not make.
        let mut tasks = vec![task_at("t1", "pending", 0)];
        ask_pending(&mut tasks, "sess-2", "t1", "which parser?", 0).unwrap();

        let rendered = render_task(&tasks[0]);

        assert!(rendered.contains("which parser?"), "{rendered}");
        assert!(rendered.contains("answer_question"), "{rendered}");
    }

    #[test]
    fn a_blocked_task_keeps_a_conductor_waiting_rather_than_finishing_it() {
        // still_open must not treat blocked as done. wait_for_tasks returns
        // early on it by a separate path, because the conductor has to act.
        let tasks = vec![task_with("t1", STATUS_BLOCKED)];
        assert_eq!(still_open(&tasks, &["t1".to_string()]), vec!["t1"]);
    }

    // ---- attribute_open_tasks: busy vs reviewing ----

    #[test]
    fn attribute_open_tasks_counts_in_review_against_the_reviewer() {
        let mut reviewed = task_at("t1", "in_review", 1_000);
        reviewed.reviewer = "sess-9".into();
        let tasks = vec![reviewed];

        let (busy, reviewing) = attribute_open_tasks(&tasks, 5_000);

        assert!(
            busy.is_empty(),
            "the target already submitted; it is not busy"
        );
        assert_eq!(reviewing, vec![("sess-9".to_string(), 4_000)]);
    }

    #[test]
    fn attribute_open_tasks_counts_everything_else_against_the_target() {
        let tasks = vec![
            task_at("t1", "pending", 1_000),
            task_at("t2", "rework", 1_000),
        ];

        let (busy, reviewing) = attribute_open_tasks(&tasks, 5_000);

        assert_eq!(busy.len(), 2);
        assert!(busy.iter().all(|(who, _)| who == "sess-2"));
        assert!(reviewing.is_empty());
    }

    #[test]
    fn reviewing_label_reads_differently_from_busy_label() {
        assert_eq!(reviewing_label(None), "");
        assert_eq!(reviewing_label(Some(90_000)), " [reviewing 1m]");
        // Distinct wording is the point: a conductor scanning the roster has
        // to be able to tell a pane holding its own work from one reviewing
        // someone else's without reading closely.
        assert_ne!(reviewing_label(Some(90_000)), busy_label(Some(90_000)));
    }

    // ---- queue_predecessor: dispatch's "queued behind" note ----

    #[test]
    fn queue_predecessor_finds_an_open_task_for_that_target() {
        let tasks = vec![task_at("t1", "pending", 0)];
        assert_eq!(queue_predecessor(&tasks, "sess-2"), Some("t1".to_string()));
    }

    #[test]
    fn queue_predecessor_ignores_finished_work_and_other_targets() {
        let mut done = task_at("t1", "done", 0);
        done.target = "sess-2".into();
        let mut elsewhere = task_at("t2", "pending", 0);
        elsewhere.target = "sess-3".into();
        let tasks = vec![done, elsewhere];

        assert_eq!(queue_predecessor(&tasks, "sess-2"), None);
    }

    #[test]
    fn queue_predecessor_prefers_the_last_queued_task_over_the_occupying_one() {
        // A fresh dispatch to an already-occupied pane with two already
        // queued waits behind the most recent of those, not behind the
        // task actually running: that is what "position in line" means.
        let mut occupying = task_at("t1", "pending", 0);
        occupying.target = "sess-2".into();
        let mut q1 = task_at("t2", "queued", 10);
        q1.target = "sess-2".into();
        let mut q2 = task_at("t3", "queued", 20);
        q2.target = "sess-2".into();
        let tasks = vec![occupying, q1, q2];

        assert_eq!(queue_predecessor(&tasks, "sess-2"), Some("t3".to_string()));
    }

    #[test]
    fn queue_predecessor_finds_a_reviewer_occupied_pane_with_no_task_of_its_own() {
        // A pane can be occupied purely as a reviewer, with no task
        // targeting it directly.
        let mut reviewing = task_at("t1", "in_review", 0);
        reviewing.target = "sess-3".into();
        reviewing.reviewer = "sess-2".into();
        let tasks = vec![reviewing];

        assert_eq!(queue_predecessor(&tasks, "sess-2"), Some("t1".to_string()));
    }

    // ---- reviewer_gone rendering ----

    #[test]
    fn status_tag_is_unchanged_when_the_reviewer_is_not_gone() {
        assert_eq!(status_tag(&task_at("t1", "pending", 0)), "pending");
    }

    #[test]
    fn render_task_flags_a_task_with_a_gone_reviewer() {
        let mut t = reviewed_task("in_review");
        t.reviewer_gone = true;

        let rendered = render_task(&t);

        assert!(
            rendered.starts_with("[in_review, reviewer gone]"),
            "{rendered}"
        );
        assert!(rendered.contains("reassign_task"), "{rendered}");
    }

    #[test]
    fn render_task_summary_flags_a_task_with_a_gone_reviewer() {
        let mut t = reviewed_task("rework");
        t.reviewer_gone = true;

        let rendered = render_task_summary(&t);

        assert!(
            rendered.starts_with("[rework, reviewer gone]"),
            "{rendered}"
        );
    }

    #[test]
    fn render_open_task_summaries_flags_a_gone_reviewer_in_wait_for_tasks_timeout() {
        // Finding 5: the timeout path used to print bare ids, so a task
        // whose reviewer died gave no hint it needed reassign_task rather
        // than another wait.
        let mut stuck = reviewed_task("in_review");
        stuck.id = "t1".into();
        stuck.reviewer_gone = true;
        let other = task_at("t2", "pending", 0);
        let mine = vec![stuck, other];

        let summaries = render_open_task_summaries(&mine, &["t1".to_string()]);

        assert_eq!(summaries.len(), 1);
        assert!(
            summaries[0].starts_with("[in_review, reviewer gone]"),
            "{}",
            summaries[0]
        );
    }

    #[test]
    fn render_open_task_summaries_keeps_open_s_order_and_skips_the_unknown() {
        let mine = vec![task_at("t1", "pending", 0), task_at("t2", "overdue", 0)];

        let summaries = render_open_task_summaries(&mine, &["t2".to_string(), "t1".to_string()]);

        assert_eq!(summaries.len(), 2);
        assert!(summaries[0].starts_with("[overdue]"), "{}", summaries[0]);
        assert!(summaries[1].starts_with("[pending]"), "{}", summaries[1]);
    }

    // ---- cancel_pending ----

    #[test]
    fn cancel_pending_cancels_an_open_task_with_a_reason() {
        let mut tasks = vec![task_at("t1", "pending", 0)];

        let outcome = cancel_pending(&mut tasks, "t1", "superseded by a later brief", 9_000);

        assert_eq!(outcome, CancelOutcome::Cancelled);
        assert_eq!(tasks[0].status, "cancelled");
        assert_eq!(tasks[0].done_ms, Some(9_000));
        assert!(tasks[0].result.contains("superseded by a later brief"));
    }

    #[test]
    fn cancel_pending_covers_every_open_status() {
        for status in ["pending", "overdue", "in_review", "rework", STATUS_BLOCKED] {
            let mut tasks = vec![task_at("t1", status, 0)];
            assert_eq!(
                cancel_pending(&mut tasks, "t1", "", 0),
                CancelOutcome::Cancelled,
                "{status} should be cancellable"
            );
        }
    }

    #[test]
    fn cancel_pending_reports_a_terminal_task_as_already_terminal_not_an_error() {
        let mut tasks = vec![task_at("t1", "done", 0)];

        let outcome = cancel_pending(&mut tasks, "t1", "", 0);

        assert_eq!(outcome, CancelOutcome::AlreadyTerminal);
        assert_eq!(
            tasks[0].status, "done",
            "a terminal task must not be rewritten"
        );
    }

    #[test]
    fn cancel_pending_reports_an_unknown_id() {
        let mut tasks = vec![task_at("t1", "pending", 0)];
        assert_eq!(
            cancel_pending(&mut tasks, "nope", "", 0),
            CancelOutcome::NotFound
        );
    }

    #[test]
    fn cancel_pending_keeps_a_result_already_submitted() {
        let mut tasks = vec![task_at("t1", "in_review", 0)];
        tasks[0].result = "found two bugs".into();

        cancel_pending(&mut tasks, "t1", "not needed after all", 0);

        assert!(tasks[0].result.contains("found two bugs"));
        assert!(tasks[0].result.contains("not needed after all"));
    }

    #[test]
    fn cancel_pending_without_a_reason_still_says_who_cancelled_it() {
        let mut tasks = vec![task_at("t1", "pending", 0)];
        cancel_pending(&mut tasks, "t1", "", 0);
        assert!(tasks[0].result.contains("Cancelled by the conductor"));
    }

    #[test]
    fn cancel_pending_clears_a_gone_reviewer_flag() {
        // Finding 6: cancelling a flagged task left "reviewer gone" showing on
        // a task that is now terminal and waiting on nobody.
        let mut tasks = vec![task_at("t1", "in_review", 0)];
        tasks[0].reviewer_gone = true;

        cancel_pending(&mut tasks, "t1", "no longer needed", 0);

        assert_eq!(tasks[0].status, "cancelled");
        assert!(!tasks[0].reviewer_gone);
    }

    // ---- reassign_pending ----

    #[test]
    fn reassign_pending_retargets_a_pending_task_and_resets_its_clock() {
        let mut tasks = vec![task_at("t1", "pending", 1_000)];
        let live = vec!["sess-5".to_string()];

        let task = reassign_pending(
            &mut tasks,
            "t1",
            "sess-1",
            Some("sess-5"),
            None,
            &live,
            9_000,
        )
        .unwrap();

        assert_eq!(task.target, "sess-5");
        assert_eq!(task.status, "pending");
        assert_eq!(task.ts_ms, 9_000);
        assert!(task.result.contains("sess-2"));
        assert!(task.result.contains("sess-5"));
    }

    #[test]
    fn reassign_pending_retargets_an_overdue_task_too() {
        let mut tasks = vec![task_at("t1", "overdue", 1_000)];
        let live = vec!["sess-5".to_string()];

        let task = reassign_pending(
            &mut tasks,
            "t1",
            "sess-1",
            Some("sess-5"),
            None,
            &live,
            9_000,
        )
        .unwrap();

        assert_eq!(
            task.status, "pending",
            "a fresh delivery is pending again, not still overdue"
        );
    }

    #[test]
    fn reassign_pending_retargets_an_abandoned_task_reviving_it_as_pending() {
        // The case reassign_task was built for: a target went away, the
        // sweep flipped the task to abandoned, and the conductor wants it
        // redelivered to someone live instead of typing the brief again.
        let mut tasks = vec![task_at("t1", STATUS_ABANDONED, 1_000)];
        tasks[0].done_ms = Some(2_000);
        tasks[0].exchanges = vec![Exchange {
            question: "earlier note".to_string(),
            answer: String::new(),
            asked_ms: 500,
        }];
        let live = vec!["sess-5".to_string()];

        let task = reassign_pending(
            &mut tasks,
            "t1",
            "sess-1",
            Some("sess-5"),
            None,
            &live,
            9_000,
        )
        .unwrap();

        assert_eq!(task.target, "sess-5");
        assert_eq!(task.status, "pending");
        assert_eq!(
            task.done_ms, None,
            "a revived task is open again, so it must not still carry a finish stamp"
        );
        assert_eq!(
            task.exchanges.len(),
            1,
            "retargeting redelivers the brief, it does not erase history"
        );
        assert_eq!(task.exchanges[0].question, "earlier note");
    }

    #[test]
    fn reassign_pending_sets_from_to_the_caller_and_notes_the_old_dispatcher() {
        let mut tasks = vec![task_at("t1", "pending", 1_000)];
        let live = vec!["sess-5".to_string()];

        let task = reassign_pending(
            &mut tasks,
            "t1",
            "sess-9",
            Some("sess-5"),
            None,
            &live,
            9_000,
        )
        .unwrap();

        assert_eq!(
            task.from, "sess-9",
            "the current conductor must own the task or wait_for_tasks refuses it"
        );
        assert!(task.result.contains("sess-1"));
        assert!(task.result.contains("sess-9"));
    }

    #[test]
    fn reassign_pending_leaves_from_alone_when_the_caller_already_owns_it() {
        let mut tasks = vec![task_at("t1", "pending", 1_000)];
        let live = vec!["sess-5".to_string()];

        let task = reassign_pending(
            &mut tasks,
            "t1",
            "sess-1",
            Some("sess-5"),
            None,
            &live,
            9_000,
        )
        .unwrap();

        assert_eq!(task.from, "sess-1");
        assert!(
            !task.result.contains("dispatcher"),
            "no dispatcher change happened, so nothing should say one did"
        );
    }

    #[test]
    fn reassign_pending_refuses_a_target_that_is_not_live() {
        let mut tasks = vec![task_at("t1", "pending", 0)];

        let err =
            reassign_pending(&mut tasks, "t1", "sess-1", Some("sess-5"), None, &[], 0).unwrap_err();

        assert_eq!(err, ReassignError::NotLive("sess-5".to_string()));
        assert_eq!(
            tasks[0].target, "sess-2",
            "a refused reassignment must not mutate the task"
        );
    }

    #[test]
    fn reassign_pending_refuses_the_same_target() {
        let mut tasks = vec![task_at("t1", "pending", 0)];
        let live = vec!["sess-2".to_string()];
        assert_eq!(
            reassign_pending(&mut tasks, "t1", "sess-1", Some("sess-2"), None, &live, 0)
                .unwrap_err(),
            ReassignError::SameTarget
        );
    }

    #[test]
    fn reassign_pending_refuses_to_retarget_onto_the_reviewer() {
        // Finding 1, second round: without this, a pending task can be
        // retargeted to its own reviewer, who then submits and calls
        // review_task on its own work. Mirrors ReviewerIsTarget from the
        // other direction.
        let mut tasks = vec![task_at("t1", "pending", 0)];
        tasks[0].reviewer = "sess-5".into();
        let live = vec!["sess-5".to_string()];
        assert_eq!(
            reassign_pending(&mut tasks, "t1", "sess-1", Some("sess-5"), None, &live, 0)
                .unwrap_err(),
            ReassignError::TargetIsReviewer
        );
        assert_eq!(
            tasks[0].target, "sess-2",
            "a refused reassignment must not mutate the task"
        );
    }

    #[test]
    fn reassign_pending_refuses_to_retarget_work_already_submitted() {
        let mut tasks = vec![task_at("t1", "in_review", 0)];
        let live = vec!["sess-5".to_string()];
        assert_eq!(
            reassign_pending(&mut tasks, "t1", "sess-1", Some("sess-5"), None, &live, 0)
                .unwrap_err(),
            ReassignError::NotOpenForRetarget
        );
    }

    #[test]
    fn reassign_pending_hands_in_review_work_to_a_new_reviewer() {
        let mut tasks = vec![task_at("t1", "in_review", 0)];
        tasks[0].reviewer = "sess-9".into();
        tasks[0].reviewer_gone = true;
        let live = vec!["sess-5".to_string()];

        let task =
            reassign_pending(&mut tasks, "t1", "sess-1", None, Some("sess-5"), &live, 0).unwrap();

        assert_eq!(task.reviewer, "sess-5");
        assert_eq!(
            task.status, "in_review",
            "reassigning the reviewer does not change the status"
        );
        assert!(
            !task.reviewer_gone,
            "the newly named reviewer was just checked live"
        );
        assert!(task.result.contains("sess-9"));
        assert!(task.result.contains("sess-5"));
    }

    #[test]
    fn reassign_pending_hands_rework_to_a_new_reviewer_too() {
        let mut tasks = vec![task_at("t1", "rework", 0)];
        tasks[0].reviewer = "sess-9".into();
        let live = vec!["sess-5".to_string()];

        let task =
            reassign_pending(&mut tasks, "t1", "sess-1", None, Some("sess-5"), &live, 0).unwrap();

        assert_eq!(task.reviewer, "sess-5");
        assert_eq!(task.status, "rework");
    }

    #[test]
    fn reassign_pending_refuses_a_reviewer_that_is_not_live() {
        let mut tasks = vec![task_at("t1", "in_review", 0)];
        tasks[0].reviewer = "sess-9".into();
        assert_eq!(
            reassign_pending(&mut tasks, "t1", "sess-1", None, Some("sess-5"), &[], 0).unwrap_err(),
            ReassignError::NotLive("sess-5".to_string())
        );
    }

    #[test]
    fn reassign_pending_refuses_the_same_reviewer() {
        let mut tasks = vec![task_at("t1", "in_review", 0)];
        tasks[0].reviewer = "sess-9".into();
        let live = vec!["sess-9".to_string()];
        assert_eq!(
            reassign_pending(&mut tasks, "t1", "sess-1", None, Some("sess-9"), &live, 0)
                .unwrap_err(),
            ReassignError::SameReviewer
        );
    }

    #[test]
    fn reassign_pending_refuses_the_target_as_its_own_reviewer() {
        let mut tasks = vec![task_at("t1", "in_review", 0)];
        tasks[0].reviewer = "sess-9".into();
        let live = vec!["sess-2".to_string()]; // sess-2 is the task's target
        assert_eq!(
            reassign_pending(&mut tasks, "t1", "sess-1", None, Some("sess-2"), &live, 0)
                .unwrap_err(),
            ReassignError::ReviewerIsTarget
        );
    }

    #[test]
    fn reassign_pending_refuses_to_reassign_a_reviewer_on_pending_work() {
        let mut tasks = vec![task_at("t1", "pending", 0)];
        let live = vec!["sess-5".to_string()];
        assert_eq!(
            reassign_pending(&mut tasks, "t1", "sess-1", None, Some("sess-5"), &live, 0)
                .unwrap_err(),
            ReassignError::NotOpenForReview
        );
    }

    #[test]
    fn reassign_pending_refuses_when_neither_field_is_given() {
        let mut tasks = vec![task_at("t1", "pending", 0)];
        assert_eq!(
            reassign_pending(
                &mut tasks,
                "t1",
                "sess-1",
                None,
                None,
                &["sess-5".to_string()],
                0
            )
            .unwrap_err(),
            ReassignError::AmbiguousChange
        );
    }

    #[test]
    fn reassign_pending_refuses_both_fields_at_once() {
        let mut tasks = vec![task_at("t1", "pending", 0)];
        let live = vec!["sess-5".to_string(), "sess-6".to_string()];
        assert_eq!(
            reassign_pending(
                &mut tasks,
                "t1",
                "sess-1",
                Some("sess-5"),
                Some("sess-6"),
                &live,
                0
            )
            .unwrap_err(),
            ReassignError::AmbiguousChange
        );
    }

    #[test]
    fn reassign_pending_reports_an_unknown_id() {
        let mut tasks = vec![task_at("t1", "pending", 0)];
        assert_eq!(
            reassign_pending(
                &mut tasks,
                "nope",
                "sess-1",
                Some("sess-5"),
                None,
                &["sess-5".to_string()],
                0
            )
            .unwrap_err(),
            ReassignError::NotFound
        );
    }

    // ---- halt race across the lock, third round: one synchronization boundary ----
    //
    // Round two closed the race with a snapshot captured inside the lock
    // plus a pre-delivery status re-check (`reassignment_still_deliverable`,
    // now gone). That fixed the read `persist_task` used to do, but left two
    // gaps of its own: the journal append for the mutation still happened
    // after the lock was released, so `set_halted` could still cancel and
    // journal first; and the re-check itself read the status through a
    // second, separate lock acquisition, leaving the same kind of gap one
    // step later. `reassign_task` and `cancel_tasks` now hold the `tasks`
    // lock across the mutation, the journal append, and (for a retarget)
    // delivery, so there is no window left for anything to land in between.

    #[test]
    fn set_halted_cannot_touch_a_task_whose_reassignment_holds_the_lock() {
        // The fix is exactly this lock: `reassign_task` now holds `tasks`
        // across its mutation, its journal append, and delivery, so
        // `set_halted`'s cancel loop -- which needs that same lock --
        // cannot run until that whole section releases it. This test
        // cannot pause `reassign_task` itself mid-body to prove the exact
        // interleaving end to end: `engine` is a concrete `SessionManager`
        // here, not a trait object, so there is no seam to freeze it at the
        // `submit_to` call, the same gap already noted on
        // `a_refused_dispatch_charges_nothing_and_records_nothing`. What is
        // testable, and is the actual mechanism the fix relies on, is that
        // holding the same lock `reassign_task` holds blocks `set_halted`
        // exactly as `reassign_task`'s own hold would.
        let (shared, dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", "pending", 0));

        let started = Arc::new(AtomicBool::new(false));
        let guard = shared.tasks.lock().unwrap();

        let halting = thread::spawn({
            let shared = shared.clone();
            let started = started.clone();
            move || {
                started.store(true, Ordering::SeqCst);
                shared.set_halted(true);
            }
        });

        while !started.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        thread::sleep(Duration::from_millis(50));
        assert_eq!(
            guard.iter().find(|t| t.id == "t1").unwrap().status,
            "pending",
            "set_halted must not touch this task while its lock is held elsewhere"
        );

        drop(guard);
        halting.join().unwrap();

        assert_eq!(
            shared.tasks.lock().unwrap()[0].status,
            "cancelled",
            "set_halted must still run once the lock is free"
        );
        let reloaded = load_brain(dir.path());
        assert_eq!(reloaded.tasks[0].status, "cancelled");
    }

    // ---- fourth round: every writer journals under the lock ----
    //
    // The third round's fix only reached `reassign_task` and `cancel_tasks`.
    // `tasks_from`'s overdue flip, `reconcile_abandoned`, `review_task`,
    // `finish_task`, `ask_task_question`, and `answer_task_question` all had
    // the identical shape: mutate under one lock acquisition, release it,
    // then append under a second, separate one, leaving the same gap for a
    // second writer to land in between. `mutate_and_journal` makes this
    // structural: it is now the only place `StoreRecord::Task` is
    // constructed for a write, so every one of these routes through it (see
    // `mutate_and_journal`'s own doc comment). What follows are persistence
    // checks per newly routed writer, proving each one's record actually
    // reaches the journal, rather than re-proving the lock's mutual
    // exclusion once per call site, which `mutate_and_journal` and
    // `set_halted_cannot_touch_a_task_whose_reassignment_holds_the_lock`
    // above already establish generically.

    #[test]
    fn set_halted_cannot_run_while_a_delivery_holds_the_gate() {
        // The other half of this round: `submit_to` now runs outside the
        // task mutex, under `delivery` instead, so this is what has to
        // block `set_halted` in its place. Same mechanism as the task-mutex
        // test above, aimed at the new gate.
        let (shared, _dir) = shared_for_test();

        let started = Arc::new(AtomicBool::new(false));
        let delivering = shared.delivery.read().unwrap();

        let halting = thread::spawn({
            let shared = shared.clone();
            let started = started.clone();
            move || {
                started.store(true, Ordering::SeqCst);
                shared.set_halted(true);
            }
        });

        while !started.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        thread::sleep(Duration::from_millis(50));
        assert!(
            !shared.is_halted(),
            "set_halted must not complete while a delivery holds the gate"
        );

        drop(delivering);
        halting.join().unwrap();
        assert!(
            shared.is_halted(),
            "set_halted must still run once the gate is free"
        );
    }

    #[test]
    fn shared_mutate_and_journal_writes_before_returning() {
        let (shared, dir) = shared_for_test();
        let t = task_at("t1", "pending", 0);
        shared.mutate_and_journal(|tasks| {
            tasks.push(t.clone());
            ((), vec![t])
        });
        let reloaded = load_brain(dir.path());
        assert_eq!(reloaded.tasks.len(), 1);
        assert_eq!(reloaded.tasks[0].id, "t1");
    }

    #[test]
    fn shared_review_task_journals_before_returning() {
        let (shared, dir) = shared_for_test();
        let mut t = task_at("t1", "in_review", 0);
        t.reviewer = "sess-9".into();
        shared.tasks.lock().unwrap().push(t);

        shared
            .review_task("sess-9", "t1", true, "looks good")
            .unwrap();

        let reloaded = load_brain(dir.path());
        assert_eq!(reloaded.tasks[0].status, "done");
        assert_eq!(reloaded.tasks[0].findings, "looks good");
    }

    #[test]
    fn shared_finish_task_journals_before_returning() {
        let (shared, dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", "pending", 0));

        shared
            .finish_task("sess-2", "t1", "the audit is clean")
            .unwrap();

        let reloaded = load_brain(dir.path());
        assert_eq!(reloaded.tasks[0].status, "done");
        assert_eq!(reloaded.tasks[0].result, "the audit is clean");
    }

    #[test]
    fn shared_ask_and_answer_task_question_each_journal_before_returning() {
        let (shared, dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", "pending", 0));

        shared
            .ask_task_question("sess-2", "t1", "what format?")
            .unwrap();
        let after_ask = load_brain(dir.path());
        assert_eq!(after_ask.tasks[0].status, STATUS_BLOCKED);

        shared
            .answer_task_question("sess-1", "t1", "use JSON")
            .unwrap();
        let after_answer = load_brain(dir.path());
        assert_eq!(after_answer.tasks[0].status, "pending");
        assert_eq!(after_answer.tasks[0].exchanges[0].answer, "use JSON");
    }

    #[test]
    fn shared_mark_delivery_failed_journals_before_returning() {
        let (shared, dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", "pending", 0));

        shared.mark_delivery_failed("t1");

        let reloaded = load_brain(dir.path());
        assert_eq!(reloaded.tasks[0].status, "error");
    }

    #[test]
    fn shared_task_status_journals_the_abandon_sweep_before_returning() {
        // Not the overdue flip `age` performs: `task_status` calls
        // `reconcile_abandoned` first, and `shared_for_test`'s engine
        // reports nothing live, so a pending task with an unreachable
        // target is swept to abandoned before `age` ever sees it, the
        // same reason `shared_reassign_task_refuses_a_reviewer_that_is_not_live`
        // reaches for `in_review` instead of `pending`. `age`'s own flip
        // is already covered directly and purely by
        // `age_marks_a_stale_pending_task_overdue_but_leaves_other_statuses_alone`;
        // reaching it through `task_status` at the `Shared` level, past a
        // live target this fixture cannot provide, is the residual gap.
        // What this can still check is that `task_status`'s own call
        // into `mutate_and_journal` (via `reconcile_abandoned`) still
        // reaches the target it asked about.
        let (shared, dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", "pending", 0));

        let t = shared.task_status("sess-1", "t1").unwrap();

        assert_eq!(t.status, STATUS_ABANDONED);
        let reloaded = load_brain(dir.path());
        assert_eq!(reloaded.tasks[0].status, STATUS_ABANDONED);
    }

    // ---- fifth round: the delivery gate covers the mutation, and cancel_tasks takes it too ----
    //
    // Two gaps the fourth round's gate left open. `reassign_task` only took
    // `delivery` for the write itself, after the retarget mutation had
    // already happened and been journaled: a Stop landing between the
    // top-level `is_halted` check and the mutation could still turn an
    // overdue or abandoned task pending after the halt, because
    // `set_halted`'s own cancel loop only ever touches tasks that are
    // already "pending" and so never sees one retargeted after it has
    // already run. And `cancel_tasks` never took the gate at all, so a
    // cancel could still land between a delivery path's status check and
    // its `submit_to` call. Fixed by moving `reassign_task`'s
    // `delivery.read()` to before the mutation, and giving `cancel_tasks`
    // the same exclusive hold `set_halted` already has.

    #[test]
    fn reassign_task_cannot_mutate_a_task_while_a_delivery_holds_the_gate() {
        // Same mechanism, same technique, as
        // `set_halted_cannot_run_while_a_delivery_holds_the_gate`, but
        // anchored at the point that changed this round: the gate now
        // covers `reassign_task`'s own mutation, not only its write, so
        // holding it here must block `set_halted` from starting at all --
        // not just from finishing. An overdue task is used deliberately:
        // `set_halted`'s cancel loop would never touch it even if it did
        // run, which is exactly why the mutation itself has to be inside
        // the gate rather than left to race it.
        let (shared, dir) = shared_for_test();
        // Through `mutate_and_journal`, not a raw push, so the journal
        // already holds an "overdue" record for `t1` to compare the final
        // one against: `set_halted`'s cancel loop skips this task entirely,
        // so nothing else would ever journal it if this test's own setup
        // did not.
        let t1 = task_at("t1", "overdue", 0);
        shared.mutate_and_journal(|tasks| {
            tasks.push(t1.clone());
            ((), vec![t1])
        });

        let started = Arc::new(AtomicBool::new(false));
        let delivering = shared.delivery.read().unwrap();

        let halting = thread::spawn({
            let shared = shared.clone();
            let started = started.clone();
            move || {
                started.store(true, Ordering::SeqCst);
                shared.set_halted(true);
            }
        });

        while !started.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        thread::sleep(Duration::from_millis(50));
        assert!(
            !shared.is_halted(),
            "set_halted must not run while reassign_task's gate is held, even before its own mutation"
        );
        assert_eq!(shared.tasks.lock().unwrap()[0].status, "overdue");

        drop(delivering);
        halting.join().unwrap();
        assert!(shared.is_halted());

        // set_halted's cancel loop never touches an overdue task, so this
        // one is exactly where it started: the point of holding the gate
        // through the mutation is that reassign_task, not set_halted, is
        // the only thing that gets to decide what happens to it next.
        let reloaded = load_brain(dir.path());
        assert_eq!(reloaded.tasks[0].status, "overdue");
    }

    #[test]
    fn cancel_tasks_cannot_run_while_a_delivery_holds_the_gate() {
        let (shared, dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", "pending", 0));

        let started = Arc::new(AtomicBool::new(false));
        let delivering = shared.delivery.read().unwrap();

        let cancelling = thread::spawn({
            let shared = shared.clone();
            let started = started.clone();
            move || {
                started.store(true, Ordering::SeqCst);
                shared.cancel_tasks(&["t1".to_string()], "no longer needed")
            }
        });

        while !started.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        thread::sleep(Duration::from_millis(50));
        assert_eq!(
            shared.tasks.lock().unwrap()[0].status,
            "pending",
            "cancel_tasks must not run while a delivery holds the gate"
        );

        drop(delivering);
        let results = cancelling.join().unwrap();

        assert_eq!(results[0], ("t1".to_string(), CancelOutcome::Cancelled));
        let reloaded = load_brain(dir.path());
        assert_eq!(reloaded.tasks[0].status, "cancelled");
    }

    // ---- note_session: a reconnected pane's kind must not go stale ----

    #[test]
    fn note_session_records_a_new_session() {
        let (shared, _dir) = shared_for_test();
        shared.note_session("sess-9", "codex", "sonnet");

        let sessions = shared.sessions_snapshot();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].kind, "codex");
        assert_eq!(sessions[0].model, "sonnet");
    }

    #[test]
    fn note_session_refreshes_the_kind_when_a_pane_reconnects_differently() {
        let (shared, dir) = shared_for_test();
        shared.note_session("sess-1", "codex", "");

        shared.note_session("sess-1", "claude", "");

        let sessions = shared.sessions_snapshot();
        assert_eq!(
            sessions.len(),
            1,
            "a reconnect must not duplicate the roster entry"
        );
        assert_eq!(sessions[0].kind, "claude");

        // Persisted, not just held in memory: a reload must see the refresh.
        let reloaded = load_brain(dir.path());
        assert_eq!(reloaded.sessions.len(), 1);
        assert_eq!(reloaded.sessions[0].kind, "claude");
    }

    #[test]
    fn note_session_refreshes_the_model_when_a_pane_reconnects_with_a_different_model() {
        let (shared, dir) = shared_for_test();
        shared.note_session("sess-1", "claude", "sonnet");

        shared.note_session("sess-1", "claude", "opus");

        let sessions = shared.sessions_snapshot();
        assert_eq!(
            sessions.len(),
            1,
            "a reconnect must not duplicate the roster entry"
        );
        assert_eq!(sessions[0].model, "opus");

        // Persisted, not just held in memory: a reload must see the refresh.
        let reloaded = load_brain(dir.path());
        assert_eq!(reloaded.sessions.len(), 1);
        assert_eq!(reloaded.sessions[0].model, "opus");
    }

    #[test]
    fn note_session_is_a_no_op_when_neither_kind_nor_model_has_changed() {
        let (shared, dir) = shared_for_test();
        shared.note_session("sess-1", "codex", "");
        shared.note_session("sess-1", "codex", "");

        // Only one record should have been appended; the file it would have
        // grown is the cheapest way to see a silent extra write.
        let contents = std::fs::read_to_string(dir.path().join("brain.jsonl")).unwrap();
        assert_eq!(contents.lines().count(), 1);
    }

    #[test]
    fn note_session_does_not_drain_a_pane_that_has_not_connected_yet() {
        // note_session runs at MCP-wiring time in spawn_session, before the
        // PTY or its engine handle exist. An earlier version drained from
        // here, which meant a restart errored every restored pane's queue
        // at spawn time, against a handle the engine did not have yet: a
        // failed attempt that was never actually attempted. A queued task
        // must survive this call untouched.
        let (shared, _dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", STATUS_QUEUED, 0));

        shared.note_session("sess-2", "claude", "sonnet");

        assert_eq!(shared.tasks_snapshot()[0].status, STATUS_QUEUED);
    }

    #[test]
    fn note_session_clears_connected_so_a_respawned_pane_must_reconnect() {
        // The readiness flag resets on respawn: a pane already marked
        // connected must go back to not-connected the moment note_session
        // says a fresh incarnation is starting, so a stale connection from
        // the last one cannot make the new one look ready before it is.
        let (shared, _dir) = shared_for_test();
        shared.mark_connected("sess-2");
        assert!(shared.is_connected("sess-2"), "sanity: it was connected");

        shared.note_session("sess-2", "claude", "sonnet");

        assert!(!shared.is_connected("sess-2"));
    }

    #[test]
    fn drain_pane_refuses_to_attempt_against_a_pane_that_has_never_connected() {
        // Isolates the gate itself, independent of any trigger: even a
        // direct drain_pane call must not touch a queued task ahead of a
        // connection, which is the property the two note_session tests
        // above and mark_connected's own test below all rest on.
        let (shared, _dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", STATUS_QUEUED, 0));

        shared.drain_pane("sess-2");

        assert_eq!(shared.tasks_snapshot()[0].status, STATUS_QUEUED);
    }

    #[test]
    fn mark_connected_is_what_actually_drains_a_pane() {
        // The other half of the note_session fix: delivery is not gone,
        // just moved to the moment a pane's MCP endpoint actually hears
        // from it. shared_for_test's engine still reports nothing live, so
        // the observable proof is the one the other drain-trigger tests
        // use: a queued task flips to error on the attempted, failed
        // delivery.
        //
        // What this does not, and cannot, show: the real wiring in
        // start_session_server (the per-session handler factory) and the
        // shared endpoint's set_session_identity are what actually call
        // mark_connected in the running app; neither runs a live HTTP
        // client against this fixture, so only mark_connected itself is
        // exercised here, directly, standing in for both.
        let (shared, _dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", STATUS_QUEUED, 0));
        assert!(!shared.is_connected("sess-2"), "sanity: not connected yet");

        shared.mark_connected("sess-2");

        assert!(shared.is_connected("sess-2"));
        assert_eq!(shared.tasks_snapshot()[0].status, "error");
    }

    #[test]
    fn set_session_identity_on_the_shared_endpoint_drains_the_pane() {
        let (shared, _dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", STATUS_QUEUED, 0));
        let handler = BrainHandler::new(shared.clone());

        handler.set_session_identity(Parameters(Identify {
            name: "sess-2".to_string(),
            kind: "claude".to_string(),
            room: String::new(),
        }));

        assert_eq!(shared.tasks_snapshot()[0].status, "error");
    }

    #[test]
    fn set_dir_sweeps_every_connected_pane_not_bare_liveness() {
        // Switched from live_ids() to `connected` specifically so this
        // would stop being untestable: shared_for_test's engine always
        // reports nothing live, but `connected` is ordinary state this
        // fixture can drive directly, so the sweep itself is now exercised
        // here rather than only trusted by inspection. The queued task has
        // to live in the *new* dir's journal, because set_dir replaces
        // `tasks` wholesale with whatever that journal holds; it is
        // `connected` (in-memory, carried across the switch) that this
        // test is proving actually gets swept.
        let (shared, _old_dir) = shared_for_test();
        shared
            .connected
            .lock()
            .unwrap()
            .insert("sess-2".to_string());

        let new_dir = tempfile::tempdir().expect("tempdir");
        append_record(
            new_dir.path(),
            &StoreRecord::Task(task_at("t1", STATUS_QUEUED, 0)),
        )
        .unwrap();

        shared.set_dir(new_dir.path().to_path_buf());

        assert_eq!(shared.tasks_snapshot()[0].status, "error");
    }

    // ---- merge_live_identity: a pane's identity survives a set_dir switch ----
    //
    // Measured 2026-09-03 on the Phase 1 build: `set_dir` replaced `sessions`
    // wholesale with the new journal's list, so a pane spawned before the
    // switch (correct kind, in memory) went back to reading whatever the new
    // journal last said about that name. Two panes spawned after the switch
    // were fine, which is what pointed at `set_dir` rather than spawn itself.

    fn agent(name: &str, kind: &str) -> AgentSession {
        AgentSession {
            name: name.to_string(),
            kind: kind.to_string(),
            model: String::new(),
        }
    }

    #[test]
    fn a_live_panes_identity_wins_over_the_new_journal() {
        let old = vec![agent("sess-1", "claude")];
        let new = vec![agent("sess-1", "codex")];
        let live = vec!["sess-1".to_string()];

        let (merged, changed) = merge_live_identity(&old, &new, &live);

        assert_eq!(
            merged[0].kind, "claude",
            "the live pane's real kind was dropped"
        );
        assert_eq!(
            changed.len(),
            1,
            "the correction must be recorded to the new dir"
        );
        assert_eq!(changed[0].kind, "claude");
    }

    #[test]
    fn a_dead_panes_identity_follows_the_journal() {
        let old = vec![agent("sess-1", "claude")];
        let new = vec![agent("sess-1", "codex")];
        let live: Vec<String> = Vec::new();

        let (merged, changed) = merge_live_identity(&old, &new, &live);

        assert_eq!(
            merged[0].kind, "codex",
            "a pane that is not live has nothing in memory to trust over the journal"
        );
        assert!(
            changed.is_empty(),
            "nothing changed, so nothing should be recorded"
        );
    }

    #[test]
    fn an_unchanged_identity_records_nothing() {
        let old = vec![agent("sess-1", "claude")];
        let new = vec![agent("sess-1", "claude")];
        let live = vec!["sess-1".to_string()];

        let (merged, changed) = merge_live_identity(&old, &new, &live);

        assert_eq!(merged[0].kind, "claude");
        assert!(
            changed.is_empty(),
            "old and new already agree; appending a record would be a no-op write"
        );
    }

    #[test]
    fn a_live_pane_missing_from_a_brand_new_journal_is_carried_forward() {
        let old = vec![agent("sess-1", "claude")];
        let new: Vec<AgentSession> = Vec::new();
        let live = vec!["sess-1".to_string()];

        let (merged, changed) = merge_live_identity(&old, &new, &live);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].kind, "claude");
        assert_eq!(changed.len(), 1);
    }

    // ---- Shared::cancel_tasks / Shared::reassign_task ----

    #[test]
    fn shared_cancel_tasks_persists_each_outcome() {
        let (shared, dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", "pending", 0));
        shared.tasks.lock().unwrap().push(task_at("t2", "done", 0));

        let results = shared.cancel_tasks(
            &["t1".to_string(), "t2".to_string(), "nope".to_string()],
            "cleanup",
        );

        assert_eq!(results[0], ("t1".to_string(), CancelOutcome::Cancelled));
        assert_eq!(
            results[1],
            ("t2".to_string(), CancelOutcome::AlreadyTerminal)
        );
        assert_eq!(results[2], ("nope".to_string(), CancelOutcome::NotFound));

        let reloaded = load_brain(dir.path());
        let t1 = reloaded.tasks.iter().find(|t| t.id == "t1").unwrap();
        assert_eq!(t1.status, "cancelled");
    }

    #[test]
    fn shared_reassign_task_refuses_a_reviewer_that_is_not_live() {
        // in_review, not pending: `shared_for_test`'s engine reports nothing
        // live, so a pending task's target would be reconciled to abandoned
        // before reassign_pending ever ran. in_review is exempt from that
        // sweep (see abandon_lost), so this is the status that actually
        // reaches the reviewer-liveness check being tested here.
        let (shared, _dir) = shared_for_test();
        let mut t = task_at("t1", "in_review", 0);
        t.reviewer = "sess-9".into();
        shared.tasks.lock().unwrap().push(t);

        let err = shared
            .reassign_task("sess-1", "t1", None, Some("sess-5"))
            .unwrap_err();

        assert_eq!(err, ReassignError::NotLive("sess-5".to_string()));
    }

    #[test]
    fn shared_reassign_task_refuses_a_retarget_while_halted() {
        // Retargeting redelivers into a terminal, same as dispatch, so Stop
        // must gate it the same way dispatch_precheck gates dispatch.
        let (shared, _dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", "pending", 0));
        shared.set_halted(true);

        let err = shared
            .reassign_task("sess-1", "t1", Some("sess-5"), None)
            .unwrap_err();

        assert_eq!(err, ReassignError::Halted);
    }

    #[test]
    fn shared_reassign_task_allows_a_reviewer_change_while_halted() {
        // Handing a task to a new reviewer types nothing into any terminal,
        // so it is not the "typed brief while halted" hazard Stop guards
        // against.
        let (shared, _dir) = shared_for_test();
        let mut t = task_at("t1", "in_review", 0);
        t.reviewer = "sess-9".into();
        shared.tasks.lock().unwrap().push(t);
        shared.set_halted(true);

        let err = shared
            .reassign_task("sess-1", "t1", None, Some("sess-5"))
            .unwrap_err();

        assert_eq!(
            err,
            ReassignError::NotLive("sess-5".to_string()),
            "halted should not be the reason this is refused"
        );
    }

    // ---- occupying_task / is_occupied ----
    //
    // The rule the whole per-pane queue rests on: what makes a pane unsafe
    // to type into right now.

    #[test]
    fn a_pane_with_its_own_pending_task_is_occupied() {
        let tasks = vec![task_at("t1", "pending", 0)]; // target sess-2
        assert!(is_occupied(&tasks, "sess-2"));
        assert!(!is_occupied(&tasks, "sess-3"));
    }

    #[test]
    fn every_status_that_holds_a_pane_occupies_it() {
        for status in ["pending", "overdue", "rework", STATUS_BLOCKED] {
            let tasks = vec![task_at("t1", status, 0)];
            assert!(
                is_occupied(&tasks, "sess-2"),
                "{status} must occupy its target"
            );
        }
    }

    #[test]
    fn a_reviewer_reviewing_an_in_review_task_is_occupied() {
        let mut t = task_at("t1", "in_review", 0);
        t.reviewer = "sess-4".into();
        let tasks = vec![t];

        assert!(is_occupied(&tasks, "sess-4"), "the reviewer is occupied");
        assert!(
            !is_occupied(&tasks, "sess-2"),
            "the target already submitted and is free"
        );
    }

    #[test]
    fn a_queued_task_does_not_occupy_its_own_target() {
        // The load-bearing property: if a queued task occupied its own
        // target, the queue could never drain.
        let tasks = vec![task_at("t1", STATUS_QUEUED, 0)];
        assert!(!is_occupied(&tasks, "sess-2"));
    }

    #[test]
    fn a_done_task_does_not_occupy_anyone() {
        let tasks = vec![task_at("t1", "done", 0)];
        assert!(!is_occupied(&tasks, "sess-2"));
    }

    #[test]
    fn occupying_task_can_exclude_a_task_from_judging_its_own_pane_occupied() {
        // The case `drain_pane` and `reassign_task` both need: a task whose
        // own status is what would occupy the pane must not count against
        // itself when deciding whether to deliver or move it.
        let tasks = vec![task_at("t1", "rework", 0)]; // target sess-2
        assert!(occupying_task(&tasks, "sess-2", "").is_some());
        assert!(
            occupying_task(&tasks, "sess-2", "t1").is_none(),
            "excluding the task itself must leave the pane looking free"
        );
    }

    // ---- next_delivery_for: what a freed pane receives ----

    #[test]
    fn next_delivery_for_is_none_when_the_pane_is_occupied() {
        let mut occupying = task_at("t1", "pending", 0);
        occupying.target = "sess-2".into();
        let mut queued = task_at("t2", STATUS_QUEUED, 5);
        queued.target = "sess-2".into();
        let tasks = vec![occupying, queued];

        assert!(next_delivery_for(&tasks, "sess-2").is_none());
    }

    #[test]
    fn next_delivery_for_is_none_when_nothing_is_pending() {
        let tasks = vec![task_at("t1", "done", 0)];
        assert!(next_delivery_for(&tasks, "sess-2").is_none());
    }

    #[test]
    fn next_delivery_for_picks_the_oldest_queued_dispatch() {
        let mut q1 = task_at("t1", STATUS_QUEUED, 10);
        q1.target = "sess-2".into();
        let mut q2 = task_at("t2", STATUS_QUEUED, 5);
        q2.target = "sess-2".into();
        let tasks = vec![q1, q2];

        let next = next_delivery_for(&tasks, "sess-2").expect("something is pending");
        assert_eq!(
            next.id, "t2",
            "the older of the two queued tasks goes first"
        );
    }

    #[test]
    fn next_delivery_for_prefers_an_undelivered_notice_over_a_queued_dispatch() {
        // A review request or rework notice is already-done work someone is
        // waiting on; a freshly queued dispatch is new work that can wait
        // one more turn.
        let mut notice = task_at("t1", "rework", 0);
        notice.target = "sess-2".into();
        notice.notice_delivered = false;
        let mut queued = task_at("t2", STATUS_QUEUED, 0);
        queued.target = "sess-2".into();
        let tasks = vec![queued, notice];

        let next = next_delivery_for(&tasks, "sess-2").expect("something is pending");
        assert_eq!(next.id, "t1", "the rework notice must go first");
    }

    #[test]
    fn next_delivery_for_ignores_a_notice_already_delivered() {
        let mut notice = task_at("t1", "rework", 0);
        notice.target = "sess-2".into();
        notice.notice_delivered = true;
        let tasks = vec![notice];

        assert!(next_delivery_for(&tasks, "sess-2").is_none());
    }

    #[test]
    fn next_delivery_for_finds_a_review_request_by_reviewer_not_target() {
        let mut notice = task_at("t1", "in_review", 0);
        notice.target = "sess-3".into();
        notice.reviewer = "sess-2".into();
        notice.notice_delivered = false;
        let tasks = vec![notice];

        let next = next_delivery_for(&tasks, "sess-2").expect("the reviewer's pane is free");
        assert_eq!(next.id, "t1");
    }

    // ---- queued_ids_for / queue_cap_refusal ----

    #[test]
    fn queued_ids_for_orders_oldest_first_and_ignores_other_panes_and_statuses() {
        let mut q_new = task_at("new", STATUS_QUEUED, 20);
        q_new.target = "sess-2".into();
        let mut q_old = task_at("old", STATUS_QUEUED, 5);
        q_old.target = "sess-2".into();
        let mut elsewhere = task_at("elsewhere", STATUS_QUEUED, 1);
        elsewhere.target = "sess-3".into();
        let mut pending = task_at("pending", "pending", 1);
        pending.target = "sess-2".into();
        let tasks = vec![q_new, q_old, elsewhere, pending];

        assert_eq!(
            queued_ids_for(&tasks, "sess-2"),
            vec!["old".to_string(), "new".to_string()]
        );
    }

    #[test]
    fn queue_cap_refusal_allows_up_to_the_cap_and_refuses_at_it() {
        let below: Vec<String> = (0..QUEUE_CAP - 1).map(|i| i.to_string()).collect();
        assert_eq!(queue_cap_refusal(&below), None);

        let at_cap: Vec<String> = (0..QUEUE_CAP).map(|i| i.to_string()).collect();
        let refusal = queue_cap_refusal(&at_cap).expect("the cap must refuse");
        assert!(refusal.contains("queue is full"), "{refusal}");
        for id in &at_cap {
            assert!(refusal.contains(id), "{refusal}");
        }
    }

    // ---- append_queued: folding a queue-depth note into a roster label ----

    #[test]
    fn append_queued_is_a_no_op_at_zero_depth() {
        assert_eq!(append_queued(" [busy 4m]".to_string(), 0), " [busy 4m]");
        assert_eq!(append_queued(String::new(), 0), "");
    }

    #[test]
    fn append_queued_folds_into_an_existing_bracket() {
        assert_eq!(
            append_queued(" [busy 4m]".to_string(), 2),
            " [busy 4m, 2 queued]"
        );
    }

    #[test]
    fn append_queued_opens_its_own_bracket_when_there_is_no_other_label() {
        assert_eq!(append_queued(String::new(), 2), " [2 queued]");
    }

    #[test]
    fn single_line_folds_every_line_break_to_a_space() {
        assert_eq!(single_line("a\r\nb\nc\rd"), "a b c d");
    }

    #[test]
    fn truncate_chars_marks_a_cut_and_leaves_short_text_alone() {
        assert_eq!(truncate_chars("short", 10), "short");
        assert_eq!(truncate_chars("abcdef", 3), "abc...");
    }

    // ---- fit_injection / truncate_bytes: shrink rather than refuse ----

    #[test]
    fn fit_injection_leaves_a_short_message_untouched() {
        let msg = fit_injection(|v| format!("wrapper: {v}"), "short");
        assert_eq!(msg, "wrapper: short");
    }

    #[test]
    fn fit_injection_shrinks_the_variable_part_to_fit() {
        let long = "x".repeat(2000);
        let msg = fit_injection(|v| format!("[pantheon] fixed wrapper: {v} end"), &long);

        assert!(
            injection_overage(&msg).is_none(),
            "message is still {} bytes, over the limit",
            msg.len()
        );
        assert!(msg.starts_with("[pantheon] fixed wrapper:"));
        assert!(msg.ends_with("end"), "the fixed wrapper must survive whole");
        assert!(msg.contains("..."), "the cut must be marked");
    }

    #[test]
    fn truncate_bytes_cuts_on_a_char_boundary() {
        // Each "é" is 2 bytes; a naive byte-index cut would land inside one
        // and panic on a non-boundary slice.
        let s = "é".repeat(20);
        let truncated = truncate_bytes(&s, 10);
        assert!(truncated.len() <= 10);
        assert!(s.is_char_boundary(truncated.len() - 3) || truncated.len() < 3);
    }

    // ---- review_request_notice / rework_notice: delivered message shape ----

    #[test]
    fn review_request_notice_names_the_task_target_and_next_step() {
        let msg = review_request_notice("abc123", "sess-2", "audit the parser");
        assert!(!msg.contains(['\r', '\n']));
        assert!(msg.contains("abc123"));
        assert!(msg.contains("sess-2"));
        assert!(msg.contains("audit the parser"));
        assert!(msg.contains("get_task_result"));
        assert!(msg.contains("review_task"));
    }

    #[test]
    fn rework_notice_names_the_task_reviewer_and_next_step() {
        let msg = rework_notice("abc123", "sess-4", "missing a test");
        assert!(!msg.contains(['\r', '\n']));
        assert!(msg.contains("abc123"));
        assert!(msg.contains("sess-4"));
        assert!(msg.contains("missing a test"));
        assert!(msg.contains("complete_task"));
    }

    // ---- STATUS_QUEUED: a queued task is open but never counted as busy ----

    #[test]
    fn a_queued_task_is_open() {
        assert!(is_open(STATUS_QUEUED));
    }

    #[test]
    fn a_queued_task_stays_in_a_wait_and_renders_with_a_plain_status_tag() {
        let t = task_at("t1", STATUS_QUEUED, 0);
        assert_eq!(
            still_open(&[t.clone()], &["t1".to_string()]),
            vec!["t1".to_string()],
            "wait_for_tasks must keep treating a queued task as still running"
        );
        assert!(render_task(&t).starts_with("[queued]"));
    }

    #[test]
    fn attribute_open_tasks_excludes_queued_from_the_busy_bucket() {
        let tasks = vec![task_at("t1", STATUS_QUEUED, 0)];
        let (busy, reviewing) = attribute_open_tasks(&tasks, 1000);
        assert!(busy.is_empty(), "a queued task has not started running");
        assert!(reviewing.is_empty());
    }

    #[test]
    fn cancel_pending_closes_a_queued_task() {
        let mut tasks = vec![task_at("t1", STATUS_QUEUED, 0)];
        assert_eq!(
            cancel_pending(&mut tasks, "t1", "no longer needed", 500),
            CancelOutcome::Cancelled
        );
        assert_eq!(tasks[0].status, "cancelled");
    }

    #[test]
    fn reassign_pending_accepts_a_queued_task_for_retarget() {
        let mut tasks = vec![task_at("t1", STATUS_QUEUED, 0)];
        let live = vec!["sess-5".to_string()];

        let t = reassign_pending(&mut tasks, "t1", "sess-1", Some("sess-5"), None, &live, 100)
            .expect("a queued task may be retargeted");

        assert_eq!(t.status, "pending", "fresh delivery, fresh status");
        assert_eq!(t.target, "sess-5");
        assert_eq!(t.ts_ms, 100, "fresh delivery, fresh clock");
    }

    // ---- finish_pending / review_pending: notice_delivered on transition ----

    #[test]
    fn finishing_reviewed_work_marks_its_review_request_undelivered() {
        let mut tasks = dispatched_task("sess-3");
        tasks[0].notice_delivered = true; // nothing pending before this call
        finish_pending(&mut tasks, "sess-2", "abc123", "done", 0).unwrap();

        assert_eq!(tasks[0].status, "in_review");
        assert!(
            !tasks[0].notice_delivered,
            "the reviewer now owes a review request"
        );
    }

    #[test]
    fn finishing_unreviewed_work_leaves_notice_delivered_alone() {
        let mut tasks = dispatched_task("");
        finish_pending(&mut tasks, "sess-2", "abc123", "done", 0).unwrap();

        assert_eq!(tasks[0].status, "done");
        assert!(tasks[0].notice_delivered, "nobody is owed a notice");
    }

    #[test]
    fn rejecting_a_review_marks_its_rework_notice_undelivered() {
        let mut tasks = dispatched_task("sess-3");
        finish_pending(&mut tasks, "sess-2", "abc123", "attempt", 0).unwrap();
        tasks[0].notice_delivered = true; // the review request was delivered
        review_pending(&mut tasks, "sess-3", "abc123", false, "missing a test", 0).unwrap();

        assert_eq!(tasks[0].status, "rework");
        assert!(
            !tasks[0].notice_delivered,
            "the target now owes a rework notice"
        );
    }

    #[test]
    fn approving_a_review_leaves_notice_delivered_alone() {
        let mut tasks = dispatched_task("sess-3");
        finish_pending(&mut tasks, "sess-2", "abc123", "attempt", 0).unwrap();
        tasks[0].notice_delivered = true;
        review_pending(&mut tasks, "sess-3", "abc123", true, "looks right", 0).unwrap();

        assert_eq!(tasks[0].status, "done");
        assert!(tasks[0].notice_delivered, "nobody is owed a notice");
    }

    // ---- notice_delivered persistence ----

    #[test]
    fn a_task_from_before_this_field_loads_as_notice_undelivered() {
        // The useful reading of `serde(default)` here: an in_review or
        // rework task written before Pantheon had any delivery mechanism at
        // all gets retried once, rather than silently staying undelivered
        // forever across the upgrade.
        let stored = r#"{"id":"old1","from":"sess-1","target":"sess-2","task":"t","status":"in_review","result":"r","ts_ms":5,"reviewer":"sess-3"}"#;
        let task: Task = serde_json::from_str(stored).expect("an older task must still load");

        assert!(!task.notice_delivered);
    }

    #[test]
    fn notice_delivered_round_trips_through_the_store() {
        let (shared, dir) = shared_for_test();
        let mut t = task_at("t1", "rework", 0);
        t.notice_delivered = false;
        shared.mutate_and_journal(|tasks| {
            tasks.push(t.clone());
            ((), vec![t])
        });

        let reloaded = load_brain(dir.path());
        assert!(!reloaded.tasks[0].notice_delivered);
    }

    // ---- set_halted: queued tasks are cancelled and drained on resume ----

    #[test]
    fn set_halted_cancels_a_queued_task_the_same_as_a_pending_one() {
        let (shared, _dir) = shared_for_test();
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", STATUS_QUEUED, 0));

        shared.set_halted(true);

        assert_eq!(shared.tasks_snapshot()[0].status, "cancelled");
    }

    // ---- drain_pane is called from every place that can free a pane ----
    //
    // `shared_for_test`'s engine reports nothing live, so `submit_to` always
    // fails inside these tests (see the module-level `shared_for_test` doc
    // comment). That is used here as the signal, not worked around: a
    // queued task left touching this failure path is proof `drain_pane` ran
    // and attempted delivery, where a queued task still sitting untouched
    // would mean the trigger never fired at all. Confirming a *successful*
    // delivery end-to-end needs a live pane, which this fixture cannot
    // provide; `next_delivery_for`'s tests above cover the selection logic
    // that a real delivery would act on.

    #[test]
    fn cancelling_a_task_drains_its_targets_queue() {
        let (shared, _dir) = shared_for_test();
        shared
            .connected
            .lock()
            .unwrap()
            .insert("sess-2".to_string());
        let mut occupying = task_at("t1", "pending", 0);
        occupying.target = "sess-2".into();
        let mut queued = task_at("t2", STATUS_QUEUED, 1);
        queued.target = "sess-2".into();
        shared.tasks.lock().unwrap().push(occupying);
        shared.tasks.lock().unwrap().push(queued);

        shared.cancel_tasks(&["t1".to_string()], "no longer needed");

        let after = shared.tasks_snapshot();
        let t2 = after.iter().find(|t| t.id == "t2").unwrap();
        assert_eq!(
            t2.status, "error",
            "drain_pane must have attempted delivery and failed in this fixture"
        );
    }

    #[test]
    fn finishing_reviewed_work_gives_the_review_request_priority_over_the_reviewers_own_queue() {
        // finish_task hands sess-3 a fresh, undelivered review request for
        // t1, which is exactly the case next_delivery_for's own priority
        // test covers: a notice goes out before anything already queued
        // behind it. t2 must stay put rather than jump the notice.
        let (shared, _dir) = shared_for_test();
        shared
            .connected
            .lock()
            .unwrap()
            .insert("sess-3".to_string());
        let mut work = task_at("t1", "pending", 0);
        work.target = "sess-2".into();
        work.reviewer = "sess-3".into();
        let mut queued_for_reviewer = task_at("t2", STATUS_QUEUED, 1);
        queued_for_reviewer.target = "sess-3".into();
        shared.tasks.lock().unwrap().push(work);
        shared.tasks.lock().unwrap().push(queued_for_reviewer);

        shared.finish_task("sess-2", "t1", "done").unwrap();

        let after = shared.tasks_snapshot();
        assert_eq!(
            after.iter().find(|t| t.id == "t1").unwrap().status,
            "in_review",
            "the review request is what drain_pane attempted"
        );
        assert_eq!(
            after.iter().find(|t| t.id == "t2").unwrap().status,
            STATUS_QUEUED,
            "the queued task must not have been delivered ahead of the notice"
        );
    }

    #[test]
    fn approving_a_review_drains_the_reviewers_own_queue() {
        let (shared, _dir) = shared_for_test();
        shared
            .connected
            .lock()
            .unwrap()
            .insert("sess-3".to_string());
        let mut in_review = task_at("t1", "in_review", 0);
        in_review.reviewer = "sess-3".into();
        let mut queued_for_reviewer = task_at("t2", STATUS_QUEUED, 1);
        queued_for_reviewer.target = "sess-3".into();
        shared.tasks.lock().unwrap().push(in_review);
        shared.tasks.lock().unwrap().push(queued_for_reviewer);

        shared
            .review_task("sess-3", "t1", true, "looks right")
            .unwrap();

        let after = shared.tasks_snapshot();
        let t2 = after.iter().find(|t| t.id == "t2").unwrap();
        assert_eq!(
            t2.status, "error",
            "the reviewer's own queue must have been drained"
        );
    }

    #[test]
    fn rejecting_a_review_drains_the_reviewers_queue_while_the_targets_notice_takes_priority() {
        // review_task(reject) touches two panes: the reviewer's, which is
        // now free and has nothing of its own pending, so its queue (t3)
        // drains; and the target's, which review_task just handed a fresh
        // undelivered rework notice, so its queue (t2) must wait behind it,
        // the same priority next_delivery_for enforces on its own.
        let (shared, _dir) = shared_for_test();
        shared
            .connected
            .lock()
            .unwrap()
            .insert("sess-2".to_string());
        shared
            .connected
            .lock()
            .unwrap()
            .insert("sess-3".to_string());
        let mut in_review = task_at("t1", "in_review", 0);
        in_review.target = "sess-2".into();
        in_review.reviewer = "sess-3".into();
        let mut queued_for_target = task_at("t2", STATUS_QUEUED, 1);
        queued_for_target.target = "sess-2".into();
        let mut queued_for_reviewer = task_at("t3", STATUS_QUEUED, 1);
        queued_for_reviewer.target = "sess-3".into();
        shared.tasks.lock().unwrap().push(in_review);
        shared.tasks.lock().unwrap().push(queued_for_target);
        shared.tasks.lock().unwrap().push(queued_for_reviewer);

        shared
            .review_task("sess-3", "t1", false, "missing a test")
            .unwrap();

        let after = shared.tasks_snapshot();
        assert_eq!(
            after.iter().find(|t| t.id == "t1").unwrap().status,
            "rework",
            "the rework notice is what drain_pane attempted on the target's pane"
        );
        assert_eq!(
            after.iter().find(|t| t.id == "t2").unwrap().status,
            STATUS_QUEUED,
            "the target's queue must not have been delivered ahead of its own rework notice"
        );
        assert_eq!(
            after.iter().find(|t| t.id == "t3").unwrap().status,
            "error",
            "the reviewer's own queue, with no notice of its own, must have been drained"
        );
    }

    #[test]
    fn drain_pane_types_nothing_while_halted() {
        // Set halted straight on the flag rather than through set_halted,
        // which would cancel this very queued task before drain_pane ever
        // saw it (see set_halted_cancels_a_queued_task_the_same_as_a_pending_one).
        // What this test isolates is drain_pane's own halted check.
        let (shared, _dir) = shared_for_test();
        shared
            .connected
            .lock()
            .unwrap()
            .insert("sess-2".to_string());
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", STATUS_QUEUED, 0));
        *shared.halted.lock().unwrap() = true;

        shared.drain_pane("sess-2");

        assert_eq!(
            shared.tasks_snapshot()[0].status,
            STATUS_QUEUED,
            "nothing is delivered while halted"
        );
    }

    #[test]
    fn drain_pane_resumes_delivery_once_unhalted() {
        // `set_halted`'s resume sweep is keyed off `live_ids()`, which
        // `shared_for_test`'s engine always reports empty, so the sweep
        // itself has nothing to iterate in this fixture and cannot be
        // exercised end to end here. What is exercisable, and is the actual
        // guard drain_pane relies on, is that halted alone is what blocks
        // it: once cleared, the same call that was a no-op above attempts
        // delivery again.
        let (shared, _dir) = shared_for_test();
        shared
            .connected
            .lock()
            .unwrap()
            .insert("sess-2".to_string());
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", STATUS_QUEUED, 0));
        *shared.halted.lock().unwrap() = true;

        shared.drain_pane("sess-2");
        assert_eq!(shared.tasks_snapshot()[0].status, STATUS_QUEUED);

        *shared.halted.lock().unwrap() = false;
        shared.drain_pane("sess-2");
        assert_eq!(
            shared.tasks_snapshot()[0].status,
            "error",
            "delivery is attempted once unhalted, even though this fixture's engine cannot succeed at it"
        );
    }

    // ---- sixth round: `delivery` excludes Stop and cancel, not delivery paths from each other ----
    //
    // Three findings from the same review. `drain_pane` (and `dispatch_task`
    // and `reassign_task`'s own delivery) only ever took `delivery.read()`,
    // and any number of readers can hold a `RwLock` at once by design, so
    // that gate never stopped two of those from running concurrently on the
    // same pane: two drains could both select the same candidate before
    // either promoted it, and a dispatch could see a pane as free in the
    // instant a drain had picked its queue head but not yet delivered it.
    // Fixed with `pane_delivery`, a mutex per pane held across the whole
    // "decide what this pane gets next, then send it" section by every
    // delivery path. `reassign_task`'s occupied branch separately dropped
    // `delivery_gate` before its own second mutation (pending to queued),
    // reopening the exact window the fourth round closed for its first one;
    // fixed by holding the gate through both, and by `requeue_if_still_pending`
    // refusing to flip a task that is no longer exactly what it was left as.
    // And the queue cap was read in one `tasks` lock acquisition, well before
    // the task's creation in another; fixed by `enqueue_within_cap`, folding
    // the check into the same lock hold as the push.
    //
    // ---- seventh round: the three tests above didn't force the race either ----
    //
    // A second review of the sixth round's own tests: a `Barrier` at
    // `drain_pane`'s or `enqueue_within_cap`'s *entrance* only guarantees
    // both threads start together, not that they reach the contested
    // internal boundary together, so the scheduler could still run one
    // thread all the way to completion before the other is even scheduled
    // -- the fixture's `submit_to` fails instantly, so that window is tiny
    // but real. And the lock-blocking test assumed a thread had reached a
    // lock after a 50ms sleep, which a merely descheduled thread would also
    // pass without ever attempting the lock at all. All three would pass on
    // the unfixed code often enough to look green.
    //
    // Fixed with `test_seam` (see its doc comment on `Shared`): production
    // code calls a test-installed hook at the exact internal boundary a
    // real race would need to land on, and the hook blocks the calling
    // thread on a channel receive instead of returning immediately. That
    // gives each test explicit control over when a thread is allowed past
    // the boundary, so the interleaving the finding describes is forced
    // every run, not hoped for. A hook that pauses one thread (test 1) or
    // simply signals it was reached (test 2), plus a bounded
    // `recv_timeout` for a *second* signal that must never arrive while
    // the first thread is still paused there, is what proves exclusion:
    // under the fix, a second arrival is architecturally impossible until
    // the first is released, so a timeout finding nothing is a guarantee,
    // not a guess; under the bug, the second thread has nothing to block
    // it and reaches the hook almost immediately, comfortably inside the
    // bound. All three tests below were run against the sixth round's own
    // fix with the relevant lock or fold-into-one-hold removed (by hand,
    // then reverted) to confirm they fail without it; see each test's own
    // comment for exactly what was reverted and how it failed.

    #[test]
    fn two_concurrent_drains_on_one_pane_deliver_the_queued_task_exactly_once() {
        // Confirmed this fails without the fix: with `pane_gate` removed
        // from `drain_pane`, `mine` reaches the seam (and sends `arrived`)
        // within microseconds of being spawned, since nothing blocks it --
        // the `recv_timeout` assertion below gets that second signal
        // instead of timing out, and fails immediately. With `pane_gate`
        // restored, `mine` blocks acquiring the pane lock `other` holds
        // and cannot reach the seam until `other` is released and has
        // fully returned, so the timeout always elapses.
        let (shared, dir) = shared_for_test();
        shared
            .connected
            .lock()
            .unwrap()
            .insert("sess-2".to_string());
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", STATUS_QUEUED, 0));

        // The hook reports reaching the seam, then pauses there (still
        // holding `pane_gate`, past selection, before the post-submit
        // journal) until this test explicitly releases it.
        let (arrived_tx, arrived_rx) = mpsc::channel::<()>();
        let (proceed_tx, proceed_rx) = mpsc::channel::<()>();
        let proceed_rx = Mutex::new(proceed_rx);
        shared.set_test_seam(move |_label| {
            let _ = arrived_tx.send(());
            let _ = proceed_rx.lock().unwrap().recv();
        });

        let other = thread::spawn({
            let shared = shared.clone();
            move || shared.drain_pane("sess-2")
        });
        arrived_rx
            .recv()
            .expect("other must reach the seam to select the only queued task");

        // `other` is now proven paused at the seam, holding `pane_gate`
        // under the fix. Spawn the second drain now, while it is still
        // paused -- this is the exact window the finding describes.
        let mine = thread::spawn({
            let shared = shared.clone();
            move || shared.drain_pane("sess-2")
        });

        // Sound, not a timing guess: while `other` still holds `pane_gate`,
        // `mine` cannot possibly reach the seam, so a second `arrived`
        // never arriving within any bound is a guarantee under the fix,
        // not luck. A bug that let both proceed would deliver it well
        // inside this bound.
        assert!(
            arrived_rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "a second drain reached the seam while the first still held the pane lock; \
             both would have selected the same queued task"
        );

        // Release both: `other` (proven waiting) and `mine` (only if it
        // also reached the seam, which the assertion above says it should
        // not have under the fix; sent regardless so neither thread can
        // hang waiting on a message this test never sends).
        let _ = proceed_tx.send(());
        let _ = proceed_tx.send(());
        other.join().unwrap();
        mine.join().unwrap();

        assert_eq!(shared.tasks_snapshot()[0].status, "error");
        let journal = std::fs::read_to_string(dir.path().join("brain.jsonl")).unwrap();
        assert_eq!(
            journal.lines().count(),
            1,
            "only one of the two drains should have found anything to deliver; \
             a second journal line means both attempted the same candidate"
        );
    }

    #[test]
    fn drain_pane_queues_behind_another_delivery_path_already_holding_the_panes_lock() {
        // `dispatch_task` cannot be driven far enough in this fixture to
        // reach `pane_delivery_lock` at all: `dispatch_precheck` refuses
        // for want of a live target before either gate is ever taken (see
        // `a_refused_dispatch_charges_nothing_and_records_nothing`), and
        // `reassign_task`'s retarget path is refused the same way (see
        // `reassign_pending_refuses_a_target_that_is_not_live`). So this
        // holds the same lock those two would hold across their own
        // decide-then-send section, directly, standing in for "a dispatch
        // arrives while a drain holds a candidate": drain_pane must queue
        // behind whichever delivery path already has the pane reserved,
        // exactly as it would behind another drain.
        //
        // Confirmed this fails without the fix: with `pane_gate` removed
        // from `drain_pane`, the seam call is reached (and `arrived_tx`
        // sent) immediately, regardless of this test's own `holding` lock,
        // since drain_pane never asks for it; the first `recv_timeout`
        // assertion below fails immediately instead of timing out. With
        // `pane_gate` restored, `draining`'s thread blocks on the same
        // `pane_lock` this test holds and cannot reach the seam at all
        // until `holding` is dropped, so the timeout always elapses.
        let (shared, dir) = shared_for_test();
        shared
            .connected
            .lock()
            .unwrap()
            .insert("sess-2".to_string());
        shared
            .tasks
            .lock()
            .unwrap()
            .push(task_at("t1", STATUS_QUEUED, 0));

        let (arrived_tx, arrived_rx) = mpsc::channel::<()>();
        shared.set_test_seam(move |_label| {
            let _ = arrived_tx.send(());
        });

        let pane_lock = shared.pane_delivery_lock("sess-2");
        let holding = pane_lock.lock().unwrap();

        let draining = thread::spawn({
            let shared = shared.clone();
            move || shared.drain_pane("sess-2")
        });

        // Not a sleep-then-peek: `drain_pane`'s seam call is placed after
        // it acquires `pane_gate`, so while this test holds the same lock
        // via `holding`, that call is architecturally unreachable -- any
        // wait here finding nothing is a guarantee, not a guess about how
        // long "long enough" is. A bug that skipped the lock entirely
        // would reach the seam within microseconds, well inside this
        // bound.
        assert!(
            arrived_rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "drain_pane reached the seam while this test still held its pane lock"
        );
        assert_eq!(
            shared.tasks_snapshot()[0].status,
            STATUS_QUEUED,
            "drain_pane must not act on this pane while another delivery path holds its lock"
        );

        drop(holding);
        // Now unblocked, drain_pane proceeds to (and past) the seam; this
        // blocks until it does, no timeout needed since it is now expected.
        arrived_rx
            .recv()
            .expect("drain_pane must reach the seam once the lock is free");
        draining.join().unwrap();

        assert_eq!(
            shared.tasks_snapshot()[0].status,
            "error",
            "released once the lock is free, same as any other drain"
        );
        let journal = std::fs::read_to_string(dir.path().join("brain.jsonl")).unwrap();
        assert_eq!(journal.lines().count(), 1);
    }

    #[test]
    fn requeue_if_still_pending_flips_a_matching_pending_task_to_queued() {
        let mut tasks = vec![task_at("t1", "pending", 0)];

        let flipped = requeue_if_still_pending(&mut tasks, "t1", "sess-2");

        assert_eq!(flipped.unwrap().status, STATUS_QUEUED);
        assert_eq!(tasks[0].status, STATUS_QUEUED);
    }

    #[test]
    fn requeue_if_still_pending_leaves_a_task_alone_once_something_else_already_decided_its_fate() {
        // The bug this guards against: reassign_task's occupied branch used
        // to flip a task to queued unconditionally, so a cancel or a Stop
        // that got there first, while the old code had briefly dropped its
        // gate, was silently overwritten, resurrecting cancelled work.
        let mut tasks = vec![task_at("t1", "cancelled", 0)];

        let flipped = requeue_if_still_pending(&mut tasks, "t1", "sess-2");

        assert!(flipped.is_none());
        assert_eq!(tasks[0].status, "cancelled");
    }

    #[test]
    fn requeue_if_still_pending_leaves_a_task_alone_when_the_target_no_longer_matches() {
        let mut tasks = vec![task_at("t1", "pending", 0)];
        tasks[0].target = "sess-9".to_string();

        let flipped = requeue_if_still_pending(&mut tasks, "t1", "sess-2");

        assert!(flipped.is_none());
        assert_eq!(tasks[0].status, "pending");
    }

    #[test]
    fn requeue_if_still_pending_reports_nothing_for_an_unknown_id() {
        let mut tasks = vec![task_at("t1", "pending", 0)];

        assert!(requeue_if_still_pending(&mut tasks, "missing", "sess-2").is_none());
    }

    #[test]
    fn enqueue_within_cap_lets_two_concurrent_callers_race_the_last_free_slot_without_overflowing()
    {
        // Confirmed this fails without the fix: with the cap check split
        // from the push into two separate `self.tasks` lock acquisitions
        // (this function's previous shape), `mine` reaches the seam within
        // microseconds of being spawned, since `other`'s check has already
        // released the lock by the time it pauses -- the `recv_timeout`
        // assertion below gets a second `arrived` instead of timing out,
        // and fails immediately. With the check and the push folded into
        // one `mutate_and_journal` hold, `mine` blocks acquiring
        // `self.tasks` until `other`'s entire closure, seam included, has
        // returned, so the timeout always elapses.
        let (shared, _dir) = shared_for_test();
        {
            // An occupying task, so `is_occupied` is true and each racer's
            // new task lands as `queued` (counted by `queued_ids_for`)
            // rather than `pending` (which would not be): the cap only
            // bites a target that is already busy, exactly like a real
            // dispatch queue.
            let mut tasks = shared.tasks.lock().unwrap();
            let mut occupying = task_at("occupant", "pending", 0);
            occupying.target = "sess-2".to_string();
            tasks.push(occupying);
            for i in 0..QUEUE_CAP - 1 {
                let mut t = task_at(&format!("q{i}"), STATUS_QUEUED, (i + 1) as u64);
                t.target = "sess-2".to_string();
                tasks.push(t);
            }
        }

        // The hook reports reaching the seam, then pauses there -- still
        // inside `enqueue_within_cap`'s `mutate_and_journal` closure, so
        // still holding `self.tasks` -- until this test releases it.
        let (arrived_tx, arrived_rx) = mpsc::channel::<()>();
        let (proceed_tx, proceed_rx) = mpsc::channel::<()>();
        let proceed_rx = Mutex::new(proceed_rx);
        shared.set_test_seam(move |_label| {
            let _ = arrived_tx.send(());
            let _ = proceed_rx.lock().unwrap().recv();
        });

        let other = thread::spawn({
            let shared = shared.clone();
            move || {
                shared.enqueue_within_cap("sess-2", |occupied| {
                    let mut t = task_at(
                        "racer-b",
                        if occupied { STATUS_QUEUED } else { "pending" },
                        100,
                    );
                    t.target = "sess-2".to_string();
                    t
                })
            }
        });
        arrived_rx
            .recv()
            .expect("other must reach the seam past its own cap check");

        // `other` is now proven paused at the seam, holding `self.tasks`
        // under the fix. Spawn the second racer now, while it is still
        // paused -- this is the exact window the finding describes.
        let mine = thread::spawn({
            let shared = shared.clone();
            move || {
                shared.enqueue_within_cap("sess-2", |occupied| {
                    let mut t = task_at(
                        "racer-a",
                        if occupied { STATUS_QUEUED } else { "pending" },
                        101,
                    );
                    t.target = "sess-2".to_string();
                    t
                })
            }
        });

        // Sound, not a timing guess: while `other` still holds
        // `self.tasks`, `mine` cannot possibly reach the seam (it cannot
        // even start its own closure), so a second `arrived` never
        // arriving within any bound is a guarantee under the fix. A split
        // check-then-push would deliver it well inside this bound.
        assert!(
            arrived_rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "a second caller reached the seam while the first still held the tasks lock \
             mid check-then-push; the cap and the push are no longer atomic"
        );

        // Release both: `other` (proven waiting) and `mine` (only if it
        // also reached the seam, which the assertion above says it should
        // not have under the fix).
        let _ = proceed_tx.send(());
        let _ = proceed_tx.send(());
        let theirs = other.join().unwrap();
        let mine = mine.join().unwrap();

        let outcomes = [mine, theirs];
        let refused = outcomes.iter().filter(|r| r.is_err()).count();
        let created = outcomes.iter().filter(|r| r.is_ok()).count();
        assert_eq!(refused, 1, "exactly one of the two must lose to the cap");
        assert_eq!(created, 1, "exactly one of the two must be created");

        let final_queued = queued_ids_for(&shared.tasks_snapshot(), "sess-2").len();
        assert_eq!(
            final_queued, QUEUE_CAP,
            "the cap must hold even when two callers raced its last free slot"
        );
    }

    #[test]
    fn enqueue_within_cap_refuses_and_creates_nothing_once_the_target_is_already_at_cap() {
        let (shared, dir) = shared_for_test();
        {
            let mut tasks = shared.tasks.lock().unwrap();
            for i in 0..QUEUE_CAP {
                let mut t = task_at(&format!("q{i}"), STATUS_QUEUED, i as u64);
                t.target = "sess-2".to_string();
                tasks.push(t);
            }
        }

        let refusal = shared
            .enqueue_within_cap("sess-2", |occupied| {
                let mut t = task_at("over", if occupied { STATUS_QUEUED } else { "pending" }, 99);
                t.target = "sess-2".to_string();
                t
            })
            .expect_err("the target is already at QUEUE_CAP");

        assert!(refusal.contains("queue is full"), "{refusal}");
        assert_eq!(
            shared.tasks_snapshot().len(),
            QUEUE_CAP,
            "nothing was created"
        );
        assert!(
            std::fs::read_to_string(dir.path().join("brain.jsonl")).is_err(),
            "nothing was journaled either: refusal wrote no file at all"
        );
    }
}
