import { useMemo } from "react";
import { type ConductorTask } from "../lib/ipc";
import { groupTasks } from "../lib/tasks";

type ConductorBarProps = {
  conductor: string;
  tasks: ConductorTask[];
  halted: boolean;
  onDemote: () => void;
  onHaltChange: (halted: boolean) => void;
  onOpenDispatch: () => void;
  onOpenTasks: () => void;
  panes: { id: string; type: import("../lib/ipc").SessionType; status: string }[];
};

// Live view of what the conductor is doing, plus the global kill-switch.
// Every dispatch also lands visibly in its target's terminal — this bar is the
// at-a-glance version. The full picture (every open task, untruncated, with
// review/blocked state) lives behind the "Tasks" button in <TaskDrawer>; a
// five-pill feed used to try to fit that here and dropped a sixth task
// entirely (BACKLOG.md).
export function ConductorBar({
  conductor,
  tasks,
  halted,
  onDemote,
  onHaltChange,
  onOpenDispatch,
  onOpenTasks,
  panes,
}: ConductorBarProps) {
  const availableTargets = useMemo(
    () => panes.filter((p) => p.status === "running" && p.id !== conductor && p.type.id !== "shell"),
    [panes, conductor]
  );

  const groups = useMemo(() => groupTasks(tasks), [tasks]);
  const pending = tasks.filter((t) => t.status === "pending").length;
  // Blocked tasks are the ones a human is most likely to need to act on (nudge
  // the conductor pane to answer). Never derived from silence — see
  // src/lib/tasks.ts — only ever true because the backend already set
  // status === "blocked".
  const needsAttention = groups.open.some((t) => t.status === "blocked");
  const openCount = groups.open.length + groups.review.length;

  return (
    <div className="condbar" data-halted={halted}>
      <span className="cond-title">
        ⌁ Conductor <b>{conductor}</b>
      </span>
      <span className="cond-count">
        {pending} pending · {tasks.length} dispatched
      </span>

      <button
        className={"ghost cond-tasks" + (needsAttention ? " attention" : "")}
        onClick={onOpenTasks}
        title="Open every task — pending, in review, rework, blocked, and recent (Ctrl+Shift+T)"
      >
        {needsAttention && <span className="cond-tasks-dot" aria-hidden="true" />}
        Tasks{openCount > 0 ? ` · ${openCount}` : ""}
      </button>

      <div className="spacer" />
      {availableTargets.length > 0 && (
        <button className="ghost" onClick={onOpenDispatch} title="Dispatch a task to an agent">
          Dispatch…
        </button>
      )}
      <button className="ghost" onClick={onDemote} title="Stop being the conductor">
        Demote
      </button>
      <button
        className={"stop" + (halted ? " on" : "")}
        onClick={() => onHaltChange(!halted)}
        title={halted ? "Resume dispatching" : "Halt all dispatch immediately"}
      >
        <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
          <circle cx="12" cy="12" r="9.5" fill="none" stroke="currentColor" strokeWidth="2" />
          <rect x="8.5" y="8.5" width="7" height="7" rx="1.6" fill="currentColor" />
        </svg>
        {halted ? "Resume" : "Stop"}
      </button>
    </div>
  );
}
