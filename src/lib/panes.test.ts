import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setConductor } from "./ipc";
import {
  decideConductorRestore,
  loadConductorId,
  loadRoster,
  restoreConductor,
  saveConductorId,
  saveRoster,
  shouldBindToProject,
  shouldFreezePersistence,
} from "./panes";
import { projectScope, scopeIsUnused } from "./storage";

// Every roster and conductor call is keyed by the project the window opened
// with. These suites use one scope throughout; the cross-project guarantees
// live in storage.test.ts, and the switch that ends persistence is covered by
// shouldFreezePersistence below.
const SCOPE = projectScope("/work/alpha");

vi.mock("./ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./ipc")>();
  return {
    ...actual,
    setConductor: vi.fn().mockResolvedValue(undefined),
  };
});

describe("conductor persistence", () => {
  beforeEach(() => localStorage.clear());

  it("round-trips the conductor id through the roster store", () => {
    expect(loadConductorId(SCOPE)).toBeNull();

    saveConductorId(SCOPE, "sess-1", ["sess-1", "sess-2"]);
    expect(loadConductorId(SCOPE)).toBe("sess-1");

    saveConductorId(SCOPE, null, ["sess-1", "sess-2"]);
    expect(loadConductorId(SCOPE)).toBeNull();
  });

  it("never persists a conductor id for a pane that is not in the roster", () => {
    saveConductorId(SCOPE, "sess-1", ["sess-2", "sess-3"]);
    expect(loadConductorId(SCOPE)).toBeNull();
  });

  it("clears a previously saved id once its pane drops out of the roster", () => {
    saveConductorId(SCOPE, "sess-1", ["sess-1", "sess-2"]);
    expect(loadConductorId(SCOPE)).toBe("sess-1");

    saveConductorId(SCOPE, "sess-1", ["sess-2"]);
    expect(loadConductorId(SCOPE)).toBeNull();
  });
});

describe("decideConductorRestore", () => {
  it("restores the exact saved pane when it is part of the roster", () => {
    expect(decideConductorRestore("sess-1", ["sess-1", "sess-2"])).toEqual({
      restore: true,
      id: "sess-1",
    });
  });

  it("clears a saved conductor whose pane is absent, rather than reassigning it", () => {
    // The roster has other panes, but none of them is the saved id, so the
    // decision must carry the saved id back for clearing, never substitute
    // one of the panes that *is* present.
    expect(decideConductorRestore("sess-1", ["sess-2", "sess-3"])).toEqual({
      restore: false,
      id: "sess-1",
    });
    expect(decideConductorRestore("sess-1", [])).toEqual({
      restore: false,
      id: "sess-1",
    });
  });

  it("has nothing to restore when no conductor was ever saved", () => {
    expect(decideConductorRestore(null, ["sess-1"])).toEqual({
      restore: false,
      id: null,
    });
  });
});

describe("restoreConductor", () => {
  beforeEach(() => localStorage.clear());
  afterEach(() => vi.mocked(setConductor).mockClear());

  it("calls setConductor exactly once with the saved id when the pane exists", async () => {
    const notice = await restoreConductor(SCOPE, "sess-1", ["sess-1", "sess-2"]);
    expect(setConductor).toHaveBeenCalledTimes(1);
    expect(setConductor).toHaveBeenCalledWith("sess-1");
    expect(notice).toBeNull();
  });

  it("clears the saved id and reports a notice instead of promoting another pane", async () => {
    saveConductorId(SCOPE, "sess-1", ["sess-1"]);
    expect(loadConductorId(SCOPE)).toBe("sess-1");

    const notice = await restoreConductor(SCOPE, "sess-1", ["sess-2"]);

    expect(setConductor).not.toHaveBeenCalled();
    expect(notice).toContain("sess-1");
    expect(loadConductorId(SCOPE)).toBeNull();
  });

  it("does nothing and reports nothing when no conductor was saved", async () => {
    const notice = await restoreConductor(SCOPE, null, ["sess-1"]);
    expect(setConductor).not.toHaveBeenCalled();
    expect(notice).toBeNull();
  });
});

describe("restoreConductor repairs the mount-time persist race", () => {
  // On mount, App.tsx's own conductor state starts null, so its persist
  // effect (keyed on that state and the panes) runs before restoreConductor's
  // setConductor call resolves, and writes an empty conductor to storage in
  // the meantime. These tests reproduce that ordering directly against the
  // storage functions, without needing a live App.tsx render.
  beforeEach(() => localStorage.clear());
  afterEach(() => vi.mocked(setConductor).mockReset().mockResolvedValue(undefined));

  it("leaves the saved id in storage after a persist(null) lands before restore resolves", async () => {
    saveConductorId(SCOPE, "sess-1", ["sess-1"]);
    expect(loadConductorId(SCOPE)).toBe("sess-1");

    // The mount-time persist effect, firing with conductor still null.
    saveConductorId(SCOPE, null, ["sess-1"]);
    expect(loadConductorId(SCOPE)).toBeNull();

    // The saved id passed in here is the one captured before the first
    // render, independent of the persist effect that already cleared
    // storage, matching how App.tsx calls this with a ref taken at mount.
    const notice = await restoreConductor(SCOPE, "sess-1", ["sess-1"]);

    expect(notice).toBeNull();
    expect(loadConductorId(SCOPE)).toBe("sess-1");
  });

  it("returns a notice, rather than silence, when the restore's setConductor rejects", async () => {
    saveConductorId(SCOPE, "sess-1", ["sess-1"]);
    saveConductorId(SCOPE, null, ["sess-1"]);
    vi.mocked(setConductor).mockRejectedValueOnce(new Error("no such pane"));

    const notice = await restoreConductor(SCOPE, "sess-1", ["sess-1"]);

    expect(notice).toContain("sess-1");
    // The pane was not actually promoted, so leaving the id cleared is
    // correct here; what must not happen is losing the failure silently.
    expect(loadConductorId(SCOPE)).toBeNull();
  });
});

describe("rosters are kept per project", () => {
  beforeEach(() => localStorage.clear());

  const A = projectScope("/work/alpha");
  const B = projectScope("/work/beta");
  const NONE = projectScope(null);

  const pane = (id: string) => ({
    id,
    typeId: "claude",
    brain: "main",
    isolate: false,
  });

  it("restores each project's own panes and never the other's", () => {
    saveRoster(A, [pane("sess-1")]);
    saveRoster(B, [pane("sess-9")]);

    expect(loadRoster(A).panes.map((p) => p.id)).toEqual(["sess-1"]);
    expect(loadRoster(B).panes.map((p) => p.id)).toEqual(["sess-9"]);
  });

  it("starts empty in a project that has never saved a roster", () => {
    saveRoster(A, [pane("sess-1")]);

    expect(loadRoster(B).panes).toEqual([]);
    expect(loadRoster(B).problems).toEqual([]);
  });

  it("keeps a no-project roster out of every project", () => {
    saveRoster(NONE, [pane("sess-1")]);

    expect(loadRoster(NONE).panes.map((p) => p.id)).toEqual(["sess-1"]);
    expect(loadRoster(A).panes).toEqual([]);
  });

  it("scopes the conductor id, so one project's is not restored in another", async () => {
    saveConductorId(A, "sess-1", ["sess-1"]);

    expect(loadConductorId(A)).toBe("sess-1");
    expect(loadConductorId(B)).toBeNull();
  });

  it("clears rather than promotes when a conductor id outlives its project's panes", async () => {
    saveConductorId(A, "sess-1", ["sess-1"]);

    // B's roster spawned different panes; the saved id names none of them.
    const notice = await restoreConductor(B, loadConductorId(A), ["sess-7", "sess-8"]);

    expect(setConductor).not.toHaveBeenCalled();
    expect(notice).toContain("sess-1");
    expect(loadConductorId(B)).toBeNull();
    // A's own record is untouched by B's failed restore.
    expect(loadConductorId(A)).toBe("sess-1");
  });
});

describe("shouldFreezePersistence", () => {
  it("keeps persisting while the selected project is the one the window opened with", () => {
    expect(shouldFreezePersistence("/work/alpha", "/work/alpha", false)).toBe(false);
    expect(shouldFreezePersistence(null, null, false)).toBe(false);
  });

  it("freezes as soon as a different project is selected", () => {
    expect(shouldFreezePersistence("/work/alpha", "/work/beta", false)).toBe(true);
    expect(shouldFreezePersistence(null, "/work/beta", false)).toBe(true);
    expect(shouldFreezePersistence("/work/alpha", null, false)).toBe(true);
  });

  it("stays frozen once frozen, including on a return to the startup project", () => {
    // Panes launched while the other project was selected are still in the
    // list, so the roster does not become trustworthy again by going back.
    expect(shouldFreezePersistence("/work/alpha", "/work/alpha", true)).toBe(true);
    expect(shouldFreezePersistence("/work/alpha", "/work/beta", true)).toBe(true);
  });
});

describe("a mid-session project switch strands neither project's roster", () => {
  beforeEach(() => localStorage.clear());

  const A = projectScope("/work/alpha");
  const B = projectScope("/work/beta");
  const pane = (id: string) => ({ id, typeId: "claude", brain: "main", isolate: false });

  // What App.tsx's persist effects do: write, unless this window has frozen.
  function persist(frozen: boolean, panes: ReturnType<typeof pane>[]) {
    if (frozen) return;
    saveRoster(A, panes);
    saveConductorId(A, panes[0]?.id ?? null, panes.map((p) => p.id));
  }

  it("stops writing the startup project's bucket once another project is picked", () => {
    // Window opens on A with one pane, and persists normally.
    let frozen = false;
    persist(frozen, [pane("sess-1")]);
    expect(loadRoster(A).panes.map((p) => p.id)).toEqual(["sess-1"]);

    // The user picks B, then launches a pane under it.
    frozen = shouldFreezePersistence("/work/alpha", "/work/beta", frozen);
    expect(frozen).toBe(true);
    persist(frozen, [pane("sess-1"), pane("sess-2")]);

    // A's saved roster is exactly what it was: sess-2 was never A's pane.
    expect(loadRoster(A).panes.map((p) => p.id)).toEqual(["sess-1"]);
    expect(loadConductorId(A)).toBe("sess-1");
    // And nothing was written into B either, so B restarts on its own roster.
    expect(loadRoster(B).panes).toEqual([]);
    expect(loadConductorId(B)).toBeNull();
  });

  it("keeps closing a pane after the switch from rewriting the old roster", () => {
    let frozen = false;
    persist(frozen, [pane("sess-1"), pane("sess-2")]);
    expect(loadRoster(A).panes.map((p) => p.id)).toEqual(["sess-1", "sess-2"]);

    frozen = shouldFreezePersistence("/work/alpha", "/work/beta", frozen);
    persist(frozen, []);

    // A pane closed under B must not empty A's remembered roster.
    expect(loadRoster(A).panes.map((p) => p.id)).toEqual(["sess-1", "sess-2"]);
    expect(loadConductorId(A)).toBe("sess-1");
  });
});

describe("the first project an empty window picks", () => {
  beforeEach(() => localStorage.clear());

  const B = projectScope("/work/beta");
  const pane = (id: string) => ({ id, typeId: "claude", brain: "main", isolate: false });

  // Startup with no project, nothing on screen, no earlier switch, destination
  // provably unused: the one case that binds instead of freezing.
  const empty = () => shouldBindToProject(null, false, false, false, scopeIsUnused(B));

  it("binds, so a first session's panes are still saved", () => {
    expect(empty()).toBe(true);
  });

  it("refuses once the window already has a project to persist to", () => {
    expect(shouldBindToProject("/work/alpha", false, false, false, true)).toBe(false);
  });

  it("refuses after an earlier switch has already frozen it", () => {
    expect(shouldBindToProject(null, true, false, false, true)).toBe(false);
  });

  it("refuses while any pane or conductor is on screen", () => {
    // Those belong to the scope the window opened with, not to the destination.
    expect(shouldBindToProject(null, false, true, false, true)).toBe(false);
    expect(shouldBindToProject(null, false, false, true, true)).toBe(false);
  });

  it("refuses a destination that already has a saved roster or conductor", () => {
    saveRoster(B, [pane("sess-4")]);
    expect(scopeIsUnused(B)).toBe(false);
    expect(empty()).toBe(false);

    localStorage.clear();
    saveConductorId(B, "sess-4", ["sess-4"]);
    expect(scopeIsUnused(B)).toBe(false);
    expect(empty()).toBe(false);
  });

  it("refuses while a pre-scoping value is still there for it to adopt", () => {
    // Binding and persisting would strand it: once a scoped copy exists,
    // nothing reads the unscoped key again.
    localStorage.setItem("pantheon.panes", "legacy-roster");
    expect(scopeIsUnused(B)).toBe(false);
    expect(empty()).toBe(false);

    localStorage.clear();
    localStorage.setItem("mosaic.conductor", "sess-1");
    expect(scopeIsUnused(B)).toBe(false);
    expect(empty()).toBe(false);
  });

  it("still binds when the leftover value belongs to another project", () => {
    // Claimed by someone else, so this project could never have adopted it and
    // has nothing to lose by persisting.
    localStorage.setItem("pantheon.panes", "alpha-roster");
    localStorage.setItem("pantheon.legacyOwner", projectScope("/work/alpha"));

    expect(scopeIsUnused(B)).toBe(true);
    expect(empty()).toBe(true);
  });

  it("treats storage it could not read as occupied, never as empty", () => {
    const getItem = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("SecurityError");
    });
    const unused = scopeIsUnused(B);
    getItem.mockRestore();

    expect(unused).toBeNull();
    expect(shouldBindToProject(null, false, false, false, unused)).toBe(false);
  });

  it("freezes on the next switch after binding, including back to nothing", () => {
    // Once bound to B, B is the project this window persists to, so returning
    // to it is not a change, and moving on from it is.
    expect(shouldFreezePersistence("/work/beta", "/work/beta", false)).toBe(false);
    expect(shouldFreezePersistence("/work/beta", "/work/gamma", false)).toBe(true);
  });
});
