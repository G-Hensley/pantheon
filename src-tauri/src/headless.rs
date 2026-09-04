use std::fmt;
use std::path::PathBuf;

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

#[cfg(test)]
mod tests {
    use super::{is_headless_quiet, LaunchEndpoint, LaunchSpec, HEADLESS_QUIET_MS};
    use std::path::PathBuf;

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
            spec.env.last().map(|(key, value)| (key.as_str(), value.as_str())),
            Some(("PANTHEON_SESSION", "sess-4"))
        );
    }

    #[test]
    fn launch_spec_retains_the_shared_fallback_without_a_token() {
        let spec = spec(LaunchEndpoint::Shared);
        assert_eq!(spec.endpoint, LaunchEndpoint::Shared);
        assert_eq!(
            spec.mcp_args,
            [
                "--mcp-config",
                "/tmp/pantheon-session/claude-mcp.json"
            ]
        );
    }

    #[test]
    fn quiet_gate_is_anchored_to_last_output_and_handles_clock_saturation() {
        assert!(!is_headless_quiet(1_000, 1_000 + HEADLESS_QUIET_MS - 1));
        assert!(is_headless_quiet(1_000, 1_000 + HEADLESS_QUIET_MS));
        assert!(!is_headless_quiet(2_000, 1_000));
    }
}
