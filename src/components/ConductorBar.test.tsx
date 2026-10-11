import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { ConductorBar } from "./ConductorBar";
import { type ConductorTask, type DispatchBudget, type SessionType } from "../lib/ipc";

const CLAUDE: SessionType = {
  id: "claude",
  label: "Claude Code",
  program: "claude",
  args: [],
  color: "#e0af68",
};

function baseProps(overrides: Partial<Parameters<typeof ConductorBar>[0]> = {}) {
  return {
    conductor: "sess-1",
    tasks: [] as ConductorTask[],
    halted: false,
    onDemote: vi.fn(),
    onHaltChange: vi.fn(),
    onOpenDispatch: vi.fn(),
    onOpenTasks: vi.fn(),
    panes: [{ id: "sess-2", type: CLAUDE, status: "running" }],
    budget: null as DispatchBudget | null,
    onResetBudget: vi.fn(),
    budgetResetPending: false,
    budgetResetError: null as string | null,
    ...overrides,
  };
}

describe("<ConductorBar> dispatch budget", () => {
  it("shows used, limit and remaining once the backend snapshot has been read", () => {
    render(<ConductorBar {...baseProps({ budget: { used: 12, limit: 40, remaining: 28 } })} />);
    expect(screen.getByText("12/40 dispatch budget used · 28 left")).toBeInTheDocument();
  });

  it("reports the budget as unknown rather than guessing before the first snapshot arrives", () => {
    render(<ConductorBar {...baseProps({ budget: null })} />);
    expect(screen.getByText("dispatch budget unknown")).toBeInTheDocument();
  });

  it("no longer shows the old lifetime dispatched count", () => {
    // Regression lock: BACKLOG.md/scope.md flagged tasks.length as a lifetime
    // count, not the remaining allowance, so this string must not come back.
    const tasks = Array.from({ length: 5 }, (_, i) => ({
      id: `t${i}`,
      from: "conductor",
      target: `sess-${i}`,
      task: `task ${i}`,
      status: "pending",
      result: "",
      ts_ms: i,
      done_ms: null,
      reviewer: "",
      findings: "",
      exchanges: [],
    }));
    render(<ConductorBar {...baseProps({ tasks, budget: { used: 1, limit: 40, remaining: 39 } })} />);
    expect(screen.queryByText(/dispatched$/)).toBeNull();
    expect(screen.getByText("5 pending")).toBeInTheDocument();
  });

  it("marks the budget exhausted once remaining reaches zero", () => {
    render(<ConductorBar {...baseProps({ budget: { used: 40, limit: 40, remaining: 0 } })} />);
    const el = screen.getByText("40/40 dispatch budget used · 0 left");
    expect(el).toHaveClass("exhausted");
  });

  it("has a Reset dispatch budget control with a clear accessible name, distinct from Stop", () => {
    render(<ConductorBar {...baseProps()} />);
    const reset = screen.getByRole("button", { name: "Reset dispatch budget" });
    expect(reset).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stop" })).toBeInTheDocument();
  });

  it("invokes onResetBudget when clicked, and only that handler", () => {
    const onResetBudget = vi.fn();
    const onHaltChange = vi.fn();
    render(<ConductorBar {...baseProps({ onResetBudget, onHaltChange })} />);
    fireEvent.click(screen.getByRole("button", { name: "Reset dispatch budget" }));
    expect(onResetBudget).toHaveBeenCalledTimes(1);
    expect(onHaltChange).not.toHaveBeenCalled();
  });

  it("disables the control and shows a pending label while a reset is in flight", () => {
    render(<ConductorBar {...baseProps({ budgetResetPending: true })} />);
    const reset = screen.getByRole("button", { name: "Reset dispatch budget" });
    expect(reset).toBeDisabled();
    expect(reset).toHaveTextContent("Resetting…");
  });

  it("surfaces a failed reset visibly instead of claiming success", () => {
    render(<ConductorBar {...baseProps({ budgetResetError: "backend not ready" })} />);
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("backend not ready");
  });

  it("shows no error alert when the last reset was not a failure", () => {
    render(<ConductorBar {...baseProps()} />);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps halted state and tasks untouched by the reset control (it only forwards the click)", () => {
    const onResetBudget = vi.fn();
    render(
      <ConductorBar
        {...baseProps({
          halted: true,
          onResetBudget,
          budget: { used: 40, limit: 40, remaining: 0 },
        })}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Reset dispatch budget" }));
    expect(onResetBudget).toHaveBeenCalledTimes(1);
    // Stop/Resume reflects the halted prop exactly as it did before this
    // feature; the reset button doesn't touch it.
    expect(screen.getByRole("button", { name: "Resume" })).toBeInTheDocument();
  });
});

describe("<ConductorBar> fleet attention", () => {
  const pending: ConductorTask = { id: "task-1", from: "sess-1", target: "sess-2",
    task: "Inspect parser", status: "pending", result: "", ts_ms: 0, done_ms: null,
    reviewer: "sess-3", findings: "", exchanges: [] };

  it.each([
    ["pending work", pending, false],
    ["ordinary review", { ...pending, status: "in_review" }, false],
    ["interrupted review", { ...pending, status: "in_review", recovery_reason: "App restarted" }, true],
    ["unavailable reviewer", { ...pending, status: "in_review", reviewer_gone: true }, true],
  ])("shows attention only when needed for %s", (_name, task, attention) => {
    const props = baseProps({ tasks: [task] });
    render(<ConductorBar {...props} />);
    const button = screen.getByRole("button", { name: "Tasks · 1" });
    if (attention) expect(button).toHaveClass("attention");
    else expect(button).not.toHaveClass("attention");
    fireEvent.click(button);
    expect(props.onOpenTasks).toHaveBeenCalledOnce();
  });

  it("clears quiet attention on fresh activity without changing the task", () => {
    const activity = { task_id: pending.id, actor: pending.target, state: "quiet", quiet_ms: 300000, waiting_on: null };
    const props = baseProps({ tasks: [pending] });
    const { rerender } = render(<ConductorBar {...props} activity={[activity]} />);
    expect(screen.getByRole("button", { name: "Tasks · 1" })).toHaveClass("attention");
    rerender(<ConductorBar {...props} activity={[{ ...activity, state: "pending", quiet_ms: 0 }]} />);
    expect(screen.getByRole("button", { name: "Tasks · 1" })).not.toHaveClass("attention");
    expect(pending.status).toBe("pending");
  });
});
