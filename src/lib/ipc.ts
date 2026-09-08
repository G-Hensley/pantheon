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
  // The launcher refuses to start this CLI without a model. Set where the CLI
  // would otherwise fall back to a model the backend's free-model guard cannot
  // see (opencode: its config, then its last-used model).
  modelRequired?: boolean;
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
  { id: "opencode", label: "opencode", program: "opencode", args: [], color: "#9ece6a", modelFlag: "-m", modelRequired: true },
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

// The models a CLI's list command offers. For opencode this is already
// filtered to ones the free-model guard will accept; for codex it is the
// catalog's "list"-visibility slugs (codex has no `models` subcommand, but
// `codex debug models` prints the same catalog as JSON). Every other program
// resolves to an empty list, so the launcher falls back to Custom for those.
export const listModels = (program: string): Promise<string[]> =>
  invoke("list_models", { program });

// ---- Conductor ----
// One question a working agent put to the conductor, and the answer (empty
// while still open). Mirrors `Exchange` in src-tauri/src/mcp.rs.
export type Exchange = {
  question: string;
  answer: string;
  asked_ms: number;
};

export type TaskUsage = {
  input_tokens: number | null;
  output_tokens: number | null;
  cache_creation_input_tokens: number | null;
  cache_read_input_tokens: number | null;
  cost_usd: number | null;
  turns: number | null;
  duration_ms: number | null;
  duration_api_ms: number | null;
};

export type ConductorTask = {
  mode?: "pane" | "headless";
  exit_code?: number | null;
  cli_session?: string;
  usage?: TaskUsage | null;
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
// The app-wide dispatch allowance: one counter for the whole app, not per
// session, since app start. See Shared.dispatches in src-tauri/src/mcp.rs.
// remaining is carried rather than computed client-side (limit - used) so
// the frontend never has its own arithmetic to get wrong about exhaustion.
export type DispatchBudget = {
  used: number;
  limit: number;
  remaining: number;
};

export type ConductorState = {
  conductor: string | null;
  halted: boolean;
  tasks: ConductorTask[];
  dispatch_budget: DispatchBudget;
};

export const conductorState = (): Promise<ConductorState> => invoke("conductor_state");
export const setConductor = (name: string | null): Promise<void> =>
  invoke("set_conductor", { name });
export const haltConductor = (halted: boolean): Promise<void> =>
  invoke("halt_conductor", { halted });

// Human-only: renews the dispatch allowance without cancelling tasks or
// changing halted state. No agent-facing MCP equivalent exists on purpose;
// this is a plain Tauri command, unreachable from the MCP loopback port an
// agent CLI talks to.
export const resetDispatchBudget = (): Promise<DispatchBudget> =>
  invoke("reset_dispatch_budget");

// ---- Dispatch (human-initiated) ----
export const dispatchTask = (target: string, task: string): Promise<{ task_id: string }> =>
  invoke("human_dispatch", { target, task });

// ---- Session requests ----
// An agent asks the backend for a new session (kind/model/reason/isolation);
// a human approves or denies it here. Mirrors `SessionRequest` and
// `SessionRequestState` in src-tauri/src/mcp.rs — frozen contract, do not
// diverge without a matching backend change from whoever owns src-tauri.
export type SessionRequestState = "pending" | "launching" | "started" | "failed" | "denied" | "stale";

export type SessionRequest = {
  request_id: string;
  kind: "claude" | "codex" | "opencode";
  model: string;
  // False for a model the free-model guard could not verify against the
  // CLI's own catalog — the launcher's model field still shows it, but
  // flagged, rather than silently trusting an unrecognized string.
  model_verified: boolean;
  reason: string;
  isolate: boolean;
  project: string | null;
  brain: string;
  requester: string;
  state: SessionRequestState;
  // Only set once a session has actually been created for this request
  // (state has reached at least "launching"); the id an approved pane must
  // reuse rather than mint its own.
  session_id: string | null;
  detail: string | null;
  created_ms: number;
  updated_ms: number;
};

// Whether a request-created pane has reached its endpoint, from the
// backend's own observation of that pane, not from anything the frontend
// infers off the channel. Mirrors `StartupState` in src-tauri/src/mcp.rs.
export type StartupState = "starting" | "connected" | "ready_timeout";

// What the backend knows about one request-created pane's startup, so the UI
// can render Starting/Connected/ready_timeout without guessing at it from
// output timing. Mirrors `StartupRecord` in src-tauri/src/mcp.rs.
export type StartupRecord = {
  pane: string;
  state: StartupState;
  admitted: boolean;
  launched_ms: number;
  connected_ms: number | null;
};

export type SessionRequestList = {
  requests: SessionRequest[];
  // Startup state for panes these requests created — keyed by pane id
  // (`SessionRequest.session_id`), not request id. See StartupRecord.
  startups: StartupRecord[];
  outstanding: number;
  admitted: number;
  outstanding_limit: number;
  admitted_limit: number;
  // False until initialize_sessions has completed its restore-id barrier;
  // approve/deny/reset are refused before that (see App.tsx's allocatorReady
  // gate), so a stale roster id can never collide with a freshly-approved one.
  allocator_ready: boolean;
};

export const listSessionRequests = (): Promise<SessionRequestList> => invoke("list_session_requests");

// editedModel is null to accept the request's own model as-is, or a string to
// override it (always allowed, since the requester's own free-model guard
// already ran server-side; this is a human's last word, not a second guard).
// channel must already have `onmessage` assigned before this call, exactly as
// spawnSession requires — see TerminalPane's attach-to-request mode.
export const approveSessionRequest = (
  requestId: string,
  editedModel: string | null,
  channel: Channel<Bytes>,
  rows: number,
  cols: number,
): Promise<SessionRequest> =>
  invoke("approve_session_request", { requestId, editedModel, channel, rows, cols });

export const denySessionRequest = (requestId: string): Promise<SessionRequest> =>
  invoke("deny_session_request", { requestId });

// Clears every request the human has not acted on. Backend-authoritative:
// the returned list is what to render next, not just an acknowledgement.
export const resetSessionRequests = (): Promise<SessionRequestList> => invoke("reset_session_requests");

// Tells the backend which pane ids survived from the last run, before any
// approval can be admitted. Call once at startup with every restored roster
// id; nothing in this module or in the approve/deny flow is safe to call
// before it resolves (see allocatorReady in App.tsx).
export const initializeSessions = (restoredIds: string[]): Promise<SessionRequestList> =>
  invoke("initialize_sessions", { restoredIds });

// Mints one fresh, backend-reserved session id for a human-launched pane
// (the launcher button), replacing the old local sess-N counter: reserving
// from the same allocator a request-approved pane draws from is what keeps
// the two id sources from ever colliding.
export const reserveSessionId = (): Promise<string> => invoke("reserve_session_id");
