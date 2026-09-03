# Headless dispatch: design

Source: `BACKLOG.md` lines 53 (dispatch headlessly) and 209 (dispatch loses
1 KiB chunks). Plan: Phase 4 in `docs/plans/2026-09-03-pantheon-repair.md`.
Line numbers are at `2907adf`, except those marked `main`, at `bb9ec1a` (the
Phase 3 merge).
CLI flags: `--help` on 2026-09-03 (claude 2.1.259, codex-cli 0.153.0,
opencode 1.18.27); no CLI was run with a prompt.

## What exists today

A pane is a PTY child built by `build_command` (`src-tauri/src/lib.rs:453`)
with `PANTHEON_SESSION` set (`lib.rs:1069`). Each session tries to start its
own MCP listener and bearer token (`lib.rs:1001-1013`), so a request on that
port is that session; if the listener fails, the session falls back to the
shared endpoint with no token and self-declares its identity
(`lib.rs:1015-1025`). `agent_mcp_wiring` (`lib.rs:647-724`) hands claude
`--mcp-config`, appended last because the flag is variadic
(`lib.rs:663-689`). `SessionHandle` (`lib.rs:28-45`) keeps the PTY handles,
worktree, `last_output`, program, and server; `session_cwd` and the final
`args` are locals of `spawn_session` (`lib.rs:984-1069`).

`dispatch_task` (`src-tauri/src/mcp.rs:968`) wraps the brief
(`dispatch_prompt`, `mcp.rs:73`), refuses 1024 bytes or more
(`MAX_INJECTION_BYTES`, `mcp.rs:39`; `dispatch_precheck`, `mcp.rs:383`),
and types it via `submit_to` (`lib.rs:346`). The task closes only through
`complete_task` (`mcp.rs:1926`, `finish_task` at `mcp.rs:865`) or
`abandon_lost` (`mcp.rs:1089`) when `liveness` (`lib.rs:410`) says the pane
is gone. `Task` (`mcp.rs:152-212`) records no exit code, usage, or delivery
mode.

## What this does not do

`SESSION_TYPES` (`src/lib/ipc.ts:34-41`) still launches every CLI
interactively. This design covers one host, claude (section 3), does not
stream the transcript live, and leaves `human_dispatch` (`lib.rs:1311`)
alone. It does not choose the model: Phase 3 (`main`, `lib.rs:997-1001` and
`1125-1127`) passes `model` and `model_flag` to the pane, and the child gets
the same pair.

## 1. The hybrid shape

A headless task is a second process for an existing pane: same `cwd`,
environment, `PANTHEON_SESSION`, endpoint, and token, so its tool calls arrive
as that session. Its stdout and stderr are pipes Pantheon reads into the task
drawer, not the PTY: foreign bytes corrupt a TUI, and screen text never
reaches the pane's agent. `TaskDrawer.tsx:191-193` renders `result`; a
running headless task adds mode and elapsed, and `busy_label`
(`mcp.rs:1044`) gains a `headless` marker.

`--session-id` needs a UUID and Pantheon ids are `sess-N`, so the CLI
conversation id is a separate fact stored on the task.

### What the process owner keeps

Nothing on `SessionHandle` can rebuild the pane's command, so `spawn_session`
stores a `LaunchSpec` on the handle before its locals die: `cwd` (the resolved
`session_cwd`); `program` and `args` as launched, including the Phase 3
`model_flag` and `model` pair and the wiring `agent_mcp_wiring` returned;
`env` (the wiring's `extra_env` plus `PANTHEON_SESSION`); and `endpoint`,
`Dedicated { url, token }` or `Shared`. The child's command is
`build_command(program, headless_args, cwd, env)` with the spec's wiring
appended last, never the pane's argv, so child and resume see exactly the
pane's endpoint, model, and environment.

## 2. Task lifecycle

`SessionManager` gains `headless: Mutex<HashMap<String, HeadlessChild>>`,
keyed by task id: the child behind the `ProcessTree` trait (section 6) plus
the pane id. One reader thread per pipe: a single thread draining stdout to
EOF deadlocks once the child fills the 64 KiB stderr pipe. stdout is
collected whole (one JSON object), stderr into a 2 KiB ring buffer; `wait()`
follows both readers. Exit maps to task state without `complete_task`:

- Exit 0 and stdout parses as the single JSON object `--output-format json`
  emits, error flag false: `finish_task(target, id, result)`, the
  `complete_task` path, so review works as today.
- Anything else: `error`, `done_ms` stamped, `result` = `exited <code>: `
  plus the JSON error text or the stderr buffer.
- The child called `complete_task` itself (allowed: it is the target):
  `accepts_result` (`mcp.rs:1210`) refuses the second write; the exit handler
  records only `exit_code` and `usage`.
- Killed by cancel, Stop, or pane close: `cancelled`. Killed by the wall-clock
  cap: `error`, reason in `result`.
- Pantheon restarted with the task open: a startup pass marks it
  `abandoned` (`reconcile_abandoned`, `mcp.rs:753`, cannot: the pane may be
  alive).
- Reviewer sends it back: `rework` as today, and Pantheon spawns the spec's
  command with `--resume <cli_session>` and the findings on stdin.

New `Task` fields, `serde(default)` for the reason at `mcp.rs:174-176` and
mirrored on `ConductorTask` (`ipc.ts:125-150`): `mode: String` (`"pane"`,
the default, or `"headless"`); `exit_code: Option<i32>`;
`cli_session: String`, the UUID passed as `--session-id`; and
`usage: Option<Usage>` (tokens, cache, `cost_usd: Option<f64>`, turns,
duration; what Phase 5 reads).

## 3. Host: claude first

The command, brief on stdin, `cwd` from the spec:

```
claude -p --output-format json --session-id <uuid> \
  --permission-mode acceptEdits --permission-prompts none \
  --max-budget-usd <cap> \
  --mcp-config <app-data>/sessions/<pane>/claude-mcp.json
```

The brief goes on stdin because argv is readable by every local process
(`lib.rs:639-645`) and briefs quote paths and code.

Why claude: its `--help` documents a single-object JSON result, a print-mode
spend cap, and `--permission-prompts none`, which denies anything that would
prompt when nobody is watching. Codex (`--json` is JSONL; no budget flag) is
a sound second host: `-c` works under `exec` and `codex exec resume <id>`
exists. OpenCode has only `--auto` for approval (its help marks it
dangerous) and is the least reliable target here (`BACKLOG.md` 572-640).

Exit codes are undocumented for all three and are measured by one real
invocation each, deliberately not done here. The claude run records: exit
code on success and on a refused permission, the JSON field names, whether
`--max-budget-usd` binds under subscription auth, whether `Bash` is denied
under `acceptEdits` plus `none`, and that the child reaches the endpoint
(have it call `list_sessions`).

## 4. Brief size and path choice

`DispatchArgs` (`mcp.rs:1664`) gains `headless: bool`, default false. The
conductor chooses; Pantheon never switches on byte count, because the paths
differ in who sees the work and who answers permission prompts. With
`headless` true, `dispatch_precheck` skips `oversize_refusal` and refuses
two targets instead: a program (`program_of`, `lib.rs:425`) that is not
claude, and a pane whose `LaunchSpec.endpoint` is `Shared`, since a
self-declared identity cannot attribute a child's `complete_task` to the
target. That reply reads `refused: <target> has no per-session
endpoint; restart the pane or send a pane brief`. `oversize_refusal` now
offers `headless: true`. The wrapper drops the CR/LF flattening and keeps
the task id, so `ask_conductor` works from the child.

The conductor sees `mode: headless` in the dispatch reply, `render_task`
(`mcp.rs:1404`, plus exit code, usage, and `cli_session`), and
`render_task_summary` (`mcp.rs:1530`). `get_task_result` and `wait_for_tasks`
need nothing else.

## 5. Concurrency and the Phase 2 queue

A headless task is open work on its target: the Phase 2 queue holds one open
task per target and starts it by spawning instead of `submit_to`. The child
shares the worktree and the identity, so two open tasks would be two agents
in one checkout with indistinguishable `complete_task` calls. `set_halted`
(`mcp.rs:684`) cancels `pending` tasks; it must also kill the children.

The ledger knows only dispatches: a human typing into the pane creates no
task, so nothing would stop a child starting while the pane's agent edits
the same checkout. The signal is `last_output` (`lib.rs:37`), stamped by the
reader thread on every byte the pane emits (`lib.rs:1104-1113`). A headless
start requires the pane quiet for `HEADLESS_QUIET_MS = 30 s`, far above
`SUBMIT_QUIET_MS` (`lib.rs:78`, 200 ms), which measures paste absorption,
not idleness. An active pane does not refuse the dispatch: the task stays
`pending`, the reply says `queued: <target> active <n>s ago, starts after
30 s quiet`, and the clock is rechecked at each start attempt. Limitation: a
pane thinking silently or blocked on a tool looks idle, so the window is a
delay, not a lock. Still the right default: the hazard is at the checkout,
the alternative is no signal, and the human sees the queued task in the
drawer and can cancel it.

## 6. Risks and bounds

- Environment. The child inherits what the pane inherits (`lib.rs:485-487`).
  `claude-mcp.json`, which holds the token, is written 0644 (observed under
  `sessions/sess-1/`); write it 0600.
- Token scope. The child is the target session: it can complete or ask on its
  task and cannot dispatch (conductor check `mcp.rs:1896-1904`, self-dispatch
  refusal `mcp.rs:393-394`).
- Turns and time. No CLI has a turn bound (`--max-turns` is not in
  `claude --help` at 2.1.259). The bounds are `--max-budget-usd` per task and
  a wall-clock cap, `HEADLESS_MAX_MS = 2 * TASK_OVERDUE_MS` (40 min;
  `mcp.rs:334`).
- Kill path. Claude's Bash tool forks, so the whole tree must die. A
  `ProcessTree` trait (`spawn`, `terminate`, `kill`) has two implementations.
  Unix: `CommandExt::process_group(0)` makes the child a group leader;
  `terminate` is SIGTERM to the group, `kill` is SIGKILL after 10 s. Windows
  (`lib.rs:464`): a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, the
  child assigned right after spawn;
  both calls are `TerminateJobObject`, and closing the last job handle kills
  the tree, so a Pantheon crash cannot leak it. Cancel (Phase 1), Stop, pane
  close in `release_session` (`lib.rs:240`), and app exit (a `RunEvent::Exit`
  pass over `headless`) all go through the trait.
- Quota. Each task is a fresh conversation paying the full system prompt;
  `MAX_DISPATCHES` (`mcp.rs:322`, 40) bounds count and the budget bounds
  cost.
- Silent denial. `--permission-prompts none` fails closed, so a task can end
  `done` after being refused the tool it needed; the JSON's turn count and
  denial text land in `result` so the conductor can tell.
