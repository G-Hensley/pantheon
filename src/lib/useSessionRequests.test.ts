import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { type Channel } from "@tauri-apps/api/core";
import { useSessionRequests, type ApproveResult } from "./useSessionRequests";
import {
  approveSessionRequest,
  denySessionRequest,
  initializeSessions,
  listSessionRequests,
  resetSessionRequests,
  type Bytes,
  type SessionRequest,
  type SessionRequestList,
} from "./ipc";

vi.mock("./ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./ipc")>();
  return {
    ...actual,
    approveSessionRequest: vi.fn(),
    denySessionRequest: vi.fn(),
    initializeSessions: vi.fn(),
    listSessionRequests: vi.fn(),
    resetSessionRequests: vi.fn(),
  };
});

beforeAll(() => {
  // `new Channel()` (real class, imported from @tauri-apps/api/core and not
  // mocked here — approve() constructing the actual thing production code
  // uses is the point) registers a callback with Tauri's IPC bridge at
  // construction time. Nothing in this file needs the real bridge: onmessage
  // assignment and invocation both go through the class's own private field,
  // never through this stand-in, so a bare id generator is enough.
  let nextId = 1;
  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    transformCallback: () => nextId++,
    unregisterCallback: () => {},
  };
});

afterEach(() => {
  vi.mocked(approveSessionRequest).mockReset();
  vi.mocked(denySessionRequest).mockReset();
  vi.mocked(initializeSessions).mockReset();
  vi.mocked(listSessionRequests).mockReset();
  vi.mocked(resetSessionRequests).mockReset();
});

function request(overrides: Partial<SessionRequest> = {}): SessionRequest {
  return {
    request_id: "req-1",
    kind: "claude",
    model: "claude-sonnet-5",
    model_verified: true,
    reason: "need a helper session",
    isolate: false,
    project: "/repo",
    brain: "main",
    requester: "sess-1",
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

// A promise this test can resolve/reject on its own schedule, standing in
// for an IPC response arriving after some delay.
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (err: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("useSessionRequests", () => {
  it("starts with the empty list, allocator not ready", () => {
    const { result } = renderHook(() => useSessionRequests());
    expect(result.current.list).toEqual({
      requests: [],
      startups: [],
      outstanding: 0,
      admitted: 0,
      outstanding_limit: 0,
      admitted_limit: 0,
      allocator_ready: false,
    });
  });

  it("beginRead issues strictly increasing tickets", () => {
    const { result } = renderHook(() => useSessionRequests());
    const a = result.current.beginRead();
    const b = result.current.beginRead();
    expect(b).toBeGreaterThan(a);
  });

  it("an older ticket never overwrites a newer one already applied (ordinary poll race)", () => {
    const { result } = renderHook(() => useSessionRequests());
    const older = result.current.beginRead();
    const newer = result.current.beginRead();
    act(() => result.current.applySnapshot(list({ admitted: 2 }), newer));
    act(() => result.current.applySnapshot(list({ admitted: 9 }), older));
    expect(result.current.list.admitted).toBe(2);
  });

  describe("initialize (the restore barrier)", () => {
    it("calls initialize_sessions with exactly the given restored ids and applies its snapshot", async () => {
      vi.mocked(initializeSessions).mockResolvedValue(list({ allocator_ready: true }));
      const { result } = renderHook(() => useSessionRequests());
      await act(async () => {
        await result.current.initialize(["sess-1", "sess-2"]);
      });
      expect(initializeSessions).toHaveBeenCalledWith(["sess-1", "sess-2"]);
      expect(result.current.list.allocator_ready).toBe(true);
    });

    it("invalidates a poll ticket already issued, even if that poll's response lands after initialize has fully finished", async () => {
      // Mirrors the restore-barrier requirement directly: a regular list
      // read that started before the frontend told the backend which ids
      // it restored must never be allowed to stand once the barrier's own
      // result is in, no matter how late that stale read resolves.
      const { result } = renderHook(() => useSessionRequests());
      const staleTicket = result.current.beginRead();
      vi.mocked(initializeSessions).mockResolvedValue(list({ allocator_ready: true, admitted: 0 }));
      await act(async () => {
        await result.current.initialize([]);
      });
      act(() => result.current.applySnapshot(list({ allocator_ready: false, admitted: 5 }), staleTicket));
      expect(result.current.list.allocator_ready).toBe(true);
      expect(result.current.list.admitted).toBe(0);
    });
  });

  describe("approve", () => {
    it("assigns the channel's onmessage handler before the approve call resolves, so output the backend sends during it is buffered rather than lost", async () => {
      const early = new Uint8Array([1, 2, 3]).buffer;
      vi.mocked(approveSessionRequest).mockImplementation(async (_id, _model, channel) => {
        // Simulate the backend already writing to this channel before the
        // command's own promise settles: the exact race the frozen
        // contract requires the frontend to survive without losing bytes.
        (channel as Channel<Bytes>).onmessage(early);
        return request({ state: "started", session_id: "sess-9" });
      });
      vi.mocked(listSessionRequests).mockResolvedValue(list());

      const { result } = renderHook(() => useSessionRequests());
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", null, 24, 80);
        await Promise.resolve();
      });

      expect(outcome.ok).toBe(true);
      if (outcome.ok) {
        expect(outcome.buffered).toEqual([early]);
        expect(outcome.sessionId).toBe("sess-9");
      }
    });

    it("on success, reconciles the visible list from a follow-up list_session_requests call, not the approve response", async () => {
      vi.mocked(approveSessionRequest).mockResolvedValue(request({ state: "started", session_id: "sess-9" }));
      vi.mocked(listSessionRequests).mockResolvedValue(
        list({ requests: [request({ state: "started", session_id: "sess-9" })], outstanding: 0, admitted: 1 }),
      );
      const { result } = renderHook(() => useSessionRequests());
      await act(async () => {
        await result.current.approve("req-1", null, 24, 80);
        await Promise.resolve();
      });
      expect(listSessionRequests).toHaveBeenCalledTimes(1);
      expect(result.current.list.admitted).toBe(1);
    });

    it("resolves ok with the backend's own session id on success", async () => {
      vi.mocked(approveSessionRequest).mockResolvedValue(request({ state: "started", session_id: "sess-9" }));
      vi.mocked(listSessionRequests).mockResolvedValue(list());
      const { result } = renderHook(() => useSessionRequests());
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", "claude-opus-5", 24, 80);
      });
      expect(approveSessionRequest).toHaveBeenCalledWith("req-1", "claude-opus-5", expect.anything(), 24, 80);
      expect(outcome).toMatchObject({ ok: true, sessionId: "sess-9" });
    });

    it("resolves not-ok, surfacing the detail, when the request ends denied", async () => {
      vi.mocked(approveSessionRequest).mockResolvedValue(request({ state: "denied", detail: "denied by human" }));
      vi.mocked(listSessionRequests).mockResolvedValue(list());
      const { result } = renderHook(() => useSessionRequests());
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", null, 24, 80);
      });
      expect(outcome).toEqual({ ok: false, error: "denied by human" });
    });

    it("resolves not-ok, surfacing the detail, when the request ends failed", async () => {
      vi.mocked(approveSessionRequest).mockResolvedValue(
        request({ state: "failed", detail: "spawn refused: isolation unavailable" }),
      );
      vi.mocked(listSessionRequests).mockResolvedValue(list());
      const { result } = renderHook(() => useSessionRequests());
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", null, 24, 80);
      });
      expect(outcome).toEqual({ ok: false, error: "spawn refused: isolation unavailable" });
    });

    it("carries the authoritative record, which can differ from the edit this call requested (e.g. a losing race against an earlier winning approval)", async () => {
      // approve_session_request's own atomic claim means a second Approve
      // (or one racing a Deny/Stop/context change) reads back whatever
      // already settled — not this call's own edit. The caller must install
      // from `request`, never from the edit it passed in.
      vi.mocked(approveSessionRequest).mockResolvedValue(
        request({ state: "started", session_id: "sess-9", model: "claude-opus-5-already-committed" }),
      );
      vi.mocked(listSessionRequests).mockResolvedValue(list());
      const { result } = renderHook(() => useSessionRequests());
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", "claude-sonnet-5-this-calls-own-edit", 24, 80);
      });
      expect(outcome.ok).toBe(true);
      if (outcome.ok) {
        expect(outcome.request.model).toBe("claude-opus-5-already-committed");
      }
    });

    it("never reports ok without a session id, even for an unexpected terminal state (e.g. stale)", async () => {
      vi.mocked(approveSessionRequest).mockResolvedValue(request({ state: "stale", session_id: null }));
      const { result } = renderHook(() => useSessionRequests());
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", null, 24, 80);
      });
      expect(outcome.ok).toBe(false);
    });

    it("on an RPC rejection, polls list_session_requests for the authoritative outcome instead of assuming failure", async () => {
      vi.mocked(approveSessionRequest).mockRejectedValue(new Error("rpc timeout"));
      vi.mocked(listSessionRequests).mockResolvedValue(
        list({ requests: [request({ state: "started", session_id: "sess-9" })] }),
      );
      const { result } = renderHook(() => useSessionRequests());
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", null, 24, 80);
      });
      expect(outcome).toMatchObject({ ok: true, sessionId: "sess-9" });
    });

    it("on an RPC rejection, reports the rejection's own message when the reconciling poll shows nothing conclusive", async () => {
      vi.mocked(approveSessionRequest).mockRejectedValue(new Error("rpc timeout"));
      vi.mocked(listSessionRequests).mockResolvedValue(list({ requests: [request({ state: "pending" })] }));
      const { result } = renderHook(() => useSessionRequests());
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", null, 24, 80);
      });
      expect(outcome).toEqual({ ok: false, error: "rpc timeout" });
    });

    it("on an RPC rejection, reports the original rejection even when the reconciling poll itself also fails", async () => {
      vi.mocked(approveSessionRequest).mockRejectedValue(new Error("rpc timeout"));
      vi.mocked(listSessionRequests).mockRejectedValue(new Error("backend not ready"));
      const { result } = renderHook(() => useSessionRequests());
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", null, 24, 80);
      });
      expect(outcome).toEqual({ ok: false, error: "rpc timeout" });
    });
  });

  describe("approve — uncertain outcomes retain the attachment", () => {
    it("the exact scenario named: invoke rejects, the first poll fails, a later snapshot reports Started — one approval call, one channel, one ready attachment, all early and late bytes forwarded", async () => {
      const early = new Uint8Array([1]).buffer;
      const late = new Uint8Array([2]).buffer;
      let capturedChannel!: Channel<Bytes>;
      vi.mocked(approveSessionRequest).mockImplementation(async (_id, _model, channel) => {
        capturedChannel = channel as Channel<Bytes>;
        capturedChannel.onmessage(early);
        throw new Error("rpc timeout");
      });
      vi.mocked(listSessionRequests).mockRejectedValueOnce(new Error("backend not ready"));

      const { result } = renderHook(() => useSessionRequests());
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", null, 24, 80);
      });

      expect(outcome).toEqual({ ok: false, error: "rpc timeout" });
      expect(approveSessionRequest).toHaveBeenCalledTimes(1);
      expect(result.current.readyAttachments).toEqual([]);

      // More output arrives while the outcome is still unknown to this
      // session — it must still land in the same buffer, not be dropped
      // for lack of anywhere to go.
      act(() => capturedChannel.onmessage(late));

      // A later snapshot notices Started — this is deliberately NOT another
      // approve() call, but exactly what App.tsx's regular poll/event
      // listener does: an ordinary applySnapshot from an unrelated read.
      act(() => {
        const ticket = result.current.beginRead();
        result.current.applySnapshot(
          list({ requests: [request({ state: "started", session_id: "sess-9" })] }),
          ticket,
        );
      });

      expect(approveSessionRequest).toHaveBeenCalledTimes(1); // still just the one call
      expect(result.current.readyAttachments).toHaveLength(1);
      const ready = result.current.readyAttachments[0];
      expect(ready.request.session_id).toBe("sess-9");
      expect(ready.channel).toBe(capturedChannel); // the original channel, not a new one
      expect(ready.buffered).toEqual([early, late]);
    });

    it("a manual retry while still uncertain does not invoke approve_session_request again or create a new channel", async () => {
      let capturedChannel!: Channel<Bytes>;
      vi.mocked(approveSessionRequest).mockImplementation(async (_id, _model, channel) => {
        capturedChannel = channel as Channel<Bytes>;
        throw new Error("rpc timeout");
      });
      vi.mocked(listSessionRequests).mockResolvedValueOnce(list({ requests: [request({ state: "pending" })] }));
      const { result } = renderHook(() => useSessionRequests());
      await act(async () => {
        await result.current.approve("req-1", null, 24, 80);
      });
      expect(approveSessionRequest).toHaveBeenCalledTimes(1);

      // The retry: same request id, still uncertain. Its own status check
      // now finds it Started.
      vi.mocked(listSessionRequests).mockResolvedValueOnce(
        list({ requests: [request({ state: "started", session_id: "sess-9" })] }),
      );
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", null, 24, 80);
      });

      expect(approveSessionRequest).toHaveBeenCalledTimes(1); // never called a second time
      expect(outcome.ok).toBe(true);
      if (outcome.ok) {
        expect(outcome.channel).toBe(capturedChannel); // the original attempt's own channel
        expect(outcome.sessionId).toBe("sess-9");
      }
    });

    it("a retry that still finds the request pending reports uncertainty again without discarding the attachment", async () => {
      vi.mocked(approveSessionRequest).mockRejectedValueOnce(new Error("rpc timeout"));
      vi.mocked(listSessionRequests).mockResolvedValueOnce(list({ requests: [request({ state: "pending" })] }));
      const { result } = renderHook(() => useSessionRequests());
      await act(async () => {
        await result.current.approve("req-1", null, 24, 80);
      });

      vi.mocked(listSessionRequests).mockResolvedValueOnce(list({ requests: [request({ state: "launching" })] }));
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", null, 24, 80);
      });
      expect(outcome.ok).toBe(false);
      expect(approveSessionRequest).toHaveBeenCalledTimes(1); // the retry never re-invoked

      // The attachment is still alive: a snapshot showing Started still
      // resolves it afterwards.
      act(() => {
        const ticket = result.current.beginRead();
        result.current.applySnapshot(
          list({ requests: [request({ state: "started", session_id: "sess-9" })] }),
          ticket,
        );
      });
      expect(result.current.readyAttachments).toHaveLength(1);
    });

    it("a confirmed terminal failure arriving later via a passive snapshot releases the attachment, with nothing to install", async () => {
      vi.mocked(approveSessionRequest).mockRejectedValue(new Error("rpc timeout"));
      vi.mocked(listSessionRequests).mockResolvedValue(list({ requests: [request({ state: "pending" })] }));
      const { result } = renderHook(() => useSessionRequests());
      await act(async () => {
        await result.current.approve("req-1", null, 24, 80);
      });

      act(() => {
        const ticket = result.current.beginRead();
        result.current.applySnapshot(list({ requests: [request({ state: "denied" })] }), ticket);
      });
      expect(result.current.readyAttachments).toEqual([]);

      // Proves the attachment was actually released, not merely skipped
      // once: a later snapshot claiming the same id is now Started must not
      // retroactively surface it, since nothing legitimately transitions a
      // request from denied back to started.
      act(() => {
        const ticket = result.current.beginRead();
        result.current.applySnapshot(
          list({ requests: [request({ state: "started", session_id: "sess-9" })] }),
          ticket,
        );
      });
      expect(result.current.readyAttachments).toEqual([]);
    });

    it("consumeReady removes an installed attachment so it is never handed out twice", async () => {
      vi.mocked(approveSessionRequest).mockRejectedValue(new Error("rpc timeout"));
      vi.mocked(listSessionRequests).mockResolvedValue(list({ requests: [request({ state: "pending" })] }));
      const { result } = renderHook(() => useSessionRequests());
      await act(async () => {
        await result.current.approve("req-1", null, 24, 80);
      });
      act(() => {
        const ticket = result.current.beginRead();
        result.current.applySnapshot(
          list({ requests: [request({ state: "started", session_id: "sess-9" })] }),
          ticket,
        );
      });
      expect(result.current.readyAttachments).toHaveLength(1);

      act(() => result.current.consumeReady(["req-1"]));
      expect(result.current.readyAttachments).toEqual([]);
    });

    it("a deferred rejection resolved well after the fact still reconciles correctly (no premature resolution from a promise that has not settled yet)", async () => {
      const rpc = deferred<SessionRequest>();
      let capturedChannel!: Channel<Bytes>;
      vi.mocked(approveSessionRequest).mockImplementation(async (_id, _model, channel) => {
        capturedChannel = channel as Channel<Bytes>;
        return rpc.promise;
      });
      const { result } = renderHook(() => useSessionRequests());
      let approvePromise!: Promise<ApproveResult>;
      act(() => {
        approvePromise = result.current.approve("req-1", null, 24, 80);
      });

      // Nothing has settled yet: no call has been made to reconcile, no
      // ready attachment exists.
      expect(result.current.readyAttachments).toEqual([]);

      await act(async () => {
        rpc.reject(new Error("rpc timeout"));
        vi.mocked(listSessionRequests).mockResolvedValueOnce(list({ requests: [request({ state: "pending" })] }));
        await approvePromise;
      });
      expect(result.current.readyAttachments).toEqual([]);

      act(() => {
        const ticket = result.current.beginRead();
        result.current.applySnapshot(
          list({ requests: [request({ state: "started", session_id: "sess-9" })] }),
          ticket,
        );
      });
      expect(result.current.readyAttachments).toHaveLength(1);
      expect(result.current.readyAttachments[0].channel).toBe(capturedChannel);
    });
  });

  describe("approve — reserved before the first invoke settles", () => {
    it("retains one attachment when approval is repeated before the first RPC settles", async () => {
      // The regression root reproduced: the reservation used to happen only
      // once an invoke had already rejected, leaving the whole window while
      // the first call's own invoke is still in flight unguarded. A second
      // approve() for the same request in that window created a second
      // channel and invoked approve_session_request again, even though the
      // backend's own claim is a one-time atomic transition.
      const rpc = deferred<SessionRequest>();
      let invocationCount = 0;
      let capturedChannel!: Channel<Bytes>;
      vi.mocked(approveSessionRequest).mockImplementation(async (_id, _model, channel) => {
        invocationCount += 1;
        capturedChannel = channel as Channel<Bytes>;
        return rpc.promise;
      });
      // The second call's own status check, taken while the first is still
      // unresolved: the backend reports the leader's own in-progress claim.
      vi.mocked(listSessionRequests).mockResolvedValue(list({ requests: [request({ state: "launching" })] }));

      const { result } = renderHook(() => useSessionRequests());
      let firstPromise!: Promise<ApproveResult>;
      let secondOutcome!: ApproveResult;
      await act(async () => {
        firstPromise = result.current.approve("req-1", null, 24, 80);
        secondOutcome = await result.current.approve("req-1", null, 24, 80);
      });

      expect(approveSessionRequest).toHaveBeenCalledTimes(1);
      expect(secondOutcome.ok).toBe(false);

      let firstOutcome!: ApproveResult;
      await act(async () => {
        rpc.resolve(request({ state: "started", session_id: "sess-9" }));
        firstOutcome = await firstPromise;
      });

      expect(invocationCount).toBe(1);
      expect(approveSessionRequest).toHaveBeenCalledTimes(1);
      expect(firstOutcome.ok).toBe(true);
      if (firstOutcome.ok) {
        expect(firstOutcome.channel).toBe(capturedChannel);
        expect(firstOutcome.sessionId).toBe("sess-9");
      }
      // The winning call's own direct return is what installs the pane here,
      // not a passive reconciliation.
      expect(result.current.readyAttachments).toEqual([]);
    });

    it("does not treat a resolved (non-rejected) Launching response as success, and releases the reservation so a later snapshot cannot promote this call's unattached channel", async () => {
      // A resolved (never rejected) response reporting anything short of
      // Started or a terminal failure only happens when this call's own
      // invoke did not win the backend's atomic claim: an already-Launching
      // request read back its leader's in-progress state, without the
      // backend ever attaching this call's channel to anything.
      vi.mocked(approveSessionRequest).mockResolvedValue(
        request({ state: "launching", session_id: "sess-9" }),
      );
      const { result } = renderHook(() => useSessionRequests());
      let outcome!: ApproveResult;
      await act(async () => {
        outcome = await result.current.approve("req-1", null, 24, 80);
      });
      expect(outcome.ok).toBe(false);
      expect(result.current.readyAttachments).toEqual([]);

      // Proves the reservation was actually released, not merely skipped
      // once: a later snapshot claiming this same request is now Started
      // must not surface a readyAttachment for it — this call's own channel
      // was never attached to anything and never will be.
      act(() => {
        const ticket = result.current.beginRead();
        result.current.applySnapshot(
          list({ requests: [request({ state: "started", session_id: "sess-9" })] }),
          ticket,
        );
      });
      expect(result.current.readyAttachments).toEqual([]);
    });
  });

  describe("deny", () => {
    it("calls deny_session_request with the request id, then reconciles the full list from a fresh read", async () => {
      vi.mocked(denySessionRequest).mockResolvedValue(request({ state: "denied" }));
      vi.mocked(listSessionRequests).mockResolvedValue(
        list({ requests: [request({ state: "denied" })], outstanding: 0 }),
      );
      const { result } = renderHook(() => useSessionRequests());
      await act(async () => {
        await result.current.deny("req-1");
      });
      expect(denySessionRequest).toHaveBeenCalledWith("req-1");
      expect(listSessionRequests).toHaveBeenCalledTimes(1);
      expect(result.current.list.requests[0]?.state).toBe("denied");
    });
  });

  describe("reset", () => {
    it("applies reset_session_requests' own returned snapshot directly, with no follow-up list read", async () => {
      vi.mocked(resetSessionRequests).mockResolvedValue(list({ admitted: 2 }));
      const { result } = renderHook(() => useSessionRequests());
      await act(async () => {
        await result.current.reset();
      });
      expect(resetSessionRequests).toHaveBeenCalledTimes(1);
      expect(listSessionRequests).not.toHaveBeenCalled();
      expect(result.current.list.admitted).toBe(2);
    });

    it("a poll ticket issued before reset never overwrites reset's own result, however late it resolves", async () => {
      const { result } = renderHook(() => useSessionRequests());
      const staleTicket = result.current.beginRead();
      vi.mocked(resetSessionRequests).mockResolvedValue(list({ admitted: 0 }));
      await act(async () => {
        await result.current.reset();
      });
      act(() => result.current.applySnapshot(list({ admitted: 99 }), staleTicket));
      // Reset preserving active requests is a backend behavior; what the
      // frontend must get right is exactly this — the reset it just asked
      // for cannot be undone by a read that predates it.
      expect(result.current.list.admitted).toBe(0);
    });
  });

  it("exposes exactly list/beginRead/applySnapshot/initialize/approve/deny/reset/readyAttachments/consumeReady", () => {
    const { result } = renderHook(() => useSessionRequests());
    expect(Object.keys(result.current).sort()).toEqual(
      [
        "applySnapshot",
        "approve",
        "beginRead",
        "consumeReady",
        "deny",
        "initialize",
        "list",
        "readyAttachments",
        "reset",
      ].sort(),
    );
  });
});
