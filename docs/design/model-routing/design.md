# Model routing

Owned by #56. That issue is authoritative for scope, acceptance and status; this document is
the reasoning it links to rather than restates.

`list_sessions` reports what a pane is, never what it is good at. This is the argument for a capability profile, and the measured session where the conductor could not tell a working OpenCode pane from a local one that produced nothing.

Moved here from `BACKLOG.md` on 2026-09-16 under `orion:SPEC-0001` FR-009, which reduces that file
to one line per item. The text below is unchanged from the section titled "Model-aware dispatch", so an issue
citing that section by name is citing this. It is a live design argument, not an archived one: the
work it describes has not been done.

---

## Model-aware dispatch

Today the conductor knows a session's id and CLI (`sess-3 (opencode)`) and
nothing about what that session is *good at*. So routing is guesswork, and the
guesses have been wrong in practice: broad web research kept going to OpenCode
sessions on a free-tier model, which is close to the worst available match for
it.

**The model is now acquired, but not routed on.** `SESSION_TYPES` in
`src/lib/ipc.ts:35-41` gives each CLI a `modelFlag` (`--model` for claude, `-m`
for codex and opencode), `src-tauri/src/lib.rs:995-998` declares the optional
`model` override and `src-tauri/src/lib.rs:1122-1127` prepends the flag, and
`note_session` (`src-tauri/src/mcp.rs:788`) stores it so the roster prints it
as `- {id} ({kind}, {model})` (`src-tauri/src/mcp.rs:915-916`). What is still
open is the capability profile and the routing decision.

**Confirmed the expensive way, 2026-08-13.** Three tasks were dispatched to
sess-4 (codex), sess-6 (opencode) and sess-8 (opencode). sess-8 was a local
model via Ollama and produced nothing at all; sess-6, listed identically as
`(opencode)`, worked fine. Nothing in the roster distinguished them, and the
conductor only caught it by reading `git status` in each pane's worktree on disk
and noticing sess-8's was clean. The user knew which pane was local; the
conductor could not. This is the same failure the liveness entry above describes,
approached from the routing side rather than the detection side: the cheapest
pane to hand work to is the one most likely to silently drop it.

`Projects/knowledge/ai-kbase/MODEL-GUIDE.md` already contains the missing
knowledge, maintained and dated, including a task-to-model routing table, per
CLI strengths, cost and quota strategy, and the OpenRouter free pool.

**The shape to aim for:** the conductor learns each pane's model and a short
capability profile, so `list_sessions` answers "who should do this" rather than
only "who is here".

Open questions before building:

- **Do not vendor a copy.** A snapshot of `MODEL-GUIDE.md` inside Pantheon is
  stale the day it is written, and the guide already carries `last_verified`
  and a 14-day cadence. Read it from a configured path, or import it through
  the agent-toolkit catalog, rather than duplicating it.
- The guide's routing table is human-shaped prose. Deciding what a
  machine-readable profile needs (strengths, context window, cost tier, tool
  support, latency expectation) is most of the work.
- What happens when the guide is absent, since not every machine will have it.
  Degrade to today's behaviour rather than failing.
