use std::fmt;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// How long an interactive pane must be silent before a caller may start a
/// headless child in the same checkout.
pub const HEADLESS_QUIET_MS: u64 = 30_000;

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
unsafe impl Send for WindowsProcessTree {}
#[cfg(windows)]
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
    kill_result.or(terminate_result)
}

#[cfg(test)]
mod tests {
    use super::{
        is_headless_quiet, spawn_process_tree, terminate_process_tree, LaunchEndpoint, LaunchSpec,
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
