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

// Mirrors `is_open` at src-tauri/src/mcp.rs:1454-1466, minus "in_review",
// which gets its own group below (BACKLOG.md: "grouped into open, awaiting
// review, and terminal work").
export const OPEN_STATUSES = ["blocked", "rework", "overdue", "pending"] as const;
export const REVIEW_STATUS = "in_review";
// Mirrors `is_terminal` at src-tauri/src/mcp.rs:224-232.
export const TERMINAL_STATUSES = ["done", "error", "cancelled", "abandoned"] as const;

// How many finished tasks the "Recent" group keeps. Bounded for the same
// reason `get_task_result` had to be (BACKLOG.md: "outgrows its own response
// limit") — unbounded terminal history regrows that same problem here.
export const RECENT_LIMIT = 8;

// Within "open", surface the states that need a human's attention first.
// blocked > rework > overdue > pending. Anything not in this map (there is
// nothing today) sorts last rather than throwing.
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

export function groupTasks(tasks: ConductorTask[]): TaskGroups {
  const open = byStatus(tasks, OPEN_STATUSES).sort((a, b) => {
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
