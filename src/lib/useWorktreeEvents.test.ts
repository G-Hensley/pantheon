import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useWorktreeEvents, type WorktreePane } from "./useWorktreeEvents";
import type { SavedWorktree } from "./ipc";

type Listener = (ev: { payload: unknown }) => void;

const listenMock = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

const WORKTREE: SavedWorktree = {
  repo: "/repo",
  path: "/repo/.worktrees/sess-1",
  branch: "agent/sess-1",
  base: "main",
};

function emit(sessionId: string, worktree: SavedWorktree) {
  const calls = listenMock.mock.calls;
  const handler = calls[calls.length - 1]?.[1] as Listener;
  handler({ payload: { sessionId, ...worktree } });
}

// A fake pane array plus a `setPanes` that actually applies the updater the
// hook passes it, and the ownership answer the hook now requires from its
// caller. `own` records an id the way App's `addSession`/`installRequestPane`
// do, at the decision rather than at the commit, so a test can set up a pane
// the caller owns but has not added yet.
//
// This helper applies the updater synchronously, which React does not, so it
// can pin the hook's branching but could never have caught the impure-updater
// defect. The real-React proof lives in paneEventOwnership.regression.test.tsx.
function fakePanes(initial: WorktreePane[] = []) {
  const panes = [...initial];
  const owned = new Set(initial.map((p) => p.id));
  const setPanes = vi.fn((updater: WorktreePane[] | ((p: WorktreePane[]) => WorktreePane[])) => {
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

describe("useWorktreeEvents", () => {
  beforeEach(() => {
    listenMock.mockReset();
    listenMock.mockResolvedValue(() => {});
  });

  it("applies the event directly when a matching pane already exists (the manual-launch case)", () => {
    const { panes, setPanes, isPaneOwned } = fakePanes([{ id: "sess-1" }]);
    renderHook(() => useWorktreeEvents(setPanes, isPaneOwned));
    act(() => emit("sess-1", WORKTREE));
    expect(panes[0].worktree).toEqual(WORKTREE);
  });

  it("does not touch an unrelated pane already in the list", () => {
    const { panes, setPanes, isPaneOwned } = fakePanes([{ id: "sess-1" }, { id: "sess-2" }]);
    renderHook(() => useWorktreeEvents(setPanes, isPaneOwned));
    act(() => emit("sess-2", WORKTREE));
    expect(panes[0].worktree).toBeUndefined();
    expect(panes[1].worktree).toEqual(WORKTREE);
  });

  it("retains an event that arrives before its pane exists, and take() recovers it exactly once — the event-before-response scenario", () => {
    const { setPanes, isPaneOwned } = fakePanes([]);
    const { result } = renderHook(() => useWorktreeEvents(setPanes, isPaneOwned));
    act(() => emit("sess-9", WORKTREE));
    expect(result.current.take("sess-9")).toEqual(WORKTREE);
    // Consumed: a pane installed for this id only ever gets it once, so a
    // second, unrelated pane-adding call for the same id later cannot
    // inherit a stale worktree.
    expect(result.current.take("sess-9")).toBeUndefined();
  });

  it("take() for a session with no retained event returns undefined without side effects", () => {
    const { setPanes, isPaneOwned } = fakePanes([]);
    const { result } = renderHook(() => useWorktreeEvents(setPanes, isPaneOwned));
    expect(result.current.take("sess-1")).toBeUndefined();
  });

  // The queued install: the caller owns the id and has enqueued its pane, but
  // the pane is not in the array yet. Applying is correct, because the update
  // that adds the pane is already queued ahead of this one; retaining would
  // strand the worktree on a pane that will never call take() again.
  it("applies, rather than retains, for an owned session whose pane is not in the array yet", () => {
    const { setPanes, owned, isPaneOwned } = fakePanes([]);
    const { result } = renderHook(() => useWorktreeEvents(setPanes, isPaneOwned));
    owned.add("sess-7");
    act(() => emit("sess-7", WORKTREE));
    expect(setPanes).toHaveBeenCalledTimes(1);
    expect(result.current.take("sess-7")).toBeUndefined();
  });

  it("drops, rather than retains, an event for an owned pane that has since been closed", () => {
    const { panes, setPanes, isPaneOwned } = fakePanes([{ id: "sess-1" }]);
    const { result } = renderHook(() => useWorktreeEvents(setPanes, isPaneOwned));
    panes.length = 0;
    act(() => emit("sess-1", WORKTREE));
    expect(result.current.take("sess-1")).toBeUndefined();
  });

  it("a retained event does not leak into an unrelated pane already present when it arrived", () => {
    const { panes, setPanes, isPaneOwned } = fakePanes([{ id: "sess-1" }]);
    const { result } = renderHook(() => useWorktreeEvents(setPanes, isPaneOwned));
    act(() => emit("sess-9", WORKTREE));
    expect(panes[0].worktree).toBeUndefined();
    expect(result.current.take("sess-9")).toEqual(WORKTREE);
  });
});
