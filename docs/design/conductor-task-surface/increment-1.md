# Conductor task surface — increment 1

Task: `7c7f3j`. Source of the problem: `BACKLOG.md` — "The conductor's five-pill
task strip hides the work it is meant to coordinate" (the section starting near
line 629 as of `4c4877b`).

## What's wrong today

`src/components/ConductorBar.tsx:42-60` reverses the task list, keeps five
entries, and truncates every brief to 38 characters. A sixth concurrent task is
not compressed, it is gone from the feed. The backend has moved past this: a
task can be `pending`, `overdue`, `in_review`, `rework`, `blocked`, `done`,
`error`, `cancelled`, or `abandoned` (`src-tauri/src/mcp.rs:157-167`,
`STATUS_BLOCKED` at `mcp.rs:244`), and `in_review`/`rework` are open, not
terminal (`is_open` at `mcp.rs:1454-1466`). The pill strip shows none of that;
every task looks the same until you hover it.

## What this increment does not do

BACKLOG.md refutes the earlier premise that PTY quiet time can identify a
blocked session — silence can mean thinking, a tool waiting on approval, a cold
local model, or a dead process, and naming any of those "blocked" is a
diagnosis Pantheon never observed. `blocked` is a state the backend now sets
explicitly, through `ask_conductor`/`answer_question` (`mcp.rs:2175`,
`mcp.rs:2263`). This increment renders that state; it does not add any timing
heuristic that infers it, and none of the code below reads `last_output` or any
PTY timing signal.

Also out of scope for increment 1, deliberately: answering a blocked task's
question from the GUI. `answer_question` is an MCP tool called by whichever
session holds the conductor role, not a Tauri command — there is no
`#[tauri::command]` for it today (checked `src-tauri/src/lib.rs`). Increment 1
makes a blocked task visible and lets the human focus that pane; wiring an
in-app answer control is follow-up work, not folded in here to keep this
change reviewable.

Board-vs-drawer was an open question in BACKLOG.md. This increment builds a
drawer: opened on demand from the conductor bar, closed by default. The
terminal panes are still the product (README's own framing); a drawer keeps
that space until a conductor asks for the detail, where a permanent board
would take it unconditionally.

## Shape

- `src/lib/tasks.ts` — pure grouping logic, no React, no timing: `groupTasks`
  sorts tasks into `open` (pending, overdue, rework, blocked — blocked first),
  `review` (in_review), and `recent` (done, error, cancelled, abandoned,
  newest-first, bounded to `RECENT_LIMIT` with a count of how many were
  dropped so the bound is never silent). Mirrors `is_open`/`is_terminal` from
  `mcp.rs` by status string only — it has no access to anything else and
  should never gain one.
- `src/components/TaskDrawer.tsx` — the surface itself. A backdrop + panel
  (same pattern as `SettingsPanel`/`DispatchDialog`), list items are real
  `<button>`s so Tab/Shift+Tab and Enter/Space work without extra handling,
  Escape closes. Selecting an item calls `onFocusPane`, which reuses
  `App.tsx`'s existing `setFocusedPane` — the same maximize-one-pane mechanism
  Ctrl+Shift+Enter already drives. A task whose target pane has since closed
  shows as such instead of silently doing nothing.
- `src/components/ConductorBar.tsx` — the 5-pill `.cond-feed` is replaced with
  a single "Tasks" button carrying an open-task count, opening the drawer.
- `src/App.tsx` — owns `taskDrawerOpen` state and a new shortcut,
  Ctrl+Shift+T, added to the existing Ctrl+Shift chord group (chosen because
  plain Ctrl+T is unclaimed by any of the three agent CLIs, same rationale as
  the rest of the block at `App.tsx:315-317`).
- `src/lib/ipc.ts` — `ConductorTask` gains `reviewer`, `findings`, and
  `exchanges` (an `Exchange[]`), matching fields the backend already
  serializes on `Task` (`mcp.rs:170-206`) but that the frontend type never
  declared.

## Tests

`src/lib/tasks.test.ts` covers the two properties `done_when` names explicitly:
a sixth concurrent open task is still present in `groupTasks(...).open` (not
dropped the way the old five-pill feed dropped it), and status classification
is a pure function of `task.status` alone — feeding it every terminal/open
status plus one `blocked` task proves nothing is ever bucketed as blocked
except the one task the backend already marked that way, with no timing input
anywhere in the function's signature.
