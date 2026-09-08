import { useCallback, useRef, useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useExitEvents } from "./useExitEvents";
import { useWorktreeEvents } from "./useWorktreeEvents";
import type { SavedWorktree } from "./ipc";

// Regression cover for the pane-event ownership decision, under real React
// state and real React scheduling.
//
// A flag mutated inside a state updater cannot reliably report ownership to
// its caller: React may evaluate the updater eagerly or defer it. The batched
// cases below exercise deferral, which a synchronous setPanes mock misses.
//
// They are also written to fail on the obvious wrong fix. Answering "is there
// a pane in the committed array" instead is pure and still wrong: an install
// can have a pane's own update already queued and not yet committed when the
// event arrives, and retaining there strands the event on a pane that is about
// to exist and will never call `take()` again. The queued-install cases below
// are the ones that catch that.

const listeners = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, listener: (event: { payload: unknown }) => void) => {
    listeners.set(name, listener);
    return () => listeners.delete(name);
  }),
}));

const WORKTREE: SavedWorktree = {
  repo: "/repo",
  path: "/repo/.worktrees/sess-9",
  branch: "agent/sess-9",
  base: "main",
};

const emitExit = (id: string) => listeners.get("session-exited")!({ payload: id });
const emitWorktree = (id: string, worktree: SavedWorktree) =>
  listeners.get("session-worktree")!({ payload: { sessionId: id, ...worktree } });

type Pane = { id: string; alreadyExited?: boolean; worktree?: SavedWorktree };

// The same wiring App.tsx has, kept deliberately small: real `useState` panes,
// an ownership ref claimed at the decision rather than at the commit, and an
// `install` that reproduces `installRequestPane`'s exact order of operations
// (claim, take both events, then enqueue the pane).
function useHarness(initial: Pane[] = []) {
  const [panes, setPanes] = useState<Pane[]>(initial);
  const owned = useRef(new Set(initial.map((p) => p.id)));
  const isPaneOwned = useCallback((id: string) => owned.current.has(id), []);
  const exits = useExitEvents(setPanes, isPaneOwned);
  const worktrees = useWorktreeEvents(setPanes, isPaneOwned);
  const install = (id: string) => {
    owned.current.add(id);
    const worktree = worktrees.take(id);
    const alreadyExited = exits.take(id);
    setPanes((current) =>
      current.some((p) => p.id === id) ? current : [...current, { id, worktree, alreadyExited }],
    );
  };
  return {
    panes,
    install,
    takeExit: (id: string) => exits.take(id),
    takeWorktree: (id: string) => worktrees.take(id),
  };
}

describe("pane event ownership under real React scheduling", () => {
  beforeEach(() => listeners.clear());

  describe("session-exited", () => {
    it("applies to an already-installed pane and retains nothing", () => {
      const { result } = renderHook(() => useHarness([{ id: "sess-1" }]));
      act(() => emitExit("sess-1"));

      expect(result.current.panes[0].alreadyExited).toBe(true);
      expect(result.current.takeExit("sess-1")).toBe(false);
    });

    // The case above, on its own, does not actually catch the old code, and it
    // is worth being precise about why. React eagerly evaluates an updater at
    // dispatch time when the fiber has no work pending, purely to see whether
    // it can bail out of rendering. A single event arriving at an idle fiber
    // therefore did run the updater synchronously, and the flag read on the
    // next line happened to be right. The moment anything is already in
    // flight, which for two events emitted back to back is the ordinary case,
    // that shortcut is gone and the updater is deferred like any other.
    it("applies to already-installed panes and retains nothing when an update is already in flight", () => {
      const { result } = renderHook(() => useHarness([{ id: "sess-1" }, { id: "sess-2" }]));
      act(() => {
        emitExit("sess-1");
        emitExit("sess-2");
      });

      expect(result.current.panes[0].alreadyExited).toBe(true);
      expect(result.current.panes[1].alreadyExited).toBe(true);
      // The half the old code got wrong: the event was applied to a live pane
      // and must not also be sitting in PendingExits afterwards. Nothing will
      // ever come along to take it, and consume-on-read is the store's own
      // stated contract.
      expect(result.current.takeExit("sess-1")).toBe(false);
      expect(result.current.takeExit("sess-2")).toBe(false);
    });

    it("applies to a pane whose install is queued but not yet committed, and retains nothing", () => {
      const { result } = renderHook(() => useHarness());
      act(() => {
        result.current.install("sess-2");
        // Arrives while the pane's own update is queued and uncommitted, which
        // is the ordinary case for a requested/approved session.
        emitExit("sess-2");
      });

      expect(result.current.panes).toHaveLength(1);
      // Fails on a committed-panes-ref fix: no pane is committed at the moment
      // the event arrives, so it would be retained and this pane would never
      // learn that its session had already ended.
      expect(result.current.panes[0].alreadyExited).toBe(true);
      expect(result.current.takeExit("sess-2")).toBe(false);
    });

    it("still retains an event for a session no pane has been claimed for", () => {
      const { result } = renderHook(() => useHarness());
      act(() => emitExit("sess-3"));

      expect(result.current.panes).toHaveLength(0);
      act(() => result.current.install("sess-3"));
      expect(result.current.panes[0].alreadyExited).toBe(true);
      // Consumed by that install, not left behind for a second one.
      expect(result.current.takeExit("sess-3")).toBe(false);
    });
  });

  describe("session-worktree", () => {
    it("applies to an already-installed pane and retains nothing", () => {
      const { result } = renderHook(() => useHarness([{ id: "sess-1" }]));
      act(() => emitWorktree("sess-1", WORKTREE));

      expect(result.current.panes[0].worktree).toEqual(WORKTREE);
      expect(result.current.takeWorktree("sess-1")).toBeUndefined();
    });

    // See the exit case above for why a second event, arriving with an update
    // already in flight, is the one that actually pins this.
    it("applies to already-installed panes and retains nothing when an update is already in flight", () => {
      const other: SavedWorktree = { ...WORKTREE, path: "/repo/.worktrees/sess-2", branch: "agent/sess-2" };
      const { result } = renderHook(() => useHarness([{ id: "sess-1" }, { id: "sess-2" }]));
      act(() => {
        emitWorktree("sess-1", WORKTREE);
        emitWorktree("sess-2", other);
      });

      expect(result.current.panes[0].worktree).toEqual(WORKTREE);
      expect(result.current.panes[1].worktree).toEqual(other);
      expect(result.current.takeWorktree("sess-1")).toBeUndefined();
      expect(result.current.takeWorktree("sess-2")).toBeUndefined();
    });

    it("applies to a pane whose install is queued but not yet committed, and retains nothing", () => {
      const { result } = renderHook(() => useHarness());
      act(() => {
        result.current.install("sess-2");
        emitWorktree("sess-2", WORKTREE);
      });

      expect(result.current.panes).toHaveLength(1);
      expect(result.current.panes[0].worktree).toEqual(WORKTREE);
      expect(result.current.takeWorktree("sess-2")).toBeUndefined();
    });

    it("still retains an event for a session no pane has been claimed for", () => {
      const { result } = renderHook(() => useHarness());
      act(() => emitWorktree("sess-3", WORKTREE));

      expect(result.current.panes).toHaveLength(0);
      act(() => result.current.install("sess-3"));
      expect(result.current.panes[0].worktree).toEqual(WORKTREE);
      expect(result.current.takeWorktree("sess-3")).toBeUndefined();
    });
  });

  // Both events for one queued install, which is the isolated-and-immediately-
  // dead session: the backend reports the worktree and then the exit, both
  // before the approval that installs the pane has settled.
  it("carries both a worktree and an exit onto one pane installed after the fact", () => {
    const { result } = renderHook(() => useHarness());
    act(() => {
      emitWorktree("sess-4", WORKTREE);
      emitExit("sess-4");
    });
    act(() => result.current.install("sess-4"));

    expect(result.current.panes[0]).toMatchObject({
      id: "sess-4",
      worktree: WORKTREE,
      alreadyExited: true,
    });
  });

  // A second, racing install for the same session (a repeated approval) must
  // not produce a second pane, and must not find either event a second time.
  it("does not duplicate a pane or re-take an event when the same session is installed twice", () => {
    const { result } = renderHook(() => useHarness());
    act(() => {
      emitWorktree("sess-5", WORKTREE);
      emitExit("sess-5");
    });
    act(() => {
      result.current.install("sess-5");
      result.current.install("sess-5");
    });

    expect(result.current.panes).toHaveLength(1);
    expect(result.current.panes[0]).toMatchObject({ worktree: WORKTREE, alreadyExited: true });
  });

  // The updaters must be pure, so running one twice has to be indistinguishable
  // from running it once. StrictMode is what actually double-invokes them, and
  // it is what the old flag-inside-the-updater form abused.
  it("is unchanged when React double-invokes the updaters", async () => {
    const { StrictMode } = await import("react");
    const { result } = renderHook(() => useHarness([{ id: "sess-1" }]), { wrapper: StrictMode });
    act(() => {
      emitWorktree("sess-1", WORKTREE);
      emitExit("sess-1");
    });

    expect(result.current.panes).toHaveLength(1);
    expect(result.current.panes[0]).toMatchObject({ worktree: WORKTREE, alreadyExited: true });
    expect(result.current.takeWorktree("sess-1")).toBeUndefined();
    expect(result.current.takeExit("sess-1")).toBe(false);
  });
});
