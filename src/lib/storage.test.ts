import { beforeEach, describe, expect, it } from "vitest";
import { readStored, writeStored } from "./storage";

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
