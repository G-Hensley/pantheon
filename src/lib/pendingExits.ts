// Retains a `session-exited` event for a session whose pane does not exist
// yet. Mirrors PendingWorktrees for the same underlying reason (see there):
// the backend can report a session's own end before the frontend has ever
// built a pane for it — not only racing an approval's own settle-then-
// respond round trip, but structurally, since TerminalPane (otherwise the
// only listener for this event) does not subscribe until it mounts, and a
// requested session's pane is not installed until well after that. An event
// that arrives first must not be silently lost: installing a genuinely
// already-dead session as though it were running would show a live-looking
// terminal for a process that will never produce another byte and can never
// report its own end again.
export class PendingExits {
  private readonly ids = new Set<string>();

  /** Record that this session's own end was already reported. */
  retain(sessionId: string): void {
    this.ids.add(sessionId);
  }

  /**
   * Whether this session's end was already reported before its pane could
   * ask. Consumes on read, the same as PendingWorktrees: a session cannot
   * exit a second time to resend the event, so once a pane has asked and
   * been told it already ended, asking again must not find it a second
   * time.
   */
  take(sessionId: string): boolean {
    const exited = this.ids.has(sessionId);
    if (exited) this.ids.delete(sessionId);
    return exited;
  }
}
