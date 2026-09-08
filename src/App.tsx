import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type DragEvent,
} from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { TerminalPane, type ExternalSession } from "./components/TerminalPane";
import { SessionLauncher } from "./components/SessionLauncher";
import { SettingsPanel } from "./components/SettingsPanel";
import { ContextSidebar } from "./components/ContextSidebar";
import { ConductorBar } from "./components/ConductorBar";
import { DispatchDialog } from "./components/DispatchDialog";
import { TaskDrawer } from "./components/TaskDrawer";
import { SessionRequests } from "./components/SessionRequests";
import { ToastContainer } from "./components/Toast";
import {
  LayoutPanel,
  type LayoutColumns,
  type LayoutMode,
} from "./components/LayoutPanel";
import {
  killSession,
  setAgentBrain,
  conductorState,
  setConductor,
  haltConductor,
  setProject as setProjectDir,
  dispatchTask,
  listSessionRequests,
  reserveSessionId,
  SESSION_TYPES,
  type SavedWorktree,
  type SessionRequest,
  type SessionType,
} from "./lib/ipc";
import { useDispatchBudget } from "./lib/useDispatchBudget";
import { useSessionRequests } from "./lib/useSessionRequests";
import { useWorktreeEvents } from "./lib/useWorktreeEvents";
import { useExitEvents } from "./lib/useExitEvents";
import {
  FROZEN_PERSISTENCE_NOTICE,
  loadConductorId,
  loadRoster,
  restoreConductor,
  saveConductorId,
  saveRoster,
  shouldBindToProject,
  shouldFreezePersistence,
} from "./lib/panes";
import { brainColorMap } from "./lib/brains";
import { projectScope, scopeIsUnused, readStored, writeStored } from "./lib/storage";
import "./App.css";

// PaneStatus type removed as it's unused after Pane type cleanup

type PaneStatus = "running" | "exited";
type Pane = {
  id: string;
  type: SessionType;
  status: PaneStatus;
  brain: string;
  isolate: boolean;
  model?: string;
  worktree?: SavedWorktree;
  // Set only for a pane installed by approving a session request: the
  // channel the backend is already writing to, plus whatever it buffered
  // before this pane existed. TerminalPane attaches to it instead of
  // spawning. Never set for an ordinary human-launched or restored pane.
  externalSession?: ExternalSession;
  // Set once, at creation, when exitEvents.take found this session's own end
  // already reported before this pane existed (see useExitEvents). Read only
  // by TerminalPane's mount effect, never persisted — like `status`, this
  // describes a specific process's run, not something a restored pane (a new
  // process) should inherit.
  alreadyExited?: boolean;
};

function App() {
  // The project this window opened with. Read once: it is both the initial
  // selection and the scope every roster read and write is keyed by, and the
  // two must agree even after the selection changes underneath them.
  const startupProject = useRef<string | null>(
    (() => {
      try {
        return readStored("project");
      } catch {
        return null;
      }
    })(),
  ).current;
  // The project this window persists to, and the scope derived from it. State
  // rather than a constant because of the one binding case in
  // `shouldBindToProject`: a window that opened with nothing may adopt the
  // first project picked. Every other project change freezes instead.
  const [boundProject, setBoundProject] = useState<string | null>(startupProject);
  const [scope, setScope] = useState(() => projectScope(startupProject));
  // The panes that were open when the app was last closed. Read once, before the
  // first render, so the CLIs come back with the window instead of after it.
  const [roster] = useState(() => loadRoster(scope));
  // A restored pane is spawned fresh, so it starts "running" whatever it was
  // doing last time; the spawn corrects it if the CLI never comes up.
  const [panes, setPanes] = useState<Pane[]>(() =>
    roster.panes.map((p) => ({ ...p, status: "running" as const })),
  );
  // Restores that did not work out — a session type that no longer exists, a CLI
  // that would not start, a worktree that cannot be reused. Shown once, together,
  // so one bad pane is a message rather than a silently missing agent.
  const [restoreProblems, setRestoreProblems] = useState<string[]>(() => roster.problems);
  const restoredIds = useRef(new Set(roster.panes.map((p) => p.id)));
  // Every session id this window has taken ownership of a pane for, recorded
  // at the moment that decision is made rather than when React commits it.
  // This is what tells a `session-worktree` or `session-exited` listener
  // whether to apply its event to a pane or retain it for a pane that does not
  // exist yet, and it deliberately answers a different question than "is there
  // a pane in `panes` right now":
  //
  //   - `installRequestPane` decides to install a pane and enqueues that
  //     update; an event for the same session can arrive before React has
  //     committed it. The committed array still has no such pane, but the
  //     update that adds it is already queued ahead of the event's own update,
  //     so applying is correct and retaining would strand the event on a pane
  //     that is about to exist and will never call `take` again.
  //   - Ids are added and never removed. An event for a pane the user has
  //     since closed is then dropped rather than retained forever for a pane
  //     that is not coming back, which is what we want: this window did own it.
  //
  // This retains one id per owned pane for this window's lifetime. It avoids
  // retaining late event payloads, but is not a constant-space history.
  //
  // Seeded from the roster, whose panes are already in the initial `panes`
  // state before the first render.
  const ownedPaneIds = useRef(new Set(roster.panes.map((p) => p.id)));
  // Stable for the life of the window, so the listener effects that depend on
  // it register once and are never torn down and re-registered.
  const isPaneOwned = useCallback((sessionId: string) => ownedPaneIds.current.has(sessionId), []);
  // The conductor pane recorded before this app was last closed, if any. Read
  // once alongside the roster: restoring the role only makes sense against
  // the exact set of panes that roster is about to spawn.
  const savedConductorId = useRef(loadConductorId(scope)).current;
  const [launcherOpen, setLauncherOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [layoutOpen, setLayoutOpen] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [layout, setLayout] = useState<LayoutMode>(() => {
    try {
      return readStored("layout") === "fit" ? "fit" : "scroll";
    } catch {
      return "scroll";
    }
  });
  const [focusedPane, setFocusedPane] = useState<string | null>(null);
  const [layoutColumns, setLayoutColumns] = useState<LayoutColumns>(() => {
    try {
      const value = readStored("layout.columns");
      return /^[1-6]$/.test(value ?? "")
        ? (Number(value) as LayoutColumns)
        : "auto";
    } catch {
      return "auto";
    }
  });
  const [paneHeight, setPaneHeight] = useState(() => {
    try {
      const value = Number(readStored("layout.paneHeight"));
      return value >= 280 && value <= 720 ? value : 420;
    } catch {
      return 420;
    }
  });
  const [selectedBrain, setSelectedBrain] = useState("main");
  // The git repo sessions run in. Isolated sessions get worktrees of it.
  const [project, setProject] = useState<string | null>(startupProject);
  // Set once the selected project has changed, which stops this window writing
  // a roster that would belong to neither project. See shouldFreezePersistence.
  const [persistenceFrozen, setPersistenceFrozen] = useState(false);
  // Which pane (if any) is the conductor. Owned by the app, never self-claimed.
  const [conductor, setConductorName] = useState<string | null>(null);
  const [conductorTasks, setConductorTasks] = useState<import("./lib/ipc").ConductorTask[]>([]);
  const [conductorHalted, setConductorHalted] = useState(false);
  // budget stays null until the first successful conductor_state read, and
  // again whenever an older backend build hasn't grown the field yet:
  // treated the same as "not known" rather than guessed at, since a wrong
  // number here would misreport exhaustion. See src/lib/useDispatchBudget.ts
  // for the reset call's pending/error handling and the ticket scheme that
  // keeps a slow, pre-reset poll from ever overwriting a post-reset read,
  // no matter how late it resolves.
  const dispatchBudget = useDispatchBudget();
  const [dispatchOpen, setDispatchOpen] = useState(false);
  const [tasksOpen, setTasksOpen] = useState(false);
  const [toasts, setToasts] = useState<string[]>([]); // dismissed task ids
  // Toasts announce results as they arrive, so anything that finished before
  // this launch is history rather than news. Without this the restored task
  // store re-announced itself in full on every start — around fifty toasts to
  // dismiss by hand.
  //
  // Deliberately not persisted. A task that finished in an earlier run is
  // already excluded by its finish time, so the dismissed set only ever needs
  // to cover the current run; persisting it would grow an unbounded id list in
  // localStorage to re-answer a question this ref already answers.
  const appStartedAt = useRef(Date.now());
  const dragId = useRef<string | null>(null);
  // The session-requests slice: last list snapshot, plus the ticketed
  // initialize/approve/deny/reset calls. See useSessionRequests.ts.
  const sessionRequests = useSessionRequests();
  const [sessionRequestsOpen, setSessionRequestsOpen] = useState(false);
  // Every request currently mid-approval, not just one: a single scalar here
  // would have the second row's approve() call clobber the first row's still-
  // in-flight "Approving…" state the moment a human opens a second request
  // while the first has not resolved yet, even though each row's own approval
  // is independently correct at the hook level (see useSessionRequests' own
  // per-request reservation). A Set, not an array: membership is all this is
  // ever queried for.
  const [approvingRequestIds, setApprovingRequestIds] = useState<ReadonlySet<string>>(() => new Set());
  const [approveRequestError, setApproveRequestError] = useState<string | null>(null);
  const [resetRequestsPending, setResetRequestsPending] = useState(false);
  // False until initialize_sessions (called once below, with the exact
  // restored roster ids) has resolved. Approve/deny/new-session all wait on
  // this: admitting anything before the backend has seen the full restored
  // set risks a fresh id colliding with one that is about to come back.
  const allocatorReady = sessionRequests.list.allocator_ready;

  // Put each restored pane's agent back in the brain it belonged to. The panes
  // themselves are already in state (and spawning); this is the half of
  // `addSession` that lives outside the pane list.
  useEffect(() => {
    for (const pane of roster.panes) {
      setAgentBrain(pane.id, pane.brain).catch(() => {});
    }
    // Runs once, against the roster captured before the first render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Restore the conductor role, but only to the exact pane that held it. If
  // that pane is not part of the roster at all (closed before this launch, or
  // dropped because its session type no longer exists), the saved id is
  // stale: clear it and say so in the same banner restore problems already
  // use, rather than leaving a promotion pointing at nothing or picking a
  // different pane. A pane that *is* part of the roster but then fails to
  // spawn is caught separately, in `noteSpawnFailure` below, once that
  // failure is actually known.
  useEffect(() => {
    restoreConductor(scope, savedConductorId, roster.panes.map((p) => p.id)).then((notice) => {
      if (notice) setRestoreProblems((problems) => [...problems, notice]);
    });
    // Runs once, against the roster and saved conductor captured before the
    // first render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // The restore-id barrier: tell the backend allocator every pane id this
  // window is about to restore, so it can never hand out (or admit a
  // session-request approval into) one of them. Every roster id qualifies,
  // whether or not its spawn goes on to succeed — this list is fixed before
  // any of those spawns resolve, which already gives it the failed restores
  // the barrier needs. Called once, even when the roster is empty, since
  // approve/deny/reset/addSession all wait on allocator_ready flipping true.
  useEffect(() => {
    sessionRequests.initialize([...restoredIds.current]).catch(() => {});
    // Runs once, against the roster captured before the first render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Remember the roster on every change, so a restart brings back whatever was
  // open at the moment it happened. Status is deliberately not stored: a
  // restored pane is a new process, so its state is whatever the new spawn
  // reports.
  useEffect(() => {
    if (persistenceFrozen) return;
    saveRoster(
      scope,
      panes.map((p) => ({
        id: p.id,
        typeId: p.type.id,
        brain: p.brain,
        isolate: p.isolate,
        model: p.model,
        worktree: p.worktree,
      })),
    );
  }, [panes, persistenceFrozen, scope]);

  // Persist the conductor id alongside the roster, so a relaunch can restore
  // the role. Keyed on both conductor and panes: a promote or demote changes
  // the id itself, and a pane closing can take the conductor's own pane with
  // it, in which case it must not be left pointing at a pane that is gone.
  useEffect(() => {
    if (persistenceFrozen) return;
    saveConductorId(
      scope,
      conductor,
      panes.map((p) => p.id),
    );
  }, [conductor, panes, persistenceFrozen, scope]);

  // The backend reports which worktree an isolated session actually got, once
  // it is live. Recording it is what lets the next launch return the pane to
  // that worktree instead of stranding it — an abandoned worktree can hold
  // uncommitted agent work that nothing in the app would point at any more.
  // For a manual launch the event always concerns a session this window has
  // already claimed (see addSession) and is applied directly; for a
  // requested/approved session it routinely arrives before that claim (see
  // useWorktreeEvents/PendingWorktrees), so `installRequestPane` and
  // `addSession` both call `worktreeEvents.take` for their own session id
  // before adding the pane.
  const worktreeEvents = useWorktreeEvents(setPanes, isPaneOwned);

  // A requested/approved session can exit before its own approval RPC ever
  // resolves — TerminalPane's own session-exited listener does not register
  // until it mounts, which for that flow is well after the process could
  // already be gone. See useExitEvents/PendingExits for why this needs the
  // same event-before-pane handling as the worktree case just above, rather
  // than leaving a genuinely dead session looking like it is still running.
  const exitEvents = useExitEvents(setPanes, isPaneOwned);

  // A pane that never started. Only reported for restored panes: a session the
  // user just launched by hand has their attention already, and the failure is
  // printed in its terminal either way.
  //
  // If the pane that failed is the one the launch tried to restore as
  // conductor, the role is retracted here rather than left standing: the
  // roster-time restore above could not have known the spawn would fail, and
  // this is the only place that finds out. It is never handed to a different
  // pane, only cleared.
  function noteSpawnFailure(id: string, message: string) {
    if (!restoredIds.current.has(id)) return;
    const isConductorPane = savedConductorId === id;
    setRestoreProblems((problems) => {
      const note = isConductorPane
        ? `Could not restore ${id}: ${message} The conductor role was not restored either.`
        : `Could not restore ${id}: ${message}`;
      return problems.includes(note) ? problems : [...problems, note];
    });
    if (isConductorPane) {
      setConductorName(null);
      setConductor(null).catch(() => {});
    }
  }

  useEffect(() => {
    let alive = true;
    const refresh = async () => {
      // Ticketed before the await, not after: a reset that starts and
      // finishes entirely during this call is what the ticket lets
      // dispatchBudget.applySnapshot recognize below, however late this
      // particular read turns out to resolve. See useDispatchBudget.ts.
      const budgetTicket = dispatchBudget.beginRead();
      try {
        const s = await conductorState();
        if (alive) {
          setConductorName(s.conductor);
          setConductorTasks(s.tasks);
          setConductorHalted(s.halted);
          dispatchBudget.applySnapshot(s.dispatch_budget, budgetTicket);
        }
      } catch {
        /* backend not ready */
      }
    };
    refresh();
    const unlisten = listen("conductor-changed", refresh);
    const poll = setInterval(refresh, 3000);
    return () => {
      alive = false;
      clearInterval(poll);
      unlisten.then((f) => f());
    };
    // dispatchBudget.beginRead and .applySnapshot both have a stable
    // identity (see useDispatchBudget's own useCallback), so listing them
    // here does not cause this effect to re-run on every render.
  }, [dispatchBudget.beginRead, dispatchBudget.applySnapshot]);

  // Same pattern as conductor-state above, applied to the session-request
  // queue: a poll plus the change event, both routed through the ticketed
  // applySnapshot so a slow read from before a reset or the restore barrier
  // can never land over what came after it.
  useEffect(() => {
    let alive = true;
    const refresh = async () => {
      const ticket = sessionRequests.beginRead();
      try {
        const snapshot = await listSessionRequests();
        if (alive) sessionRequests.applySnapshot(snapshot, ticket);
      } catch {
        /* backend not ready */
      }
    };
    refresh();
    const unlisten = listen("session-requests-changed", refresh);
    const poll = setInterval(refresh, 3000);
    return () => {
      alive = false;
      clearInterval(poll);
      unlisten.then((f) => f());
    };
    // sessionRequests.beginRead/.applySnapshot have a stable identity (see
    // useSessionRequests' own useCallback).
  }, [sessionRequests.beginRead, sessionRequests.applySnapshot]);

  async function toggleConductor(id: string) {
    const next = conductor === id ? null : id;
    setConductorName(next);
    await setConductor(next).catch(() => {});
  }


  // Keep the backend's context directory in step with the active project —
  // on startup (restoring the remembered one) and on every change.
  useEffect(() => {
    setProjectDir(project).catch(() => {});
  }, [project]);

  async function pickProject() {
    try {
      const dir = await open({
        directory: true,
        title: "Pick the git repo Pantheon sessions should work in",
      });
      if (typeof dir === "string") {
        setProject(dir);
        // An empty window that opened with no project may bind to the first
        // project picked and go on persisting there. Anything else freezes.
        // Both are batched with setProject, so the persist effects below act on
        // the new scope, or stop, on the same render the project arrives on.
        if (
          shouldBindToProject(
            boundProject,
            persistenceFrozen,
            panes.length > 0,
            conductor !== null,
            scopeIsUnused(projectScope(dir)),
          )
        ) {
          setBoundProject(dir);
          setScope(projectScope(dir));
        } else {
          setPersistenceFrozen((frozen) => shouldFreezePersistence(boundProject, dir, frozen));
        }
        try {
          writeStored("project", dir);
        } catch {
          /* ignore */
        }
      }
    } catch {
      /* cancelled */
    }
  }

  // Ordered list of brains that currently have panes, "main" always first.
  const brainList = useMemo(() => {
    const set = [...new Set(panes.map((p) => p.brain))];
    set.sort((a, b) => (a === "main" ? -1 : b === "main" ? 1 : 0));
    return set;
  }, [panes]);
const colorMap = useMemo(() => brainColorMap(brainList), [brainList]);

  // Manual launches mint their id from the same backend allocator a
  // request-approved pane draws from (reserve_session_id), never a local
  // counter, so the two sources can never hand out the same id. Refused
  // outright before the restore barrier resolves — see allocatorReady.
  async function addSession(type: SessionType, isolate = false, model?: string) {
    if (!allocatorReady) return;
    let id: string;
    try {
      id = await reserveSessionId();
    } catch {
      return;
    }
    // Claimed before anything is taken or enqueued, and in the same
    // uninterrupted run of synchronous code as the `setPanes` below, so no
    // event for this id can be seen while the claim is only half made.
    ownedPaneIds.current.add(id);
    // Always empty/false in practice for both of these: the id was minted a
    // moment ago and nothing has been spawned under it yet, so neither event
    // can have happened. Taking them here rather than assuming that keeps
    // every pane-adding path going through the same recovery, not just the
    // ones that actually need it.
    const worktree = worktreeEvents.take(id);
    const alreadyExited = exitEvents.take(id);
    setPanes((p) => [...p, { id, type, status: "running", brain: "main", isolate, model, worktree, alreadyExited }]);
    setAgentBrain(id, "main").catch(() => {});
    setLauncherOpen(false);
  }

  function closePane(id: string) {
    killSession(id).catch(() => {});
    setPanes((p) => p.filter((x) => x.id !== id));
    setFocusedPane((focused) => (focused === id ? null : focused));
  }

  // ---- Session requests (approve/deny/reset) ----
  // Installs a pane from an authoritative SessionRequest — never from
  // anything captured before an approval RPC, and never from the human's own
  // edited-model input directly. A repeated or racing approval can settle on
  // an accepted model, brain or kind different from what this particular
  // call asked for (the backend may have normalized the edit, or this call
  // may have lost a race to an earlier one that already claimed the
  // request), and the pane must reflect what was actually committed. Refuses
  // to install a second pane for a session id that already has one, so a
  // retried or passively-reconciled approval can never duplicate a pane the
  // first attempt (or a concurrent one) already installed.
  // A `session-worktree` or `session-exited` event for this exact session
  // may already have arrived and found no pane to land on: the backend can
  // report either — a live isolated worktree, or the process already having
  // ended — strictly before the approval RPC this pane is installed from
  // ever resolves (see useWorktreeEvents and useExitEvents). Recovering both
  // here, before the pane is built, is what keeps that ordering from
  // silently stranding an isolated request's actual worktree, or installing
  // an already-dead session as though it were still running.
  function installRequestPane(request: SessionRequest, attachment: ExternalSession) {
    const type = SESSION_TYPES.find((t) => t.id === request.kind);
    const sessionId = request.session_id;
    if (!type || !sessionId) {
      setApproveRequestError(`Unknown session kind "${request.kind}" for request ${request.request_id}.`);
      return;
    }
    // Claimed here, at the decision, not at the commit: the `setPanes` below
    // is queued, and an event for this session arriving before React runs it
    // belongs to the pane that update is about to produce. Idempotent, so a
    // repeated or racing approval for the same session changes nothing.
    ownedPaneIds.current.add(sessionId);
    const worktree = worktreeEvents.take(sessionId);
    const alreadyExited = exitEvents.take(sessionId);
    setPanes((p) => {
      if (p.some((pane) => pane.id === sessionId)) return p;
      return [
        ...p,
        {
          id: sessionId,
          type,
          status: "running",
          brain: request.brain,
          isolate: request.isolate,
          model: request.model,
          externalSession: attachment,
          worktree,
          alreadyExited,
        },
      ];
    });
    setAgentBrain(sessionId, request.brain).catch(() => {});
  }

  // Keep request mutations behind the restore barrier. In particular, reset's
  // read-ticket barrier must not invalidate the initialization snapshot.
  // Handler checks complement the disabled controls.
  async function handleApproveRequest(requestId: string, editedModel: string | null) {
    if (!allocatorReady) return;
    setApprovingRequestIds((prev) => {
      const next = new Set(prev);
      next.add(requestId);
      return next;
    });
    setApproveRequestError(null);
    // No terminal exists yet to size this from — TerminalPane fits and
    // resizes it for real the moment the pane mounts, same as it does for a
    // manual launch's own initial guess.
    const result = await sessionRequests.approve(requestId, editedModel, 24, 80);
    setApprovingRequestIds((prev) => {
      if (!prev.has(requestId)) return prev;
      const next = new Set(prev);
      next.delete(requestId);
      return next;
    });
    if (!result.ok) {
      setApproveRequestError(result.error);
      return;
    }
    installRequestPane(result.request, { channel: result.channel, buffered: result.buffered });
  }

  // A previously uncertain approval (its RPC rejected, and neither its own
  // fallback poll nor any poll since could yet prove Started or a terminal
  // failure) that a later snapshot — this session's own regular poll, most
  // likely — has now confirmed Started, with no further click required. The
  // attachment's channel is the exact one the backend was already handed;
  // installRequestPane's session-id dedup is what keeps this from racing a
  // concurrent manual retry into two panes for the same session.
  useEffect(() => {
    if (sessionRequests.readyAttachments.length === 0) return;
    for (const ready of sessionRequests.readyAttachments) {
      installRequestPane(ready.request, { channel: ready.channel, buffered: ready.buffered });
    }
    sessionRequests.consumeReady(sessionRequests.readyAttachments.map((r) => r.request.request_id));
    // installRequestPane closes over state setters only (setPanes/setAgentBrain
    // are stable, setApproveRequestError's identity doesn't need to retrigger
    // this); reacting to readyAttachments itself is the whole point.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionRequests.readyAttachments]);

  async function handleDenyRequest(requestId: string) {
    if (!allocatorReady) return;
    try {
      await sessionRequests.deny(requestId);
    } catch (err) {
      setApproveRequestError(err instanceof Error ? err.message : String(err));
    }
  }

  async function handleResetRequests() {
    if (!allocatorReady) return;
    setResetRequestsPending(true);
    try {
      await sessionRequests.reset();
    } catch (err) {
      setApproveRequestError(err instanceof Error ? err.message : String(err));
    } finally {
      setResetRequestsPending(false);
    }
  }

  useEffect(() => {
    try {
      writeStored("layout", layout);
      writeStored("layout.columns", String(layoutColumns));
      writeStored("layout.paneHeight", String(paneHeight));
    } catch {
      /* ignore */
    }
  }, [layout, layoutColumns, paneHeight]);

  function markExited(id: string) {
    setPanes((p) => p.map((x) => (x.id === id ? { ...x, status: "exited" } : x)));
  }

  function setPaneIsolation(id: string, isolate: boolean) {
    setPanes((p) => p.map((x) => (x.id === id ? { ...x, isolate } : x)));
  }

  // Re-home a pane (and its agent) to a brain. The backend re-scopes on the
  // agent's next tool call.
  function assignBrain(paneId: string, brain: string) {
    setPanes((p) => p.map((x) => (x.id === paneId ? { ...x, brain } : x)));
    setAgentBrain(paneId, brain).catch(() => {});
  }

  function nextBrainName() {
    let n = 2;
    while (brainList.includes(`brain-${n}`)) n++;
    return `brain-${n}`;
  }

  // ---- Dispatch ----
  async function handleDispatch(target: string, task: string) {
    await dispatchTask(target, task);
  }

  function dismissToast(taskId: string) {
    setToasts((t) => [...t, taskId]);
  }

  // ---- drag wiring ----
  // Chromium needs a real dataTransfer payload on dragstart and an explicit
  // dropEffect on dragover, or it shows the "no drop" cursor even when the
  // target would accept. (Tauri's native dragDropEnabled must also be off.)
  function onDragStart(e: DragEvent, id: string) {
    dragId.current = id;
    e.dataTransfer.setData("text/plain", id);
    e.dataTransfer.effectAllowed = "move";
  }
  function allowDrop(e: DragEvent) {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
  }
  function handleDropOnBrain(brain: string, draggedId?: string) {
    const id = draggedId || dragId.current;
    if (id) assignBrain(id, brain);
    dragId.current = null;
  }
  function handleDropOnNewBrain(draggedId?: string) {
    const id = draggedId || dragId.current;
    if (id) assignBrain(id, nextBrainName());
    dragId.current = null;
  }
  function onDropPane(e: DragEvent, brain: string) {
    e.preventDefault();
    handleDropOnBrain(brain, e.dataTransfer.getData("text/plain"));
  }

  // Capture app shortcuts before xterm sees them. Plain Ctrl+K/Ctrl+B belong
  // to terminal applications, so Pantheon uses Ctrl+Shift chords instead.
  useEffect(() => {
    const focusPaneAt = (index: number) => {
      const pane = document.querySelectorAll<HTMLElement>(".pane")[index];
      if (!pane) return;
      const id = pane.dataset.paneId;
      // While a pane is maximized, move the spotlight to the requested one rather
      // than dropping out of focus mode. The rail button beside this shortcut
      // switches panes, and its tooltip advertises the shortcut as the same
      // action, so exiting instead made the two disagree. Outside focus mode
      // there is no spotlight to move and the rAF below just focuses the terminal.
      if (id) setFocusedPane((current) => (current ? id : current));
      requestAnimationFrame(() =>
        requestAnimationFrame(() =>
          pane.querySelector<HTMLElement>(".xterm-helper-textarea")?.focus(),
        ),
      );
    };
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.shiftKey && e.key.toLowerCase() === "k") {
        e.preventDefault();
        e.stopPropagation();
        setLauncherOpen((o) => !o);
      } else if (mod && e.shiftKey && e.key === ",") {
        e.preventDefault();
        e.stopPropagation();
        setSettingsOpen((o) => !o);
      } else if (mod && e.shiftKey && e.key.toLowerCase() === "b") {
        e.preventDefault();
        e.stopPropagation();
        setSidebarOpen((o) => !o);
      } else if (mod && e.shiftKey && e.key.toLowerCase() === "t") {
        e.preventDefault();
        e.stopPropagation();
        setTasksOpen((o) => !o);
      } else if (mod && e.shiftKey && /^[1-9]$/.test(e.key)) {
        e.preventDefault();
        e.stopPropagation();
        focusPaneAt(Number(e.key) - 1);
      } else if (mod && e.shiftKey && e.key === "Enter") {
        const pane = document.activeElement?.closest<HTMLElement>(".pane");
        const id = pane?.dataset.paneId;
        if (!id) return;
        e.preventDefault();
        e.stopPropagation();
        setFocusedPane((current) => (current === id ? null : id));
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  // Same-brain panes cluster together; order is stable so drags don't remount.
  const ordered = useMemo(
    () => [...panes].sort((a, b) => brainList.indexOf(a.brain) - brainList.indexOf(b.brain)),
    [panes, brainList],
  );
  const count = Math.min(panes.length, 6);
  const grouped = brainList.length > 1;

  return (
    <div className="app">
      <header className="bar">
        <h1>Pantheon</h1>
        <span className="tag">cockpit</span>
        <div className="spacer" />
        <span className="count">
          {panes.length} session{panes.length === 1 ? "" : "s"}
          {grouped ? ` · ${brainList.length} brains` : ""}
        </span>
        <button
          className={"ghost" + (project ? " on" : "")}
          onClick={pickProject}
          title={project ?? "No project set — sessions run where the app launched"}
        >
          {project ? `⌂ ${project.split(/[\\/]/).filter(Boolean).pop()}` : "Pick project"}
        </button>
        <button
          className={"ghost" + (sidebarOpen ? " on" : "")}
          onClick={() => setSidebarOpen((o) => !o)}
          title="Shared brain (Ctrl+Shift+B)"
        >
          Shared brain
        </button>
        <button
          className={"ghost" + (layout === "scroll" ? " on" : "")}
          onClick={() => setLayoutOpen(true)}
          title="Customize terminal layout"
        >
          Layout: {layoutColumns === "auto" ? "auto" : `${layoutColumns} col`}
        </button>
        <button
          className="ghost"
          onClick={() => setSettingsOpen(true)}
          title="Appearance (Ctrl+Shift+,)"
        >
          Appearance
        </button>
        <button
          className={"ghost" + (sessionRequestsOpen ? " on" : "")}
          onClick={() => setSessionRequestsOpen(true)}
          title="Session requests from agents, pending your approval"
        >
          Session requests
          {sessionRequests.list.outstanding > 0 && (
            <span className="badge">{sessionRequests.list.outstanding}</span>
          )}
        </button>
        <button
          className="primary"
          onClick={() => setLauncherOpen(true)}
          disabled={!allocatorReady}
          title={allocatorReady ? undefined : "Restoring session state…"}
        >
          + New session <kbd>Ctrl Shift K</kbd>
        </button>
      </header>

      {(restoreProblems.length > 0 || persistenceFrozen) && (
        <div className="restore-problem" role="status">
          <span>
            {[...restoreProblems, ...(persistenceFrozen ? [FROZEN_PERSISTENCE_NOTICE] : [])].join(
              " · ",
            )}
          </span>
          <div className="spacer" />
          {restoreProblems.length > 0 && (
            <button className="ghost" onClick={() => setRestoreProblems([])}>
              Dismiss
            </button>
          )}
        </div>
      )}

      {conductor && (
        <ConductorBar
          conductor={conductor}
          tasks={conductorTasks}
          halted={conductorHalted}
          onDemote={() => toggleConductor(conductor)}
          onHaltChange={haltConductor}
          onOpenDispatch={() => setDispatchOpen(true)}
          onOpenTasks={() => setTasksOpen(true)}
          panes={panes}
          budget={dispatchBudget.budget}
          onResetBudget={dispatchBudget.reset}
          budgetResetPending={dispatchBudget.resetPending}
          budgetResetError={dispatchBudget.resetError}
        />
      )}

      <div className="body">
        {focusedPane && (
          <nav className="spotlight-rail" aria-label="Live sessions">
            {ordered.map((p, index) => (
              <button
                key={p.id}
                className={"spotlight-item" + (focusedPane === p.id ? " active" : "")}
                onClick={() => setFocusedPane(p.id)}
                title={`Switch to ${p.type.label} ${p.id} (Ctrl+Shift+${index + 1})`}
              >
                <span className="dot" style={{ background: p.type.color }} />
                <span>{p.type.label}</span>
                <small>{index + 1}</small>
              </button>
            ))}
          </nav>
        )}
        <main
          className="grid"
          data-count={count}
          data-layout={layout}
          data-columns={layoutColumns}
          data-focused={focusedPane !== null}
          style={
            {
              "--layout-columns": layoutColumns === "auto" ? 1 : layoutColumns,
              "--pane-min-height": `${paneHeight}px`,
            } as CSSProperties
          }
        >
          {panes.length === 0 ? (
            <div className="empty">
              <div className="empty-title">No sessions yet</div>
              <div className="empty-sub">
                Open live agents in panes, then drag them together — onto each
                other or a brain in the sidebar — to share context.
              </div>
              <button
                className="primary"
                onClick={() => setLauncherOpen(true)}
                disabled={!allocatorReady}
                title={allocatorReady ? undefined : "Restoring session state…"}
              >
                + New session <kbd>Ctrl Shift K</kbd>
              </button>
            </div>
          ) : (
            ordered.map((p) => {
              const color = colorMap[p.brain];
              return (
                <section
                  className={
                    "pane" +
                    (grouped ? " grouped" : "") +
                    (focusedPane === p.id ? " focused" : "")
                  }
                  key={p.id}
                  data-pane-id={p.id}
                  data-status={p.status}
                  style={grouped ? ({ "--brain": color } as CSSProperties) : undefined}
                  onDragOver={allowDrop}
                  onDrop={(e) => onDropPane(e, p.brain)}
                >
                  <div
                    className="pane-head"
                    draggable
                    onDragStart={(e) => onDragStart(e, p.id)}
                    title="Drag onto another pane or a brain to connect"
                  >
                    <span className="dot" style={{ background: p.type.color }} />
                    <span className="pane-label">{p.type.label}</span>
                    <span className="pane-id">{p.id}</span>
                    {grouped && (
                      <span className="brain-chip" style={{ background: color, color: "#16161e" }}>
                        {p.brain}
                      </span>
                    )}
                    {p.isolate && (
                      <span className="pane-iso" title="Runs in its own git worktree + branch">
                        ⑂ isolated
                      </span>
                    )}
                    {p.status === "exited" && <span className="pane-ended">ended</span>}
                    <div className="spacer" />
<button
          className={"pane-cond" + (conductor === p.id ? " on" : "")}
          title={
            conductor === p.id
              ? "Conductor — click to demote"
              : "Make this pane the conductor"
          }
          aria-label={conductor === p.id ? "Demote conductor" : "Make conductor"}
          onClick={() => toggleConductor(p.id)}
        >
          ⌁
        </button>
                    <button
                      className={"pane-focus" + (focusedPane === p.id ? " on" : "")}
                      title={focusedPane === p.id ? "Show all panes" : "Maximize this pane"}
                      aria-pressed={focusedPane === p.id}
                      onClick={() => setFocusedPane((current) => (current === p.id ? null : p.id))}
                    >
                      {focusedPane === p.id ? "Restore" : "Maximize"}
                    </button>
<button className="pane-x" title="Close session" aria-label="Close session" onClick={() => closePane(p.id)}>
          ✕
        </button>
                  </div>
<TerminalPane
                     sessionId={p.id}
                     type={p.type}
                     isolate={p.isolate}
                     cwd={project ?? undefined}
                     reuseWorktree={p.worktree}
                     model={p.model}
                     onExit={markExited}
                     onIsolationChange={setPaneIsolation}
                     onSpawnError={noteSpawnFailure}
                     externalSession={p.externalSession}
                     startupState={sessionRequests.list.startups.find((s) => s.pane === p.id)?.state}
                     alreadyExited={p.alreadyExited}
                   />
                </section>
              );
            })
          )}
        </main>

        {sidebarOpen && (
          <ContextSidebar
            brains={brainList}
            colorMap={colorMap}
            panes={panes.map((p) => ({ id: p.id, brain: p.brain, label: p.type.label }))}
            selectedBrain={selectedBrain}
            onSelectBrain={setSelectedBrain}
            onDropBrain={handleDropOnBrain}
            onDropNewBrain={handleDropOnNewBrain}
            onClose={() => setSidebarOpen(false)}
          />
        )}
      </div>

      {launcherOpen && (
        <SessionLauncher
          project={project}
          onPick={addSession}
          onClose={() => setLauncherOpen(false)}
        />
      )}
      {settingsOpen && <SettingsPanel onClose={() => setSettingsOpen(false)} />}
      {layoutOpen && (
        <LayoutPanel
          mode={layout}
          columns={layoutColumns}
          paneHeight={paneHeight}
          onMode={setLayout}
          onColumns={setLayoutColumns}
          onPaneHeight={setPaneHeight}
          onClose={() => setLayoutOpen(false)}
        />
      )}
      {dispatchOpen && (
        <DispatchDialog
          panes={panes}
          conductorId={conductor}
          onClose={() => setDispatchOpen(false)}
          onDispatch={handleDispatch}
        />
      )}
      {tasksOpen && (
        <TaskDrawer
          tasks={conductorTasks}
          panes={panes}
          onClose={() => setTasksOpen(false)}
          onFocusPane={(id) => {
            setFocusedPane(id);
            setTasksOpen(false);
          }}
        />
      )}
      {sessionRequestsOpen && (
        <SessionRequests
          list={sessionRequests.list}
          onApprove={handleApproveRequest}
          approvingIds={approvingRequestIds}
          approveError={approveRequestError}
          onDeny={handleDenyRequest}
          onReset={handleResetRequests}
          resetPending={resetRequestsPending}
          onClose={() => {
            setSessionRequestsOpen(false);
            setApproveRequestError(null);
          }}
        />
      )}
      <ToastContainer
        tasks={conductorTasks.filter(
          (t) =>
            !toasts.includes(t.id) &&
            t.done_ms !== null &&
            t.done_ms >= appStartedAt.current,
        )}
        onDismiss={dismissToast}
      />
    </div>
  );
}

export default App;
