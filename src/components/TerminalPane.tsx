import { useEffect, useRef, useState } from "react";
import { Channel } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import {
  toBytes,
  writeSession,
  resizeSession,
  spawnSession,
  type Bytes,
  type SavedWorktree,
  type SessionType,
  type StartupState,
} from "../lib/ipc";
import { TERM_FONT } from "../lib/themes";
import { useAppearance } from "../lib/appearance";

// `spawn_session` rejects with a structured SpawnError, not a string, so that a
// refusal can say *why*. Interpolating that object directly would print
// "[object Object]" — swallowing the reason precisely when the user most needs
// it, since the commonest refusal is "you asked for isolation and could not
// have it". Non-Tauri rejections (a thrown Error, a plain string) still arrive
// here, so fall back rather than assume the shape.
function spawnErrorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) {
    const err = e as { message?: unknown; isolation?: { reason?: unknown } };
    const message = typeof err.message === "string" ? err.message : String(e);
    // The reason is what tells the user whether to retry, pick a git repo, or
    // deliberately continue without isolation.
    const reason = err.isolation?.reason;
    return typeof reason === "string" ? `${message} (${reason})` : message;
  }
  return String(e);
}

function isIsolationUnavailable(e: unknown): boolean {
  return Boolean(e && typeof e === "object" && "kind" in e && e.kind === "isolationUnavailable");
}

// One xterm terminal bound to one backend session. Owns the terminal lifecycle,
// the output channel, keystroke write-back, container-driven resize, and live
// re-theming when the app appearance changes.
// A session request that a human already approved, with a channel the
// backend is (or is about to be) writing to. `buffered` holds whatever that
// channel's temporary handler collected between the approval call and this
// pane actually mounting — see useSessionRequests.approve, which assigns the
// handler synchronously before the approval RPC so nothing sent early is
// lost. This pane takes over the channel outright; it never calls
// spawnSession, since the backend already did the equivalent work as part of
// approving the request.
export type ExternalSession = { channel: Channel<Bytes>; buffered: Bytes[] };

export function TerminalPane({
  sessionId,
  type,
  isolate,
  cwd,
  reuseWorktree,
  model,
  onExit,
  onIsolationChange,
  onSpawnError,
  externalSession,
  startupState,
  alreadyExited,
}: {
  sessionId: string;
  type: SessionType;
  isolate?: boolean;
  cwd?: string;
  // The worktree this session ran in before the app was last closed. Passed
  // through so a restored isolated pane rejoins its own worktree rather than
  // abandoning it and cutting a second one.
  reuseWorktree?: SavedWorktree;
  model?: string;
  onExit: (id: string) => void;
  // Isolation was refused and the user chose to carry on without it, so the
  // pane's remembered isolation has to change with it.
  onIsolationChange: (id: string, isolate: boolean) => void;
  // A spawn that never started. The pane shows the reason itself; this lets the
  // app say so somewhere the user is looking, which matters most on startup
  // when several panes come back at once.
  onSpawnError?: (id: string, message: string) => void;
  // Set only for a pane installed from an approved session request. When
  // present, this pane attaches to the given channel instead of spawning.
  externalSession?: ExternalSession;
  // The backend's own observation of this pane's startup, looked up by the
  // caller from the latest session-requests list (see App.tsx). Undefined
  // until that list has been read at least once, or if this pane's request
  // has not registered a startup record yet — both render the same as
  // "starting", since neither is evidence of a connection.
  startupState?: StartupState;
  // True when this exact session id's own end was already reported — either
  // recovered once at mount from before this pane was ever installed (see
  // App.tsx's exitEvents.take; only ever meaningful alongside
  // externalSession, since a manually spawned pane exists before spawning
  // could possibly fail this fast), or patched in later by App's shared
  // useExitEvents hook onto an already-installed pane. This pane's own
  // listener below is registered only at mount and does not actually
  // subscribe until well after that (listen() is async), so either without
  // this the pane would show as running forever for a process that already
  // ended and can never emit that event again.
  alreadyExited?: boolean;
}) {
  const elRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const fitFrameRef = useRef<number | null>(null);
  const scheduleFitRef = useRef<() => void>(() => {});
  const lastSizeRef = useRef({ rows: 0, cols: 0 });
  const { theme, appearance } = useAppearance();
  const [isolationError, setIsolationError] = useState<string | null>(null);
  const continueWithoutIsolationRef = useRef<() => void>(() => {});
  // Guards every path that can report this pane's own end (the alreadyExited
  // mount branch below, this pane's own live listener, and the post-mount
  // effect further down) so a session that ends exactly once is only ever
  // reported here once, no matter which path observes it.
  const exitReportedRef = useRef(false);
  // The queued-output state and the flush-then-write operation built from it,
  // lifted out of the mount effect so a second effect can reach them too.
  // Batched output arriving on the channel is queued here and only actually
  // written to the terminal on the next animation frame (see the mount
  // effect below); anything that reports this pane's end has to flush that
  // queue first, or an end marker written straight to the terminal can print
  // ahead of output that arrived earlier but is still waiting for its frame.
  // `writeAfterQueuedOutputRef` is assigned inside the mount effect, the same
  // pattern `scheduleFitRef` and `continueWithoutIsolationRef` already use
  // for a mount-scoped function another effect needs to call without forcing
  // an exhaustive-deps re-run on every render.
  const outputQueueRef = useRef<Uint8Array[]>([]);
  const outputBytesRef = useRef(0);
  const outputFrameRef = useRef<number | null>(null);
  const writeAfterQueuedOutputRef = useRef<(data: string) => void>(() => {});

  // Create the terminal once for this pane.
  useEffect(() => {
    const term = new Terminal({
      theme: theme.xterm,
      fontFamily: TERM_FONT,
      fontSize: appearance.fontSize,
      cursorBlink: true,
      allowProposedApi: true,
      scrollback: 5000,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(elRef.current!);
    fit.fit();
    termRef.current = term;
    fitRef.current = fit;
    exitReportedRef.current = false;
    outputQueueRef.current = [];
    outputBytesRef.current = 0;
    outputFrameRef.current = null;

    let resizeInFlight = false;
    let desiredSize: { rows: number; cols: number } | null = null;
    let alive = true;
    const pumpResize = async () => {
      if (resizeInFlight) return;
      resizeInFlight = true;
      while (alive && desiredSize) {
        const next = desiredSize;
        desiredSize = null;
        await resizeSession(sessionId, next.rows, next.cols).catch(() => {});
      }
      resizeInFlight = false;
    };

    const fitAndResize = () => {
      fitFrameRef.current = null;
      const el = elRef.current;
      // Focus mode temporarily hides the other panes. Do not collapse their
      // PTYs to zero columns; the observer runs again when they reappear.
      if (!el || el.clientWidth < 40 || el.clientHeight < 40) return;
      try {
        fit.fit();
      } catch {
        return;
      }
      const last = lastSizeRef.current;
      if (term.rows === last.rows && term.cols === last.cols) return;
      lastSizeRef.current = { rows: term.rows, cols: term.cols };
      desiredSize = { rows: term.rows, cols: term.cols };
      void pumpResize();
    };
    const scheduleFit = () => {
      if (fitFrameRef.current !== null) cancelAnimationFrame(fitFrameRef.current);
      fitFrameRef.current = requestAnimationFrame(fitAndResize);
    };
    scheduleFitRef.current = scheduleFit;
    lastSizeRef.current = { rows: term.rows, cols: term.cols };

    // Clipboard: xterm would otherwise swallow Ctrl+V and send a literal ^V.
    // Returning false declines the event so the browser's native paste reaches
    // xterm's textarea. Ctrl+C copies when there's a selection, else falls
    // through as SIGINT (Windows Terminal behavior).
    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown") return true;
      const ctrl = e.ctrlKey && !e.altKey;
      if (ctrl && e.key.toLowerCase() === "v") return false;
      if (ctrl && e.key.toLowerCase() === "c") {
        const sel = term.getSelection();
        if (sel) {
          navigator.clipboard?.writeText(sel).catch(() => {});
          return false;
        }
      }
      return true;
    });

    term.onData((data) => {
      writeSession(sessionId, data).catch(() => {});
    });

    const channel = externalSession?.channel ?? new Channel<Bytes>();
    const flushOutput = () => {
      outputFrameRef.current = null;
      const queue = outputQueueRef.current;
      if (queue.length === 0) return;
      outputBytesRef.current = 0;
      if (queue.length === 1) {
        term.write(queue.shift()!);
        return;
      }
      const total = queue.reduce((n, chunk) => n + chunk.byteLength, 0);
      const merged = new Uint8Array(total);
      let offset = 0;
      for (const chunk of queue.splice(0)) {
        merged.set(chunk, offset);
        offset += chunk.byteLength;
      }
      term.write(merged);
    };
    channel.onmessage = (msg) => {
      const bytes = toBytes(msg);
      outputQueueRef.current.push(bytes);
      outputBytesRef.current += bytes.byteLength;
      if (outputBytesRef.current >= 64 * 1024) {
        if (outputFrameRef.current !== null) cancelAnimationFrame(outputFrameRef.current);
        flushOutput();
      } else if (outputFrameRef.current === null) {
        outputFrameRef.current = requestAnimationFrame(flushOutput);
      }
    };
    // Shared with the alreadyExited prop effect below (via the ref), so a
    // pane's end is always reported through the same flush-then-write
    // operation, whichever of the three paths observes it: nothing that
    // reports an ending may write straight to the terminal while output
    // that arrived earlier is still sitting in the queue.
    const writeAfterQueuedOutput = (data: string) => {
      if (outputFrameRef.current !== null) cancelAnimationFrame(outputFrameRef.current);
      flushOutput();
      term.write(data);
    };
    writeAfterQueuedOutputRef.current = writeAfterQueuedOutput;

    const ro = new ResizeObserver(scheduleFit);
    ro.observe(elRef.current!);

    if (externalSession) {
      // Already spawned as part of approving the request; take over the
      // buffer this channel's temporary handler collected before this pane
      // existed, in the order it arrived, then fall through to the same
      // onmessage handler above for everything from here on.
      for (const msg of externalSession.buffered) {
        const bytes = toBytes(msg);
        outputQueueRef.current.push(bytes);
        outputBytesRef.current += bytes.byteLength;
      }
      if (outputQueueRef.current.length > 0 && outputFrameRef.current === null) {
        outputFrameRef.current = requestAnimationFrame(flushOutput);
      }
      if (alreadyExited) {
        // The event this pane's own listener below exists to catch already
        // happened, before this pane could exist to catch it, and a session
        // cannot exit a second time to resend it. Replay whatever it wrote
        // before dying (queued just above), then report the same ending the
        // listener would have, through the same path, rather than leaving
        // this pane looking like a live, running process forever.
        exitReportedRef.current = true;
        writeAfterQueuedOutput("\r\n\x1b[38;5;245m[session ended]\x1b[0m\r\n");
        onExit(sessionId);
      }
    } else {
      if (isolate) {
        term.write("\x1b[38;5;245m[pantheon] creating an isolated git worktree…\x1b[0m\r\n");
      }
      const start = (withIsolation: boolean) => {
        setIsolationError(null);
        spawnSession(sessionId, channel, type.program, type.args, term.rows, term.cols, {
          isolate: withIsolation,
          cwd,
          // Only an isolated attempt may rejoin the saved worktree. Carrying on
          // without isolation means there is no worktree to rejoin, and handing
          // one over anyway would ask the backend for a contradiction.
          reuseWorktree: withIsolation ? reuseWorktree : undefined,
          model: model && model.length > 0 ? model : undefined,
          modelFlag: type.modelFlag,
        }).catch((e) => {
          const message = spawnErrorText(e);
          writeAfterQueuedOutput(`\r\n\x1b[31m[spawn error] ${message}\x1b[0m\r\n`);
          if (isIsolationUnavailable(e)) setIsolationError(message);
          onSpawnError?.(sessionId, message);
        });
      };
      continueWithoutIsolationRef.current = () => {
        onIsolationChange(sessionId, false);
        start(false);
      };
      start(Boolean(isolate));
    }

    const unlisten = listen<string>("session-exited", (ev) => {
      if (ev.payload === sessionId && !exitReportedRef.current) {
        exitReportedRef.current = true;
        writeAfterQueuedOutput("\r\n\x1b[38;5;245m[session ended]\x1b[0m\r\n");
        onExit(sessionId);
      }
    });

    term.focus();

    return () => {
      alive = false;
      desiredSize = null;
      ro.disconnect();
      if (fitFrameRef.current !== null) cancelAnimationFrame(fitFrameRef.current);
      if (outputFrameRef.current !== null) cancelAnimationFrame(outputFrameRef.current);
      scheduleFitRef.current = () => {};
      continueWithoutIsolationRef.current = () => {};
      writeAfterQueuedOutputRef.current = () => {};
      unlisten.then((f) => f());
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
    // sessionId is stable for a pane's lifetime; theme/appearance handled below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  // Catches the pre-listener-window race the alreadyExited-at-mount branch
  // above cannot: a session-exited event that lands after this pane already
  // existed (so App's shared useExitEvents hook patched alreadyExited onto
  // it directly, rather than retaining it) but before this pane's own
  // listener above finished subscribing (listen() resolves asynchronously,
  // so there is always a gap after mount during which this pane cannot yet
  // catch its own event). alreadyExited flips true exactly once, from a
  // prop update rather than at this component's own mount, and a session
  // cannot exit a second time to resend the event through the listener that
  // missed it — so react to that prop change here instead of waiting for a
  // listener subscription that will never see it. exitReportedRef keeps this
  // from double-reporting whichever of the three paths (this one, the mount
  // branch above, or the live listener above) actually observes a given
  // session's end.
  //
  // Routed through writeAfterQueuedOutputRef, the same flush-then-write
  // operation the mount branch and the live listener use, rather than
  // writing to the terminal directly: output that arrived on the channel
  // just before this prop flipped can still be sitting in the queue,
  // waiting for its animation frame, and a direct write would print the end
  // marker ahead of it.
  useEffect(() => {
    if (!alreadyExited || exitReportedRef.current) return;
    exitReportedRef.current = true;
    writeAfterQueuedOutputRef.current("\r\n\x1b[38;5;245m[session ended]\x1b[0m\r\n");
    onExit(sessionId);
  }, [alreadyExited, sessionId, onExit]);

  // Live re-theme + font-size on the already-open terminal.
  useEffect(() => {
    const term = termRef.current;
    const fit = fitRef.current;
    if (!term || !fit) return;
    term.options.theme = theme.xterm;
    term.options.fontSize = appearance.fontSize;
    scheduleFitRef.current();
  }, [theme.id, theme.xterm, appearance.fontSize, sessionId]);

  return (
    <div className="pane-term-wrap">
      {isolationError && (
        <div className="pane-spawn-error" role="alert">
          <span>{isolationError}</span>
          <button type="button" onClick={() => continueWithoutIsolationRef.current()}>
            Continue without isolation
          </button>
        </div>
      )}
      {externalSession && startupState !== "connected" && (
        <div className="pane-attach-status" role="status" data-state={startupState ?? "starting"}>
          {startupState === "ready_timeout"
            ? "No connection observed within 120s. Check its state under Session requests."
            : "Starting…"}
        </div>
      )}
      <div className="pane-term" ref={elRef} />
    </div>
  );
}
