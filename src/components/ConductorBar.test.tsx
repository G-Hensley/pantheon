import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ConductorBar } from "./ConductorBar";
import { type ConductorTask, type SessionType } from "../lib/ipc";

const CLAUDE: SessionType = {
  id: "claude",
  label: "Claude Code",
  program: "claude",
  args: [],
  color: "#e0af68",
};

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

function renderBar(tasks: ConductorTask[], overrides: Record<string, unknown> = {}) {
  const panes = tasks.map((t) => ({ id: t.target, type: CLAUDE, status: "running" }));
  return render(
    <ConductorBar
      conductor="sess-1"
      tasks={tasks}
      halted={false}
      onDemote={() => {}}
      onHaltChange={() => {}}
      onOpenDispatch={() => {}}
      onOpenTasks={() => {}}
      panes={panes}
      {...overrides}
    />,
  );
}

describe("<ConductorBar> as fixed top chrome", () => {
  // The bar is a fixed-height strip directly under the title bar. It went wrong
  // once by trying to *list* work in that space: `.cond-feed` held five pills,
  // scrolled horizontally, and painted a 15px scrollbar across the top of the
  // window at every width from 1024 to 1920 (measured in WebView2). The fix was
  // to stop listing work here at all and move it behind the Tasks button. These
  // tests hold that shape, because CSS alone cannot: a list that grows with the
  // task count will overflow whatever the stylesheet says.
  it("lists no tasks in the strip itself", () => {
    const { container } = renderBar(
      Array.from({ length: 12 }, (_, i) => makeTask({ status: "submitted", ts_ms: i })),
    );
    expect(container.querySelector(".cond-feed")).toBeNull();
    expect(container.querySelector(".cond-task")).toBeNull();
  });

  it("does not grow with the number of tasks", () => {
    // The structural version of "must not introduce horizontal scrolling at
    // 1477x427": one task and fifty produce the same number of elements, so
    // there is nothing whose width depends on how much work is in flight.
    const one = renderBar([makeTask()]).container.querySelectorAll(".condbar *").length;
    const many = renderBar(
      Array.from({ length: 50 }, (_, i) => makeTask({ ts_ms: i })),
    ).container.querySelectorAll(".condbar *").length;
    expect(many).toBe(one);
  });

  it("does not render a task brief in the strip, however long the brief is", () => {
    const long = "y".repeat(400);
    const { container } = renderBar([makeTask({ task: long })]);
    expect(container.querySelector(".condbar")?.textContent).not.toContain(long);
  });
});

describe("<ConductorBar> task count", () => {
  it("counts queued, submitted and accepted work as open", () => {
    // The delivery states are open work: a conductor glancing at the bar has to
    // see that six things are in flight, not zero because none of them carries
    // a status this build shipped with.
    const statuses = ["queued", "submitted", "accepted", "blocked", "rework", "overdue"];
    renderBar(statuses.map((status, i) => makeTask({ status, ts_ms: i })));
    expect(screen.getByRole("button", { name: /Tasks · 6/ })).toBeInTheDocument();
  });

  it("counts work awaiting review alongside open work", () => {
    renderBar([makeTask({ status: "accepted" }), makeTask({ status: "in_review" })]);
    expect(screen.getByRole("button", { name: /Tasks · 2/ })).toBeInTheDocument();
  });

  it("stops counting a task once it reaches a terminal state", () => {
    renderBar([
      makeTask({ status: "accepted" }),
      makeTask({ status: "done", done_ms: 1 }),
      makeTask({ status: "abandoned", done_ms: 2 }),
    ]);
    expect(screen.getByRole("button", { name: /Tasks · 1/ })).toBeInTheDocument();
  });

  it("counts a status it has never heard of rather than reporting it as nothing", () => {
    renderBar([makeTask({ status: "renegotiating" })]);
    expect(screen.getByRole("button", { name: /Tasks · 1/ })).toBeInTheDocument();
  });
});

describe("<ConductorBar> keyboard access", () => {
  it("exposes every control as a real button, so Tab and Enter reach all of them", () => {
    // "task status and attention states remain keyboard accessible"
    // (increment-1.md). Nothing here is a div with an onClick.
    const { container } = renderBar([makeTask({ status: "blocked" })]);
    const clickable = container.querySelectorAll(".condbar button");
    expect(clickable.length).toBeGreaterThanOrEqual(3);
    for (const el of clickable) {
      expect(el.tagName).toBe("BUTTON");
    }
  });

  it("opens the drawer from the Tasks button", () => {
    const onOpenTasks = vi.fn();
    const { container } = renderBar([makeTask({ status: "submitted" })], { onOpenTasks });
    const tasksButton = container.querySelector<HTMLButtonElement>(".cond-tasks");
    expect(tasksButton).not.toBeNull();
    tasksButton?.click();
    expect(onOpenTasks).toHaveBeenCalled();
  });
});
