import { useEffect, useState } from "react";
import type { Dispatch, SetStateAction } from "react";
import { listen } from "@tauri-apps/api/event";
import { PendingWorktrees } from "./pendingWorktrees";
import type { SavedWorktree, SessionWorktreeEvent } from "./ipc";

// The minimum a caller's own pane type must offer for this hook to apply a
// `session-worktree` event to it directly. App.tsx's real Pane type has many
// more fields; this hook only ever touches these two.
export type WorktreePane = { id: string; worktree?: SavedWorktree };

export type UseWorktreeEvents = {
  // Recover and clear a `session-worktree` event that arrived before this
  // window owned a pane for the session. Call this exactly once, at the
  // moment a new pane for `sessionId` is about to be added, and fold the
  // result into that pane's own `worktree` field: `undefined` when nothing was
  // retained, which is the ordinary case for every non-isolated session and
  // for a manual launch (see below).
  take: (sessionId: string) => SavedWorktree | undefined;
};

// Applies a `session-worktree` event directly to a pane this window owns
// (the manual-launch case: `addSession` takes ownership of the id before the
// backend can possibly have spawned anything for it), and retains one for a
// session it does not own a pane for (the requested/approved case; see
// PendingWorktrees for why that ordering is routine, and why losing the event
// instead of retaining it would matter).
//
// Decide ownership synchronously before enqueueing a pure updater. React may
// defer that updater, so a flag mutated inside it cannot report a match to the
// caller. Committed panes alone also miss an install that is already queued.
// See App.tsx's ownedPaneIds.
export function useWorktreeEvents<P extends WorktreePane>(
  setPanes: Dispatch<SetStateAction<P[]>>,
  isPaneOwned: (sessionId: string) => boolean,
): UseWorktreeEvents {
  const [pending] = useState(() => new PendingWorktrees());

  useEffect(() => {
    const unlisten = listen<SessionWorktreeEvent>("session-worktree", (ev) => {
      const { sessionId, ...worktree } = ev.payload;
      if (!isPaneOwned(sessionId)) {
        pending.retain(sessionId, worktree);
        return;
      }
      setPanes((p) => p.map((x) => (x.id === sessionId ? { ...x, worktree } : x)));
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [pending, setPanes, isPaneOwned]);

  return { take: (sessionId: string) => pending.take(sessionId) };
}
