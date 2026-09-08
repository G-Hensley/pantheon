import { useEffect, useState } from "react";
import type { Dispatch, SetStateAction } from "react";
import { listen } from "@tauri-apps/api/event";
import { PendingExits } from "./pendingExits";

// The minimum a caller's own pane type must offer for this hook to apply a
// `session-exited` event to it directly (see below) or check whether one
// already has a pane to land on. App.tsx's real Pane type has many more
// fields; this hook only ever touches this one.
export type ExitTrackedPane = { id: string; alreadyExited?: boolean };

export type UseExitEvents = {
  // Whether this session's own end was already reported before this window
  // owned a pane for it. Call this exactly once, at the moment a pane for
  // `sessionId` is about to be installed, and fold the result into that pane's own
  // "already exited" handling (see TerminalPane's `alreadyExited` prop) —
  // `false` is the ordinary case for every session whose process is still
  // running, and for every manually launched pane (which always exists
  // before its process could possibly have exited).
  take: (sessionId: string) => boolean;
};

// A global, always-registered counterpart to TerminalPane's own per-pane
// `session-exited` listener, registered once at App's own mount, which is
// necessarily before any pane, manual or requested, can exist. Reaching a
// pane's own listener is not enough on its own: that listener registers via
// `listen()`, an async call, and does not actually subscribe until the
// promise it returns resolves, a window after the pane already exists during
// which its own listener cannot yet have caught anything. So this hook,
// unlike its per-pane counterpart, is authoritative for the "already exited"
// fact itself: for a pane this window owns it patches that fact directly onto
// the pane (`alreadyExited: true`), which TerminalPane reacts to whenever it
// changes, not only at its own mount. The real terminal write and onExit
// still happen there, since only the component holding the terminal ref can
// perform those. For a session this window does not own a pane for, the event
// is retained instead, for a later `take()` call to recover at the moment
// that pane is finally installed (see PendingExits).
//
// Decide ownership before enqueueing a pure updater. React may evaluate an
// updater eagerly or defer it, so a flag mutated inside it cannot answer the
// caller reliably. Ownership includes queued installs: an event update follows
// the queued pane install even before that pane appears in committed state.
// See App.tsx's ownedPaneIds.
export function useExitEvents<P extends ExitTrackedPane>(
  setPanes: Dispatch<SetStateAction<P[]>>,
  isPaneOwned: (sessionId: string) => boolean,
): UseExitEvents {
  const [pending] = useState(() => new PendingExits());

  useEffect(() => {
    const unlisten = listen<string>("session-exited", (ev) => {
      const sessionId = ev.payload;
      // Decided before the update is enqueued, from state this callback can
      // actually read synchronously, so the updater below stays pure.
      if (!isPaneOwned(sessionId)) {
        pending.retain(sessionId);
        return;
      }
      setPanes((p) =>
        p.map((x) =>
          x.id === sessionId && !x.alreadyExited ? { ...x, alreadyExited: true } : x,
        ),
      );
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [pending, setPanes, isPaneOwned]);

  return { take: (sessionId: string) => pending.take(sessionId) };
}
