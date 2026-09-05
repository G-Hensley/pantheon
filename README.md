# Pantheon

**A desktop cockpit for running several AI coding agents side by side, coordinated instead of siloed.**

Each agent gets a live terminal pane. Panes can be grouped into a shared
context store so one agent's decision becomes another's knowledge without a
person relaying it by hand, and any pane can be promoted to conductor to fan
work out to the rest and collect the results.

**Status:** working prototype. The session engine, shared context, worktree
isolation, and conductor all function, but see [Known gaps and limitations](#known-gaps-and-limitations)
before relying on it. Runs on Windows and Linux: the terminal layer is
`portable-pty`, which is ConPTY on Windows and a Unix PTY elsewhere. macOS is
unexercised rather than ruled out; nothing in the code targets Windows or Linux
specifically, but nobody has run it there.

## Why it exists

Running Claude Code, Codex, and opencode in parallel panes is easy. Getting
them to act like a team instead of three strangers duplicating each other's
work is the actual problem: nothing tells one agent what another just
decided, and nothing lets a person hand out tasks without babysitting every
pane. Pantheon answers both. A shared MCP server lets agents record decisions
and read each other's, git worktree isolation lets them edit the same repo
without clashing, and a conductor role lets one pane fan tasks out to the
others and collect what comes back.

## Capabilities

- Runs Claude Code, Codex, opencode, or a plain shell (PowerShell on Windows,
  bash on Linux) in parallel panes, each on its own pseudo-terminal.
- Connects agents to a **shared brain**, an in-process MCP server they use to
  record decisions and facts, broadcast, and read what the others have
  decided.
- Groups panes into **brains**: agents in the same brain share context, agents
  in different brains are isolated from each other. Drag a pane to re-home it.
- **Isolates** a session in its own git worktree and branch, so parallel
  agents editing one repo never clash.
- Promotes one pane to **conductor**, which can hand tasks to other sessions
  and collect their results.

## 60-second demo

1. Open Pantheon and pick the git repo you want agents working in.
2. Press **Ctrl+Shift+K** and launch two or three sessions (a mix of Claude
   Code, Codex, or opencode). Isolate is on by default, so each gets its own
   worktree and branch.
3. Drag pane headers together to put them in the same brain. They can now see
   each other's recorded decisions.
4. Click **⌁** on one pane to promote it to conductor, then type a task and
   press Enter. The conductor dispatches work to the other panes by typing
   into their visible terminals, so every instruction is something you can
   watch happen.
5. Watch the conductor bar for task pills as work completes, and check the
   context sidebar for the decisions agents recorded along the way.

## Quick start

On Windows, install `Pantheon_0.1.0_x64-setup.exe` and launch from the Start
Menu, or run from a checkout:

```powershell
pnpm install
.\dev.cmd        # sets up the MSVC environment, then `pnpm tauri dev`
```

On Linux, run from a checkout:

```bash
pnpm install
./dev.sh         # thin wrapper for `pnpm tauri dev`
```

To build and register Pantheon in the Linux app grid:

```bash
./install-desktop.sh
```

When upgrading from Mosaic, the installer safely adopts the latest packaged
UI state, retires the old launcher, and preserves legacy worktree and context
data. Quit Mosaic or Pantheon before running it so WebKit can close its state
database cleanly.

To build the bundles, use `.\build.cmd` on Windows or `./build.sh` on Linux.
Artifacts land under `src-tauri/target/release/`:

| Path | What it is |
|---|---|
| `pantheon.exe` | Windows standalone, no install needed |
| `bundle/nsis/Pantheon_0.1.0_x64-setup.exe` | Windows installer with a Start Menu entry |
| `pantheon` | Linux standalone |
| `bundle/deb/`, `bundle/appimage/` | Linux packages |

Requires Rust and pnpm on both platforms, plus the Visual Studio 2022 Build
Tools on Windows (both `.cmd` scripts call `vcvars64.bat`) or the GTK and
WebKitGTK development packages on Linux. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the full setup and test workflow.

## Usage

1. **Pick project** in the title bar: the git repo agents work in. It is
   remembered between runs, and the shared brain writes its notes to that
   project's `.pantheon/context/`.
2. **Ctrl+Shift+K** opens the launcher. Pick a session type. *Isolate* is on
   by default, giving that session its own worktree and branch.
3. **Drag a pane header** onto another pane, or onto a brain in the sidebar,
   to put them in the same brain.
4. **⌁ on a pane** promotes it to conductor. Dispatched tasks are typed into
   the target's visible terminal, so you see every instruction, once the
   target is free to receive it: a busy pane gets the brief queued instead
   (see Guardrails). **Stop** halts all dispatch immediately. **Tasks** (or
   **Ctrl+Shift+T**) opens every task (queued, pending, overdue, in review,
   rework, or blocked on your answer) without truncating any of them, plus a
   bounded recent-history tail; picking one focuses its pane.
5. **Layout: scroll** keeps every terminal at a comfortable minimum height and
   scrolls the cockpit. Open **Layout** to choose automatic or fixed 1 to 6
   column arrangements and a minimum pane height, or switch to **Fit window**
   when you want every pane visible at once. **Maximize** expands one pane
   without stopping or remounting the others; use **Restore** to return to the
   grid.
6. **Ctrl+Shift+B** toggles the sidebar; **Ctrl+Shift+,** opens appearance
   settings; **Ctrl+Shift+K** opens the session launcher. The Shift modifier
   keeps common terminal controls such as Ctrl+B and Ctrl+K available to
   agents. **Ctrl+Shift+1…9** focuses a session; **Ctrl+Shift+Enter** maximizes
   or restores the active terminal; **Ctrl+Shift+T** opens the conductor's task
   list.

## How agents connect

Every session gets its own MCP endpoint on a random loopback port, wired in at
launch through arguments and environment only:

| Session | Mechanism |
|---|---|
| Claude Code | `--mcp-config <per-session file>` (additive; your other MCP servers still load) |
| Codex | `-c mcp_servers.pantheon.url=…` |
| opencode | `OPENCODE_CONFIG=<per-session file>` (merged over your global config) |
| Shell | none |

Pantheon never writes to your global agent config. Because a port is only ever
handed to one session, Pantheon knows which agent is calling from the connection
alone: the agent never declares a name and cannot claim another's.

Agents get these tools: `record_decision`, `record_fact`, `broadcast`,
`get_shared_context`, `search_context`, `list_sessions`, `dispatch` (conductor
only), `cancel_task` (conductor only), `reassign_task` (conductor only),
`complete_task`, `review_task`, `get_task_result`, `wait_for_tasks`,
`ask_conductor`, `answer_question` (conductor only), `set_session_identity`.
`cancel_task` closes any open task with a reason, several at once if you name
them together, queued tasks included: nothing is journaled for a queued task
that never got dispatched, the same as any other refusal. `reassign_task`
moves a pending, overdue, queued, or already-abandoned task to a new live
target (redelivering the brief, or re-queuing it if that target is itself
busy), or hands an in_review or rework task to a new live reviewer, so a
session that is stuck, gone, or already given up on does not have to leave
the work stranded. `review_task` changes the task's record and delivers the
result: approving frees the reviewer's own pane for its queue, and rejecting
types a rework notice straight into the target's terminal, so the conductor
does not have to relay what the review found by hand. `get_task_result` is no
longer dispatcher-only: the task's target and reviewer may also read it by
full id, since both are told to call it by the notice they receive; a
no-id listing of every dispatched task still returns only to the dispatcher.

## How agents are briefed

Tools alone don't change behaviour. An agent that isn't told the other panes
are usable capacity will quietly do everything itself. Pantheon briefs agents on
two channels, because they answer different questions:

| Channel | Delivered | Says |
|---|---|---|
| MCP server instructions | Once, on connect | You are in Pantheon, here is the shared brain, here is what the conductor role means if you're given it |
| Composer prefill | On promotion to conductor | You are the conductor *now*, here are the live sessions by name and model |

The prefill is typed into the pane's input but **not sent**. Add your first
instruction after it and press Enter, and the agent gets its role and its task
together. Nothing is dispatched or spent until you do. It stays short for that
reason; the detail lives in the connect-time instructions instead.

All three agent CLIs surface the connect-time instructions to their model,
verified by asking a live Claude Code, Codex, and opencode session to quote
the first sentence back.

The second exists because MCP hands a client its instructions once, at connect
time, while the conductor is chosen by the user later and can change mid-run.
An agent promoted at minute ten has never been told it now commands the
workspace, so the role change is typed into its terminal the moment it
happens; it is the only channel that reaches an already-running agent.

Dispatch returns a `task_id` immediately rather than blocking, so a conductor
is meant to fan several tasks out and then collect them; `get_task_result`
with no `task_id` returns every open task plus the most recent finished ones
(`RECENT_FINISHED` in `src-tauri/src/mcp.rs`), with the whole history behind
an `include_all` flag.

`wait_for_tasks` is the other half. It blocks until the ids it is given reach a
terminal state, then returns the same results, so a conductor with nothing else
queued does not have to guess a polling interval. It defaults to 45 seconds and
is capped at 55, sized to the host's own MCP transport rather than to how long a
task actually takes: measured 2026-09-03, a Claude Code pane's transport kills
the call somewhere between 45 and 110 seconds, so a call this size is one in a
short series, not the whole wait. A timeout says so explicitly and cancels
nothing: the agents keep working and their results are still accepted, so
calling `wait_for_tasks` again with the same ids is the normal way to keep
waiting. A pane that dies mid-wait ends the wait rather than holding it open,
because its task becomes `abandoned`, a terminal state like any other finish.
That does not cover a task already `in_review`: the submitted work already
exists, so its target dying does not abandon it, and a dead reviewer is
flagged on the task rather than ending it. A wait on that task instead runs to
its own timeout, whose message lists every task still open, flag included, so
a stuck review is visible even though nothing has finished it.

Dispatch used to be one-way, so an agent that hit a genuine ambiguity mid-task
could only guess, stall, or ask the human in its own terminal. With five panes
working at once, that made the human the synchronisation point for questions
they had not asked and lacked the context to answer, which is the exact cost
conducting was supposed to remove. `ask_conductor` routes the question to the
agent that wrote the brief instead. The task shows as `blocked`, which is open
but not progressing, and `wait_for_tasks` returns early when one appears so the
conductor answers rather than waiting on a pane that is waiting on it. Bounded
at five questions per task, and a question that goes unanswered tells the agent
to use its own judgement and state the assumption rather than stalling. The
exchange is kept on the task, so an answer given once is not asked again.

## Guardrails

- Only the conductor can dispatch, and the app assigns that role. An agent
  cannot claim it. Depth is bounded structurally: a dispatched agent is not
  the conductor, so it cannot dispatch onward.
- 40 dispatches per run, and a Stop that cancels everything still pending.
  A task older than 20 minutes is relabelled overdue, but that is a reporting
  signal only: nothing is terminated, the agent process keeps running, and a
  late result is still accepted.
- A task whose target pane's process has exited is marked `abandoned`, which is
  terminal and distinct from `cancelled`, which a human chose. Liveness is
  checked when the roster is read, when work is dispatched, and when results are
  collected, so a conductor is never offered a dead pane and never waits on one.
  A pane that fails its liveness probe for any other reason is reported alive:
  being slow to notice a death costs a wait, being wrong about one costs the
  result.
- An unnamed reviewer prefers a live session running a different CLI kind than
  the target, falling back to any other live session only when no such
  candidate exists. That is `CONTRIBUTING.md`'s cross-model review rule
  applied automatically rather than left to the conductor to remember; naming
  a reviewer explicitly is unaffected by it.
- A pane is occupied while it holds an open task as target (pending, overdue,
  rework, or blocked) or as reviewer (in_review). Dispatching to an occupied
  pane queues the brief instead of typing over whatever the pane is already
  doing; the response says which task it is queued behind and at what
  position. Each pane holds at most 3 queued briefs; a fourth is refused,
  naming the ones already queued, and nothing is journaled for a refusal.
  `list_sessions` shows the depth on a busy pane, for example
  `[busy 4m, 2 queued]`.
- Whenever a pane stops being occupied, whatever is next for it (an
  undelivered review request or rework notice first, then the oldest queued
  brief) is typed in automatically: no broadcast, no second dispatch, and
  nothing is typed while the pane is still occupied. Nothing is delivered
  while halted; unhalting resumes it. An oversized brief or review finding is
  shortened to fit rather than refused, since a system-generated notice has
  no caller to refuse to; the full text stays reachable by task id.
- A worktree branch is deleted only when it has no commits of its own, so
  committed agent work is never silently discarded. Cleanup also refuses to
  remove a dirty worktree, preserving uncommitted changes on disk.

## Project structure

| Path | Contents |
|---|---|
| `src/` | React UI: panes, brains sidebar, conductor bar, theming |
| `src-tauri/src/lib.rs` | PTY session engine and Tauri commands |
| `src-tauri/src/mcp.rs` | The shared brain: MCP server, context store, dispatch |
| `src-tauri/src/worktree.rs` | Git worktree isolation |
| `ui-gallery/` | Standalone design explorations, not part of the build |

## Known gaps and limitations

- **Isolated work has no merge path.** A session's branch (`pantheon/<id>-<uid>`)
  survives when it has commits, but the UI never shows the branch name or a
  diff.
- **A finished agent can leave its task open.** Completion is an explicit
  `complete_task` call, so a session that does the work and never makes that
  call is indistinguishable, to the conductor, from one still thinking. A pane
  whose *process* has exited is now handled: its open tasks reach a terminal
  `abandoned` status and `list_sessions` marks it `DEAD`, so nothing waits on
  work nobody is doing. A pane that is alive but has quietly stopped answering
  is still indistinguishable from one that is thinking hard, and deliberately
  so: silence is not evidence, and guessing would discard real work.
- **Dirty worktree recovery is manual.** Closing a session refuses to remove
  its dirty worktree, preserving uncommitted edits on disk, but the UI does
  not yet show the preserved path or offer a recovery workflow.
- The markdown mirror under `.pantheon/context/` is written for humans to read
  and is never read back. The shared context itself does persist: entries,
  sessions, and tasks are rehydrated from `brain.jsonl` when the app opens a
  project.
- Fit layout is intended for at most 6 panes; scroll layout remains usable
  beyond that.
- Frontend coverage is thin: `pnpm test` runs Vitest over a small set of
  cases, well short of the UI as a whole. The Rust side is covered by unit
  tests in `lib.rs`, `mcp.rs`, and `worktree.rs`.
  `src-tauri/tests/pty_truncation.rs` is a ConPTY measurement harness rather
  than a regression test, is Windows-only, and is ignored by default and run
  deliberately (see its header for the command).
- Single-user, single-machine threat model. See [SECURITY.md](SECURITY.md)
  for what that means in practice before pointing Pantheon at anything
  sensitive.

## Verification

```bash
cd src-tauri
cargo test        # Rust unit tests: PTY engine, shared brain, worktree isolation
cd ..
pnpm build        # tsc in strict mode, then the Vite production build
pnpm test         # Vitest frontend tests
```

CI runs both on every pull request (see [CONTRIBUTING.md](CONTRIBUTING.md)),
along with `cargo fmt --check`, `cargo clippy`, and dependency audits. Run them
locally first so review starts from a green branch.

## Contributing and security

Contributions are welcome; see [CONTRIBUTING.md](CONTRIBUTING.md) for the
build, test, and branching workflow. For vulnerability reports, see
[SECURITY.md](SECURITY.md) rather than opening a public issue.

## License

[MIT](LICENSE)
