// Grouping for the conductor task surface (TaskDrawer). Deliberately pure and
// deliberately narrow: it sorts `ConductorTask`s by the `status` string the
// backend already decided, and nothing else.
//
// BACKLOG.md ("The conductor's five-pill task strip hides the work it is
// meant to coordinate") refuted using PTY/terminal silence to infer a blocked
// session — the same silence can mean thinking, a tool waiting on approval, a
// cold local model, or a dead process. `blocked` only exists here because
// `src-tauri/src/mcp.rs` sets it explicitly, through `ask_conductor` /
// `answer_question`. This module must never grow a timing- or output-based
// signal of its own; it only reads `status`.
import { type ConductorTask, type Exchange } from "./ipc";

// The delivery half of the lifecycle, from
// docs/design/coordination-delivery/increment-1.md. Each state claims exactly
// what Mosaic has observed and nothing more:
//
//   queued     the dispatch is recorded; nothing has been written to a
//              terminal yet.
//   submitted  the prompt and its separate Enter both reached the terminal.
//              This is not evidence that the model read either of them.
//   accepted   the named target acknowledged the task over MCP. This is the
//              first state that counts as active agent work.
//
// All three are open: none is a finish, and none may be inferred from terminal
// output or silence, for the reason in this module's header comment.
export const DELIVERY_STATUSES = ["queued", "submitted", "accepted"] as const;

// Open work, ordered the way the drawer's "Open" section lists it. The order
// ranks by how stuck the work is, not by where it sits in the lifecycle:
//
//   blocked    a human has to answer before anything moves
//   rework     a reviewer sent it back
//   overdue    past its reporting window, still running (legacy signal)
//   submitted  delivered and never acknowledged. The state this increment
//              exists to expose, because it can expire into abandoned.
//   pending    records written before the lifecycle existed. Mapped
//              conservatively: delivered, acknowledgement unknown.
//   queued     recorded but not delivered yet, so nothing has gone wrong.
//   accepted   a named agent said it is working. Least in need of attention.
//
// This is the ranking of open work, not the test for it: groupTasks decides
// what is open by elimination (see below), so a status missing from this list
// still shows up, it just sorts last. `pending` stays because old task records
// still carry it.
export const OPEN_STATUSES = [
  "blocked",
  "rework",
  "overdue",
  "submitted",
  "pending",
  "queued",
  "accepted",
] as const;

export const REVIEW_STATUS = "in_review";
// Mirrors `is_terminal` at src-tauri/src/mcp.rs:224-232.
export const TERMINAL_STATUSES = ["done", "error", "cancelled", "abandoned"] as const;

// How many finished tasks the "Recent" group keeps. Bounded for the same
// reason `get_task_result` had to be (BACKLOG.md: "outgrows its own response
// limit") — unbounded terminal history regrows that same problem here.
export const RECENT_LIMIT = 8;

// Within "open", surface the states that need a human's attention first, in
// the order OPEN_STATUSES declares. A status missing from the map sorts last
// rather than throwing.
const OPEN_PRIORITY: Record<string, number> = Object.fromEntries(
  OPEN_STATUSES.map((s, i) => [s, i]),
);

export type TaskGroups = {
  open: ConductorTask[];
  review: ConductorTask[];
  recent: ConductorTask[];
  // How many terminal tasks exist beyond `recent`. Shown in the UI rather
  // than dropped silently — a bound the user can't see is indistinguishable
  // from data loss.
  recentHiddenCount: number;
};

function byStatus(tasks: ConductorTask[], statuses: readonly string[]): ConductorTask[] {
  const set = new Set<string>(statuses);
  return tasks.filter((t) => set.has(t.status));
}

const REVIEW_SET = new Set<string>([REVIEW_STATUS]);
const TERMINAL_SET = new Set<string>(TERMINAL_STATUSES);

export function groupTasks(tasks: ConductorTask[]): TaskGroups {
  // Open is what is left over, not a fixed list. The backend's vocabulary moves
  // ahead of this file, because the Rust half of the dispatch lifecycle lands as
  // its own change, and a status this module has not heard of yet has to surface
  // as unfinished work rather than vanish from all three groups: a task that
  // silently disappeared is the bug this drawer was built to end. So only
  // "finished" and "awaiting review" are claimed positively, both mirroring the
  // backend's own predicates, and everything else is open.
  const open = tasks
    .filter((t) => !REVIEW_SET.has(t.status) && !TERMINAL_SET.has(t.status))
    .sort((a, b) => {
      const pa = OPEN_PRIORITY[a.status] ?? OPEN_STATUSES.length;
      const pb = OPEN_PRIORITY[b.status] ?? OPEN_STATUSES.length;
      if (pa !== pb) return pa - pb;
      return b.ts_ms - a.ts_ms; // newest first within one priority tier
    });

  const review = byStatus(tasks, [REVIEW_STATUS]).sort((a, b) => b.ts_ms - a.ts_ms);

  const terminal = byStatus(tasks, TERMINAL_STATUSES).sort(
    (a, b) => (b.done_ms ?? b.ts_ms) - (a.done_ms ?? a.ts_ms),
  );

  return {
    open,
    review,
    recent: terminal.slice(0, RECENT_LIMIT),
    recentHiddenCount: Math.max(0, terminal.length - RECENT_LIMIT),
  };
}

// Shared wording so the drawer (and anything built on top of it later) agree
// on what each status is called.
export const STATUS_LABEL: Record<string, string> = {
  queued: "Queued",
  submitted: "Submitted",
  accepted: "Accepted",
  pending: "Pending",
  overdue: "Overdue",
  rework: "Rework",
  blocked: "Blocked",
  in_review: "In review",
  done: "Done",
  error: "Error",
  cancelled: "Cancelled",
  abandoned: "Abandoned",
};

export function statusLabel(status: string): string {
  return STATUS_LABEL[status] ?? status;
}

// The open question on a blocked task, if any. Mirrors the backend's own
// invariant (mcp.rs: "the last entry with an empty answer is the open
// question; there is at most one, because asking blocks").
export function openQuestion(task: ConductorTask): Exchange | undefined {
  for (let i = task.exchanges.length - 1; i >= 0; i--) {
    if (task.exchanges[i].answer === "") return task.exchanges[i];
  }
  return undefined;
}
