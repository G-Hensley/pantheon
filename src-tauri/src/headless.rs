use std::collections::VecDeque;
use std::fmt;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Deserialize;
use uuid::Uuid;

/// How long an interactive pane must be silent before a caller may start a
/// headless child in the same checkout.
pub const HEADLESS_QUIET_MS: u64 = 30_000;
pub const HEADLESS_TERMINATION_GRACE: Duration = Duration::from_secs(10);
/// Interactive close remains responsive and completes cleanup promptly. Normal
/// task timeouts retain the longer grace above, where no UI callback is waiting.
pub(crate) const HEADLESS_CLOSE_GRACE: Duration = Duration::from_millis(500);
const STDERR_RING_BYTES: usize = 2 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub enum LaunchEndpoint {
    Dedicated { url: String, token: String },
    Shared,
}

impl fmt::Debug for LaunchEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dedicated { url, .. } => formatter
                .debug_struct("Dedicated")
                .field("url", url)
                .field("token", &"<redacted>")
                .finish(),
            Self::Shared => formatter.write_str("Shared"),
        }
    }
}

/// Everything Pantheon must retain to reproduce a pane's launch environment.
#[derive(Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub cwd: PathBuf,
    pub program: String,
    /// The exact argv used for the interactive pane.
    pub args: Vec<String>,
    /// Model selection kept separately so a headless command can reuse it
    /// without copying the interactive pane's argv.
    pub model_args: Vec<String>,
    /// MCP argv must remain last because Claude's `--mcp-config` is variadic.
    pub mcp_args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub endpoint: LaunchEndpoint,
}

impl LaunchSpec {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn for_session(
        cwd: PathBuf,
        program: String,
        pane_args: Vec<String>,
        model: Option<&str>,
        model_flag: Option<&str>,
        mcp_args: Vec<String>,
        mut mcp_env: Vec<(String, String)>,
        session_id: &str,
        endpoint: LaunchEndpoint,
    ) -> Self {
        let model_args = match (model, model_flag) {
            (Some(model), Some(flag)) => vec![flag.to_string(), model.to_string()],
            _ => Vec::new(),
        };
        let mut args = model_args.clone();
        args.extend(pane_args);
        args.extend(mcp_args.iter().cloned());
        mcp_env.push(("PANTHEON_SESSION".to_string(), session_id.to_string()));

        Self {
            cwd,
            program,
            args,
            model_args,
            mcp_args,
            env: mcp_env,
            endpoint,
        }
    }
}

/// Pure policy check. The dispatch caller owns queueing and decides when to
/// call the runner again.
pub fn is_headless_quiet(last_output: u64, now: u64) -> bool {
    now.saturating_sub(last_output) >= HEADLESS_QUIET_MS
}

/// Platform process-tree control retained independently from the child handle
/// so app shutdown can terminate a runner owned by another thread.
pub(crate) trait ProcessTree: Send + Sync {
    fn terminate(&self) -> io::Result<()>;
    fn kill(&self) -> io::Result<()>;
}

pub(crate) struct SpawnedProcessTree {
    pub child: Child,
    pub tree: Arc<dyn ProcessTree>,
}

#[cfg(unix)]
struct UnixProcessTree {
    process_group: libc::pid_t,
}

#[cfg(unix)]
impl UnixProcessTree {
    fn signal(&self, signal: libc::c_int) -> io::Result<()> {
        // A negative pid addresses the whole process group. The child is its
        // group leader because spawn configured process_group(0).
        if unsafe { libc::kill(-self.process_group, signal) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(error)
        }
    }
}

#[cfg(unix)]
impl ProcessTree for UnixProcessTree {
    fn terminate(&self) -> io::Result<()> {
        self.signal(libc::SIGTERM)
    }

    fn kill(&self) -> io::Result<()> {
        self.signal(libc::SIGKILL)
    }
}

#[cfg(windows)]
struct WindowsProcessTree {
    job: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
// SAFETY: the Job Object handle stays valid until Drop, and Windows permits
// TerminateJobObject and CloseHandle to be called from any process thread.
unsafe impl Send for WindowsProcessTree {}
#[cfg(windows)]
// SAFETY: operations do not mutate Rust-owned memory, and the kernel
// synchronizes concurrent access to the Job Object handle.
unsafe impl Sync for WindowsProcessTree {}

#[cfg(windows)]
impl ProcessTree for WindowsProcessTree {
    fn terminate(&self) -> io::Result<()> {
        self.terminate_job()
    }

    fn kill(&self) -> io::Result<()> {
        self.terminate_job()
    }
}

#[cfg(windows)]
impl WindowsProcessTree {
    fn terminate_job(&self) -> io::Result<()> {
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;

        if unsafe { TerminateJobObject(self.job, 1) } != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(windows)]
impl Drop for WindowsProcessTree {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
}

#[cfg(unix)]
pub(crate) fn spawn_process_tree(command: &mut Command) -> io::Result<SpawnedProcessTree> {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
    let child = command.spawn()?;
    let process_group = libc::pid_t::try_from(child.id())
        .map_err(|_| io::Error::other("child pid did not fit pid_t"))?;
    Ok(SpawnedProcessTree {
        child,
        tree: Arc::new(UnixProcessTree { process_group }),
    })
}

#[cfg(windows)]
pub(crate) fn spawn_process_tree(command: &mut Command) -> io::Result<SpawnedProcessTree> {
    use std::mem::{size_of, zeroed};
    use std::os::windows::io::AsRawHandle;
    use std::ptr;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
    if job.is_null() {
        return Err(io::Error::last_os_error());
    }

    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let configured = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            ptr::from_ref(&limits).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if configured == 0 {
        let error = io::Error::last_os_error();
        unsafe { CloseHandle(job) };
        return Err(error);
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            unsafe { CloseHandle(job) };
            return Err(error);
        }
    };
    let process = child.as_raw_handle() as HANDLE;
    if unsafe { AssignProcessToJobObject(job, process) } == 0 {
        let error = io::Error::last_os_error();
        let _ = child.kill();
        let _ = child.wait();
        unsafe { CloseHandle(job) };
        return Err(error);
    }

    Ok(SpawnedProcessTree {
        child,
        tree: Arc::new(WindowsProcessTree { job }),
    })
}

pub(crate) fn terminate_process_tree(tree: &dyn ProcessTree, grace: Duration) -> io::Result<()> {
    let terminate_result = tree.terminate();
    thread::sleep(grace);
    let kill_result = tree.kill();
    match (terminate_result, kill_result) {
        (_, Ok(())) => Ok(()),
        (_, Err(kill_error)) => Err(kill_error),
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HeadlessUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    pub turns: Option<u64>,
    pub duration_ms: Option<u64>,
    pub duration_api_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadlessOutcome {
    pub result: String,
    pub cli_session: String,
    pub usage: Option<HeadlessUsage>,
    pub exit_code: Option<i32>,
    pub stderr: String,
}

#[derive(Debug)]
pub enum HeadlessError {
    SessionNotFound(String),
    Unsupported(String),
    SharedEndpoint(String),
    InvalidBudget,
    Spawn(io::Error),
    Input(io::Error),
    Output(io::Error),
    Wait(io::Error),
    Timeout {
        timeout: Duration,
        exit_code: Option<i32>,
        stderr: String,
    },
    Cancelled {
        exit_code: Option<i32>,
        usage: Option<Box<HeadlessUsage>>,
    },
    Exited {
        exit_code: Option<i32>,
        message: String,
        stderr: String,
        usage: Option<Box<HeadlessUsage>>,
    },
    InvalidJson {
        error: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
}

impl fmt::Display for HeadlessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled { .. } => formatter.write_str("headless child cancelled"),
            Self::SessionNotFound(id) => write!(formatter, "no live session '{id}'"),
            Self::Unsupported(program) => {
                write!(
                    formatter,
                    "headless dispatch does not support {program} yet"
                )
            }
            Self::SharedEndpoint(id) => write!(
                formatter,
                "refused: {id} has no per-session endpoint; restart the pane or send a pane brief"
            ),
            Self::InvalidBudget => {
                formatter.write_str("budget_usd must be finite and greater than zero")
            }
            Self::Spawn(error) => write!(formatter, "headless child failed to spawn: {error}"),
            Self::Input(error) => write!(formatter, "headless child rejected its brief: {error}"),
            Self::Output(error) => write!(formatter, "headless child output failed: {error}"),
            Self::Wait(error) => write!(formatter, "headless child wait failed: {error}"),
            Self::Timeout {
                timeout, stderr, ..
            } => write!(
                formatter,
                "headless child exceeded {} ms: {stderr}",
                timeout.as_millis()
            ),
            Self::Exited {
                exit_code,
                message,
                stderr,
                ..
            } => write!(
                formatter,
                "headless child exited {}: {}{}",
                display_exit_code(*exit_code),
                message,
                display_stderr(stderr)
            ),
            Self::InvalidJson {
                error,
                stdout,
                stderr,
                ..
            } => write!(
                formatter,
                "headless child returned invalid JSON ({error}): {stdout}{}",
                display_stderr(stderr)
            ),
        }
    }
}

impl std::error::Error for HeadlessError {}

fn display_exit_code(exit_code: Option<i32>) -> String {
    exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "without an exit code".to_string())
}

fn display_stderr(stderr: &str) -> String {
    if stderr.is_empty() {
        String::new()
    } else {
        format!(": {stderr}")
    }
}

#[derive(Deserialize)]
struct ClaudeJson {
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    errors: Vec<String>,
    #[serde(default)]
    subtype: String,
    #[serde(default)]
    terminal_reason: String,
    session_id: String,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    usage: Option<ClaudeTokenUsage>,
    #[serde(default)]
    total_cost_usd: Option<f64>,
    #[serde(default)]
    num_turns: Option<u64>,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    duration_api_ms: Option<u64>,
}

#[derive(Deserialize)]
struct ClaudeTokenUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
}

fn parse_claude_output(
    stdout: &[u8],
    stderr: String,
    exit_code: Option<i32>,
) -> Result<HeadlessOutcome, HeadlessError> {
    let parsed: ClaudeJson =
        serde_json::from_slice(stdout).map_err(|error| HeadlessError::InvalidJson {
            error: error.to_string(),
            exit_code,
            stdout: String::from_utf8_lossy(stdout).into_owned(),
            stderr: stderr.clone(),
        })?;
    let usage = if parsed.usage.is_some()
        || parsed.total_cost_usd.is_some()
        || parsed.num_turns.is_some()
        || parsed.duration_ms.is_some()
        || parsed.duration_api_ms.is_some()
    {
        let tokens = parsed.usage.unwrap_or(ClaudeTokenUsage {
            input_tokens: None,
            output_tokens: None,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        });
        Some(HeadlessUsage {
            input_tokens: tokens.input_tokens,
            output_tokens: tokens.output_tokens,
            cache_creation_input_tokens: tokens.cache_creation_input_tokens,
            cache_read_input_tokens: tokens.cache_read_input_tokens,
            cost_usd: parsed.total_cost_usd,
            turns: parsed.num_turns,
            duration_ms: parsed.duration_ms,
            duration_api_ms: parsed.duration_api_ms,
        })
    } else {
        None
    };

    if parsed.is_error || exit_code != Some(0) {
        let mut details = parsed.errors;
        if let Some(result) = parsed.result.filter(|s| !s.is_empty()) {
            details.push(result);
        }
        if !parsed.subtype.is_empty() {
            details.push(parsed.subtype);
        }
        if !parsed.terminal_reason.is_empty() {
            details.push(parsed.terminal_reason);
        }
        return Err(HeadlessError::Exited {
            exit_code,
            message: details.join(": "),
            stderr,
            usage: usage.map(Box::new),
        });
    }

    Ok(HeadlessOutcome {
        result: parsed.result.ok_or_else(|| HeadlessError::InvalidJson {
            error: "successful JSON result is missing result text".into(),
            exit_code,
            stdout: String::from_utf8_lossy(stdout).into_owned(),
            stderr: stderr.clone(),
        })?,
        cli_session: parsed.session_id,
        usage,
        exit_code,
        stderr,
    })
}

fn normalized_program(program: &str) -> String {
    program
        .to_ascii_lowercase()
        .trim_end_matches(".exe")
        .trim_end_matches(".cmd")
        .to_string()
}

fn build_claude_command(spec: &LaunchSpec, cli_session: &str, budget_usd: f64) -> Command {
    let headless_args = [
        "-p".to_string(),
        "--output-format".to_string(),
        "json".to_string(),
        "--session-id".to_string(),
        cli_session.to_string(),
        "--permission-mode".to_string(),
        "acceptEdits".to_string(),
        "--permission-prompts".to_string(),
        "none".to_string(),
        "--max-budget-usd".to_string(),
        budget_usd.to_string(),
    ];

    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("cmd.exe");
        command.arg("/c");
        command.arg(&spec.program);
        command
    };
    #[cfg(not(windows))]
    let mut command = Command::new(&spec.program);

    command.args(headless_args);
    command.args(&spec.model_args);
    // Must stay last because Claude treats every following argument as another
    // MCP config path.
    command.args(&spec.mcp_args);
    command.current_dir(&spec.cwd);
    command.envs(spec.env.iter().map(|(key, value)| (key, value)));
    command
}

enum ReaderResult {
    Stdout(io::Result<Vec<u8>>),
    Stderr(io::Result<Vec<u8>>),
}

struct RawOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    timed_out: bool,
    input_error: Option<io::Error>,
}

pub(crate) struct HeadlessChild {
    child: Child,
    tree: Arc<dyn ProcessTree>,
    cli_session: String,
    started: Instant,
    input: Option<JoinHandle<io::Result<()>>>,
    reader_rx: mpsc::Receiver<ReaderResult>,
    readers: Vec<JoinHandle<()>>,
}

impl HeadlessChild {
    pub(crate) fn spawn(
        pane_id: &str,
        spec: &LaunchSpec,
        brief: &str,
        budget_usd: f64,
    ) -> Result<Self, HeadlessError> {
        if matches!(spec.endpoint, LaunchEndpoint::Shared) {
            return Err(HeadlessError::SharedEndpoint(pane_id.to_string()));
        }
        if normalized_program(&spec.program) != "claude" {
            return Err(HeadlessError::Unsupported(spec.program.clone()));
        }
        if !budget_usd.is_finite() || budget_usd <= 0.0 {
            return Err(HeadlessError::InvalidBudget);
        }

        let cli_session = Uuid::new_v4().to_string();
        let command = build_claude_command(spec, &cli_session, budget_usd);
        Self::spawn_command(command, brief, cli_session)
    }

    fn spawn_command(
        mut command: Command,
        brief: &str,
        cli_session: String,
    ) -> Result<Self, HeadlessError> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let started = Instant::now();
        let mut process = spawn_process_tree(&mut command).map_err(HeadlessError::Spawn)?;
        let stdout = process.child.stdout.take().ok_or_else(|| {
            HeadlessError::Spawn(io::Error::other("headless stdout was not piped"))
        })?;
        let stderr = process.child.stderr.take().ok_or_else(|| {
            HeadlessError::Spawn(io::Error::other("headless stderr was not piped"))
        })?;
        let (reader_tx, reader_rx) = mpsc::channel();
        let stdout_tx = reader_tx.clone();
        let stdout_reader = thread::spawn(move || {
            let mut stdout = stdout;
            let mut bytes = Vec::new();
            let result = stdout.read_to_end(&mut bytes).map(|_| bytes);
            let _ = stdout_tx.send(ReaderResult::Stdout(result));
        });
        let stderr_reader = thread::spawn(move || {
            let result = read_stderr_ring(stderr, STDERR_RING_BYTES);
            let _ = reader_tx.send(ReaderResult::Stderr(result));
        });

        let input = process.child.stdin.take().map(|mut stdin| {
            let brief = brief.as_bytes().to_vec();
            thread::spawn(move || stdin.write_all(&brief))
        });

        Ok(Self {
            child: process.child,
            tree: process.tree,
            cli_session,
            started,
            input,
            reader_rx,
            readers: vec![stdout_reader, stderr_reader],
        })
    }

    pub(crate) fn cli_session(&self) -> &str {
        &self.cli_session
    }

    pub(crate) fn process_tree(&self) -> Arc<dyn ProcessTree> {
        self.tree.clone()
    }

    pub(crate) fn wait(mut self, timeout: Duration) -> Result<HeadlessOutcome, HeadlessError> {
        let raw = self.collect_output(timeout, HEADLESS_TERMINATION_GRACE)?;
        let exit_code = raw.status.code();
        let stderr = String::from_utf8_lossy(&raw.stderr).into_owned();
        if raw.timed_out {
            return Err(HeadlessError::Timeout {
                timeout,
                exit_code,
                stderr,
            });
        }
        if let Some(error) = raw.input_error {
            return Err(HeadlessError::Input(error));
        }
        parse_claude_output(&raw.stdout, stderr, exit_code)
    }

    fn collect_output(
        &mut self,
        timeout: Duration,
        termination_grace: Duration,
    ) -> Result<RawOutput, HeadlessError> {
        let deadline = self.started + timeout;
        let mut stdout = None;
        let mut stderr = None;
        let mut timed_out = false;
        while stdout.is_none() || stderr.is_none() {
            let now = Instant::now();
            let message = if now >= deadline {
                Err(mpsc::RecvTimeoutError::Timeout)
            } else {
                self.reader_rx.recv_timeout(deadline - now)
            };
            match message {
                Ok(ReaderResult::Stdout(result)) => stdout = Some(result),
                Ok(ReaderResult::Stderr(result)) => stderr = Some(result),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    timed_out = true;
                    terminate_process_tree(self.tree.as_ref(), termination_grace)
                        .map_err(HeadlessError::Output)?;
                    while stdout.is_none() || stderr.is_none() {
                        match self.reader_rx.recv().map_err(|_| {
                            HeadlessError::Output(io::Error::other(
                                "headless output readers disconnected",
                            ))
                        })? {
                            ReaderResult::Stdout(result) => stdout = Some(result),
                            ReaderResult::Stderr(result) => stderr = Some(result),
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(HeadlessError::Output(io::Error::other(
                        "headless output readers disconnected",
                    )))
                }
            }
        }

        for reader in self.readers.drain(..) {
            reader.join().map_err(|_| {
                HeadlessError::Output(io::Error::other("headless output reader panicked"))
            })?;
        }
        let stdout = stdout.unwrap().map_err(HeadlessError::Output)?;
        let stderr = stderr.unwrap().map_err(HeadlessError::Output)?;
        // Both pipes are drained before probing or waiting, so neither child
        // stream can fill and deadlock the parent at process exit. Keep the
        // deadline active even if a child closes its pipes and continues to run.
        let status = loop {
            if let Some(status) = self.child.try_wait().map_err(HeadlessError::Wait)? {
                break status;
            }
            if !timed_out && Instant::now() >= deadline {
                timed_out = true;
                terminate_process_tree(self.tree.as_ref(), termination_grace)
                    .map_err(HeadlessError::Output)?;
                break self.child.wait().map_err(HeadlessError::Wait)?;
            }
            thread::sleep(Duration::from_millis(10));
        };
        let input_error = self.input.take().and_then(|input| match input.join() {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error),
            Err(_) => Some(io::Error::other("headless input writer panicked")),
        });
        Ok(RawOutput {
            status,
            stdout,
            stderr,
            timed_out,
            input_error,
        })
    }
}

fn read_stderr_ring(mut stderr: impl Read, capacity: usize) -> io::Result<Vec<u8>> {
    let mut ring = VecDeque::with_capacity(capacity);
    let mut buffer = [0u8; 8 * 1024];
    loop {
        let read = stderr.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        for byte in &buffer[..read] {
            if ring.len() == capacity {
                ring.pop_front();
            }
            ring.push_back(*byte);
        }
    }
    Ok(ring.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::{
        build_claude_command, is_headless_quiet, parse_claude_output, spawn_process_tree,
        terminate_process_tree, HeadlessChild, HeadlessError, LaunchEndpoint, LaunchSpec,
        HEADLESS_QUIET_MS,
    };
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{Duration, Instant};

    fn spec(endpoint: LaunchEndpoint) -> LaunchSpec {
        LaunchSpec::for_session(
            PathBuf::from("/tmp/pantheon-session"),
            "claude".to_string(),
            vec!["--dangerously-skip-permissions".to_string()],
            Some("claude-sonnet-4-5"),
            Some("--model"),
            vec![
                "--mcp-config".to_string(),
                "/tmp/pantheon-session/claude-mcp.json".to_string(),
            ],
            vec![("PANTHEON_MCP_TOKEN".to_string(), "secret".to_string())],
            "sess-4",
            endpoint,
        )
    }

    #[test]
    fn success_requires_result_text() {
        assert!(matches!(
            parse_claude_output(
                br#"{"session_id":"uuid","is_error":false}"#,
                String::new(),
                Some(0)
            ),
            Err(HeadlessError::InvalidJson { .. })
        ));
    }

    #[test]
    fn measured_budget_error_retains_error_array_and_usage_without_result() {
        let error = parse_claude_output(br#"{"session_id":"uuid","is_error":true,"subtype":"error_max_budget_usd","terminal_reason":"budget_exhausted","errors":["Reached maximum budget ($0.05)"],"total_cost_usd":0.24,"usage":{"input_tokens":1,"cache_creation_input_tokens":2000}}"#, String::new(), Some(1)).unwrap_err();
        match error {
            HeadlessError::Exited {
                exit_code,
                message,
                usage,
                ..
            } => {
                assert_eq!(exit_code, Some(1));
                assert!(message.contains("Reached maximum budget ($0.05)"));
                assert!(message.contains("budget_exhausted"));
                assert_eq!(usage.unwrap().cost_usd, Some(0.24));
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn json_error_flag_at_exit_zero_is_still_an_error() {
        assert!(matches!(
            parse_claude_output(
                br#"{"session_id":"uuid","is_error":true,"result":"failed"}"#,
                String::new(),
                Some(0)
            ),
            Err(HeadlessError::Exited { .. })
        ));
    }

    #[test]
    fn launch_spec_retains_dedicated_endpoint_and_exact_pane_launch() {
        let endpoint = LaunchEndpoint::Dedicated {
            url: "http://127.0.0.1:43123/mcp".to_string(),
            token: "secret".to_string(),
        };
        let spec = spec(endpoint.clone());

        assert_eq!(spec.endpoint, endpoint);
        assert_eq!(spec.cwd, PathBuf::from("/tmp/pantheon-session"));
        assert_eq!(spec.program, "claude");
        assert_eq!(
            spec.args,
            [
                "--model",
                "claude-sonnet-4-5",
                "--dangerously-skip-permissions",
                "--mcp-config",
                "/tmp/pantheon-session/claude-mcp.json",
            ]
        );
        assert_eq!(
            spec.env
                .last()
                .map(|(key, value)| (key.as_str(), value.as_str())),
            Some(("PANTHEON_SESSION", "sess-4"))
        );
    }

    #[test]
    fn launch_spec_retains_the_shared_fallback_without_a_token() {
        let spec = spec(LaunchEndpoint::Shared);
        assert_eq!(spec.endpoint, LaunchEndpoint::Shared);
        assert_eq!(
            spec.mcp_args,
            ["--mcp-config", "/tmp/pantheon-session/claude-mcp.json"]
        );
    }

    #[test]
    fn quiet_gate_is_anchored_to_last_output_and_handles_clock_saturation() {
        assert!(!is_headless_quiet(1_000, 1_000 + HEADLESS_QUIET_MS - 1));
        assert!(is_headless_quiet(1_000, 1_000 + HEADLESS_QUIET_MS));
        assert!(!is_headless_quiet(2_000, 1_000));
    }

    #[test]
    fn claude_command_uses_print_mode_budget_model_and_mcp_config_last() {
        let spec = spec(LaunchEndpoint::Dedicated {
            url: "http://127.0.0.1:43123/mcp".to_string(),
            token: "secret".to_string(),
        });
        let command = build_claude_command(&spec, "cli-session", 1.25);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        // On Windows the program runs through `cmd.exe /c <program>`, so the
        // headless arguments begin after those two.
        #[cfg(windows)]
        {
            assert_eq!(&args[..2], ["/c", "claude"]);
        }
        #[cfg(windows)]
        let args = args[2..].to_vec();

        assert_eq!(
            args,
            [
                "-p",
                "--output-format",
                "json",
                "--session-id",
                "cli-session",
                "--permission-mode",
                "acceptEdits",
                "--permission-prompts",
                "none",
                "--max-budget-usd",
                "1.25",
                "--model",
                "claude-sonnet-4-5",
                "--mcp-config",
                "/tmp/pantheon-session/claude-mcp.json",
            ]
        );
        assert!(!args.iter().any(|arg| arg == "--max-turns"));
        assert_eq!(
            command.get_current_dir(),
            Some(std::path::Path::new("/tmp/pantheon-session"))
        );
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == "PANTHEON_SESSION"),
            Some((
                std::ffi::OsStr::new("PANTHEON_SESSION"),
                Some(std::ffi::OsStr::new("sess-4"))
            ))
        );
    }

    #[test]
    fn parses_claude_json_fixture_with_session_and_usage() {
        let outcome = parse_claude_output(
            include_bytes!("../tests/fixtures/claude-output.json"),
            String::new(),
            Some(0),
        )
        .unwrap();

        assert_eq!(outcome.result, "Implemented and verified the change.");
        assert_eq!(outcome.cli_session, "4f1ea546-80af-4fad-a3c9-9fc94892561e");
        assert_eq!(outcome.exit_code, Some(0));
        let usage = outcome.usage.unwrap();
        assert_eq!(usage.input_tokens, Some(412));
        assert_eq!(usage.output_tokens, Some(287));
        assert_eq!(usage.cache_creation_input_tokens, Some(203));
        assert_eq!(usage.cache_read_input_tokens, Some(1100));
        assert_eq!(usage.cost_usd, Some(0.037));
        assert_eq!(usage.turns, Some(3));
        assert_eq!(usage.duration_ms, Some(1842));
        assert_eq!(usage.duration_api_ms, Some(1301));
    }

    #[test]
    fn shared_endpoint_is_refused_before_spawning() {
        let error = match HeadlessChild::spawn(
            "sess-shared",
            &spec(LaunchEndpoint::Shared),
            "brief",
            1.0,
        ) {
            Ok(_) => panic!("a shared endpoint cannot identify its headless child"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            HeadlessError::SharedEndpoint(ref id) if id == "sess-shared"
        ));
        assert_eq!(
            error.to_string(),
            "refused: sess-shared has no per-session endpoint; restart the pane or send a pane brief"
        );
    }

    #[test]
    fn codex_and_opencode_are_unsupported_in_this_increment() {
        for program in ["codex", "opencode"] {
            let mut launch = spec(LaunchEndpoint::Dedicated {
                url: "http://127.0.0.1:43123/mcp".to_string(),
                token: "secret".to_string(),
            });
            launch.program = program.to_string();
            let error = match HeadlessChild::spawn("sess-4", &launch, "brief", 1.0) {
                Ok(_) => panic!("{program} must not spawn in the Claude-only increment"),
                Err(error) => error,
            };
            assert!(matches!(
                error,
                HeadlessError::Unsupported(ref found) if found == program
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn readers_drain_one_megabyte_of_stderr_without_deadlocking() {
        let mut command = Command::new("sh");
        command.args(["-c", "head -c 1048576 /dev/zero >&2"]);
        let mut child =
            HeadlessChild::spawn_command(command, "", "reader-test".to_string()).unwrap();

        let raw = child
            .collect_output(Duration::from_secs(5), Duration::from_millis(100))
            .unwrap();
        assert!(raw.status.success());
        assert!(!raw.timed_out);
        assert_eq!(raw.stderr.len(), 2 * 1024);
        assert!(raw.stderr.iter().all(|byte| *byte == 0));
    }

    #[cfg(unix)]
    #[test]
    fn timeout_terminates_the_whole_process_tree() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 60 & sleep 60"]);
        let mut child =
            HeadlessChild::spawn_command(command, "", "timeout-test".to_string()).unwrap();

        let raw = child
            .collect_output(Duration::from_millis(20), Duration::from_millis(50))
            .unwrap();
        assert!(raw.timed_out);
    }

    #[cfg(unix)]
    #[test]
    fn timeout_still_applies_after_a_child_closes_both_output_pipes() {
        let mut command = Command::new("sh");
        command.args(["-c", "exec 1>&- 2>&-; sleep 60"]);
        let mut child =
            HeadlessChild::spawn_command(command, "", "closed-pipes-test".to_string()).unwrap();

        let started = Instant::now();
        let raw = child
            .collect_output(Duration::from_millis(20), Duration::from_millis(50))
            .unwrap();
        assert!(raw.timed_out);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn unix_process_tree_kills_a_shell_and_both_sleep_children() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 60 & sleep 60"]);
        let mut process = spawn_process_tree(&mut command).unwrap();
        let process_group = process.child.id() as libc::pid_t;

        terminate_process_tree(process.tree.as_ref(), Duration::from_millis(100)).unwrap();
        process.child.wait().unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let result = unsafe { libc::kill(-process_group, 0) };
            if result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "process group {process_group} survived termination"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
