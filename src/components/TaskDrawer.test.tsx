import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { TaskDrawer } from "./TaskDrawer";
import { type ConductorTask, type SessionType } from "../lib/ipc";
import { statusLabel } from "../lib/tasks";

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

function makePane(id: string) {
  return { id, type: CLAUDE, status: "running" };
}

describe("<TaskDrawer>", () => {
  it("renders all six concurrent open tasks — none dropped past the old five-item cap", () => {
    const tasks = Array.from({ length: 6 }, (_, i) =>
      makeTask({ target: `sess-${i}`, task: `distinct task brief number ${i}` }),
    );
    const panes = tasks.map((t) => makePane(t.target));
    render(<TaskDrawer tasks={tasks} panes={panes} onClose={() => {}} onFocusPane={() => {}} />);
    for (const t of tasks) {
      expect(screen.getByText(t.task)).toBeInTheDocument();
    }
  });

  it("never truncates a long brief", () => {
    const long = "x".repeat(200);
    const task = makeTask({ task: long, target: "sess-1" });
    render(
      <TaskDrawer tasks={[task]} panes={[makePane("sess-1")]} onClose={() => {}} onFocusPane={() => {}} />,
    );
    expect(screen.getByText(long)).toBeInTheDocument();
  });

  it("focuses the target pane when an open task is activated", () => {
    const task = makeTask({ target: "sess-7" });
    const onFocusPane = vi.fn();
    render(
      <TaskDrawer
        tasks={[task]}
        panes={[makePane("sess-7")]}
        onClose={() => {}}
        onFocusPane={onFocusPane}
      />,
    );
    fireEvent.click(screen.getByTitle("Focus Claude Code sess-7"));
    expect(onFocusPane).toHaveBeenCalledWith("sess-7");
  });

  it("disables a task whose pane has since closed instead of silently doing nothing", () => {
    const task = makeTask({ target: "sess-gone" });
    const onFocusPane = vi.fn();
    render(<TaskDrawer tasks={[task]} panes={[]} onClose={() => {}} onFocusPane={onFocusPane} />);
    const button = screen.getByTitle("sess-gone is no longer open");
    expect(button).toBeDisabled();
    fireEvent.click(button);
    expect(onFocusPane).not.toHaveBeenCalled();
  });

  it("closes on Escape", () => {
    const onClose = vi.fn();
    render(<TaskDrawer tasks={[]} panes={[]} onClose={onClose} onFocusPane={() => {}} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("lists a blocked task ahead of pending/overdue/rework, and shows its open question", () => {
    const blocked = makeTask({
      target: "sess-b",
      status: "blocked",
      exchanges: [{ question: "Which schema?", answer: "", asked_ms: 1 }],
    });
    const pending = makeTask({ target: "sess-p", status: "pending" });
    render(
      <TaskDrawer
        tasks={[pending, blocked]}
        panes={[makePane("sess-b"), makePane("sess-p")]}
        onClose={() => {}}
        onFocusPane={() => {}}
      />,
    );
    const focusButtons = screen.getAllByTitle(/^Focus Claude Code/);
    expect(focusButtons[0]).toHaveAttribute("title", "Focus Claude Code sess-b");
    expect(screen.getByText(/Which schema\?/)).toBeInTheDocument();
  });

  it("bounds the Recent section and says how many finished tasks are hidden", () => {
    const terminal = Array.from({ length: 11 }, (_, i) =>
      makeTask({ target: `d-${i}`, status: "done", done_ms: i }),
    );
    render(
      <TaskDrawer
        tasks={terminal}
        panes={terminal.map((t) => makePane(t.target))}
        onClose={() => {}}
        onFocusPane={() => {}}
      />,
    );
    expect(screen.getByText(/3 older finished tasks not shown/)).toBeInTheDocument();
  });

  it("shows an empty state instead of three empty sections when nothing has ever been dispatched", () => {
    render(<TaskDrawer tasks={[]} panes={[]} onClose={() => {}} onFocusPane={() => {}} />);
    expect(screen.getByText("No tasks dispatched yet.")).toBeInTheDocument();
  });
});

describe("<TaskDrawer> dispatch lifecycle", () => {
  function renderTasks(tasks: ConductorTask[]) {
    return render(
      <TaskDrawer
        tasks={tasks}
        panes={tasks.map((t) => makePane(t.target))}
        onClose={() => {}}
        onFocusPane={() => {}}
      />,
    );
  }

  it("names queued, submitted and accepted as three different things", () => {
    // docs/design/coordination-delivery/increment-1.md draws the line at
    // acceptance: "`submitted` ... does not mean the model saw the prompt.
    // `accepted` is the first state that counts as active agent work." If the
    // drawer showed one word for all three, a conductor would read a prompt
    // nobody has looked at as work in progress.
    renderTasks([
      makeTask({ target: "sess-q", status: "queued" }),
      makeTask({ target: "sess-s", status: "submitted" }),
      makeTask({ target: "sess-a", status: "accepted" }),
    ]);
    expect(screen.getByText("Queued")).toBeInTheDocument();
    expect(screen.getByText("Submitted")).toBeInTheDocument();
    expect(screen.getByText("Accepted")).toBeInTheDocument();
  });

  it("says each state in words, not only in colour", () => {
    // "Accessibility: task status and attention states ... do not rely on color
    // alone." The pill's own text is what survives greyscale, a colour-blind
    // reader, and a screen reader; `data-status` is only the hook the
    // stylesheet hangs its second, visual distinction on (src/App.css.test.ts).
    renderTasks([
      makeTask({ target: "sess-q", status: "queued" }),
      makeTask({ target: "sess-s", status: "submitted" }),
      makeTask({ target: "sess-a", status: "accepted" }),
    ]);
    const labels = ["Queued", "Submitted", "Accepted"].map((text) => screen.getByText(text));
    for (const [i, status] of ["queued", "submitted", "accepted"].entries()) {
      expect(labels[i]).toHaveAttribute("data-status", status);
      expect(labels[i].textContent?.trim()).not.toBe("");
    }
    expect(new Set(labels.map((l) => l.textContent)).size).toBe(3);
  });

  it("lists an unacknowledged submitted task above queued and accepted work", () => {
    renderTasks([
      makeTask({ target: "sess-a", status: "accepted", ts_ms: 3 }),
      makeTask({ target: "sess-q", status: "queued", ts_ms: 2 }),
      makeTask({ target: "sess-s", status: "submitted", ts_ms: 1 }),
    ]);
    const order = screen
      .getAllByTitle(/^Focus Claude Code/)
      .map((b) => b.getAttribute("title")?.replace("Focus Claude Code ", ""));
    expect(order).toEqual(["sess-s", "sess-q", "sess-a"]);
  });

  it("keeps accepted work under open, never under Recent", () => {
    // Acceptance is the start of the work, not the end of it.
    renderTasks([makeTask({ target: "sess-a", status: "accepted", task: "acknowledged brief" })]);
    expect(screen.getByText("acknowledged brief")).toBeInTheDocument();
    expect(screen.getByText("Nothing finished yet.")).toBeInTheDocument();
  });

  it("shows six concurrent tasks in six different lifecycle states", () => {
    // done_when: "tests cover ... six concurrent tasks". Six at once, each in a
    // different state, is the version that also proves none of them is folded
    // into another one's label.
    const statuses = ["queued", "submitted", "accepted", "blocked", "rework", "overdue"];
    const tasks = statuses.map((status, i) =>
      makeTask({ target: `sess-${status}`, status, task: `brief for ${status}`, ts_ms: i }),
    );
    renderTasks(tasks);

    for (const status of statuses) {
      expect(screen.getByText(`brief for ${status}`)).toBeInTheDocument();
    }
    const pills = statuses.map((s) => screen.getByText(new RegExp(`^${statusLabel(s)}$`)));
    expect(new Set(pills.map((p) => p.getAttribute("data-status"))).size).toBe(6);
  });

  it("renders a status it has never heard of instead of dropping the task", () => {
    // The Rust half of the lifecycle lands separately, so the drawer may be the
    // older of the two. Unknown work stays visible and keeps its raw status as
    // its label rather than becoming an unexplained blank pill.
    renderTasks([
      makeTask({ target: "sess-x", status: "renegotiating", task: "from a newer backend" }),
    ]);
    expect(screen.getByText("from a newer backend")).toBeInTheDocument();
    expect(screen.getByText("renegotiating")).toHaveAttribute("data-status", "renegotiating");
  });
});
