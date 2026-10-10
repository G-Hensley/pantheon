import { useEffect, useMemo, useRef } from "react";
import { type ConductorTask, type SessionType, type TaskActivity, type SessionRequest } from "../lib/ipc";
import { groupTasks, openQuestion, statusLabel, taskActionPane, activityLabel, type TaskGroups } from "../lib/tasks";

import { FleetOverview } from "./FleetOverview";

type PaneRef = { id: string; type: SessionType; status: string };

type TaskDrawerProps = {
  tasks: ConductorTask[];
  activity?: TaskActivity[];
  requests?: SessionRequest[];
  onOpenRequests?: () => void;
  panes: PaneRef[];
  onClose: () => void;
  // Focuses (maximizes) the task's target pane — the same mechanism
  // Ctrl+Shift+Enter and the pane's own Maximize button already drive.
  onFocusPane: (id: string) => void;
};

// The full, truthful replacement for the old five-pill `.cond-feed`: every
// open task, none of them truncated, grouped the way BACKLOG.md described —
// open work, work awaiting review, and a bounded recent-history tail — with
// pending/overdue/rework/blocked each carrying their own visible status pill.
//
// Every list item is a real <button>, so Tab / Shift+Tab and Enter / Space
// work without any extra keyboard wiring, matching the rest of the app's
// dialogs (SettingsPanel, DispatchDialog): no focus trap, Escape closes.
export function TaskDrawer({ tasks, panes, activity = [], requests = [], onOpenRequests, onClose, onFocusPane }: TaskDrawerProps) {
  const closeRef = useRef<HTMLButtonElement>(null);
  const groups = useMemo(() => groupTasks(tasks), [tasks]);
  const paneMap = useMemo(() => {
    const map: Record<string, PaneRef> = {};
    for (const p of panes) map[p.id] = p;
    return map;
  }, [panes]);

  useEffect(() => {
    closeRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  function activate(task: ConductorTask) {
    const actor = taskActionPane(task);
    if (!paneMap[actor]) return;
    onFocusPane(actor);
  }

  return (
    <div className="taskdrawer-backdrop" onClick={onClose}>
      <div
        className="taskdrawer"
        role="dialog"
        aria-label="Conductor tasks"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="taskdrawer-head">
          <span className="taskdrawer-title">Tasks</span>
          <button
            ref={closeRef}
            className="taskdrawer-x"
            onClick={onClose}
            title="Close (Esc)"
            aria-label="Close"
          >
            ✕
          </button>
        </div>

        <div className="taskdrawer-body">
          <FleetOverview panes={panes} tasks={tasks} activity={activity} requests={requests}
            onFocusPane={onFocusPane} onOpenRequests={onOpenRequests} />
          {tasks.length === 0 ? (
            <div className="taskdrawer-empty">No tasks dispatched yet.</div>
          ) : (
            <>
              <TaskSection
                title="Open"
                hint="pending · overdue · rework · blocked · queued"
                items={groups.open}
                activity={activity}
                paneMap={paneMap}
                onActivate={activate}
                emptyLabel="Nothing open."
              />
              <TaskSection
                title="Awaiting review"
                items={groups.review}
                activity={activity}
                paneMap={paneMap}
                onActivate={activate}
                emptyLabel="Nothing waiting on a reviewer."
              />
              <TaskSection
                title="Recent"
                items={groups.recent}
                activity={activity}
                paneMap={paneMap}
                onActivate={activate}
                emptyLabel="Nothing finished yet."
                hiddenCount={groups.recentHiddenCount}
              />
            </>
          )}
        </div>
      </div>
    </div>
  );
}

function TaskSection({
  title,
  hint,
  items,
  paneMap,
  activity,
  onActivate,
  emptyLabel,
  hiddenCount,
}: {
  title: string;
  hint?: string;
  items: ConductorTask[];
  paneMap: Record<string, PaneRef>;
  activity: TaskActivity[];
  onActivate: (task: ConductorTask) => void;
  emptyLabel: string;
  hiddenCount?: TaskGroups["recentHiddenCount"];
}) {
  return (
    <div className="taskdrawer-section">
      <div className="taskdrawer-section-title">
        <span>
          {title} <span className="taskdrawer-section-count">({items.length})</span>
        </span>
        {hint && <span className="taskdrawer-section-hint">{hint}</span>}
      </div>
      {items.length === 0 ? (
        <div className="taskdrawer-section-empty">{emptyLabel}</div>
      ) : (
        items.map((t) => <TaskItem key={t.id} task={t} pane={paneMap[taskActionPane(t)]} targetPane={paneMap[t.target]} activity={activity.find((a) => a.task_id === t.id)} onActivate={onActivate} />)
      )}
      {!!hiddenCount && (
        <div className="taskdrawer-hidden-note">
          + {hiddenCount} older finished task{hiddenCount === 1 ? "" : "s"} not shown.
        </div>
      )}
    </div>
  );
}

function TaskItem({
  task,
  pane,
  targetPane,
  activity,
  onActivate,
}: {
  task: ConductorTask;
  pane: PaneRef | undefined;
  targetPane: PaneRef | undefined;
  activity: TaskActivity | undefined;
  onActivate: (task: ConductorTask) => void;
}) {
  const question = task.status === "blocked" ? openQuestion(task) : undefined;

  return (
    <button
      type="button"
      className="taskitem"
      onClick={() => onActivate(task)}
      disabled={!pane}
      title={pane ? `Focus ${pane.type.label} ${pane.id}` : `${taskActionPane(task)} is no longer open`}
    >
      <div className="taskitem-top">
        {targetPane && <span className="taskitem-agent">{targetPane.type.label}</span>}
        <span className="taskitem-target">{task.target}</span>
        <span className="taskitem-mode">{task.mode ?? "pane"}</span>
        {task.exit_code != null && <span className="taskitem-exit">exit {task.exit_code}</span>}
        {!targetPane && <span className="taskitem-pane-closed">worker pane closed</span>}
        <span className="taskitem-status" data-status={task.status}>
          {statusLabel(task.status)}
        </span>
      </div>
      <div className="taskitem-brief">{task.task}</div>
      {question && <div className="taskitem-question">Waiting for {task.from} to answer: {question.question}</div>}
      {activity?.state === "quiet" && <div className="taskitem-question">{activityLabel(activity)}</div>}
      {task.recovery_reason && <div className="taskitem-question">{task.recovery_reason}</div>}
      {task.reviewer_gone && !task.recovery_reason && <div className="taskitem-question">Reviewer unavailable; reassignment needed.</div>}
      {task.status === "in_review" && task.reviewer && (
        <div className="taskitem-meta">
          Awaiting review by <b>{task.reviewer}</b>
        </div>
      )}
      {task.status === "rework" && (task.reviewer || task.findings) && (
        <div className="taskitem-meta">
          {task.reviewer && (
            <>
              Sent back by <b>{task.reviewer}</b>
              {task.findings ? ": " : ""}
            </>
          )}
          {task.findings}
        </div>
      )}
      {["done", "error", "abandoned", "cancelled", "in_review"].includes(task.status) && task.result && (
        <div className="taskitem-meta">{task.result}</div>
      )}
    </button>
  );
}
