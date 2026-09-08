import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// App-level cover for the half of the pane-event ownership fix that lives in
// App itself rather than in the hooks: `installRequestPane` claims the session
// id before it takes either event and before it enqueues the pane, so an event
// arriving while that pane's own update is still queued is applied to it
// rather than retained for a pane that will never ask again.
//
// The hook-level regressions in src/lib/paneEventOwnership.regression.test.tsx
// pin the hooks with a harness that mirrors this ordering. This file pins the
// real thing, because a harness cannot prove App itself still does it.

const listeners = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
const hooks = vi.hoisted(() => ({
  // Called synchronously by installRequestPane immediately after its setPanes,
  // which is exactly the moment the pane's update is queued and uncommitted.
  onSetAgentBrain: null as null | ((sessionId: string) => void),
  // The snapshot every list/initialize read returns. Swapped per test so the
  // restore barrier (allocator_ready) can be held closed.
  list: () => ({}) as Record<string, unknown>,
}));
const calls = vi.hoisted(() => ({ approve: 0, deny: 0, reset: 0 }));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, listener: (event: { payload: unknown }) => void) => {
    listeners.set(name, listener);
    return () => listeners.delete(name);
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => null) }));
vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: unknown = null;
  },
  invoke: vi.fn(async () => undefined),
}));

const REQUEST = {
  request_id: "req-1",
  kind: "claude",
  model: "claude-sonnet-5",
  model_verified: true,
  reason: "need a helper session",
  isolate: false,
  project: "/repo",
  brain: "main",
  requester: "sess-2",
  state: "pending",
  session_id: "sess-99",
  detail: null,
  created_ms: 0,
  updated_ms: 0,
};
const LIST = {
  requests: [REQUEST],
  startups: [],
  outstanding: 1,
  admitted: 0,
  outstanding_limit: 3,
  admitted_limit: 10,
  allocator_ready: true,
};

vi.mock("./lib/ipc", async () => {
  const actual = await vi.importActual<typeof import("./lib/ipc")>("./lib/ipc");
  return {
    ...actual,
    listSessionRequests: vi.fn(async () => hooks.list()),
    initializeSessions: vi.fn(async () => hooks.list()),
    approveSessionRequest: vi.fn(async () => {
      calls.approve += 1;
      return { ...REQUEST, state: "started" };
    }),
    denySessionRequest: vi.fn(async () => {
      calls.deny += 1;
      return hooks.list();
    }),
    resetSessionRequests: vi.fn(async () => {
      calls.reset += 1;
      return { ...hooks.list(), requests: [] };
    }),
    reserveSessionId: vi.fn(async () => "sess-1"),
    killSession: vi.fn(async () => undefined),
    conductorState: vi.fn(async () => ({
      conductor: null,
      halted: false,
      tasks: [],
      dispatch_budget: { used: 0, limit: 10, remaining: 10 },
    })),
    setConductor: vi.fn(async () => undefined),
    haltConductor: vi.fn(async () => undefined),
    setProject: vi.fn(async () => undefined),
    dispatchTask: vi.fn(async () => undefined),
    setAgentBrain: vi.fn(async (sessionId: string) => {
      hooks.onSetAgentBrain?.(sessionId);
    }),
  };
});

// The real one owns an xterm instance and a resize observer; neither is what
// this file is about. It renders the pane facts it is given so they can be
// asserted from the DOM.
vi.mock("./components/TerminalPane", () => ({
  TerminalPane: ({ sessionId, alreadyExited }: { sessionId: string; alreadyExited?: boolean }) => (
    <div data-testid="pane" data-session={sessionId} data-already-exited={String(!!alreadyExited)} />
  ),
}));

import App from "./App";

describe("App pane-event ownership", () => {
  beforeEach(() => {
    listeners.clear();
    hooks.onSetAgentBrain = null;
    hooks.list = () => LIST;
    calls.approve = 0;
    calls.deny = 0;
    calls.reset = 0;
    localStorage.clear();
  });

  it("applies a session-exited event that arrives while an approved pane's own update is still queued", async () => {
    // Fired from inside installRequestPane's own synchronous run, after
    // setPanes has been called and before React has committed it.
    hooks.onSetAgentBrain = (sessionId) => {
      listeners.get("session-exited")?.({ payload: sessionId });
    };

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /Session requests/ }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Approve" })).toBeEnabled());
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Approve" }));
    });

    const pane = await screen.findByTestId("pane");
    expect(pane).toHaveAttribute("data-session", "sess-99");
    // The claim in installRequestPane is what makes this true. Without it the
    // event is retained instead, for a pane that has already been installed
    // and will never call take() again, and this pane spends its life looking
    // like a session that is still running.
    expect(pane).toHaveAttribute("data-already-exited", "true");
  });

  it("still installs a pane normally when no event arrives", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /Session requests/ }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Approve" })).toBeEnabled());
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Approve" }));
    });

    const pane = await screen.findByTestId("pane");
    expect(pane).toHaveAttribute("data-already-exited", "false");
  });

  // Reset must not invalidate initialization's snapshot while restore is
  // pending. The handler guard complements the disabled control.
  describe("restore barrier", () => {
    const NOT_READY = { ...LIST, allocator_ready: false };

    async function openRequests() {
      hooks.list = () => NOT_READY;
      render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: /Session requests/ }));
      await waitFor(() =>
        expect(
          screen.getByText(
            "Restoring session state: approve, deny and reset are disabled until this finishes.",
          ),
        ).toBeInTheDocument(),
      );
    }

    it("disables approve, deny and reset together while the allocator is not ready", async () => {
      await openRequests();
      expect(screen.getByRole("button", { name: "Approve" })).toBeDisabled();
      expect(screen.getByRole("button", { name: "Deny" })).toBeDisabled();
      expect(screen.getByRole("button", { name: "Reset" })).toBeDisabled();
    });

    // Two barriers, and it is worth recording which one this proves. jsdom
    // does not dispatch a click on a disabled button, so with the buttons gated
    // this passes on the button gate alone. Removing only the button gate and
    // re-running it is what shows the handler guards are live: it still passes,
    // because the handlers refuse on their own. Removing both is what makes it
    // fail. Measured, in that order, rather than assumed.
    it("reaches no backend call if a barred button is clicked anyway", async () => {
      await openRequests();
      for (const name of ["Approve", "Deny", "Reset"]) {
        await act(async () => {
          fireEvent.click(screen.getByRole("button", { name }));
        });
      }
      expect(calls).toEqual({ approve: 0, deny: 0, reset: 0 });
    });

    it("admits all three once the allocator is ready", async () => {
      render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: /Session requests/ }));
      await waitFor(() => expect(screen.getByRole("button", { name: "Reset" })).toBeEnabled());
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Deny" }));
      });
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Reset" }));
      });
      expect(calls.deny).toBe(1);
      expect(calls.reset).toBe(1);
    });
  });
});
