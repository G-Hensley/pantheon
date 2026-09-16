# Backlog

Ideas are not commitments. Promote an item only once its trigger, ownership,
security boundary, and validation method are understood.

This file holds capabilities Pantheon does not have yet. Findings from
point-in-time reviews of existing code go to `IMPROVEMENT-AUDIT.md`, which is
deliberately untracked (see `.gitignore`), so a fresh clone will not have it.

Triaged 2026-09-16 under `orion:SPEC-0001` FR-009. Twenty sections went in:
fifteen described work that had shipped and are now in
[`docs/archive/2026-09-16-backlog-delivered.md`](docs/archive/2026-09-16-backlog-delivered.md),
five remain below, and none was untracked. Nothing needed filing as a new issue,
because the tracker cutover recorded in `docs/github-tracking-migration.md` had
already given every open item an owner.

Each section below names the issue that owns it. The issue owns scope,
acceptance and status; the section holds the reasoning and the measurements the
issue links to rather than restates. A section leaves this file when its issue
closes, and the file is deleted when the last one goes.

## Where the archived sections went

An open issue that cites a section by name will find it at this table. The text
is unchanged; only its location moved.

| Section | Now at |
| --- | --- |
| Prior art: nobody has solved the OpenCode local-model timeout | [archive](docs/archive/2026-09-16-backlog-delivered.md#prior-art-nobody-has-solved-the-opencode-local-model-timeout) |
| Dispatch headlessly (`opencode run`) instead of typing into the TUI | [archive](docs/archive/2026-09-16-backlog-delivered.md#dispatch-headlessly-opencode-run-instead-of-typing-into-the-tui) |
| Local models: pick one that survives a cold start | [archive](docs/archive/2026-09-16-backlog-delivered.md#local-models-pick-one-that-survives-a-cold-start) |
| Liveness: tell a slow agent apart from a dead one | [archive](docs/archive/2026-09-16-backlog-delivered.md#liveness-tell-a-slow-agent-apart-from-a-dead-one) |
| `get_task_result` with no id outgrows its own response limit | [archive](docs/archive/2026-09-16-backlog-delivered.md#get_task_result-with-no-id-outgrows-its-own-response-limit) |
| Dispatch loses whole 1 KiB chunks from the head of a long prompt | [archive](docs/archive/2026-09-16-backlog-delivered.md#dispatch-loses-whole-1-kib-chunks-from-the-head-of-a-long-prompt) |
| The limit, checked against the dispatches that had already gone silent | [archive](docs/archive/2026-09-16-backlog-delivered.md#the-limit-checked-against-the-dispatches-that-had-already-gone-silent) |
| A conductor cannot wait for a dispatch, only re-ask whether it landed | [archive](docs/archive/2026-09-16-backlog-delivered.md#a-conductor-cannot-wait-for-a-dispatch-only-re-ask-whether-it-landed) |
| A blocked agent can only ask the human, never the conductor | [archive](docs/archive/2026-09-16-backlog-delivered.md#a-blocked-agent-can-only-ask-the-human-never-the-conductor) |
| Nothing enforced cross-model review, so "done" meant self-certified | [archive](docs/archive/2026-09-16-backlog-delivered.md#nothing-enforced-cross-model-review-so-done-meant-self-certified) |
| Dispatch to a busy pane typed the new brief over the running one | [archive](docs/archive/2026-09-16-backlog-delivered.md#dispatch-to-a-busy-pane-typed-the-new-brief-over-the-running-one) |
| Two unseen notices for one pane deadlocked each other | [archive](docs/archive/2026-09-16-backlog-delivered.md#two-unseen-notices-for-one-pane-deadlocked-each-other) |
| The conductor's five-pill task strip hides the work it is meant to coordinate | [archive](docs/archive/2026-09-16-backlog-delivered.md#the-conductors-five-pill-task-strip-hides-the-work-it-is-meant-to-coordinate) |
| Orchestrator may open the sessions the work needs | [archive](docs/archive/2026-09-16-backlog-delivered.md#orchestrator-may-open-the-sessions-the-work-needs) |
| Dispatch allowance is invisible and reset is coupled to Stop | [archive](docs/archive/2026-09-16-backlog-delivered.md#dispatch-allowance-is-invisible-and-reset-is-coupled-to-stop) |

---

## Model-aware dispatch

**Owned by #56.** That issue is authoritative for scope, acceptance and status.

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

---

## Guardrail: OpenCode sessions must stay on free OpenRouter models

**Owned by #53.** That issue is authoritative for scope, acceptance and status.

OpenRouter is configured with a real account, so an OpenCode pane can select a
paid model and silently spend money. Pantheon now refuses a paid opencode model
at launch (`is_paid_openrouter_model`, `src-tauri/src/lib.rs:173-184`, enforced
at 1014-1024), but it still cannot see what a pane costs and the account-side
budget remains unenforced.

The naive implementation is wrong in a specific way worth writing down:

**Do not detect "free" by checking that prompt and completion pricing are
zero.** `MODEL-GUIDE.md` records that a model can price prompt and completion at
zero while still charging per request, per generated image, or per audio clip.
The documented signal is the `:free` suffix on the model id, plus the
`openrouter/free` meta-id which selects from the live free pool.

Also account for:

- **Free quota is account-wide**, not per session: 20 requests/minute, and 50
  requests/day until $10 of lifetime credit has been purchased, 1,000/day after.
  A fan-out across several OpenCode panes shares one budget and can exhaust the
  daily allowance quickly. An orchestrator that spawns sessions needs to know
  this before it spawns them.
- **`openrouter/free` does not promise a stable model identity.** It selects a
  compatible model per request, so two calls can land on different models. Pin
  an explicit `:free` id where reproducibility matters.
- **Free is explicitly not a reliability tier.** The guide notes free
  availability and latency vary. This is almost certainly why dispatches to
  OpenCode panes ran long often enough to expose the discarded-result bug fixed
  in `fix/late-task-completion`; the two issues share a root cause in tier
  choice.
- **It is still hosted inference.** Free does not mean private. Do not route
  work over private code or credentials to this tier without checking the
  selected provider's data policy.

Enforcement point is undecided and matters: Pantheon can only realistically
constrain what it launches, so this may belong in OpenCode's own config
(generated from `.agents/`, per the toolkit's sync) rather than in Pantheon. If
Pantheon enforces it, it needs a way to observe the model actually in use, which
it does not have today.

**Partly done, 2026-08-11.** `~/.config/opencode/opencode.json` now pins both
`model` and `small_model` to `openrouter/openrouter/free`, the Free Models
Router, with `max_price` zeros and `allow_fallbacks: false`.

The router is a better guard than pinning one `:free` model, for a reason worth
keeping: it cannot drift to a paid model, and it survives a model leaving the
free pool. That is not hypothetical, `inclusionai/ling-3.0-flash:free` had
already disappeared between the MODEL-GUIDE snapshot and the live catalogue.
Its tradeoff is no stable model identity between requests.

Care is needed with the sibling routers. `openrouter/free` prices prompt and
completion at 0, but `openrouter/auto`, `openrouter/fusion`, and
`openrouter/pareto-code` all report `-1`, meaning variable and billable. Pinning
the wrong router looks equally tidy and spends money.

**Launch-time enforcement, 2026-09-04.** Pantheon now refuses to start an
opencode pane without an explicit model, and refuses any `openrouter/*` id that
is not `openrouter/free`, `openrouter/openrouter/free`, or a `:free` id, compared
case-insensitively (`opencode_model_guard` in `src-tauri/src/lib.rs`, with the
launcher marking the field required). The missing-model refusal is the point:
without `-m`, opencode falls back to its config's `model` and then to whatever
model it used last, and Pantheon can see neither. The 2026-08-11 config pin is
no longer in force on the development machine (the global config carries no
top-level `model` as of this date), which is exactly the case the launch guard
covers.

Still outstanding, and still the only real guarantee: the account-side state
(zero balance, auto top-up off, payment method removed, no BYOK keys). Config
is declared intent; the balance is the enforcement.

---

## Restoring panes does not restore the agents that occupied them

**Owned by #72.** That issue is authoritative for scope, acceptance and status.

Session restore is partly built, but the honest boundary matters more than the
word "restore". `src/lib/panes.ts:1-13` states what survives: what each pane was,
not what it was doing. The roster stores the pane id, current session type,
brain, isolation flag, and saved worktree at `src/lib/panes.ts:19-31`.
`src/App.tsx:44-57` turns those records into fresh running panes on launch, and
`src/App.tsx:119-144` restores brain assignments and saves roster changes.

What comes back is topology and configuration. The PTY and child process died
with the previous app. Conversation history lives inside the agent, internal
reasoning is not exposed through MCP, terminal scrollback belonged to the old
frontend and PTY, and in-flight requests cannot cross a server restart. A
restored pane is therefore a fresh agent wearing an old pane id. UI and
documentation should say "restore pane layout" or "reopen panes", never imply
that Pantheon resumed an agent's context.

Worktree identity is the exception because it is durable state outside the
process. The frontend records the worktree reported by the backend so the next
launch can return to it (`src/App.tsx:146-158`), and
`src-tauri/src/worktree.rs:20-32` persists the fields needed to re-adopt it.
The safety rule at `src-tauri/src/worktree.rs:168-180` is the important part:
an existing directory is never written off merely because it cannot be
reattached. It may contain uncommitted agent work. Restore must refuse or surface
that condition, never silently create a replacement that strands the old work,
and never delete a dirty worktree as cleanup.

**The proposed project `.pantheon/layout.json` is not the path forward.** It was a
design for a system that did not yet restore panes. The implementation now owns
roster and layout state end to end in the frontend:
`src/lib/panes.ts:105-120` reads and writes `pantheon.panes`, while
`src/App.tsx:62-95` reads layout settings and the selected project from
`localStorage`. Adding a second layout file now would create two authorities for
the same pane order, brain assignment, isolation flag, and layout settings.
Conflict-resolution rules between them would be complexity caused by the new
store, not by the product.

The tradeoff that motivated project-scoped storage is still real.
`localStorage` is machine-local and `pantheon.panes` is one global key
(`src/lib/panes.ts:17`). A roster created for one repository can therefore be
read after the selected project changes, even though its brain assignments and
worktree references belong to the earlier repository. Project movement and
another machine are separate cases: a project-owned file travels with the
checkout but writes product UI state into the repository; a project-keyed
frontend store stays local but can distinguish repositories without adding
tracked or untracked files. The current implementation chose frontend
ownership, so the next step is to make that choice project-aware rather than
reviving `layout.json`.

Conductor identity restore is now implemented. `src/lib/panes.ts` persists the
conductor pane id beside the roster, under its own storage key, and only while
that id names a pane still in the roster; a promote, a demote, or the
conductor's own pane closing all pass through this on every change. On launch,
`src/App.tsx` restores the role only to that exact pane once the roster has
been built: `restoreConductor` calls `setConductor` for the saved id when it
is part of the roster, or clears the saved id and adds a notice to the
existing restore-problems banner when it is not, without ever promoting a
different pane instead. A pane that is part of the roster but fails to spawn
is caught separately, once that failure is known: `noteSpawnFailure` retracts
the role the same way, clear rather than reassigned. The Tauri command at
`src-tauri/src/lib.rs` still only forwards the value, and `src-tauri/src/mcp.rs`
still keeps it in memory; nothing on the backend persists, so this restore is
frontend-owned end to end, matching the roster it rides beside. One gap this
does not close: `Shared::set_conductor` in mcp.rs only briefs a pane already
connected to MCP, and restore runs before any pane has spawned, so the
restored pane gets no composer briefing and discovers the role only when it
calls `list_sessions` on connect. That fix belongs in the backend, at connect
time, not here.

**The shape still to aim for.** Key roster and conductor state by a stable
project identity instead of one global `localStorage` bucket. Continue
treating each pane as a new process, preserve saved worktree references, and
make partial restore failures visible without discarding the panes that
remain valid. Layout settings may stay machine-local; the question is which
settings are genuinely project-specific, not whether every setting can be put
into one file.

Open questions worth settling before building:

- **Project identity.** A normalized absolute path is simple but changes when a
  repository moves. Repository metadata can survive a move but needs a stable,
  non-secret identifier and a fallback for non-Git projects.
- **Storage scope.** Project-keyed `localStorage` matches the implementation and
  avoids repository files. A project-owned store is portable across machines
  but changes the repository boundary and needs an ignore and migration policy.
- **Restore timing.** Automatic reopening is fast and is today's behavior. An
  explicit prompt gives the user a way to start fresh when a saved roster is
  large or stale.
- **What layout belongs to the project.** Pane membership and worktrees clearly
  do. Window height and column preference may be user and machine preferences
  rather than repository state.
- **Dirty worktree recovery.** Reuse is already safer than replacement. If
  reattachment fails, the UI still needs to lead the user to the preserved path
  and explain why the pane was not reopened.

**Project scoping shipped, task `rd85p4`.** The roster and the conductor id are
now keyed by the project the window opened with, so a roster built for one
repository is no longer read back after the selected project changes. Four of
the five open questions above are settled by that, and the fifth is answered
more narrowly than it was asked.

*Project identity* is the normalized absolute path already stored under
`pantheon.project`, encoded with `encodeURIComponent` rather than hashed so a
misbehaving restore can still be diagnosed by reading the key. A repository that
moves therefore starts a fresh roster. That is the known cost of the simple
option, and it was taken deliberately: the alternative pulls git metadata reads
into the frontend to key a machine-local convenience store. *Storage scope* stays
project-keyed `localStorage`, which is the frontend ownership this section
already argued for, with "no project" given its own bucket rather than sharing
the unscoped key. *Restore timing* is unchanged, still automatic. *What layout
belongs to the project* is answered by leaving `layout`, `layout.columns` and
`layout.paneHeight` unscoped: they are machine preferences, and only the roster
and conductor id name panes, brains and worktree paths that mean anything solely
inside one repository. *Dirty worktree recovery* is untouched and still open.

**Two things the scoping had to get right, both of which took a design change.**

*A window that changes project mid-session stops persisting.* The roster is read
once at mount but written on every pane change, so the scope a write uses cannot
simply follow the selection. Keying writes by the currently selected project
saves panes launched under the second project into the first project's bucket,
which is the contamination the scoping exists to prevent. Keying them by the
startup project instead records those same panes as belonging to a repository
they were never opened in. Neither produces a roster worth restoring, so once
the selected project changes the window stops writing altogether, says so in the
existing restore-problems banner, and leaves both saved rosters and every live
pane exactly as they are. A restart reloads against the new project and
persistence resumes there. The freeze is one-way for the life of the window:
returning to the startup project does not make the roster trustworthy again,
because the panes launched meanwhile are still in the list. Reopening panes on a
live project switch is a larger question about what happens to running agents,
and it is not answered here.

*With one exception, for the case the freeze would otherwise break.* A window
that opened with no project at all is every first launch, and there the ordinary
path is to pick a project and then launch panes. Freezing on that pick would mean
a first session's panes were never saved, which is a worse product than the one
this entry set out to fix. So an empty window may bind to the first project it is
given and go on persisting there. Every condition has to hold: the window opened
with no project, it has no panes and no conductor on screen, it has not already
frozen, and the destination is provably unused. "Provably" is the load-bearing
word. The check is three-valued, and storage that refused a read reports neither
true nor false, so an unknown destination is treated exactly like an occupied
one. It is also stricter than "no saved roster": a destination still holding a
pre-scoping value it could later have adopted counts as occupied, because binding
and persisting would strand that value permanently, nothing reading the unscoped
key again once a scoped copy exists. A value already claimed by another project
does not count against it, since this project could never have adopted it. This
is the whole exception. A window with panes on screen still freezes, so does one
that has already switched once, and so does A to B to A.

*Adopting the pre-scoping roster is ownership-bound, not first-come.* The single
global `pantheon.panes` has to become some project's roster exactly once. The
claim is written before the copy, so a write that fails partway leaves a record
of which scope was mid-adoption: that scope retries on its next read, and every
other scope reads past the leftover value instead of inheriting it. A claim that
cannot be recorded refuses the adoption outright rather than taking the value
unguarded, because an unguarded adoption is precisely the cross-project leak
being fixed; nothing is deleted on that path, so the value is still there to
adopt once storage accepts writes again.

The claim outlives the value it guarded, which was not the first design. Clearing
it on success looked tidy and was wrong: an unscoped key can reappear, most
plausibly from an older build writing the key it still knows about, and a claim
that had been tidied away would let whichever project read next inherit another
repository's panes. A regression test covers exactly that sequence. One short
permanent string is the price of the adoption being once-only rather than
once-at-a-time.

*One claim covers the roster and the conductor together*, which was also not the
first design and matters more than it looks. A claim per key seems obviously
right until the two halves separate: adopting the roster into A and then failing
to copy the conductor leaves the conductor unclaimed, and the next project to
read it takes it. The result is not a dangling id that gets cleared harmlessly.
Pane ids are `sess-N` counters that restart from the same numbers in every
project, so a conductor id adopted into a project whose roster came from
somewhere else lands on that project's own same-numbered pane and silently
promotes the wrong agent. The roster and the conductor are halves of one window
state and are adopted as one, or not at all.

---

## The conductor cannot see or reclaim a pane's context window

**Owned by #61 and #63.** That issue is authoritative for scope, acceptance and status.

A long fan-out slowly poisons itself. Every dispatched brief, every tool result,
and every file an agent read stays in that agent's context window, and nothing
in Pantheon ever gives it back. Panes degrade in the order they were first used,
and the conductor, which is the one component positioned to notice, is the one
component with no way to see it.

**First, a vocabulary collision worth fixing before anything is built.**
"Context" already means something else here. `get_shared_context` at
`src-tauri/src/mcp.rs:1815`, `search_context`, and the `context-changed` event
all refer to the shared brain store: decisions and facts agents publish to each
other. That store is small, durable, and deliberately shared. A context *window*
is large, per agent, invisible, and disposable. Naming a new tool `clear_context`
would read as "wipe the brain" to anyone who learned the existing vocabulary
first. Whatever gets built should say "context window" or "session memory" in
full, every time.

**Pantheon does not know how full any window is, and cannot currently find out.**
`list_sessions` (`src-tauri/src/mcp.rs:1857`) reports the pane id, its CLI, its
brain, its role, and whether it is busy, overdue, or dead. Nothing about
occupancy. The deeper obstacle is that the backend never reads what a pane says:
the reader thread at `src-tauri/src/lib.rs:1104-1120` stamps `last_output` on
arrival and forwards the bytes straight down a channel to the frontend
renderer. Pantheon knows *when* a pane spoke and never *what* it said. Every CLI
in `SESSION_TYPES` prints a context percentage in its own status line, and
Pantheon's own architecture is what stops it from reading one.

Clearing is the easier half. Dispatch already works by typing into the target's
TUI through `SessionManager::submit_to` (`src-tauri/src/lib.rs:346`), so a
`/clear` travels the same path a brief does. Two cautions come with that:
`submit_to` returns whether the write reached the PTY, which is not the same as
the CLI having acted on it, and the clear command differs per CLI, so it belongs
next to the `SessionType` records in `src/lib/ipc.ts:34-41` rather than
hard-coded at the call site.

**Clearing is destructive and Pantheon cannot undo it.** This is the same loss
described in "Restoring panes does not restore the agents that occupied them",
except deliberate: a cleared pane is a fresh agent wearing an old pane id. So
the interlock matters more than the feature. A pane holding an open task must
never be cleared, and that is the one fact Pantheon does hold reliably, in the
task table that `busy_label` (`src-tauri/src/mcp.rs:1044`) already reads. The
honest pairing is clear-plus-brain: anything worth surviving the wipe should be
written to shared context first, which is what that store is for.

**The shape to aim for:** `list_sessions` carries a context-occupancy signal
alongside busy and dead, the conductor can clear an idle pane it owns, and the
clear is refused rather than queued when the pane has open work.

Open questions before building:

- **Where the number comes from.** Three candidates, none clean. Scraping the
  CLI status line means teaching the backend to read pane output and parse a
  different format per CLI, and re-parse it whenever any of them changes.
  A self-report tool is honest and cheap, but arrives only as often as an agent
  chooses to call it, and an agent in trouble stops calling anything. An estimate
  Pantheon keeps by counting bytes it typed in and bytes it saw come back needs
  no parsing and is systematically low, because it cannot see the files and tool
  results the agent read on its own. The estimate is probably the right first
  cut: it is always available, never blocks, and only has to be good enough to
  rank panes against each other.
- **Whether an estimate is worth showing.** A number that is confidently wrong
  is worse than no number. If it ships as an estimate it must be labelled as one
  in the roster line, not rendered as a measurement.
- **Autonomy.** Same escalation shape as "Orchestrator may open the sessions the
  work needs", and strictly worse if it goes wrong: spawning a pane wastes
  quota, clearing one destroys work in progress. Human approval by default.
- **Verification.** After the clear is typed, how does the conductor learn it
  took effect? Without an observation channel the only evidence is the pane's
  own next answer, which is circular.
- **Bounding.** A conductor that can clear panes can clear them in a loop, and a
  freshly cleared agent gives worse answers than the one it replaced. A ceiling
  belongs here for the same reason `MAX_DISPATCHES` exists.

---

## A pane whose model is failing looks exactly like a healthy idle one

**Owned by #60 and #62.** That issue is authoritative for scope, acceptance and status.

Liveness today detects one thing: process exit. A pane whose child has gone is
marked `[DEAD, process exited]` at `src-tauri/src/mcp.rs:628`, and its open work
is settled as abandoned. Every other failure is invisible. A provider that
returns 429, a local model that never produces a first byte, an OpenRouter model
that has left the free pool: in all three the CLI is alive, the process is
healthy, the pane is idle, and the roster line is identical to a pane that is
genuinely free. The cheapest pane to hand work to stays the one most likely to
drop it.

This is already documented from two other directions. "Model-aware dispatch"
records the sess-4/sess-6/sess-8 run of 2026-08-13, where the local-Ollama pane
produced nothing and nothing in the roster distinguished it from the OpenCode
pane that worked. "Prior art: nobody has solved the OpenCode local-model
timeout" establishes that the 30s interactive ceiling is upstream and unfixed,
so silent local failures are the expected case here, not an anomaly.

**Scope this against what the free-model router already absorbs.** Per the
guardrail entry, `~/.config/opencode/opencode.json` pins
`openrouter/openrouter/free`, which selects per request from the live free pool.
That already survives one model leaving the pool, so "swap to another free
OpenRouter model" is largely solved for the single-model case and should not be
rebuilt. What the router does not absorb is the two failures actually worth
handling: the account-wide free quota (20 requests/minute, 50/day below $10
lifetime credit) and the local-provider hang.

**The quota case makes naive per-pane fallback actively harmful.** Free quota is
account-wide, not per session, so when it runs out it runs out for every OpenCode
pane at once. Rotating a rate-limited pane onto a different free model walks it
into the same wall, and doing that across a fan-out burns the retry budget of
every pane in parallel. The correct response to a 429 is to stop dispatching and
tell the human, or to fall back to a local model, never to rotate within the
tier that just refused.

**Detection has to be external to the failing agent, which is the hard part.**
The obvious design, an agent that reports its own provider error through a tool,
fails precisely when it is needed: the inference loop that would make that call
is the thing that broke. So the signal must come from Pantheon, and Pantheon's
only current evidence is the `last_output` timestamp written at
`src-tauri/src/lib.rs:1113`. That timestamp is more useful than it currently
looks. A pane that emits a short burst seconds after a dispatch and then goes
silent has almost certainly errored; a slow local model is quiet for a long time
and then talks. Shape of silence, not duration of silence, is what separates
them, and Pantheon records enough to tell the difference without reading a byte
of content.

**Switching the model is blocked on a fact Pantheon never acquires.**
`SESSION_TYPES` (`src/lib/ipc.ts:34-41`) launches every CLI bare, with no model
flag, so Pantheon does not know what a pane is running and has no handle on it.
Typing `/models` into OpenCode opens an interactive picker, which the
type-then-Enter path in `submit_to` cannot drive reliably. The paths that do work
are non-interactive: relaunch the pane as `opencode -m provider/model`, or send
the work through `opencode run -m provider/model` as the headless-dispatch entry
proposes. Both make this entry depend on Pantheon learning and passing the model,
which is "Model-aware dispatch". That entry is the prerequisite, not a parallel
nice-to-have.

**The shape to aim for:** a per-pane health state that distinguishes "working
slowly" from "the provider refused", and a bounded recovery ladder the conductor
can climb: re-dispatch to a different live pane first (cheapest, and already
possible today), then a model switch, then mark the pane unusable and say so in
the roster. Each step recorded on the task so the human can see what was tried.

Open questions before building:

- **Relaunching to change a model destroys the pane's conversation**, which is
  the same irreversible act as the clear described above. The two features share
  a mechanism and should share one consent rule rather than growing two.
- **Where the fallback list comes from.** `MODEL-GUIDE.md` again, read from a
  configured path, not vendored. The same staleness argument applies.
- **Telling an error apart from a terse success.** An agent that did the work
  and answered in one line also produces a short burst then silence. The task
  table knows whether `complete_task` was called, which is the disambiguator,
  but only after the fact.
- **Bounding the ladder.** A task failing for its own reasons will happily walk
  a switch loop through the entire free pool. Cap attempts per task, not per
  pane, and make exhaustion a visible refusal rather than a quiet stall.
- **Who is told.** A silent automatic recovery hides exactly the signal the
  human needs when a provider is degrading. Recovery should be loud in the
  conductor feed even when it succeeds.
