import type { SavedWorktree } from "./ipc";

// Retains a `session-worktree` event until the pane it belongs to exists.
//
// The backend reports the worktree a session actually got right after the
// session is spawned and inserted into its own session map, strictly before
// it reports the session as launched (see `spawn_session_inner` in the
// backend: the emit happens first, the launched signal a few lines later).
// For a manually launched pane that ordering never matters, because the
// pane already exists in App's own state before spawn is ever invoked. For
// a requested/approved session, though, the pane is not installed until the
// approval itself settles — which is exactly the signal that happens after
// this event — so this event routinely arrives with no pane to land on.
//
// Losing it is not cosmetic: it is the one authoritative record of which
// git worktree an isolated session actually landed in, and both the next
// restore and the app's own persisted roster depend on it (see
// `choose_worktree` in the backend, and App.tsx's roster-saving effect).
export class PendingWorktrees {
  private readonly byId = new Map<string, SavedWorktree>();

  /** Record an event that found no pane to land on yet. */
  retain(sessionId: string, worktree: SavedWorktree): void {
    this.byId.set(sessionId, worktree);
  }

  /**
   * Recover and clear a retained event for a pane about to be installed.
   * Consuming on read means a pane installed for this session id can only
   * ever pick this up once — a later, unrelated reuse of the same id (which
   * should not happen, but costs nothing to guard) can never inherit a
   * stale worktree left over from a previous take.
   */
  take(sessionId: string): SavedWorktree | undefined {
    const worktree = this.byId.get(sessionId);
    if (worktree) this.byId.delete(sessionId);
    return worktree;
  }
}
