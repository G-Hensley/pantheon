import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { SessionRequests } from "./SessionRequests";
import { type SessionRequest, type SessionRequestList } from "../lib/ipc";

function request(overrides: Partial<SessionRequest> = {}): SessionRequest {
  return {
    request_id: "req-1",
    kind: "claude",
    model: "claude-sonnet-5",
    model_verified: true,
    reason: "need a helper session",
    isolate: true,
    project: "/repo",
    brain: "main",
    requester: "sess-2",
    state: "pending",
    session_id: null,
    detail: null,
    created_ms: 0,
    updated_ms: 0,
    ...overrides,
  };
}

function list(overrides: Partial<SessionRequestList> = {}): SessionRequestList {
  return {
    requests: [],
    startups: [],
    outstanding: 0,
    admitted: 0,
    outstanding_limit: 3,
    admitted_limit: 10,
    allocator_ready: true,
    ...overrides,
  };
}

function baseProps(overrides: Partial<Parameters<typeof SessionRequests>[0]> = {}) {
  return {
    list: list(),
    onApprove: vi.fn(),
    approvingIds: new Set<string>(),
    approveError: null as string | null,
    onDeny: vi.fn(),
    onReset: vi.fn(),
    resetPending: false,
    onClose: vi.fn(),
    ...overrides,
  };
}

describe("<SessionRequests>", () => {
  it("shows host, model, project, isolation, brain and reason for an open request", () => {
    render(<SessionRequests {...baseProps({ list: list({ requests: [request()] }) })} />);
    expect(screen.getByText("claude")).toBeInTheDocument();
    expect(screen.getByDisplayValue("claude-sonnet-5")).toBeInTheDocument();
    expect(screen.getByText("/repo")).toBeInTheDocument();
    expect(screen.getByText("isolated")).toBeInTheDocument();
    expect(screen.getByText("brain: main")).toBeInTheDocument();
    expect(screen.getByText("need a helper session")).toBeInTheDocument();
  });

  it("shows the remaining allowance counts", () => {
    render(
      <SessionRequests
        {...baseProps({ list: list({ outstanding: 1, outstanding_limit: 3, admitted: 4, admitted_limit: 10 }) })}
      />,
    );
    expect(screen.getByText("1/3 outstanding · 4/10 admitted")).toBeInTheDocument();
  });

  it("labels an unedited, backend-unverified custom model as unverified", () => {
    render(
      <SessionRequests {...baseProps({ list: list({ requests: [request({ model_verified: false })] }) })} />,
    );
    expect(screen.getByText("unverified custom model")).toBeInTheDocument();
  });

  it("shows no unverified label for a verified model left unedited", () => {
    render(<SessionRequests {...baseProps({ list: list({ requests: [request({ model_verified: true })] }) })} />);
    expect(screen.queryByText(/unverified/)).toBeNull();
  });

  it("labels an edited model as unverified even if the original model was verified", () => {
    render(<SessionRequests {...baseProps({ list: list({ requests: [request({ model_verified: true })] }) })} />);
    fireEvent.change(screen.getByDisplayValue("claude-sonnet-5"), { target: { value: "claude-opus-5" } });
    expect(screen.getByText("edited — unverified custom model")).toBeInTheDocument();
  });

  it("Approve sends null as the edited model when the human left it unchanged", () => {
    const onApprove = vi.fn();
    render(<SessionRequests {...baseProps({ list: list({ requests: [request()] }), onApprove })} />);
    fireEvent.click(screen.getByRole("button", { name: "Approve" }));
    expect(onApprove).toHaveBeenCalledWith("req-1", null);
  });

  it("Approve sends the trimmed edited model when the human changed it", () => {
    const onApprove = vi.fn();
    render(<SessionRequests {...baseProps({ list: list({ requests: [request()] }), onApprove })} />);
    fireEvent.change(screen.getByDisplayValue("claude-sonnet-5"), { target: { value: "  claude-opus-5  " } });
    fireEvent.click(screen.getByRole("button", { name: "Approve" }));
    expect(onApprove).toHaveBeenCalledWith("req-1", "claude-opus-5");
  });

  it("Deny calls onDeny with the request id", () => {
    const onDeny = vi.fn();
    render(<SessionRequests {...baseProps({ list: list({ requests: [request()] }), onDeny })} />);
    fireEvent.click(screen.getByRole("button", { name: "Deny" }));
    expect(onDeny).toHaveBeenCalledWith("req-1");
  });

  it("disables Approve/Deny/model input and shows Approving… only for the request currently being approved, not other open requests", () => {
    render(
      <SessionRequests
        {...baseProps({
          list: list({ requests: [request({ request_id: "req-1" }), request({ request_id: "req-2" })] }),
          approvingIds: new Set(["req-1"]),
        })}
      />,
    );
    const approveButtons = screen.getAllByRole("button", { name: /Approve|Approving/ });
    expect(approveButtons[0]).toHaveTextContent("Approving…");
    expect(approveButtons[0]).toBeDisabled();
    expect(approveButtons[1]).toHaveTextContent("Approve");
    expect(approveButtons[1]).not.toBeDisabled();
  });

  it("shows Approving… and disables both rows when two different requests are approved at once, neither clobbering the other", () => {
    // The bug this covers: a single scalar "which one is approving" state
    // would have the second row's approval starting overwrite the first
    // row's own still-in-flight indicator.
    render(
      <SessionRequests
        {...baseProps({
          list: list({ requests: [request({ request_id: "req-1" }), request({ request_id: "req-2" })] }),
          approvingIds: new Set(["req-1", "req-2"]),
        })}
      />,
    );
    const approveButtons = screen.getAllByRole("button", { name: /Approve|Approving/ });
    expect(approveButtons[0]).toHaveTextContent("Approving…");
    expect(approveButtons[0]).toBeDisabled();
    expect(approveButtons[1]).toHaveTextContent("Approving…");
    expect(approveButtons[1]).toBeDisabled();
  });

  it("disables Approve/Deny/model input for every open request, and shows a notice, while the allocator is not ready", () => {
    render(
      <SessionRequests {...baseProps({ list: list({ requests: [request()], allocator_ready: false }) })} />,
    );
    expect(
      screen.getByText("Restoring session state: approve, deny and reset are disabled until this finishes."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Approve" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Deny" })).toBeDisabled();
    expect(screen.getByDisplayValue("claude-sonnet-5")).toBeDisabled();
  });

  // Reset discards every pending request. Doing that before the backend has
  // seen the restored roster is no safer than admitting one, so it sits behind
  // the same barrier as Approve and Deny rather than beside it.
  it("disables Reset while the allocator is not ready, alongside Approve and Deny", () => {
    const onReset = vi.fn();
    render(<SessionRequests {...baseProps({ onReset, list: list({ allocator_ready: false }) })} />);
    const reset = screen.getByRole("button", { name: "Reset" });
    expect(reset).toBeDisabled();
    fireEvent.click(reset);
    expect(onReset).not.toHaveBeenCalled();
  });

  it("shows Reset as pending and disabled while a reset is in flight, and calls onReset otherwise", () => {
    const onReset = vi.fn();
    const { rerender } = render(<SessionRequests {...baseProps({ onReset, resetPending: true })} />);
    const resetting = screen.getByRole("button", { name: "Resetting…" });
    expect(resetting).toBeDisabled();

    rerender(<SessionRequests {...baseProps({ onReset, resetPending: false })} />);
    fireEvent.click(screen.getByRole("button", { name: "Reset" }));
    expect(onReset).toHaveBeenCalledTimes(1);
  });

  it("surfaces a failed approval visibly", () => {
    render(<SessionRequests {...baseProps({ approveError: "spawn refused: isolation unavailable" })} />);
    expect(screen.getByRole("alert").textContent).toContain("spawn refused: isolation unavailable");
  });

  it("shows no error alert when the last approval was not a failure", () => {
    render(<SessionRequests {...baseProps()} />);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("calls onClose on Escape and on clicking the close button", () => {
    const onClose = vi.fn();
    render(<SessionRequests {...baseProps({ onClose })} />);
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("shows terminal requests under History, separately from open ones, without Approve/Deny controls", () => {
    render(
      <SessionRequests
        {...baseProps({
          list: list({
            requests: [request({ request_id: "req-1", state: "pending" }), request({ request_id: "req-2", state: "denied" })],
          }),
        })}
      />,
    );
    expect(screen.getByText("History")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Approve" })).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: "Deny" })).toHaveLength(1);
  });

  it("shows an empty-open-queue message when only terminal requests exist", () => {
    render(<SessionRequests {...baseProps({ list: list({ requests: [request({ state: "started" })] }) })} />);
    expect(screen.getByText("No open requests.")).toBeInTheDocument();
  });
});
