import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TerminalPane } from "./TerminalPane";
import { SESSION_TYPES } from "../lib/ipc";
import type { ExternalSession } from "./TerminalPane";

const state = vi.hoisted(() => ({ writes: [] as Array<string | Uint8Array>, listener: null as null | ((event: { payload: string }) => void) }));
vi.mock("@xterm/xterm", () => ({ Terminal: class {
  options: Record<string, unknown> = {}; rows = 24; cols = 80;
  loadAddon() {} open() {} focus() {} dispose() {} onData() {}
  attachCustomKeyEventHandler() {} getSelection() { return ""; }
  write(value: string | Uint8Array) { state.writes.push(value); }
} }));
vi.mock("@xterm/addon-fit", () => ({ FitAddon: class { fit() {} } }));
vi.mock("@tauri-apps/api/core", () => ({ Channel: class { onmessage = () => {}; }, invoke: vi.fn(async () => {}) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async (_name: string, listener: (event: { payload: string }) => void) => { state.listener = listener; return () => {}; }) }));
vi.mock("../lib/appearance", () => ({ useAppearance: () => ({ theme: { id: "test", xterm: {} }, appearance: { fontSize: 14 } }) }));

const frames = new Map<number, FrameRequestCallback>();
let frameId = 0;
beforeEach(() => {
  state.writes.length = 0; state.listener = null; frames.clear(); frameId = 0;
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => { frames.set(++frameId, callback); return frameId; });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
});
afterEach(() => vi.unstubAllGlobals());
function flushFrames() { for (const [id, callback] of [...frames]) { frames.delete(id); callback(0); } }
function outputText() { return state.writes.map((value) => typeof value === "string" ? value : new TextDecoder().decode(value)).join(""); }
function props() { return { sessionId: "sess-99", type: SESSION_TYPES[0], onExit: vi.fn(), onIsolationChange: vi.fn(), externalSession: { channel: { onmessage() {} }, buffered: [[65]] } as unknown as ExternalSession }; }

describe("requested terminal exit handoff", () => {
  it("writes buffered output before an exit already known at mount", () => {
    const p = props();
    render(<TerminalPane {...p} alreadyExited />);
    expect(outputText().startsWith("A")).toBe(true);
    expect(outputText()).toContain("[session ended]");
    act(() => state.listener?.({ payload: p.sessionId }));
    expect(p.onExit).toHaveBeenCalledTimes(1);
  });
  it("flushes buffered output before a later authoritative exit prop", () => {
    const p = props();
    const rendered = render(<TerminalPane {...p} alreadyExited={false} />);
    rendered.rerender(<TerminalPane {...p} alreadyExited />);
    act(flushFrames);
    expect(outputText().startsWith("A")).toBe(true);
    expect(outputText()).toContain("[session ended]");
    act(() => state.listener?.({ payload: p.sessionId }));
    expect(p.onExit).toHaveBeenCalledTimes(1);
  });
  it("does not double-report when alreadyExited stays true across a rerender with a new onExit", () => {
    const p = props();
    const rendered = render(<TerminalPane {...p} alreadyExited />);
    expect(p.onExit).toHaveBeenCalledTimes(1);
    // A rerender that changes onExit's identity (e.g. a parent re-render)
    // re-runs the alreadyExited effect, since onExit is in its dependency
    // array — exitReportedRef must still stop it from reporting a second time.
    const onExit2 = vi.fn();
    rendered.rerender(<TerminalPane {...p} onExit={onExit2} alreadyExited />);
    expect(onExit2).not.toHaveBeenCalled();
    expect(p.onExit).toHaveBeenCalledTimes(1);
  });
});
