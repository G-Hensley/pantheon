import { useMemo } from "react";
import type { ConductorTask, SessionRequest, SessionType, TaskActivity } from "../lib/ipc";
import { activityLabel, taskActor, TERMINAL_STATUSES } from "../lib/tasks";

const terminalStatuses = new Set<string>(TERMINAL_STATUSES);

type Props = {
  panes: { id: string; type: SessionType; status: string }[];
  tasks: ConductorTask[];
  activity: TaskActivity[];
  requests: SessionRequest[];
  onFocusPane: (id: string) => void;
  onOpenRequests?: () => void;
};

export function FleetOverview({ panes, tasks, activity, requests, onFocusPane, onOpenRequests }: Props) {
  const { heldByActor, answersByConductor } = useMemo(() => {
    const heldByActor = new Map<string, ConductorTask[]>();
    const answersByConductor = new Map<string, number>();
    for (const task of tasks) {
      if (task.status === "blocked") {
        answersByConductor.set(task.from, (answersByConductor.get(task.from) ?? 0) + 1);
      }
      if (terminalStatuses.has(task.status) || task.recovery_reason) continue;
      const actor = taskActor(task);
      const held = heldByActor.get(actor) ?? [];
      held.push(task);
      heldByActor.set(actor, held);
    }
    return { heldByActor, answersByConductor };
  }, [tasks]);
  const activityByTask = useMemo(() => new Map(activity.map((entry) => [entry.task_id, entry])), [activity]);
  const pending = requests.filter((request) => request.state === "pending");
  return (
    <section className="fleet-overview" aria-label="Fleet overview">
      <h3>Fleet</h3>
      {pending.length > 0 && onOpenRequests && (
        <button type="button" className="fleet-approvals" onClick={onOpenRequests}>
          {pending.length} session request{pending.length === 1 ? "" : "s"} awaiting your approval
        </button>
      )}
      {panes.map((pane) => {
        const held = heldByActor.get(pane.id) ?? [];
        const answers = answersByConductor.get(pane.id) ?? 0;
        return (
          <button key={pane.id} type="button" className="fleet-pane"
            onClick={() => onFocusPane(pane.id)} aria-label={`Open pane ${pane.id}`}>
            <span><b>{pane.type.label} {pane.id}</b></span>
            <span>{pane.status !== "running" ? pane.status
              : held.length === 0 ? "No dispatched task" : held.map((task) => {
                const observed = activityByTask.get(task.id);
                return `${task.id}: ${observed ? activityLabel(observed) : task.status}`;
              }).join(" · ")}</span>
            {answers > 0 && <span className="fleet-attention">{answers} question{answers === 1 ? "" : "s"} to answer</span>}
          </button>
        );
      })}
      <p className="fleet-note">Quiet does not identify an approval prompt or prove completion. Open the pane to check.</p>
    </section>
  );
}
