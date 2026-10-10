import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { FleetOverview } from "./FleetOverview";
import type { ConductorTask, SessionRequest, SessionType } from "../lib/ipc";

const type: SessionType = { id: "codex", label: "Codex", program: "codex", args: [], color: "#fff" };
const panes = ["worker", "conductor", "free"].map((id) => ({ id, type, status: "running" }));
const task: ConductorTask = { id: "task-1", from: "conductor", target: "worker", task: "Inspect parser",
  status: "pending", result: "", ts_ms: 0, done_ms: null, reviewer: "", findings: "", exchanges: [] };
const request: SessionRequest = { request_id: "request-1", kind: "codex", model: "", model_verified: false,
  reason: "Need a reviewer", isolate: true, project: "/fixture", brain: "main", requester: "conductor",
  state: "pending", session_id: null, detail: null, created_ms: 0, updated_ms: 0 };

describe("FleetOverview", () => {
  it("names quiet work without inferring an approval wait or completion", () => {
    const focus = vi.fn();
    render(<FleetOverview panes={panes} tasks={[task]} requests={[]} onFocusPane={focus}
      activity={[{ task_id: task.id, actor: "worker", state: "quiet", quiet_ms: 360000, waiting_on: null }]} />);
    expect(screen.getByText("task-1: No observed activity for 6m; task still open")).toBeInTheDocument();
    expect(screen.queryByText(/waiting.*approval/i)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open pane worker" }));
    expect(focus).toHaveBeenCalledWith("worker");
    expect(task.status).toBe("pending");
  });

  it("counts the conductor's questions and links pending approvals without approving", () => {
    const open = vi.fn();
    render(<FleetOverview panes={panes} tasks={[{ ...task, status: "blocked" }]} requests={[request]}
      activity={[]} onFocusPane={() => {}} onOpenRequests={open} />);
    expect(screen.getByText("1 question to answer")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "1 session request awaiting your approval" }));
    expect(open).toHaveBeenCalledOnce();
    expect(request.state).toBe("pending");
  });

  it("keeps panes visible without treating absence of a dispatched task as idle", () => {
    render(<FleetOverview panes={panes} tasks={[]} activity={[]} requests={[{ ...request, state: "denied" }]}
      onFocusPane={() => {}} onOpenRequests={() => {}} />);
    expect(screen.getAllByText("No dispatched task")).toHaveLength(3);
    expect(screen.queryByText(/awaiting your approval/)).not.toBeInTheDocument();
  });
});

describe("FleetOverview ownership exclusions", () => {
  it.each<[string, ConductorTask]>([
    ["interrupted review", { ...task, status: "in_review", reviewer: "worker", recovery_reason: "App restarted" }],
    ...["done", "error", "cancelled", "abandoned"].map((status) => [status, { ...task, status }] as [string, ConductorTask]),
  ])("does not assign %s to a reused pane", (_name, excluded) => {
    render(<FleetOverview panes={panes} tasks={[excluded]} activity={[]} requests={[]} onFocusPane={() => {}} />);
    const worker = screen.getByRole("button", { name: "Open pane worker" });
    expect(within(worker).getByText("No dispatched task")).toBeInTheDocument();
    expect(worker).not.toHaveTextContent(task.id);
  });

  it("shows a stopped pane's state even when a task still names it", () => {
    render(<FleetOverview panes={[{ ...panes[0], status: "exited" }]} tasks={[task]}
      activity={[]} requests={[]} onFocusPane={() => {}} />);
    const worker = screen.getByRole("button", { name: "Open pane worker" });
    expect(within(worker).getByText("exited")).toBeInTheDocument();
    expect(worker).not.toHaveTextContent(task.id);
    expect(worker).not.toHaveTextContent("No dispatched task");
  });
});
