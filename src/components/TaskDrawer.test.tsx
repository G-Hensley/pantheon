import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { TaskDrawer } from "./TaskDrawer";
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

function makePane(id: string) {
  return { id, type: CLAUDE, status: "running" };
}

describe("<TaskDrawer>", () => {
  it("shows headless mode and a zero exit code", () => {
    const task = makeTask({ mode: "headless", status: "done", exit_code: 0 });
    render(<TaskDrawer tasks={[task]} panes={[]} onClose={() => {}} onFocusPane={() => {}} />);
    expect(screen.getByText("headless")).toBeInTheDocument();
    expect(screen.getByText("exit 0")).toBeInTheDocument();
  });
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
