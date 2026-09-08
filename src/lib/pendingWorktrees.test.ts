import { describe, expect, it } from "vitest";
import { PendingWorktrees } from "./pendingWorktrees";
import type { SavedWorktree } from "./ipc";

const WORKTREE: SavedWorktree = {
  repo: "/repo",
  path: "/repo/.worktrees/sess-1",
  branch: "agent/sess-1",
  base: "main",
};

describe("PendingWorktrees", () => {
  it("take returns undefined when nothing was ever retained for that session id", () => {
    const pending = new PendingWorktrees();
    expect(pending.take("sess-1")).toBeUndefined();
  });

  it("take recovers a retained event — the event-before-response ordering this exists for", () => {
    const pending = new PendingWorktrees();
    pending.retain("sess-1", WORKTREE);
    expect(pending.take("sess-1")).toEqual(WORKTREE);
  });

  it("take consumes the event: a second take for the same id returns undefined", () => {
    const pending = new PendingWorktrees();
    pending.retain("sess-1", WORKTREE);
    pending.take("sess-1");
    expect(pending.take("sess-1")).toBeUndefined();
  });

  it("tracks multiple in-flight sessions independently", () => {
    const pending = new PendingWorktrees();
    const other: SavedWorktree = { ...WORKTREE, path: "/repo/.worktrees/sess-2", branch: "agent/sess-2" };
    pending.retain("sess-1", WORKTREE);
    pending.retain("sess-2", other);
    expect(pending.take("sess-2")).toEqual(other);
    expect(pending.take("sess-1")).toEqual(WORKTREE);
  });

  it("a later retain for the same id replaces the earlier one rather than accumulating", () => {
    const pending = new PendingWorktrees();
    const later: SavedWorktree = { ...WORKTREE, path: "/repo/.worktrees/sess-1-v2" };
    pending.retain("sess-1", WORKTREE);
    pending.retain("sess-1", later);
    expect(pending.take("sess-1")).toEqual(later);
  });
});
