import { describe, expect, it } from "vitest";
import { type ConductorTask } from "./ipc";
import {
  groupTasks,
  openQuestion,
  OPEN_STATUSES,
  RECENT_LIMIT,
  statusLabel,
  TERMINAL_STATUSES,
} from "./tasks";

const TERMINAL_SET = new Set<string>(TERMINAL_STATUSES);

let nextId = 0;
function makeTask(overrides: Partial<ConductorTask> = {}): ConductorTask {
  nextId += 1;
  return {
    id: `t${nextId}`,
    from: "conductor",
    target: `sess-${nextId}`,
    task: `task ${nextId}`,
    status: "pending",
    result: "",
    ts_ms: nextId,
    done_ms: null,
    reviewer: "",
    findings: "",
    exchanges: [],
    ...overrides,
  };
}

describe("groupTasks", () => {
  it("keeps a sixth concurrent open task visible — the exact regression the old five-pill feed had", () => {
    const tasks = Array.from({ length: 6 }, (_, i) =>
      makeTask({ id: `open-${i}`, status: "pending", ts_ms: i }),
    );
    const groups = groupTasks(tasks);
    expect(groups.open).toHaveLength(6);
    expect(groups.open.map((t) => t.id).sort()).toEqual(tasks.map((t) => t.id).sort());
  });

  it("never drops open work to make room for terminal history", () => {
    const open = Array.from({ length: 10 }, (_, i) => makeTask({ id: `open-${i}`, status: "pending" }));
    const terminal = Array.from({ length: 20 }, (_, i) =>
      makeTask({ id: `done-${i}`, status: "done", done_ms: i }),
    );
    const groups = groupTasks([...open, ...terminal]);
    expect(groups.open).toHaveLength(10);
  });

  it("sorts every known status into exactly one of open / review / recent, ordered blocked-first", () => {
    const statuses = [
      "blocked",
      "rework",
      "overdue",
      "pending",
      "in_review",
      "done",
      "error",
      "cancelled",
      "abandoned",
    ];
    const tasks = statuses.map((status, i) =>
      makeTask({ id: status, status, ts_ms: i, done_ms: TERMINAL_SET.has(status) ? i : null }),
    );
    const groups = groupTasks(tasks);

    expect(groups.open.map((t) => t.id)).toEqual(["blocked", "rework", "overdue", "pending"]);
    expect(groups.review.map((t) => t.id)).toEqual(["in_review"]);
    expect(groups.recent.map((t) => t.id).sort()).toEqual(
      ["abandoned", "cancelled", "done", "error"].sort(),
    );

    // The property "never infer blocked from silence" depends on: the only
    // task ever bucketed with status "blocked" is the one the backend
    // already marked that way. groupTasks has no other input to work from.
    const blocked = groups.open.filter((t) => t.status === "blocked");
    expect(blocked).toHaveLength(1);
    expect(blocked[0].id).toBe("blocked");
  });

  it("classifies purely from task.status — there is no timing input to promote a quiet task to blocked", () => {
    // ConductorTask carries no PTY/output/last-activity field at all (see
    // src/lib/ipc.ts); ts_ms only marks when the task was dispatched. A task
    // that has looked "pending" for a long time is still just pending.
    const longPending = makeTask({ status: "pending", ts_ms: 0 });
    const freshPending = makeTask({ status: "pending", ts_ms: 999_999_999 });
    const groups = groupTasks([longPending, freshPending]);
    expect(groups.open.every((t) => t.status === "pending")).toBe(true);
    expect(groups.open.some((t) => t.status === "blocked")).toBe(false);
  });

  it("bounds recent terminal history and reports how many are hidden, newest first", () => {
    const terminal = Array.from({ length: RECENT_LIMIT + 3 }, (_, i) =>
      makeTask({ id: `done-${i}`, status: "done", done_ms: i }),
    );
    const groups = groupTasks(terminal);
    expect(groups.recent).toHaveLength(RECENT_LIMIT);
    expect(groups.recentHiddenCount).toBe(3);
    expect(groups.recent[0].id).toBe(`done-${RECENT_LIMIT + 2}`);
  });

  it("does not hide the bound — hiddenCount is 0 when nothing is cut", () => {
    const terminal = Array.from({ length: RECENT_LIMIT }, (_, i) =>
      makeTask({ id: `done-${i}`, status: "done", done_ms: i }),
    );
    expect(groupTasks(terminal).recentHiddenCount).toBe(0);
  });
});

describe("openQuestion", () => {
  it("returns the last unanswered exchange", () => {
    const task = makeTask({
      status: "blocked",
      exchanges: [
        { question: "first?", answer: "yes", asked_ms: 1 },
        { question: "second?", answer: "", asked_ms: 2 },
      ],
    });
    expect(openQuestion(task)?.question).toBe("second?");
  });

  it("returns undefined once every question has an answer", () => {
    const task = makeTask({ exchanges: [{ question: "q", answer: "a", asked_ms: 1 }] });
    expect(openQuestion(task)).toBeUndefined();
  });

  it("returns undefined when nothing was ever asked", () => {
    expect(openQuestion(makeTask({ exchanges: [] }))).toBeUndefined();
  });
});

describe("statusLabel", () => {
  it("has a distinct, non-empty label for every status the surface must separate", () => {
    const all = [...OPEN_STATUSES, "in_review", ...TERMINAL_STATUSES];
    const labels = all.map(statusLabel);
    expect(labels.every((l) => l.length > 0)).toBe(true);
    expect(new Set(labels).size).toBe(labels.length);
  });
});
