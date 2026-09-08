import { describe, expect, it } from "vitest";
import { PendingExits } from "./pendingExits";

describe("PendingExits", () => {
  it("take returns false when nothing was ever retained for that session id", () => {
    const pending = new PendingExits();
    expect(pending.take("sess-1")).toBe(false);
  });

  it("take recovers a retained event — the event-before-response ordering this exists for", () => {
    const pending = new PendingExits();
    pending.retain("sess-1");
    expect(pending.take("sess-1")).toBe(true);
  });

  it("take consumes the event: a second take for the same id returns false", () => {
    const pending = new PendingExits();
    pending.retain("sess-1");
    pending.take("sess-1");
    expect(pending.take("sess-1")).toBe(false);
  });

  it("tracks multiple in-flight sessions independently", () => {
    const pending = new PendingExits();
    pending.retain("sess-1");
    expect(pending.take("sess-2")).toBe(false);
    expect(pending.take("sess-1")).toBe(true);
  });

  it("retaining the same id twice is harmless — still a single, single-use record", () => {
    const pending = new PendingExits();
    pending.retain("sess-1");
    pending.retain("sess-1");
    expect(pending.take("sess-1")).toBe(true);
    expect(pending.take("sess-1")).toBe(false);
  });
});
