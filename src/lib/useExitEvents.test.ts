import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useExitEvents, type ExitTrackedPane } from "./useExitEvents";

type Listener = (ev: { payload: string }) => void;

const listenMock = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

function emit(sessionId: string) {
  const calls = listenMock.mock.calls;
  const handler = calls[calls.length - 1]?.[1] as Listener;
  handler({ payload: sessionId });
}

// A fake pane array plus a `setPanes` that actually applies the updater the
// hook passes it, the same helper shape as useWorktreeEvents.test.ts, plus the
// ownership answer the hook now requires from its caller. `own` records an id
// the way App's `addSession`/`installRequestPane` do, at the decision rather
// than at the commit, so a test can set up a pane the caller owns but has not
// added yet.
//
// Note what this helper cannot show: it applies the updater synchronously,
// which React does not. That is deliberate here (these are unit tests of the
// hook's own branching) and it is exactly why it could never have caught the
// impure-updater defect. The real-React proof lives in
// paneEventOwnership.regression.test.tsx.
function fakePanes(initial: ExitTrackedPane[] = []) {
  const panes = [...initial];
  const owned = new Set(initial.map((p) => p.id));
  const setPanes = vi.fn((updater: ExitTrackedPane[] | ((p: ExitTrackedPane[]) => ExitTrackedPane[])) => {
    const next = typeof updater === "function" ? updater(panes) : updater;
    panes.length = 0;
    panes.push(...next);
  });
  return {
    panes,
    setPanes,
    owned,
    isPaneOwned: (sessionId: string) => owned.has(sessionId),
  };
}

describe("useExitEvents", () => {
  beforeEach(() => {
    listenMock.mockReset();
    listenMock.mockResolvedValue(() => {});
  });

  it("retains an event that arrives before its pane exists, and take() recovers it exactly once — the event-before-mount scenario", () => {
    const { setPanes, isPaneOwned } = fakePanes([]);
    const { result } = renderHook(() => useExitEvents(setPanes, isPaneOwned));
    act(() => emit("sess-9"));
    expect(result.current.take("sess-9")).toBe(true);
    // Consumed: a pane installed for this id only ever learns it once, and a
    // session cannot exit a second time to resend the event.
    expect(result.current.take("sess-9")).toBe(false);
  });

  it("patches alreadyExited directly onto an existing pane instead of relying on a listener that may not be subscribed yet", () => {
    const { panes, setPanes, isPaneOwned } = fakePanes([{ id: "sess-1" }]);
    const { result } = renderHook(() => useExitEvents(setPanes, isPaneOwned));
    act(() => emit("sess-1"));
    // TerminalPane's own per-pane listener registers via an async listen()
    // call and is not guaranteed to have subscribed yet at this point (the
    // pre-listener-window race) — so this hook is the one that marks the
    // fact directly, and TerminalPane reacts to the prop change instead of
    // depending on catching the event itself. Never retained, since a match
    // was found; a later take() for this id finds nothing left to recover.
    expect(setPanes).toHaveBeenCalledTimes(1);
    expect(panes[0].alreadyExited).toBe(true);
    expect(result.current.take("sess-1")).toBe(false);
  });

  it("does not retain, and does not re-patch, a session whose pane already shows alreadyExited", () => {
    const { panes, setPanes, isPaneOwned } = fakePanes([{ id: "sess-1", alreadyExited: true }]);
    const { result } = renderHook(() => useExitEvents(setPanes, isPaneOwned));
    act(() => emit("sess-1"));
    expect(panes[0].alreadyExited).toBe(true);
    expect(result.current.take("sess-1")).toBe(false);
  });

  it("take() for a session with no retained event returns false without side effects", () => {
    const { setPanes, isPaneOwned } = fakePanes([]);
    const { result } = renderHook(() => useExitEvents(setPanes, isPaneOwned));
    expect(result.current.take("sess-1")).toBe(false);
  });

  // The queued install: the caller has taken ownership of the id and enqueued
  // the pane, but the pane is not in the array yet. Applying is correct here,
  // because the update that adds the pane is already queued ahead of this one.
  it("applies, rather than retains, for an owned session whose pane is not in the array yet", () => {
    const { setPanes, owned, isPaneOwned } = fakePanes([]);
    const { result } = renderHook(() => useExitEvents(setPanes, isPaneOwned));
    owned.add("sess-7");
    act(() => emit("sess-7"));
    expect(setPanes).toHaveBeenCalledTimes(1);
    expect(result.current.take("sess-7")).toBe(false);
  });

  // Ownership is never given up, so an event for a pane the user has since
  // closed is dropped instead of accumulating for a pane that is not coming
  // back. The pane list no longer holds it, so the updater is a no-op.
  it("drops, rather than retains, an event for an owned pane that has since been closed", () => {
    const { panes, setPanes, isPaneOwned } = fakePanes([{ id: "sess-1" }]);
    const { result } = renderHook(() => useExitEvents(setPanes, isPaneOwned));
    panes.length = 0;
    act(() => emit("sess-1"));
    expect(result.current.take("sess-1")).toBe(false);
  });

  it("tracks multiple in-flight sessions independently", () => {
    const { setPanes, isPaneOwned } = fakePanes([]);
    const { result } = renderHook(() => useExitEvents(setPanes, isPaneOwned));
    act(() => {
      emit("sess-1");
      emit("sess-2");
    });
    expect(result.current.take("sess-2")).toBe(true);
    expect(result.current.take("sess-1")).toBe(true);
  });
});
