import { beforeEach, describe, expect, it, vi } from "vitest";
import { projectScope, readScoped, readStored, writeScoped, writeStored } from "./storage";

describe("renamed local storage", () => {
  beforeEach(() => localStorage.clear());

  it("adopts a Mosaic value when Pantheon has not written one", () => {
    localStorage.setItem("mosaic.project", "/work/project");

    expect(readStored("project")).toBe("/work/project");
    expect(localStorage.getItem("pantheon.project")).toBe("/work/project");
    expect(localStorage.getItem("mosaic.project")).toBeNull();
  });

  it("keeps the Pantheon value when both identities exist", () => {
    localStorage.setItem("mosaic.layout", "scroll");
    localStorage.setItem("pantheon.layout", "fit");

    expect(readStored("layout")).toBe("fit");
    expect(localStorage.getItem("mosaic.layout")).toBe("scroll");
  });

  it("writes only the Pantheon identity", () => {
    writeStored("isolate", "1");

    expect(localStorage.getItem("pantheon.isolate")).toBe("1");
  });
});

describe("project-scoped storage", () => {
  beforeEach(() => localStorage.clear());

  const A = projectScope("/work/alpha");
  const B = projectScope("/work/beta");
  const NONE = projectScope(null);

  it("keeps each project's value in its own bucket", () => {
    writeScoped("panes", A, "alpha-roster");
    writeScoped("panes", B, "beta-roster");

    expect(readScoped("panes", A)).toBe("alpha-roster");
    expect(readScoped("panes", B)).toBe("beta-roster");
  });

  it("does not read one project's value from another", () => {
    writeScoped("panes", A, "alpha-roster");

    expect(readScoped("panes", B)).toBeNull();
  });

  it("gives no-project its own bucket rather than the unscoped key", () => {
    writeScoped("panes", NONE, "no-project-roster");

    expect(readScoped("panes", NONE)).toBe("no-project-roster");
    expect(localStorage.getItem("pantheon.panes")).toBeNull();
    expect(readScoped("panes", A)).toBeNull();
  });

  it("treats an empty or whitespace project as no project", () => {
    expect(projectScope("")).toBe(projectScope(null));
    expect(projectScope("   ")).toBe(projectScope(null));
  });

  it("distinguishes paths that differ only after encoding", () => {
    expect(projectScope("/work/a b")).not.toBe(projectScope("/work/a%20b"));
  });

  it("adopts a pre-scoping roster into the first project that reads it", () => {
    localStorage.setItem("pantheon.panes", "legacy-roster");

    expect(readScoped("panes", A)).toBe("legacy-roster");
    expect(localStorage.getItem("pantheon.panes:" + A)).toBe("legacy-roster");
    // The value it replaced, and the claim that guarded it, are both gone.
    expect(localStorage.getItem("pantheon.panes")).toBeNull();
    // The claim outlives the value, so no second project can ever adopt it.
    expect(localStorage.getItem("pantheon.legacyOwner")).toBe(A);
  });

  it("adopts a pre-rename roster through the Mosaic identity too", () => {
    localStorage.setItem("mosaic.panes", "very-old-roster");

    expect(readScoped("panes", A)).toBe("very-old-roster");
    expect(localStorage.getItem("pantheon.panes:" + A)).toBe("very-old-roster");
    expect(localStorage.getItem("mosaic.panes")).toBeNull();
  });

  it("never adopts a leftover legacy roster into a second project", () => {
    localStorage.setItem("pantheon.panes", "legacy-roster");
    expect(readScoped("panes", A)).toBe("legacy-roster");

    // Even if something puts the legacy key back, B is not its owner.
    localStorage.setItem("pantheon.panes", "legacy-roster");
    expect(readScoped("panes", B)).toBeNull();
    expect(localStorage.getItem("pantheon.panes:" + B)).toBeNull();
  });

  it("refuses adoption outright when the claim cannot be recorded", () => {
    localStorage.setItem("pantheon.panes", "legacy-roster");
    const setItem = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("QuotaExceededError");
    });

    // Unguarded adoption is what leaks one project's panes into another, so a
    // claim that cannot be written refuses rather than taking the value.
    expect(readScoped("panes", A)).toBeNull();
    setItem.mockRestore();

    // Nothing was destroyed: the value is still there to adopt later.
    expect(localStorage.getItem("pantheon.panes")).toBe("legacy-roster");
    expect(readScoped("panes", A)).toBe("legacy-roster");
  });

  it("retries an adoption whose copy failed, and only for the owning scope", () => {
    // The state a partial write leaves behind: the claim recorded, the copy
    // never written, the value it would have replaced still in place.
    localStorage.setItem("pantheon.panes", "legacy-roster");
    localStorage.setItem("pantheon.legacyOwner", A);
    expect(localStorage.getItem("pantheon.panes:" + A)).toBeNull();

    // B must not take it, even though no scoped copy exists yet.
    expect(readScoped("panes", B)).toBeNull();
    // A retries and completes.
    expect(readScoped("panes", A)).toBe("legacy-roster");
    expect(localStorage.getItem("pantheon.panes:" + A)).toBe("legacy-roster");
    expect(localStorage.getItem("pantheon.panes")).toBeNull();
    expect(localStorage.getItem("pantheon.legacyOwner")).toBe(A);
  });

  it("finishes a cleanup that left both the copy and the legacy value", () => {
    localStorage.setItem("pantheon.panes", "legacy-roster");
    localStorage.setItem("pantheon.panes:" + A, "legacy-roster");
    localStorage.setItem("pantheon.legacyOwner", A);

    expect(readScoped("panes", A)).toBe("legacy-roster");
    expect(localStorage.getItem("pantheon.panes")).toBeNull();
    expect(localStorage.getItem("pantheon.legacyOwner")).toBe(A);
  });

  it("prefers a project's own value over a legacy one it does not own", () => {
    localStorage.setItem("pantheon.panes", "legacy-roster");
    localStorage.setItem("pantheon.legacyOwner", A);
    writeScoped("panes", B, "beta-roster");

    expect(readScoped("panes", B)).toBe("beta-roster");
    // B's read left A's claim and the value it guards untouched.
    expect(localStorage.getItem("pantheon.panes")).toBe("legacy-roster");
    expect(readScoped("panes", A)).toBe("legacy-roster");
  });

  it("scopes the conductor id, under the same claim as the roster", () => {
    localStorage.setItem("pantheon.conductor", "sess-3");

    expect(readScoped("conductor", A)).toBe("sess-3");
    expect(readScoped("conductor", B)).toBeNull();
    expect(localStorage.getItem("pantheon.legacyOwner")).toBe(A);
  });

  it("never splits a pre-scoping roster and conductor across two projects", () => {
    // The hazard a per-key claim allowed: A adopts the roster, the conductor
    // copy does not land, and B then reads a conductor that names a pane in
    // A's roster. Pane ids are `sess-N` counters that repeat across projects,
    // so that id does not dangle in B, it points at B's own same-numbered pane.
    localStorage.setItem("pantheon.panes", "alpha-roster");
    localStorage.setItem("pantheon.conductor", "sess-1");

    // A adopts the roster and claims the pre-scoping state as a whole.
    expect(readScoped("panes", A)).toBe("alpha-roster");
    expect(localStorage.getItem("pantheon.legacyOwner")).toBe(A);
    // The conductor half never got copied: still unscoped, still unadopted.
    expect(localStorage.getItem("pantheon.conductor")).toBe("sess-1");
    expect(localStorage.getItem("pantheon.conductor:" + A)).toBeNull();

    // B must inherit neither half.
    expect(readScoped("conductor", B)).toBeNull();
    expect(readScoped("panes", B)).toBeNull();
    expect(localStorage.getItem("pantheon.conductor:" + B)).toBeNull();

    // A still completes its own adoption afterwards.
    expect(readScoped("conductor", A)).toBe("sess-1");
  });

  it("keeps one claim across both keys even when the roster half is absent", () => {
    localStorage.setItem("pantheon.conductor", "sess-2");

    expect(readScoped("panes", A)).toBeNull();
    // Reading an absent roster claims nothing, so B is still a candidate...
    expect(localStorage.getItem("pantheon.legacyOwner")).toBeNull();
    // ...until some scope actually adopts, and then it owns both keys.
    expect(readScoped("conductor", B)).toBe("sess-2");
    expect(localStorage.getItem("pantheon.legacyOwner")).toBe(B);
    expect(readScoped("conductor", A)).toBeNull();
  });
});
