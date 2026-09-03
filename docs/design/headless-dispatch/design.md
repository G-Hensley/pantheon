# Headless dispatch: design

Source: `BACKLOG.md` "Dispatch headlessly (`opencode run`) instead of typing
into the TUI" (line 53) and "Dispatch loses whole 1 KiB chunks" (line 209).
Plan: Phase 4 in `docs/plans/2026-09-03-pantheon-repair.md`. Line numbers
below are at `2907adf`. CLI flags were read from `--help` on this machine on
2026-09-03 (claude 2.1.259, codex-cli 0.153.0, opencode 1.18.27); no CLI was
run with a prompt.

## What exists today

A pane is a PTY child built by `build_command` (`src-tauri/src/lib.rs:453`),
which copies Pantheon's whole environment into it (`lib.rs:485-487`) and sets
`PANTHEON_SESSION` (`lib.rs:1069`). Every session gets its own MCP listener
and bearer token (`lib.rs:1004-1010`), so a request on that port is that
session. `agent_mcp_wiring` (`lib.rs:647`) hands each CLI the endpoint in its
own dialect: claude gets `--mcp-config <app-data>/sessions/<id>/claude-mcp.json`
appended last because the flag is variadic (`lib.rs:663-689`); codex gets
`-c mcp_servers.pantheon.url=...` with the token in `PANTHEON_MCP_TOKEN`
(`lib.rs:690-702`); opencode gets `OPENCODE_CONFIG` (`lib.rs:703-724`).
`release_session` (`lib.rs:240-248`) shuts the listener and deletes the
directory.

`dispatch_task` (`src-tauri/src/mcp.rs:968`) wraps the brief with
`dispatch_prompt` (`mcp.rs:73`), refuses 1024 bytes or more
(`MAX_INJECTION_BYTES`, `mcp.rs:39`; `dispatch_precheck`, `mcp.rs:383`),
records the task `pending`, and types it via `submit_to` (`lib.rs:346`). The
wrapper costs 84 plus 113 bytes with a six-character conductor id and a 32-hex
task id, so 826 bytes of brief get through; "about 830" (`BACKLOG.md`,
`mcp.rs:33-36`) is right to the rounding. The task closes only through
`complete_task` (`mcp.rs:1926`, `finish_task` at `mcp.rs:865`) or
`abandon_lost` (`mcp.rs:1089`) when `liveness` (`lib.rs:410`) says the pane's
process is gone. `Task` (`mcp.rs:152-212`) has no exit code, no usage, and no
record of how the work was delivered.

## What this does not do

It does not replace the pane: `SESSION_TYPES` (`src/lib/ipc.ts:34-41`) still
launches every CLI interactively. It covers one host, claude (section 3). It
does not stream the child's transcript live (that needs `stream-json` and a
parser). It leaves `human_dispatch` (`lib.rs:1311`) alone. It does not choose
the model: the pane launches bare today, Phase 3 owns that, and whatever
Phase 3 passes to the pane is passed to the child with `--model`.

## 1. The hybrid shape

A headless task is a second process for an existing pane: same `cwd`, same
environment as `build_command` produces, same `PANTHEON_SESSION`, same MCP
endpoint and token, so its tool calls arrive as that session. It is not on
the pane's PTY; its stdout and stderr are pipes Pantheon reads.

Its output goes to the task drawer, not the pane. The pane's screen belongs to
a full-screen TUI; feeding another process's bytes into that xterm corrupts
it, and shows the text to the human only, never to the pane's agent.
`TaskDrawer.tsx:191-193` already renders `result` for `done` and `error`; a
running headless task adds one line (mode, elapsed), and the roster entry
gains a `headless` marker in `busy_label` (`mcp.rs:1044`).

The CLI's session id is not Pantheon's: `--session-id` needs a UUID and
Pantheon ids are `sess-N`. The binding is the endpoint and `PANTHEON_SESSION`;
the CLI conversation id is a separate fact stored on the task.

## 2. Task lifecycle

`SessionManager` gains `headless: Mutex<HashMap<String, HeadlessChild>>`,
keyed by task id: a `std::process::Child` in its own process group plus the
pane id. A thread per child drains stdout and stderr, then `wait()`s. Exit
maps to task state without `complete_task`:

- Exit 0 and stdout parses as the single JSON object `--output-format json`
  emits, error flag false: `finish_task(target, id, result)` with the JSON's
  result text. Same path as `complete_task`, so a named reviewer still gets
  `in_review` and a waiver still means `done`.
- Anything else (non-zero exit, error flag true, unparsable stdout): `error`,
  `done_ms` stamped, `result` = `exited <code>: ` plus the JSON error text or
  the last 2 KiB of stderr.
- The child called `complete_task` itself (allowed: it is the target):
  `accepts_result` (`mcp.rs:1210`) refuses the second write and the exit
  handler records only `exit_code` and `usage`.
- Killed by cancel, Stop, or pane close: `cancelled`. Killed by the wall-clock
  cap: `error`, reason in `result`.
- Pantheon restarted with the task open: no child is tracked, so a startup
  pass marks it `abandoned`. `reconcile_abandoned` (`mcp.rs:753`) cannot,
  because the pane may be alive.
- Reviewer sends it back: `rework` as today, and Pantheon spawns the same
  command with `--resume <cli_session>` and the findings on stdin.

New `Task` fields, all `serde(default)` for the reason at `mcp.rs:174-176`,
mirrored on `ConductorTask` (`ipc.ts:125-150`):

- `mode: String`: `"pane"` (default, and what every stored task loads as) or
  `"headless"`.
- `exit_code: Option<i32>`.
- `cli_session: String`: the UUID Pantheon generated and passed as
  `--session-id`, known before the child prints anything.
- `usage: Option<Usage>`: `input_tokens`, `output_tokens`,
  `cache_read_tokens`, `cache_write_tokens`, `cost_usd: Option<f64>`,
  `turns`, `duration_ms`, copied from the JSON; what Phase 5 reads. The
  struct is Pantheon's; the CLI's field names come from the measuring run.

## 3. Host: claude first

Command Pantheon runs, with the brief on stdin and `cwd` set to the pane's
directory:

```
claude -p --output-format json --session-id <uuid> \
  --permission-mode acceptEdits --permission-prompts none \
  --max-budget-usd <cap> \
  --mcp-config <app-data>/sessions/<pane>/claude-mcp.json
```

`--mcp-config` stays last (`lib.rs:663-665`) and the file is the pane's own,
unchanged. The brief goes on stdin because argv is readable by every local
process (`lib.rs:639-645`) and briefs quote paths and code.
`--strict-mcp-config` is left off to match the pane (`lib.rs:659-662`).

Why claude:

- Its `--help` documents a single-object JSON result (`--output-format json`),
  a print-mode spend cap (`--max-budget-usd`), and an answer to who approves
  a prompt when nobody is watching (`--permission-prompts none`: anything
  that would prompt is denied). Codex has `--json` (JSONL, a stream to parse)
  and `-o <file>` for the last message, with sandbox flags but no budget.
  OpenCode has `--format json` and nothing for spend or approval beyond
  `--auto`, which its own help marks dangerous.
- OpenCode is the slowest and least reliable target on this machine
  (`BACKLOG.md` lines 572-640: free tier, 50 requests/day shared by every
  pane, variable latency). Its one measured headless win (line 53) is a cold
  local model beating a 30 s pane timeout: a reason to add it second. Codex
  is a sound second host too: `-c` works identically under `exec`, and
  `codex exec resume <id>` exists.

Exit codes are undocumented for all three and must be measured with one real
invocation each; deliberately not done here. The claude run is a trivial
brief through the command above, recording: exit code on success and on a
refused permission, the JSON field names, whether `--max-budget-usd` binds
under subscription auth or only an API key, whether `Bash` is denied under
`acceptEdits` plus `none` (then `--allowedTools` is the next decision), and
that the child reaches the endpoint (have it call `list_sessions`).

## 4. Brief size and path choice

`DispatchArgs` (`mcp.rs:1664`) gains `headless: bool`, default false. The
conductor chooses; Pantheon does not switch on byte count, because the paths
differ in who sees the work and who answers permission prompts, and that
must not change as a side effect of prompt length. With `headless` true,
`dispatch_precheck` skips `oversize_refusal` and instead refuses a target
whose program (`program_of`, `lib.rs:425`) is not claude. The pane refusal
gains one clause: dispatch with `headless: true` if the target supports it.
The wrapper drops the CR/LF flattening and keeps the task id, so
`ask_conductor` works from the child.

The conductor sees `mode: headless` in the dispatch reply, in `render_task`
(`mcp.rs:1404`, plus exit code, usage, and `cli_session`), and in
`render_task_summary` (`mcp.rs:1530`). `get_task_result` and `wait_for_tasks`
need nothing else: the task is `pending` until exit, and exit reaches a
terminal state both already handle.

## 5. Concurrency and the Phase 2 queue

A headless task counts as open work on its target, and the queue holds it as
it holds a pane brief: one open task per target, started by spawning instead
of `submit_to` when its turn comes. The child never touches the PTY, so it
cannot corrupt the pane agent's input, but it shares the worktree and the
identity. Two agents editing one checkout at once is what isolation exists to
prevent, and a child's `complete_task` or `ask_conductor` is
indistinguishable from the pane's. `set_halted` (`mcp.rs:684`) already
cancels `pending` tasks; it must also kill the children. Parallel children in
their own worktrees would be a different target, not this session.

## 6. Risks and bounds

- Environment. The child inherits what the pane inherits (`lib.rs:485-487`),
  which it needs for the CLI's auth; the token is in the config file, never
  on argv. One existing gap to close here: `claude-mcp.json` is written mode
  0644 (observed `-rw-rw-r--` under `sessions/sess-1/`), readable by any
  local user. Write it 0600.
- Token scope. The child is the target session: it can complete or ask on its
  task and cannot dispatch (conductor check `mcp.rs:1896-1904`, self-dispatch
  refusal `mcp.rs:393-394`).
- Turns. `--max-turns` is not in `claude --help` at 2.1.259 (grep count 0),
  and codex and opencode have no turn bound either. The bounds are
  `--max-budget-usd` per task and a wall-clock cap, `HEADLESS_MAX_MS = 2 *
  TASK_OVERDUE_MS` (40 min; `mcp.rs:334`): SIGTERM to the process group, 10 s
  grace, SIGKILL. The group matters because claude's Bash tool spawns
  subprocesses.
- Kill path. Cancel (Phase 1), Stop, pane close in `release_session`, and app
  exit all kill the session's children; without the last two a child keeps
  spending after its pane is gone.
- Quota. Each headless task is a fresh conversation paying the full system
  prompt; `MAX_DISPATCHES` (`mcp.rs:322`, 40) bounds count and the budget
  bounds cost. On opencode the cost would be the account-wide free quota a
  fan-out exhausts: another reason that host is not first.
- Silent denial. `--permission-prompts none` fails closed, so a task can end
  `done` having been refused the tool it needed. The JSON's turn count and
  any denial text land in `result` so the conductor can tell.
