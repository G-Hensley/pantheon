import { useCallback, useRef, useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useExitEvents } from "./useExitEvents";
import { useWorktreeEvents } from "./useWorktreeEvents";
import type { SavedWorktree } from "./ipc";

const listeners = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, listener: (event: { payload: unknown }) => void) => {
    listeners.set(name, listener);
    return () => listeners.delete(name);
  }),
}));
type Pane = { id: string; status: string; alreadyExited: boolean; worktree?: SavedWorktree };
function useHarness() {
  const [panes, setPanes] = useState<Pane[]>([]);
  // Mirrors App.tsx's ownedPaneIds: an id is claimed at the moment the install
  // is decided, not when React commits the pane, which is the whole point in a
  // test built around queued updates.
  const owned = useRef(new Set<string>());
  const isPaneOwned = useCallback((id: string) => owned.current.has(id), []);
  const exits = useExitEvents(setPanes, isPaneOwned);
  const worktrees = useWorktreeEvents(setPanes, isPaneOwned);
  return {
    panes,
    install(id: string) {
      owned.current.add(id);
      const alreadyExited = exits.take(id);
      const worktree = worktrees.take(id);
      setPanes((current) => [...current, { id, alreadyExited, worktree, status: alreadyExited ? "exited" : "running" }]);
    },
  };
}
describe("request lifecycle with queued React updates", () => {
  beforeEach(() => listeners.clear());
  it("retains exit between installation request and pane mount", () => {
    const { result } = renderHook(useHarness);
    act(() => {
      result.current.install("sess-99");
      listeners.get("session-exited")!({ payload: "sess-99" });
    });
    const pane = result.current.panes[0];
    expect(pane.alreadyExited || pane.status === "exited").toBe(true);
  });
  it("preserves an early worktree through queued installation and roster serialization", () => {
    const worktree: SavedWorktree = { repo: "/repo", path: "/repo/.worktrees/sess-99", branch: "agent/sess-99", base: "main" };
    const { result } = renderHook(useHarness);
    act(() => {
      listeners.get("session-worktree")!({ payload: { sessionId: "sess-99", ...worktree } });
      result.current.install("sess-99");
    });
    const restored = JSON.parse(JSON.stringify(result.current.panes));
    expect(restored[0].worktree).toEqual(worktree);
  });
});
