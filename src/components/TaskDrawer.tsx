import { useEffect, useMemo, useRef } from "react";
import { type ConductorTask, type SessionType } from "../lib/ipc";
import { groupTasks, openQuestion, statusLabel, type TaskGroups } from "../lib/tasks";

type PaneRef = { id: string; type: SessionType; status: string };

type TaskDrawerProps = {
  tasks: ConductorTask[];
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
export function TaskDrawer({ tasks, panes, onClose, onFocusPane }: TaskDrawerProps) {
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
    if (!paneMap[task.target]) return; // pane closed — nothing to focus
    onFocusPane(task.target);
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
          {tasks.length === 0 ? (
            <div className="taskdrawer-empty">No tasks dispatched yet.</div>
          ) : (
            <>
              <TaskSection
                title="Open"
                hint="pending · overdue · rework · blocked"
                items={groups.open}
                paneMap={paneMap}
                onActivate={activate}
                emptyLabel="Nothing open."
              />
              <TaskSection
                title="Awaiting review"
                items={groups.review}
                paneMap={paneMap}
                onActivate={activate}
                emptyLabel="Nothing waiting on a reviewer."
              />
              <TaskSection
                title="Recent"
                items={groups.recent}
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
  onActivate,
  emptyLabel,
  hiddenCount,
}: {
  title: string;
  hint?: string;
  items: ConductorTask[];
  paneMap: Record<string, PaneRef>;
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
        items.map((t) => <TaskItem key={t.id} task={t} pane={paneMap[t.target]} onActivate={onActivate} />)
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
  onActivate,
}: {
  task: ConductorTask;
  pane: PaneRef | undefined;
  onActivate: (task: ConductorTask) => void;
}) {
  const question = task.status === "blocked" ? openQuestion(task) : undefined;

  return (
    <button
      type="button"
      className="taskitem"
      onClick={() => onActivate(task)}
      disabled={!pane}
      title={pane ? `Focus ${pane.type.label} ${task.target}` : `${task.target} is no longer open`}
    >
      <div className="taskitem-top">
        {pane && <span className="taskitem-agent">{pane.type.label}</span>}
        <span className="taskitem-target">{task.target}</span>
        <span className="taskitem-mode">{task.mode ?? "pane"}</span>
        {task.exit_code != null && <span className="taskitem-exit">exit {task.exit_code}</span>}
        {!pane && <span className="taskitem-pane-closed">pane closed</span>}
        <span className="taskitem-status" data-status={task.status}>
          {statusLabel(task.status)}
        </span>
      </div>
      <div className="taskitem-brief">{task.task}</div>
      {question && <div className="taskitem-question">Waiting on you: {question.question}</div>}
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
      {(task.status === "done" || task.status === "error") && task.result && (
        <div className="taskitem-meta">{task.result}</div>
      )}
    </button>
  );
}
