const CURRENT_PREFIX = "pantheon.";
const LEGACY_PREFIX = "mosaic.";

/** Read Pantheon UI state, adopting the pre-rename key on first access. */
export function readStored(name: string): string | null {
  const currentKey = `${CURRENT_PREFIX}${name}`;
  const current = localStorage.getItem(currentKey);
  if (current !== null) return current;

  const legacyKey = `${LEGACY_PREFIX}${name}`;
  const legacy = localStorage.getItem(legacyKey);
  if (legacy === null) return null;

  try {
    localStorage.setItem(currentKey, legacy);
    localStorage.removeItem(legacyKey);
  } catch {
    // Returning the value still restores this launch. The legacy key remains
    // available for another migration attempt when storage becomes writable.
  }
  return legacy;
}

export function writeStored(name: string, value: string): void {
  localStorage.setItem(`${CURRENT_PREFIX}${name}`, value);
}

// State that belongs to one project rather than to the machine.
//
// The roster and the conductor id name panes, brains and worktree paths that
// only mean anything inside the repository they were created for. Held in one
// global key, a roster built for repository A was read back after the selected
// project changed to B, pointing B's window at A's worktrees. Layout settings
// are genuinely machine-local and deliberately keep the unscoped keys.

/**
 * A storage scope derived from the selected project. Branded so a raw path
 * cannot be passed where a scope is expected: the two are the same runtime
 * type and only the derivation makes a path safe to use as a key.
 */
export type ProjectScope = string & { readonly __projectScope: unique symbol };

// Its own bucket rather than the unscoped key, so "no project" is a scope like
// any other and never shares storage with a real repository.
const NO_PROJECT_SCOPE = "~none";

/** Derive the storage scope for a selected project path. */
export function projectScope(project: string | null): ProjectScope {
  const path = project !== null && project.trim() !== "" ? project : null;
  // encodeURIComponent rather than a hash: a misbehaving restore is diagnosed
  // by reading the key, and localStorage has no key-length limit worth a hash.
  return (path === null ? NO_PROJECT_SCOPE : encodeURIComponent(path)) as ProjectScope;
}

function scopedKey(name: string, scope: ProjectScope): string {
  return `${CURRENT_PREFIX}${name}:${scope}`;
}

// Which scope claimed the pre-scoping state, if any.
//
// ONE claim for every scoped key together, not one per key. The roster and the
// conductor id are halves of a single window state: the conductor names a pane
// in the roster, and pane ids are `sess-N` counters that repeat across projects,
// so a conductor id adopted into a project whose roster came from somewhere else
// does not dangle, it silently points at a different project's same-numbered
// pane. Per-key claims allowed exactly that. Adopting the roster into A and then
// failing to copy the conductor left the conductor unclaimed, and the next
// project to read it took it.
//
// Kept permanently, not cleared once adoption completes. An unscoped value can
// reappear at any time, most plausibly from an older build of Pantheon writing
// the key it still knows about, and a claim that had been tidied away would let
// whichever project read next inherit it. The claim outliving the values it
// guarded is what makes the adoption once-only rather than once-at-a-time.
const LEGACY_OWNER_KEY = `${CURRENT_PREFIX}legacyOwner`;

// The keys that pre-scoping state can be adopted from, and that a destination
// has to be clear of before an empty window may bind to it.
const SCOPED_NAMES = ["panes", "conductor"] as const;

// Drop the value a scoped copy replaced. The claim stays behind on purpose.
function dropAdoptedLegacy(name: string): void {
  try {
    localStorage.removeItem(`${CURRENT_PREFIX}${name}`);
  } catch {
    // Retried on the next read by the scope the claim names.
  }
}

/**
 * Read project-scoped state, adopting a pre-scoping value exactly once and
 * only into the scope that claimed it.
 *
 * Adoption is ownership-bound rather than first-come. An unscoped value is
 * claimed by writing the claim *before* the copy, so a write that fails partway
 * leaves a record of who was mid-adoption: that scope retries, and every other
 * scope reads past the leftover value instead of inheriting another project's
 * panes. A claim that cannot be recorded refuses the adoption rather than
 * taking the value unguarded, because an unguarded adoption is the
 * cross-project contamination this scoping exists to stop. Nothing is deleted
 * on that path, so the value is still there to adopt once storage accepts
 * writes again.
 */
export function readScoped(name: string, scope: ProjectScope): string | null {
  const key = scopedKey(name, scope);
  const ownerKey = LEGACY_OWNER_KEY;
  const scoped = localStorage.getItem(key);

  if (scoped !== null) {
    // An adoption that wrote the copy but could not clear what it replaced
    // leaves both, as does an older build writing the unscoped key again.
    // Either way the scoped copy is this build's authority, so finish the
    // cleanup here rather than leaving the value for another project to find.
    if (
      localStorage.getItem(`${CURRENT_PREFIX}${name}`) !== null &&
      localStorage.getItem(ownerKey) === scope
    ) {
      dropAdoptedLegacy(name);
    }
    return scoped;
  }

  // readStored, so a value still under the pre-rename identity is normalised
  // before it is considered for adoption.
  const legacy = readStored(name);
  if (legacy === null) return null;

  const owner = localStorage.getItem(ownerKey);
  if (owner !== null && owner !== scope) return null;

  if (owner === null) {
    try {
      localStorage.setItem(ownerKey, scope);
    } catch {
      return null;
    }
  }

  try {
    localStorage.setItem(key, legacy);
    dropAdoptedLegacy(name);
  } catch {
    // The claim is recorded, so only this scope will retry and the value stays
    // where it is. Returning it still restores this launch.
  }
  return legacy;
}

/** Write project-scoped state. */
export function writeScoped(name: string, scope: ProjectScope, value: string): void {
  localStorage.setItem(scopedKey(name, scope), value);
}

/**
 * Whether a project's storage provably holds nothing this window could
 * overwrite or strand. `null` means that could not be determined, which is not
 * the same as "empty" and must never be treated as one.
 *
 * Used to gate the one exception to the persistence freeze, so the check is
 * deliberately stricter than "no scoped value". A destination is only unused
 * when, for both the roster and the conductor id:
 *
 * - it has no scoped value of its own, and
 * - there is no pre-scoping value this scope could still adopt.
 *
 * The second clause is the one that is easy to get wrong. An unclaimed legacy
 * value is state this project would have inherited on a later launch; binding
 * to the project and persisting over it would strand that value permanently,
 * because once a scoped copy exists nothing ever reads the legacy key again.
 * A legacy value already claimed by another scope is different: this project
 * could never have adopted it, so it is not this project's to lose.
 */
export function scopeIsUnused(scope: ProjectScope): boolean | null {
  try {
    const owner = localStorage.getItem(LEGACY_OWNER_KEY);
    const legacyAdoptable = owner === null || owner === scope;
    for (const name of SCOPED_NAMES) {
      if (localStorage.getItem(scopedKey(name, scope)) !== null) return false;
      if (!legacyAdoptable) continue;
      if (localStorage.getItem(`${CURRENT_PREFIX}${name}`) !== null) return false;
      if (localStorage.getItem(`${LEGACY_PREFIX}${name}`) !== null) return false;
    }
    return true;
  } catch {
    // Storage refused a read. Uncertain, never "empty".
    return null;
  }
}
