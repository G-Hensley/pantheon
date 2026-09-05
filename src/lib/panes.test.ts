import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setConductor } from "./ipc";
import {
  decideConductorRestore,
  loadConductorId,
  restoreConductor,
  saveConductorId,
} from "./panes";

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
    expect(loadConductorId()).toBeNull();

    saveConductorId("sess-1", ["sess-1", "sess-2"]);
    expect(loadConductorId()).toBe("sess-1");

    saveConductorId(null, ["sess-1", "sess-2"]);
    expect(loadConductorId()).toBeNull();
  });

  it("never persists a conductor id for a pane that is not in the roster", () => {
    saveConductorId("sess-1", ["sess-2", "sess-3"]);
    expect(loadConductorId()).toBeNull();
  });

  it("clears a previously saved id once its pane drops out of the roster", () => {
    saveConductorId("sess-1", ["sess-1", "sess-2"]);
    expect(loadConductorId()).toBe("sess-1");

    saveConductorId("sess-1", ["sess-2"]);
    expect(loadConductorId()).toBeNull();
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
    // The roster has other panes, but none of them is the saved id — the
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
    const notice = await restoreConductor("sess-1", ["sess-1", "sess-2"]);
    expect(setConductor).toHaveBeenCalledTimes(1);
    expect(setConductor).toHaveBeenCalledWith("sess-1");
    expect(notice).toBeNull();
  });

  it("clears the saved id and reports a notice instead of promoting another pane", async () => {
    saveConductorId("sess-1", ["sess-1"]);
    expect(loadConductorId()).toBe("sess-1");

    const notice = await restoreConductor("sess-1", ["sess-2"]);

    expect(setConductor).not.toHaveBeenCalled();
    expect(notice).toContain("sess-1");
    expect(loadConductorId()).toBeNull();
  });

  it("does nothing and reports nothing when no conductor was saved", async () => {
    const notice = await restoreConductor(null, ["sess-1"]);
    expect(setConductor).not.toHaveBeenCalled();
    expect(notice).toBeNull();
  });
});
