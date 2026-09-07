import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useDispatchBudget } from "./useDispatchBudget";
import { conductorState, resetDispatchBudget, type ConductorState } from "./ipc";

vi.mock("./ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./ipc")>();
  return { ...actual, resetDispatchBudget: vi.fn(), conductorState: vi.fn() };
});

afterEach(() => {
  vi.mocked(resetDispatchBudget).mockReset();
  vi.mocked(conductorState).mockReset();
});

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

function state(overrides: Partial<ConductorState> = {}): ConductorState {
  return {
    conductor: "sess-1",
    halted: false,
    tasks: [],
    dispatch_budget: { used: 0, limit: 40, remaining: 40 },
    ...overrides,
  };
}

describe("useDispatchBudget", () => {
  it("starts with no known budget and no pending/error state", () => {
    const { result } = renderHook(() => useDispatchBudget());
    expect(result.current.budget).toBeNull();
    expect(result.current.resetPending).toBe(false);
    expect(result.current.resetError).toBeNull();
  });

  it("applySnapshot reflects a regular poll/event read outside of a reset", () => {
    const { result } = renderHook(() => useDispatchBudget());
    const ticket = result.current.beginRead();
    act(() => result.current.applySnapshot({ used: 3, limit: 40, remaining: 37 }, ticket));
    expect(result.current.budget).toEqual({ used: 3, limit: 40, remaining: 37 });
  });

  it("beginRead issues strictly increasing tickets", () => {
    const { result } = renderHook(() => useDispatchBudget());
    const a = result.current.beginRead();
    const b = result.current.beginRead();
    const c = result.current.beginRead();
    expect(b).toBeGreaterThan(a);
    expect(c).toBeGreaterThan(b);
  });

  it("an older ticket never overwrites a newer one already applied, reset aside", () => {
    const { result } = renderHook(() => useDispatchBudget());
    const older = result.current.beginRead();
    const newer = result.current.beginRead();
    act(() => result.current.applySnapshot({ used: 9, limit: 40, remaining: 31 }, newer));
    act(() => result.current.applySnapshot({ used: 1, limit: 40, remaining: 39 }, older));
    // The newer poll's result stands even though the older one is applied
    // second; this is the general out-of-order-response case the same
    // ticket/latest-applied comparison handles, with no reset involved.
    expect(result.current.budget).toEqual({ used: 9, limit: 40, remaining: 31 });
  });

  it("is pending while the reset call is outstanding, and reconciles from a fresh conductor_state read on success", async () => {
    const resetCall = deferred<{ used: number; limit: number; remaining: number }>();
    vi.mocked(resetDispatchBudget).mockReturnValue(resetCall.promise);
    // Deliberately different from what resetDispatchBudget itself would
    // return, so a passing test proves the reconciling read's value is what
    // gets applied, not the reset command's own inline response.
    vi.mocked(conductorState).mockResolvedValue(
      state({ dispatch_budget: { used: 1, limit: 40, remaining: 39 } }),
    );
    const { result } = renderHook(() => useDispatchBudget());

    let resetPromise!: Promise<void>;
    act(() => {
      resetPromise = result.current.reset();
    });
    expect(result.current.resetPending).toBe(true);
    expect(result.current.resetError).toBeNull();
    expect(conductorState).not.toHaveBeenCalled();

    await act(async () => {
      resetCall.resolve({ used: 0, limit: 40, remaining: 40 });
      await resetPromise;
    });

    expect(conductorState).toHaveBeenCalledTimes(1);
    expect(result.current.resetPending).toBe(false);
    expect(result.current.resetError).toBeNull();
    expect(result.current.budget).toEqual({ used: 1, limit: 40, remaining: 39 });
  });

  it("on rejection of the reset call itself, leaves the last known budget unchanged and surfaces a visible error", async () => {
    const { result } = renderHook(() => useDispatchBudget());
    const ticket = result.current.beginRead();
    act(() => result.current.applySnapshot({ used: 5, limit: 40, remaining: 35 }, ticket));
    vi.mocked(resetDispatchBudget).mockRejectedValueOnce(new Error("ipc unavailable"));

    await act(async () => {
      await result.current.reset();
    });

    expect(result.current.resetPending).toBe(false);
    expect(result.current.resetError).toBe("ipc unavailable");
    expect(conductorState).not.toHaveBeenCalled();
    // The count is not silently zeroed or cleared just because the call failed.
    expect(result.current.budget).toEqual({ used: 5, limit: 40, remaining: 35 });
  });

  it("on rejection of the reconciling conductor_state read, surfaces a visible error rather than assuming success", async () => {
    const { result } = renderHook(() => useDispatchBudget());
    const ticket = result.current.beginRead();
    act(() => result.current.applySnapshot({ used: 5, limit: 40, remaining: 35 }, ticket));
    vi.mocked(resetDispatchBudget).mockResolvedValueOnce({ used: 0, limit: 40, remaining: 40 });
    vi.mocked(conductorState).mockRejectedValueOnce(new Error("backend not ready"));

    await act(async () => {
      await result.current.reset();
    });

    expect(result.current.resetPending).toBe(false);
    expect(result.current.resetError).toBe("backend not ready");
    // The reset command itself may well have succeeded on the backend, but
    // this frontend has no confirmed number to show, so the old one stands
    // rather than being replaced with a guess.
    expect(result.current.budget).toEqual({ used: 5, limit: 40, remaining: 35 });
  });

  it("a later successful reset clears an earlier error", async () => {
    const { result } = renderHook(() => useDispatchBudget());
    vi.mocked(resetDispatchBudget).mockRejectedValueOnce(new Error("first attempt failed"));
    await act(async () => {
      await result.current.reset();
    });
    expect(result.current.resetError).toBe("first attempt failed");

    vi.mocked(resetDispatchBudget).mockResolvedValueOnce({ used: 0, limit: 40, remaining: 40 });
    vi.mocked(conductorState).mockResolvedValueOnce(
      state({ dispatch_budget: { used: 0, limit: 40, remaining: 40 } }),
    );
    await act(async () => {
      await result.current.reset();
    });
    expect(result.current.resetError).toBeNull();
    expect(result.current.budget).toEqual({ used: 0, limit: 40, remaining: 40 });
  });

  it("discards a pre-reset ticket's result even after the reset and its reconciliation have both fully finished", async () => {
    // This is the case a plain in-flight boolean cannot cover: poll P starts
    // before the reset, and its response does not arrive until well after
    // reset() (including its own reconciling read) has already resolved and
    // pending has already gone back to false.
    const { result } = renderHook(() => useDispatchBudget());
    const staleTicket = result.current.beginRead(); // poll P "starts" here

    vi.mocked(resetDispatchBudget).mockResolvedValueOnce({ used: 0, limit: 40, remaining: 40 });
    vi.mocked(conductorState).mockResolvedValueOnce(
      state({ dispatch_budget: { used: 0, limit: 40, remaining: 40 } }),
    );
    await act(async () => {
      await result.current.reset();
    });
    expect(result.current.resetPending).toBe(false);
    expect(result.current.budget).toEqual({ used: 0, limit: 40, remaining: 40 });

    // Only now, well after the reset has fully settled, does P's stale
    // response finally land.
    act(() => result.current.applySnapshot({ used: 40, limit: 40, remaining: 0 }, staleTicket));

    // The fresh, reconciled count survives; the stale exhausted reading
    // from before the reset is rejected outright, not merely delayed.
    expect(result.current.budget).toEqual({ used: 0, limit: 40, remaining: 40 });
  });

  it("discards a pre-reset ticket's result when it lands during the reset instead", async () => {
    const resetCall = deferred<{ used: number; limit: number; remaining: number }>();
    vi.mocked(resetDispatchBudget).mockReturnValue(resetCall.promise);
    vi.mocked(conductorState).mockResolvedValue(
      state({ dispatch_budget: { used: 0, limit: 40, remaining: 40 } }),
    );
    const { result } = renderHook(() => useDispatchBudget());
    const preResetTicket = result.current.beginRead();
    act(() => result.current.applySnapshot({ used: 39, limit: 40, remaining: 1 }, preResetTicket));

    let resetPromise!: Promise<void>;
    act(() => {
      resetPromise = result.current.reset();
    });

    // The stale pre-reset poll's response lands mid-reset, on the same
    // ticket it was issued before the reset began.
    act(() => result.current.applySnapshot({ used: 40, limit: 40, remaining: 0 }, preResetTicket));
    expect(result.current.budget).toEqual({ used: 39, limit: 40, remaining: 1 }); // unchanged

    await act(async () => {
      resetCall.resolve({ used: 0, limit: 40, remaining: 40 });
      await resetPromise;
    });

    expect(result.current.budget).toEqual({ used: 0, limit: 40, remaining: 40 });
  });

  it("rejects a read whose ticket was issued during the reset window even if it resolves after the reconciling read", async () => {
    // A poll tick that starts after reset() is called but before
    // resetDispatchBudget() resolves is racing the backend's own reset
    // processing: there is no way to know client-side whether its number
    // reflects pre- or post-reset state. Its ticket is issued (and so is
    // always smaller than) the reconciling read's, which only starts once
    // resetDispatchBudget() has already resolved: so even if this read's
    // own response happens to land after the reconciling one, it must not
    // override it. The reconciling read is a floor on trustworthiness, not
    // just "whatever happened most recently."
    const { result } = renderHook(() => useDispatchBudget());
    vi.mocked(resetDispatchBudget).mockResolvedValueOnce({ used: 0, limit: 40, remaining: 40 });
    vi.mocked(conductorState).mockResolvedValueOnce(
      state({ dispatch_budget: { used: 0, limit: 40, remaining: 40 } }),
    );

    let midTicket!: number;
    await act(async () => {
      const resetPromise = result.current.reset();
      midTicket = result.current.beginRead();
      await resetPromise;
    });
    expect(result.current.budget).toEqual({ used: 0, limit: 40, remaining: 40 });

    // This mid-window read's response only arrives now, after reset() has
    // already fully settled.
    act(() => result.current.applySnapshot({ used: 7, limit: 40, remaining: 33 }, midTicket));
    expect(result.current.budget).toEqual({ used: 0, limit: 40, remaining: 40 });
  });

  it("resumes accepting ordinary reads once the reset has finished", async () => {
    const { result } = renderHook(() => useDispatchBudget());
    vi.mocked(resetDispatchBudget).mockResolvedValueOnce({ used: 0, limit: 40, remaining: 40 });
    vi.mocked(conductorState).mockResolvedValueOnce(
      state({ dispatch_budget: { used: 0, limit: 40, remaining: 40 } }),
    );
    await act(async () => {
      await result.current.reset();
    });
    expect(result.current.resetPending).toBe(false);

    const ticket = result.current.beginRead();
    act(() => result.current.applySnapshot({ used: 2, limit: 40, remaining: 38 }, ticket));
    expect(result.current.budget).toEqual({ used: 2, limit: 40, remaining: 38 });
  });

  it("calls the reset command exactly once per reset(), with no arguments an agent could supply", async () => {
    vi.mocked(resetDispatchBudget).mockResolvedValueOnce({ used: 0, limit: 40, remaining: 40 });
    vi.mocked(conductorState).mockResolvedValueOnce(state());
    const { result } = renderHook(() => useDispatchBudget());
    await act(async () => {
      await result.current.reset();
    });
    expect(resetDispatchBudget).toHaveBeenCalledTimes(1);
    expect(resetDispatchBudget).toHaveBeenCalledWith();
  });

  it("exposes no way to read or mutate halted or task state: it only tracks the budget", () => {
    const { result } = renderHook(() => useDispatchBudget());
    expect(Object.keys(result.current).sort()).toEqual(
      ["applySnapshot", "beginRead", "budget", "reset", "resetError", "resetPending"].sort(),
    );
  });
});
