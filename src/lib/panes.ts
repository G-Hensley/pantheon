// Remembering which agent CLIs were open, so reopening Pantheon brings the
// workspace back rather than an empty grid.
//
// The project directory and the layout already survive a restart; the panes did
// not, so every agent had to be relaunched by hand. This is the roster half of
// that: what each pane *was*, not what it was doing. Nothing about a live
// session is stored — a restored pane is freshly spawned, so its status comes
// from the new spawn, and its scrollback is gone with the old process.
//
// localStorage, alongside `pantheon.project` and `pantheon.layout*`, rather than the
// Rust store: the roster is UI state the frontend owns end to end, it is written
// on every pane change, and keeping it beside the project it belongs with means
// one place to look when a restore misbehaves.
//
// Keyed by project, because a roster names panes, brains and worktree paths that
// only mean anything inside the repository they were created for. Every entry
// point therefore takes the scope its caller resolved at startup rather than
// reading the selected project itself: the window persists to the project it
// opened with, and `shouldFreezePersistence` below decides when it must stop.

import { SESSION_TYPES, setConductor, type SavedWorktree, type SessionType } from "./ipc";
import { readScoped, writeScoped, type ProjectScope } from "./storage";

const KEY = "panes";
const CONDUCTOR_KEY = "conductor";

// What is persisted per pane. The session *type id* is stored rather than the
// type object, so a pane restored after an upgrade picks up the current
// program, args and colour instead of resurrecting stale ones.
export type StoredPane = {
  id: string;
  typeId: string;
  brain: string;
  isolate: boolean;
  model?: string;
  worktree?: SavedWorktree;
};

// A stored pane with its session type resolved — ready to become a live pane.
export type RestoredPane = Omit<StoredPane, "typeId"> & { type: SessionType };

export type Roster = {
  panes: RestoredPane[];
  // One message per entry that could not be restored. A bad entry is dropped
  // from the restore, never allowed to throw, and never a reason to discard the
  // rest of the roster.
  problems: string[];
};

const EMPTY: Roster = { panes: [], problems: [] };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

// A worktree reference is only useful if every field survived; a partial one
// would send the backend looking for a directory it cannot identify.
function parseWorktree(value: unknown): SavedWorktree | undefined {
  if (!isRecord(value)) return undefined;
  const { repo, path, branch, base } = value;
  if (
    typeof repo !== "string" ||
    typeof path !== "string" ||
    typeof branch !== "string" ||
    typeof base !== "string" ||
    !path
  ) {
    return undefined;
  }
  return { repo, path, branch, base };
}

// Turn stored JSON into panes, reporting rather than throwing on anything it
// cannot use. Pure, so the parsing rules can be reasoned about (and exercised)
// without a browser.
export function parseRoster(raw: string | null): Roster {
  if (!raw) return EMPTY;
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch {
    // Corrupt roster: start empty rather than blocking startup. The file is
    // rewritten from live state as soon as anything changes.
    return { panes: [], problems: ["Saved sessions could not be read, so none were restored."] };
  }
  if (!Array.isArray(data)) return EMPTY;

  const panes: RestoredPane[] = [];
  const problems: string[] = [];
  const seen = new Set<string>();

  for (const entry of data) {
    if (!isRecord(entry)) continue;
    const id = typeof entry.id === "string" ? entry.id : "";
    if (!id || seen.has(id)) continue;
    const type = SESSION_TYPES.find((t) => t.id === entry.typeId);
    if (!type) {
      // The session type is gone from this build — nothing to launch.
      problems.push(`Could not restore ${id}: its session type is no longer available.`);
      continue;
    }
    seen.add(id);
    panes.push({
      id,
      type,
      brain: typeof entry.brain === "string" && entry.brain ? entry.brain : "main",
      isolate: entry.isolate === true,
      model: typeof entry.model === "string" ? entry.model : undefined,
      worktree: parseWorktree(entry.worktree),
    });
  }
  return { panes, problems };
}

// Read the remembered roster. Never throws: a browser that denies localStorage
// simply starts with no sessions.
export function loadRoster(scope: ProjectScope): Roster {
  try {
    return parseRoster(readScoped(KEY, scope));
  } catch {
    return EMPTY;
  }
}

export function saveRoster(scope: ProjectScope, panes: StoredPane[]): void {
  try {
    writeScoped(KEY, scope, JSON.stringify(panes));
  } catch {
    /* ignore, persistence is best-effort, it must never break the app */
  }
}

// The conductor pane, alongside the roster it must belong to. Stored under
// its own key (same storage as the roster) rather than folded into the
// roster JSON, so a pane save does not have to reason about conductor state
// and vice versa. A missing key and an empty string both mean "no conductor".

/** Read the persisted conductor pane id, or null if none is stored. */
export function loadConductorId(scope: ProjectScope): string | null {
  try {
    const raw = readScoped(CONDUCTOR_KEY, scope);
    return raw ? raw : null;
  } catch {
    return null;
  }
}

/**
 * Persist the conductor id, but only while it names a pane still in the
 * roster. An id pointing at a pane that is no longer open is worse than no
 * conductor at all: left in storage, the next launch would try to restore a
 * role to a pane that will never be spawned. Called whenever the conductor
 * changes (promote, demote) and whenever the roster does (a pane closing can
 * take the conductor's pane with it), so a stale id never lingers.
 */
export function saveConductorId(
  scope: ProjectScope,
  conductor: string | null,
  paneIds: string[],
): void {
  try {
    const id = conductor !== null && paneIds.includes(conductor) ? conductor : null;
    writeScoped(CONDUCTOR_KEY, scope, id ?? "");
  } catch {
    /* ignore, persistence is best-effort, it must never break the app */
  }
}

// What launch should do with a saved conductor id, given which panes are
// actually part of the roster about to run. There are only ever two
// outcomes for a non-null saved id: restore that exact pane, or none at all.
// Never a substitute: a missing or failed pane means no conductor, not a
// different one.
export type ConductorRestoreDecision =
  | { restore: true; id: string }
  | { restore: false; id: string | null };

/**
 * Decide, without touching the backend, whether a saved conductor id can be
 * restored. Pure, so "never promote a different pane" can be checked without
 * a live app or a spawned process: the id is either exactly the saved one or
 * the decision carries no id to promote.
 */
export function decideConductorRestore(
  savedId: string | null,
  paneIds: string[],
): ConductorRestoreDecision {
  if (savedId === null) return { restore: false, id: null };
  if (paneIds.includes(savedId)) return { restore: true, id: savedId };
  return { restore: false, id: savedId };
}

/**
 * Apply a launch-time conductor restore: promote the saved pane if it is
 * part of the roster, or clear the stale saved id if it is not. Returns a
 * short notice to surface when the id could not be restored (the saved
 * conductor was null, so there is nothing to say), or null when there is
 * nothing to report.
 *
 * This only covers what is knowable at launch, before any pane has actually
 * spawned: whether the saved pane is part of the roster at all. A pane that
 * is part of the roster but then fails to spawn is a separate, asynchronous
 * failure the caller learns about later (see `noteSpawnFailure` in App.tsx)
 * and must retract the same way: clear the conductor, never hand it to a
 * different pane.
 *
 * On a successful restore this also writes the id back to storage. On
 * mount, `conductor` state starts null, so App.tsx's own persist effect
 * (keyed on the live conductor and panes) runs before this function's
 * `setConductor` call resolves and writes an empty conductor to storage in
 * the meantime, erasing the very id this function is about to restore.
 * Storage would eventually repair itself once the backend's
 * `conductor-changed` event round-trips back to that effect, but relying on
 * that round trip means a rejected `setConductor`, or the app closing before
 * the event arrives, would lose the saved id with nothing to show for it.
 * Writing it back here, immediately after the promotion actually succeeds,
 * closes that window instead of hoping the round trip wins the race. A
 * rejection is reported as a notice rather than swallowed, for the same
 * reason: silence is what made the original loss invisible.
 */
export async function restoreConductor(
  scope: ProjectScope,
  savedId: string | null,
  paneIds: string[],
): Promise<string | null> {
  const decision = decideConductorRestore(savedId, paneIds);
  if (decision.id === null) return null;
  if (decision.restore) {
    try {
      await setConductor(decision.id);
    } catch {
      return `Conductor was not restored: pane ${decision.id} could not be promoted.`;
    }
    saveConductorId(scope, decision.id, paneIds);
    return null;
  }
  saveConductorId(scope, null, paneIds);
  return `Conductor was not restored: pane ${decision.id} is no longer open.`;
}

// Where the `sess-N` counter has to resume from.
//
// Restored panes already hold their old ids, so a counter starting at 0 would
// hand the next session an id that is still live — the backend refuses the
// duplicate and the new pane never starts. Seeding past the highest restored
// number keeps new ids clear of them.
export function seedCounter(ids: string[]): number {
  let highest = 0;
  for (const id of ids) {
    const match = /^sess-(\d+)$/.exec(id);
    if (match) highest = Math.max(highest, Number(match[1]));
  }
  return highest;
}

/**
 * Whether this window must stop persisting its roster and conductor id.
 *
 * The roster is loaded once, against the project the window opened with, but
 * saved on every pane change. Keying the save by whatever project is selected
 * at the time would write panes launched under a second project into the first
 * project's bucket, which is the contamination this scoping exists to stop;
 * keying it by the startup project would record those same panes as belonging
 * to a repository they were never opened in. Neither is a roster worth
 * restoring, so once the selected project changes the window stops writing
 * altogether and says so, leaving both projects' saved rosters and every live
 * pane exactly as they are. A restart reloads against the newly selected
 * project and persistence resumes there.
 *
 * Freezing is one-way for the life of the window. Returning to the startup
 * project does not make the roster trustworthy again, because panes launched
 * while the other project was selected are still in the list. Pure, so
 * "a second project can never write into the first" can be checked without a
 * live app.
 */
export function shouldFreezePersistence(
  startupProject: string | null,
  selectedProject: string | null,
  alreadyFrozen: boolean,
): boolean {
  if (alreadyFrozen) return true;
  return selectedProject !== startupProject;
}

/**
 * What a window should do when the selected project changes: bind to the new
 * project and keep persisting, or freeze.
 *
 * Binding is one narrow exception to the freeze, for the case the freeze
 * otherwise breaks: a window that opened with no project at all, which is every
 * first launch. There, "pick a project, then launch panes" is the ordinary path,
 * and freezing on the pick would mean a first session's panes are never saved.
 *
 * Every one of these has to hold, and each rules out a way binding could lose
 * something:
 *
 * - the window opened with no project, so there is no earlier project whose
 *   roster this window is the live continuation of;
 * - it has no panes, live or restored, and no conductor, so there is nothing
 *   already on screen that belongs to the previous scope;
 * - it has not frozen already, so this is not a second move;
 * - and the destination is *provably* unused, so persisting into it cannot
 *   overwrite a saved roster or strand a pre-scoping value it would otherwise
 *   have adopted.
 *
 * `destinationUnused` is deliberately three-valued and only `true` will do.
 * Storage that refused a read reports `null`, and an unknown destination is
 * treated exactly like an occupied one.
 *
 * This is not a general live rebind. A window with panes on screen, or one that
 * has already switched once, still freezes, and so does the A to B to A case.
 */
export function shouldBindToProject(
  boundProject: string | null,
  alreadyFrozen: boolean,
  hasPanes: boolean,
  hasConductor: boolean,
  destinationUnused: boolean | null,
): boolean {
  if (boundProject !== null) return false;
  if (alreadyFrozen) return false;
  if (hasPanes || hasConductor) return false;
  return destinationUnused === true;
}

/** Shown once the window has stopped persisting, until it is restarted. */
export const FROZEN_PERSISTENCE_NOTICE =
  "Project changed. Open sessions keep running, but this window has stopped " +
  "saving its session list. Restart Pantheon to load the new project's sessions.";
