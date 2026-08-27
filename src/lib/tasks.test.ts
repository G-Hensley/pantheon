import { describe, expect, it } from "vitest";
import { type ConductorTask } from "./ipc";
import {
  DELIVERY_STATUSES,
  groupTasks,
  openQuestion,
  OPEN_STATUSES,
  RECENT_LIMIT,
  REVIEW_STATUS,
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
      "submitted",
      "pending",
      "queued",
      "accepted",
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

    expect(groups.open.map((t) => t.id)).toEqual([
      "blocked",
      "rework",
      "overdue",
      "submitted",
      "pending",
      "queued",
      "accepted",
    ]);
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

  it("does not promote a long-submitted task to accepted, however long it has sat there", () => {
    // Same property, on the new states: acceptance is an acknowledgement the
    // backend records, never something elapsed time is allowed to imply.
    const stale = makeTask({ status: "submitted", ts_ms: 0 });
    const fresh = makeTask({ status: "submitted", ts_ms: 999_999_999 });
    const groups = groupTasks([stale, fresh]);
    expect(groups.open.every((t) => t.status === "submitted")).toBe(true);
    expect(groups.open.some((t) => t.status === "accepted")).toBe(false);
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

describe("dispatch delivery lifecycle", () => {
  // docs/design/coordination-delivery/increment-1.md: "`submitted` means Mosaic
  // wrote the prompt and Enter to the terminal. It does not mean the model saw
  // the prompt. `accepted` is the first state that counts as active agent
  // work." Three different answers, so the surface must never fold them into
  // one another.
  it("keeps queued, submitted and accepted as three separate open states", () => {
    const tasks = DELIVERY_STATUSES.map((status, i) => makeTask({ id: status, status, ts_ms: i }));
    const groups = groupTasks(tasks);

    expect(groups.open.map((t) => t.id).sort()).toEqual([...DELIVERY_STATUSES].sort());
    expect(groups.review).toHaveLength(0);
    expect(groups.recent).toHaveLength(0);
  });

  it("ranks an unacknowledged submitted task above queued and accepted work", () => {
    // Submitted is the anomaly this increment exists to expose: Mosaic wrote
    // the prompt and nothing acknowledged it, so it can expire. Queued has not
    // been delivered yet and accepted is confirmed to be moving, so neither is
    // stuck and neither outranks it.
    const tasks = [
      makeTask({ id: "accepted", status: "accepted", ts_ms: 3 }),
      makeTask({ id: "queued", status: "queued", ts_ms: 2 }),
      makeTask({ id: "submitted", status: "submitted", ts_ms: 1 }),
    ];
    expect(groupTasks(tasks).open.map((t) => t.id)).toEqual(["submitted", "queued", "accepted"]);
  });

  it("treats accepted as active work, not as finished work", () => {
    // "accepted" is the first state that counts as active agent work, which is
    // exactly why it must not drift into the terminal group: a conductor who
    // saw it under Recent would read acknowledged work as delivered work.
    const groups = groupTasks([makeTask({ id: "a", status: "accepted", done_ms: null })]);
    expect(groups.open.map((t) => t.id)).toEqual(["a"]);
    expect(groups.recent).toHaveLength(0);
    expect(TERMINAL_STATUSES).not.toContain("accepted");
  });

  it("shows six concurrent tasks spread across the lifecycle, none folded away", () => {
    // done_when names six concurrent tasks. Six distinct lifecycle states at
    // once is the harder version: nothing may be dropped, and no state may be
    // merged into another one's bucket or label.
    const statuses = ["queued", "submitted", "accepted", "blocked", "rework", "overdue"];
    const tasks = statuses.map((status, i) => makeTask({ id: status, status, ts_ms: i }));
    const groups = groupTasks(tasks);

    expect(groups.open).toHaveLength(6);
    expect(new Set(groups.open.map((t) => t.status)).size).toBe(6);
    expect(new Set(groups.open.map((t) => statusLabel(t.status))).size).toBe(6);
  });

  it("shows a status it has never heard of as open rather than dropping it", () => {
    // The Rust half of the lifecycle lands as its own change, so this file will
    // at some point be older than the backend it is reading. An unrecognised
    // status has to surface as unfinished work: a task in none of the three
    // groups is invisible, which is the failure the drawer replaced.
    const groups = groupTasks([makeTask({ id: "future", status: "renegotiating" })]);
    expect(groups.open.map((t) => t.id)).toEqual(["future"]);
  });

  it("ranks an unrecognised status last instead of throwing", () => {
    const tasks = [
      makeTask({ id: "future", status: "renegotiating", ts_ms: 2 }),
      makeTask({ id: "blocked", status: "blocked", ts_ms: 1 }),
    ];
    expect(groupTasks(tasks).open.map((t) => t.id)).toEqual(["blocked", "future"]);
  });

  it("claims terminal and in-review positively, never by elimination", () => {
    // Open is the leftover bucket, so the two that are not must stay pinned to
    // the backend's own predicates. If they drifted, finished work would start
    // reappearing as open.
    const groups = groupTasks([
      makeTask({ id: "r", status: REVIEW_STATUS }),
      ...TERMINAL_STATUSES.map((status, i) => makeTask({ id: status, status, done_ms: i })),
    ]);
    expect(groups.open).toHaveLength(0);
    expect(groups.review.map((t) => t.id)).toEqual(["r"]);
    expect(groups.recent).toHaveLength(TERMINAL_STATUSES.length);
  });

  it("keeps ranking legacy pending records, which predate the lifecycle", () => {
    // Old task records on disk still say "pending". They keep working and are
    // not re-interpreted as any of the new states, which would claim more than
    // Mosaic ever observed about them.
    expect(OPEN_STATUSES).toContain("pending");
    const groups = groupTasks([makeTask({ id: "old", status: "pending" })]);
    expect(groups.open.map((t) => t.id)).toEqual(["old"]);
    expect(statusLabel("pending")).toBe("Pending");
  });

  it("ranks every delivery state, so none of them sorts into the unknown tier", () => {
    for (const status of DELIVERY_STATUSES) {
      expect(OPEN_STATUSES).toContain(status);
    }
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

  it("names each delivery state in words, so the pill never depends on its colour", () => {
    // "Accessibility: task status and attention states ... do not rely on
    // color alone" (increment-1.md). The label is the half of that the
    // stylesheet cannot take away: with CSS off entirely, the three states are
    // still three different words.
    expect(DELIVERY_STATUSES.map(statusLabel)).toEqual(["Queued", "Submitted", "Accepted"]);
  });

  it("falls back to the raw status rather than rendering an empty pill", () => {
    expect(statusLabel("renegotiating")).toBe("renegotiating");
  });
});
