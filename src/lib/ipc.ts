import { Channel, invoke } from "@tauri-apps/api/core";

// Raw channel messages are normally ArrayBuffers, but Tauri historically sent
// small raw messages as number[] (fixed in 2.5.1). Handle every shape.
export type Bytes = ArrayBuffer | ArrayBufferView | number[];

export function toBytes(msg: Bytes): Uint8Array {
  if (msg instanceof ArrayBuffer) return new Uint8Array(msg);
  if (ArrayBuffer.isView(msg)) {
    return new Uint8Array(msg.buffer, msg.byteOffset, msg.byteLength);
  }
  return new Uint8Array(msg);
}

export type SessionType = {
  id: string;
  label: string;
  program: string;
  args: string[];
  color: string;
  modelFlag?: string;
};

// Which plain shell the Shell pane launches. The webview user agent is the only
// platform signal available here without adding a Tauri OS plugin, and it is
// enough: WebView2 reports Windows, WebKitGTK and WKWebView do not. Guarded for
// the jsdom test environment, which has a user agent but no real platform.
const IS_WINDOWS =
  typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);

// The launchable session types. The three agent CLIs are wired to the shared
// brain automatically at launch (see agent_mcp_wiring in the backend); Shell is
// a plain terminal with no MCP wiring. Only the shell differs by platform: the
// agent CLIs carry the same command name everywhere.
export const SESSION_TYPES: SessionType[] = [
  IS_WINDOWS
    ? { id: "shell", label: "Shell", program: "powershell.exe", args: ["-NoLogo"], color: "#7dcfff" }
    : { id: "shell", label: "Shell", program: "bash", args: [], color: "#7dcfff" },
  { id: "claude", label: "Claude Code", program: "claude", args: [], color: "#e0af68", modelFlag: "--model" },
  { id: "codex", label: "Codex", program: "codex", args: [], color: "#bb9af7", modelFlag: "-m" },
  { id: "opencode", label: "opencode", program: "opencode", args: [], color: "#9ece6a", modelFlag: "-m" },
];

// The git worktree an isolated session runs in. Reported by the backend once the
// session is live (the `session-worktree` event) and handed back on the next
// launch so a restored pane returns to the worktree it was already working in
// instead of stranding it — see `choose_worktree` in the backend.
export type SavedWorktree = {
  repo: string;
  path: string;
  branch: string;
  base: string;
};

export type SessionWorktreeEvent = SavedWorktree & { sessionId: string };

export function spawnSession(
  sessionId: string,
  channel: Channel<Bytes>,
  program: string,
  args: string[],
  rows: number,
  cols: number,
  opts?: { cwd?: string; isolate?: boolean; reuseWorktree?: SavedWorktree; model?: string; modelFlag?: string },
): Promise<void> {
  return invoke("spawn_session", {
    sessionId,
    channel,
    program,
    args,
    rows,
    cols,
    cwd: opts?.cwd,
    isolate: opts?.isolate,
    reuseWorktree: opts?.reuseWorktree,
    model: opts?.model,
    modelFlag: opts?.modelFlag,
  });
}

export const writeSession = (sessionId: string, data: string): Promise<void> =>
  invoke("write_session", { sessionId, data });

export const resizeSession = (sessionId: string, rows: number, cols: number): Promise<void> =>
  invoke("resize_session", { sessionId, rows, cols });

export const killSession = (sessionId: string): Promise<void> =>
  invoke("kill_session", { sessionId });

// ---- Shared brain (MCP) ----
export type ContextEntry = {
  kind: string; // "decision" | "fact" | "broadcast"
  author: string;
  topic: string;
  body: string;
  ts_ms: number;
  room: string; // which brain
};
export type AgentIdentity = { name: string; kind: string };
export type ContextSnapshot = { entries: ContextEntry[]; sessions: AgentIdentity[] };
export type McpInfo = { url: string; port: number };

export const getContext = (): Promise<ContextSnapshot> => invoke("get_context");
export const mcpInfo = (): Promise<McpInfo> => invoke("mcp_info");
export const setAgentBrain = (name: string, brain: string): Promise<void> =>
  invoke("set_agent_brain", { name, brain });

// Tell the backend which project is active, so the shared brain writes its
// markdown into that project's .pantheon/context instead of a global pile.
export const setProject = (path: string | null): Promise<void> =>
  invoke("set_project", { path });

export const projectIsRepo = (dir: string): Promise<boolean> =>
  invoke("project_is_repo", { dir });

export const initProjectRepo = (dir: string): Promise<void> =>
  invoke("init_project_repo", { dir });

// ---- Conductor ----
// One question a working agent put to the conductor, and the answer (empty
// while still open). Mirrors `Exchange` in src-tauri/src/mcp.rs.
export type Exchange = {
  question: string;
  answer: string;
  asked_ms: number;
};

export type ConductorTask = {
  id: string;
  from: string;
  target: string;
  task: string;
  // pending | overdue | in_review | rework | blocked | done | error |
  // cancelled | abandoned — mirrors the `status` doc comment on `Task` in
  // src-tauri/src/mcp.rs.
  // "overdue" is still running: past the reporting threshold but not
  // cancelled, and its result is still accepted. "in_review" and "rework" are
  // open too: the work exists but has not been signed off. "blocked" means
  // the agent asked the conductor a question and is waiting on the answer —
  // set only by an explicit ask_conductor/answer_question exchange, never
  // inferred from silence (see src/lib/tasks.ts). "abandoned" is terminal:
  // the pane holding the work is gone.
  status: string;
  result: string;
  // Stamped once at dispatch and never moved. This is not when the task
  // finished — see done_ms.
  ts_ms: number;
  // When the task reached a terminal state, or null while it is still live
  // (including in_review and rework, neither of which is finished).
  done_ms: number | null;
  // Session that must sign off before this counts as done. Empty means
  // review was waived.
  reviewer: string;
  // What the reviewer said, whether approved or rejected.
  findings: string;
  // Questions this task's agent asked the conductor, in order. The last entry
  // with an empty `answer` is the open question, if any.
  exchanges: Exchange[];
};
export type ConductorState = {
  conductor: string | null;
  halted: boolean;
  tasks: ConductorTask[];
};

export const conductorState = (): Promise<ConductorState> => invoke("conductor_state");
export const setConductor = (name: string | null): Promise<void> =>
  invoke("set_conductor", { name });
export const haltConductor = (halted: boolean): Promise<void> =>
  invoke("halt_conductor", { halted });

// ---- Dispatch (human-initiated) ----
export const dispatchTask = (target: string, task: string): Promise<{ task_id: string }> =>
  invoke("human_dispatch", { target, task });
