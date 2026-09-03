# Design: Headless Dispatch
Source: `BACKLOG.md` - "Dispatch headlessly (`opencode run`) instead of typing into the TUI" and "Dispatch loses whole 1 KiB chunks".

## Overview
This design enables "headless" dispatching of tasks where the agent is invoked via a CLI command (`opencode run`, `claude -p`, or `codex exec`) instead of through the Pantheon TUI. This removes the 830-byte brief ceiling and allows for larger context injection while maintaining the session's shared brain and state.

## 1. The Hybrid Shape
Interactive panes remain the primary product. A dispatched headless task runs as a child process bound to the same session ID and brain as the parent session.

**Output Mechanism:**
The output of a headless task will be displayed in a **drawer** within the pane that initiated the dispatch. 
*Rationale:* A drawer preserves the primary workspace of the interactive pane while providing a dedicated area for the child process's logs/output. It avoids the complexity of spawning a new interactive pane for a task that is intentionally non-interactive.

## 2. Task Lifecycle & Result Mapping
Headless tasks bypass the manual `complete_task` step by mapping process signals to the task state.

- **Success:** When the child process exits with code `0`, the task is automatically marked `done`. The final message of the JSON/stream-JSON output is captured as the `result`.
- **Error:** A non-zero exit code maps to `error`. The `stderr` or the final JSON error message is captured as the `result`.
- **Result Storage:** The JSON result is stored in the `Task` object's `result` field.
- **Usage Tracking:** Token usage counts (provided by the host CLI) are captured and stored in a new `usage` field on the task for Phase 5 analytics.

## 3. Host Priority
**Recommendation:** `opencode run` should be the primary host for headless dispatch.
*Rationale:* `opencode` is the native host for this project. It provides the most consistent integration with our existing MCP wiring and offers a clean `-s <session_id>` flag for session binding.

**Command Line:**
`opencode run -m provider/model -s <session_id> --format json --prompt "..."`

## 4. Brief Size & Dispatch Logic
The 830-byte brief limit does not apply to headless tasks. 

**Dispatch Tool Logic:**
The `dispatch` tool will inspect the invocation source. If the source is a CLI call (detected via session headers or a specific `headless: true` flag in the request), it will bypass the character count check. 
**Conductor View:** The conductor will see these tasks in the standard `open` bucket, but with a `headless: true` property in the metadata to distinguish them from TUI-initiated tasks.

## 5. Concurrency
If a headless task is dispatched to a pane where the interactive agent is currently busy (mid-task), the headless task is queued in the **Phase 2 queue**.
*Rationale:* This prevents the child process from competing for the same LLM provider quota or "brain" attention simultaneously with an active TUI task, which could lead to interleaved context or race conditions in the shared brain.

## 6. Risks & Mitigations
- **Credentials/MCP Token Exposure:** The child process inherits the session's environment. We must ensure the child process is restricted to the same MCP permissions as the parent.
- **Quota Burn:** Headless tasks can run long-running loops. We will implement a `max_turns` limit for headless tasks (default 50).
- **Runaway Child:** A process that never exits. The TUI will provide a "Kill Task" button in the drawer, which sends a `SIGTERM` to the child process.

## Implementation Notes
- Match style of `docs/design/conductor-task-surface/increment-1.md`.
- No em dashes.
- Reviewer: `sess-2`.
