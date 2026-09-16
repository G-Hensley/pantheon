# Pantheon: remaining work, reconciled

## Audit 2026-09-16

Routed here from `G-Hensley/projects` under `orion:SPEC-0001` FR-010. Source commit
`58ee9415d0ab6e25ba184d8833a7075e5de104bb`, written 2026-09-09 against `main` at `965dbe4`.

The plan is an inventory, and it predates most of this repository's issue tracker: the cutover
recorded in [`../github-tracking-migration.md`](../github-tracking-migration.md) filed 23 issues on
2026-09-14, five days after the plan was written. So the plan is read through this audit rather than
on its own.

**Every item gets exactly one of three dispositions.** Delivered: it is done, with a commit, a pull
request or a closed issue naming it. Tracked: an open issue in this repository owns it. Open:
nothing owns it. "Open" is not the same as "nobody's": an item can be open here and belong to
another repository, and the owner column says which. A compound item is split so that each half
gets its own disposition rather than a hedge.

| | Items |
| --- | --- |
| Delivered | 5 |
| Tracked | 18 |
| Open | 12 |
| **Total** | **35** |

The plan's own text is unchanged below this section. Where the audit and the plan disagree, the
audit is later and wins.

### Section 1: status corrections (4 items)

All four merged, as the plan says. Three were carried forward as acceptance issues rather than
closed outright, which is what the plan asked for; the carry-forward is a separate item, not a
reason to call the original anything but delivered.

| Item | Disposition | Owner |
| --- | --- | --- |
| `s9xc2s` dispatch allowance | Delivered, PR #46 | Verification in an isolated fixture is #71 |
| `qf7d1f` two unseen delivery notices | Delivered, PR #47 | Nothing carried forward |
| `rd85p4` project-scoped roster | Delivered, PR #48 | Verification across an app restart is #72 |
| `jz8nsh` conductor session requests | Delivered, PR #49 | The acceptance issue the plan asked for is #73 |

### Section 2: open work from the task records (8 items)

| Item | Disposition | Owner |
| --- | --- | --- |
| `3xcwdb` raise the dispatch brief limit | **Delivered.** Issue #51 closed; `4f7d894` raised the Linux Codex limit to 8192 bytes with the fidelity evidence the plan asked for, now at [`../dispatch-composer-evidence.md`](../dispatch-composer-evidence.md) | Closed |
| `rg3wmk` capability profiles for routing | Tracked | #56 |
| `y4fz0h` headless dispatch for OpenCode | Tracked | #58 |
| `e67nyf` enforce free-tier OpenRouter at the account | Tracked | #53 |
| `kyzzsx` authenticate the fallback MCP endpoint | Tracked | #54. Still guarded: live approval before any code change |
| `q0h7c4` tell a conductor when panes sit idle | Tracked | #55 |
| `t9x41e` report the timeout gap upstream | Tracked | #57 |
| `zmk94k` measure whether disabling tools shrinks the prompt | Tracked | #59 |

### Section 3: model and context-window control (4 items)

The plan's suggested issue split was taken exactly, one issue per bullet.

| Item | Disposition | Owner |
| --- | --- | --- |
| Per-pane health state from the shape of silence | Tracked | #60 |
| Conductor-initiated context window clear, with the open-task interlock | Tracked | #61 |
| Conductor-initiated model change for an unresponsive pane | Tracked | #62 |
| Context window occupancy in `list_sessions` | Tracked | #63 |

The design these four scope is [`../design/context-window/design.md`](../design/context-window/design.md),
which each issue links. The plan's constraint 3, that a 429 must never trigger a switch within the
tier that just refused, is the one a reviewer should check survives into whatever is built.

### Section 4: workspace cleanup (7 items)

The plan's own suggested split, one row per bullet.

| Item | Disposition | Owner |
| --- | --- | --- |
| Pantheon: a merge path from the UI | Tracked | #68 |
| Pantheon: surface the preserved path when a dirty worktree blocks a close | Tracked | #68 |
| Pantheon: reap or re-adopt its own worktrees after a crash | Tracked | #68 |
| Lexicon: a worktree inventory and classifier | **Open** | Lexicon. Nothing filed there, and it is not this repository's to file |
| Lexicon: a reclaim skill for the clean-and-merged class | **Open** | Lexicon |
| Lexicon: a report-only session-start hook | **Open** | Lexicon, and blocked on the delivery question the plan raises: `lexicon sync` writes agents, skills and MCP config, never `settings.json` |
| Tree: the one-time cleanup of 20 worktrees and 21 merged branches | **Open** | `G-Hensley/projects` |

The plan's measurements are from 2026-09-09 and were not re-taken for this audit. Its own caveat
stands: that the three dirty `/tmp` worktrees are superseded by PR #49 is inferred from filenames
and has never been checked against a diff.

### Section 5: gaps with no record anywhere (2 items)

| Item | Disposition | Owner |
| --- | --- | --- |
| The five README limitations, triaged as one piece of work | Tracked | #69 |
| A pane alive but not answering | Tracked | #60, which is where the plan itself puts it |

### Section 6: ideas, not commitments (9 items)

Item 5 is compound and is split, because its two halves have different owners. Nothing here is
filed as an issue by this audit: an idea becomes an issue when somebody decides to do it, and an
audit is not that decision. `../ideas/README.md` points at the open ones and states the trigger.

| Item | Disposition | Owner |
| --- | --- | --- |
| 1. An attention rail | **Open** | Nobody |
| 2. A next-approval hotkey | **Open** | Nobody |
| 3. Listen for the bell rather than for text | **Open** | Nobody |
| 4. Desktop notification when unfocused | **Open** | Nobody |
| 5a. Extend headless dispatch, which has no TUI approval surface | Tracked | #58 |
| 5b. Per-project permission allowlists and an approval policy | **Open** | Nobody |
| 6. One pane status line instead of three features | Tracked | #60 and #63 together, which is the same consolidation argued from the other direction |
| 7. An opt-in tool to read the last N lines of a pane | **Open** | Nobody. Needs a decision record before an issue: it reverses a deliberate architectural choice |
| 8. Focus layout | **Open** | Nobody |

### Section 7: not Pantheon's problem (1 item)

| Item | Disposition | Owner |
| --- | --- | --- |
| A check that a work item naming a merged pull request does not stay open | **Open** | Lexicon. Recorded by the plan so the finding is not lost, and still unfiled |

### What this audit did not do

It did not re-run the plan's measurements: the worktree and branch counts, the dirty `/tmp`
directories, and the timing figures are all as of 2026-09-09 and are read as dated evidence. It did
not verify that each tracked issue's scope actually covers the plan item it is matched to beyond
reading both; where an issue is narrower than the plan bullet, the issue wins, because the issue is
what somebody will work from.

## The plan as routed, unchanged

Two sentences below are now false by the act of routing. The plan says it "sits in the tree docs
rather than `applications/pantheon/docs/`", and its method note says `gh issue list` returned zero
open issues. Both were true when written and neither is edited, because a dated record that is
quietly corrected stops being evidence of what was believed at the time.

Status: proposed. This is an inventory, not a build plan. It exists to be converted into
GitHub issues in `G-Hensley/pantheon`, replacing the `.tasks/` JSON records the project is
moving away from. Nothing here is scheduled, and no item is claimed.

Written 2026-09-09 against `main` at `965dbe4`. Every claim below was read from the
repository, the task records, the merged pull requests, or a command run on this machine.
Where something was inferred rather than observed it says so.

It sits in the tree docs rather than `applications/pantheon/docs/` for the same reason
`2026-09-06-managed-project-work-map/pantheon.md` does: it is an input to a tree-wide
migration, and one of its findings is owned by Lexicon rather than by Pantheon.

## 1. Status corrections to make first

Four task records say `review`. All four merged. The project's own `AGENTS.md` says a status
that no longer matches reality is worse than no status, so these should be closed as part of
the migration rather than carried into GitHub as open issues.

| Task | Title | Landed |
| --- | --- | --- |
| `s9xc2s` | Show and reset the dispatch allowance without cancelling tasks | PR #46, 2026-09-07 |
| `qf7d1f` | Stop two unseen delivery notices for one pane blocking each other | PR #47, 2026-09-07 |
| `rd85p4` | Key the pane roster and conductor to the project they belong to | PR #48, 2026-09-08 |
| `jz8nsh` | Let the conductor request sessions with human approval | PR #49, 2026-09-08 |

`jz8nsh` is the one exception worth carrying forward. The code merged; the feature has never
been run in the live app. Its own notes say a request-created pane reports `Starting` until
its endpoint connects, and that whether an agent is genuinely ready to work is only knowable
from an acknowledged first task, which needs a human at the keyboard. Carry it as a small
acceptance issue, not as a build issue.

## 2. Open work, from the task records

**P1, the only one.**

- **Raise the dispatch brief limit above one 1 KiB chunk** (`3xcwdb`). Briefs are truncated at
  1024 bytes on the pane-typing path. The current behavior is a refusal at that boundary,
  which bounds the damage and is not the fix. Needs a real-agent test proving 8 KiB arrives
  byte for byte at both the Codex and OpenCode composers, then raising the ceiling to the
  largest proven safe bound. A 2026-09-06 correction is worth keeping with the issue: this
  does **not** block headless dispatch, which never touches `submit_to`.

**P2.**

- **Capability profiles for routing** (`rg3wmk`). `list_sessions` reports a pane's id, CLI,
  brain, role and busy state, and now its model, but nothing about what it is good at. Read
  the profile from `knowledge/ai-kbase/MODEL-GUIDE.md` at a configured path rather than
  vendoring a snapshot that is stale the day it is written. Degrade to today's behavior when
  the guide is absent. This is the prerequisite for the model-health work in section 3.
- **Headless dispatch for OpenCode** (`y4fz0h`). Claude runs headless already. OpenCode is
  still confined to the TUI, which is where the upstream 30-second timeout lives, and that
  timeout is documented upstream as unfixed.
- **Enforce free-tier OpenRouter at the account, not the config** (`e67nyf`). Launch-time
  guards ship, but config is declared intent. The enforcement is zero balance, auto top-up
  off, no payment method, no BYOK keys, with the four checks written down.
- **Authenticate the fallback MCP endpoint, or refuse to use it** (`kyzzsx`). Security
  relevant and guarded, so it needs live approval before any code change. When a session
  cannot bind its own per-session port it falls back to a shared endpoint with no auth
  middleware, where identity is self-declared through `set_session_identity`. A caller there
  can impersonate the conductor and dispatch, or impersonate a target and fake a
  `complete_task` or `review_task`.

**P3.**

- **Tell a conductor when panes sit idle** (`q0h7c4`). `idle_pane_nudge` and `nudge_is_due`
  are already written and tested on the unmerged `feat/wait-and-nudge` (fb1f041); only the
  `wait_for_tasks` half of that branch was taken.
- **Report the TUI versus headless timeout gap upstream** (`t9x41e`). Measured here at 33.2s
  and 32.7s completing headless against 30.4s and 30.5s cancelled in a pane. No upstream
  issue documents that asymmetry, and it localises the bug to the interactive path.
- **Measure whether disabling tools shrinks OpenCode's prompt** (`zmk94k`). Narrow,
  self-contained, cheap to hand to an idle pane.

## 3. Conductor control over a pane's model and context window

Requested 2026-09-09. Partial prior art exists in `BACKLOG.md` under "The conductor cannot
see or reclaim a pane's context window" and "A pane whose model is failing looks exactly
like a healthy idle one". Both are design-only, neither has a task record, and neither
covers the automatic recovery this section asks for. They should become issues together,
because they are one mechanism wearing two names.

**The goal.** The conductor can clear a pane's context window, and can change a pane's
model. When a pane's model stops answering, whether that is a provider error, a rate limit,
or a local model that never produces a first byte, the conductor can move it to a working
model rather than waiting on a pane that will never reply. This matters most for the
`opencode` CLI, across all three of its model families: local models, OpenRouter models, and
opencode's own hosted models.

**What is already true, and shortens the work.**

- Pantheon knows the model now. `SESSION_TYPES` in `src/lib/ipc.ts` carries a `modelFlag`
  per CLI (`--model` for claude, `-m` for codex and opencode), `lib.rs` prepends it at
  launch, `note_session` stores it, and the roster prints it. The old blocker, that Pantheon
  launched every CLI bare and had no handle on the model, is gone.
- Clearing travels a path that already works. Dispatch types into the target's TUI through
  `SessionManager::submit_to`, and a clear command travels the same way. The command differs
  per CLI, so it belongs beside the `SESSION_TYPES` records rather than hard-coded at the
  call site.
- The interlock has a reliable source. A pane holding an open task must never be cleared or
  relaunched, and the task table that `busy_label` reads is exactly the fact Pantheon does
  hold reliably. Refuse rather than queue.

**What is genuinely hard, in order.**

1. **Changing a model destroys the pane's conversation, and so does clearing it.** Relaunching
   as `opencode -m provider/model` is one of only two paths that work; typing `/models` opens
   an interactive picker that the type-then-Enter path cannot drive. The other path is routing
   the work through `opencode run -m provider/model`, which is the headless entry in `y4fz0h`.
   Both are irreversible in the same way. The two features should share one consent rule
   rather than growing two.
2. **Detection has to come from outside the failing agent.** The obvious design, an agent that
   reports its own provider error through a tool, fails precisely when it is needed: the
   inference loop that would make the call is the thing that broke. Pantheon's only current
   evidence is the `last_output` timestamp. That is more useful than it looks. A pane that
   emits a short burst seconds after a dispatch and then goes quiet has almost certainly
   errored; a slow local model is quiet for a long time and then talks. Shape of silence, not
   duration of silence, is the discriminator, and Pantheon already records enough to tell them
   apart without reading a byte of content.
3. **A 429 must not trigger a switch within the same tier.** OpenRouter free quota is
   account-wide, not per session: 20 requests per minute, and 50 per day below $10 of lifetime
   credit. When it runs out it runs out for every OpenCode pane at once. Rotating a
   rate-limited pane onto another free model walks it into the same wall, and doing that
   across a fan-out burns every pane's retry budget in parallel. The correct response to a
   rate limit is to stop dispatching and tell the human, or to fall back to a local model,
   never to rotate within the tier that just refused. This is the single most important
   design constraint in this section.
4. **The three model families fail differently and need different ladders.** A local model
   that never produces a first byte is a hang, and the recovery is a different provider
   entirely. An OpenRouter free model that has left the pool is already absorbed by the
   `openrouter/free` router, which selects per request from the live pool, so that case is
   largely solved and should not be rebuilt. An account-wide quota exhaustion is the case
   above. Opencode's own hosted models are a third billing and availability surface and need
   their own answer before they are added to any fallback list.
5. **Any auto-selected target must pass the existing launch guard.** `opencode_model_guard`
   refuses an `openrouter/*` id that is not `openrouter/free`, `openrouter/openrouter/free`,
   or a `:free` id, and refuses an opencode pane with no explicit model at all. A recovery
   ladder that picks a target the guard then rejects is a second failure on top of the first.
6. **Verification after the act.** Once a clear is typed, how does the conductor learn it took
   effect? `submit_to` returns whether the write reached the PTY, which is not the same as the
   CLI having acted on it. Without an observation channel the only evidence is the pane's own
   next answer, which is circular. This is the same missing channel that blocks context
   occupancy measurement, and it is worth solving once for both.
7. **Bounding.** A conductor that can clear panes can clear them in a loop, and a freshly
   cleared agent gives worse answers than the one it replaced. A task failing for its own
   reasons will happily walk a switch loop through the entire free pool. Cap attempts per
   task rather than per pane, and make exhaustion a visible refusal rather than a quiet
   stall. `MAX_DISPATCHES` exists for exactly this reason.
8. **Recovery must be loud even when it succeeds.** A silent automatic switch hides the one
   signal the human needs when a provider is degrading.

**A vocabulary trap to fix before anything is built.** "Context" already means the shared
brain store here: `get_shared_context`, `search_context`, the `context-changed` event. That
store is small, durable and deliberately shared. A context *window* is large, per agent,
invisible and disposable. A tool named `clear_context` would read as "wipe the brain" to
anyone who learned the existing vocabulary first. Say "context window" or "session memory"
in full, every time.

**Suggested issue split.**

- Per-pane health state that distinguishes working slowly from the provider refused, built
  on the shape of silence in `last_output`.
- Conductor-initiated context window clear, with the open-task interlock and one shared
  consent rule.
- Conductor-initiated model change for an unresponsive pane, with the per-family ladder and
  the no-rotation-within-a-refusing-tier rule.
- Context window occupancy in `list_sessions`, so the conductor can rank panes before it
  decides to clear one. Ship it labelled as an estimate or not at all: a number that is
  confidently wrong is worse than no number.

## 4. Workspace cleanup

Requested 2026-09-09. Not currently in `BACKLOG.md` in any form. It is two separate problems
that look like one.

**Problem A: the pileup that exists today, which Pantheon did not create.**

Measured on this machine, 2026-09-09:

- 21 registered worktrees for the pantheon repository, 20 besides the main checkout.
- 21 local branches, all fully merged into `main`.
- Zero branches matching `pantheon/<id>-<uid>`, which is the naming Pantheon's isolation
  uses. So every one of these was created by hand during agent sessions, not by the app.
- The 2026-09-06 work map counted 12 worktrees and 14 merged branches. Roughly one new
  worktree per day since.

Three carry uncommitted changes, and all three live under `/tmp`, which does not survive a
reboot:

| Worktree | Changed files | Branch |
| --- | --- | --- |
| `/tmp/resumed-pantheon-frontend-final-fix` | 20 | `work/frontend-final-fix` |
| `/tmp/approved-pantheon-frontend` | 18 | `feat/session-request-frontend` |
| `/tmp/approved-pantheon-session-requests` | 4 | `work/session-requests` |

Their contents look like the session-requests frontend work that merged as PR #49, so the
dirt is probably superseded. That is an inference from filenames, not a diff against `main`,
and it should be verified before anything is deleted. Until then it is uncommitted work
sitting in a directory the operating system will clear.

This is not Pantheon's to fix, and the numbers are the argument: Pantheon is one producer of
worktrees among several, and currently not the largest one. A reaper built inside Pantheon
would solve the smaller half of the problem in the place fewest sessions can reach it. See
"Ownership" below.

**Problem B: Pantheon never reaps what it creates.**

The per-session half is built and is careful. `worktree.rs` has `remove`, which refuses a
dirty worktree rather than deleting it, keeps a branch that carries unique commits, and is
idempotent when the worktree is already gone. `reattach` never writes off an existing
directory merely because it cannot be reattached, because it may hold uncommitted agent work.
Those are the right rules.

What is missing is everything above one session:

- `remove` only runs on an explicit session close. An app crash, a kill, or a restart leaves
  the worktree behind, and restore only reattaches the ones the roster saved.
- Nothing enumerates worktrees across app runs, so there is no surface that could show a
  pileup, let alone act on one.
- Dirty worktree recovery is manual and invisible. Closing a session preserves the directory
  and the UI never says where it is. This is already listed as a known gap in the README.
- **There is no merge path at all.** A session's branch survives when it has commits, but the
  UI never shows the branch name or a diff. This is the root cause rather than a separate
  gap: work can never reach the merged state that would make it safe to reap, so nothing is
  ever reapable, so the directory count only goes up.

**Ownership: mostly Lexicon, with a narrow piece that must stay in Pantheon.**

The general reaper belongs in Lexicon, because worktrees accumulate from agent sessions
across every host and repository, not from one application. Three things cannot move there,
because they depend on facts only Pantheon holds:

- Pantheon is the only thing that knows which worktree belongs to which live pane. An
  external reaper cannot tell a stale directory from one a pane is working in right now.
- Orphaning its own worktrees on crash or restart is Pantheon's defect to fix.
- The merge path is a UI feature that cannot live in Lexicon at all, and it is the
  precondition for everything else: without it, work never reaches the merged state a
  reaper is allowed to collect.

**Hook and skill, doing different jobs.** Deleting a worktree is irreversible for
uncommitted work, and a hook that deletes on session end deletes precisely the work an agent
forgot to commit. The global rule to preserve uncommitted work settles the split:
automation reports, judgment reclaims.

- **Hook, report only, never deletes.** Deterministic, no model judgment. Prints at session
  start when the count crosses a threshold and stays silent otherwise, the same shape as the
  existing `check-tree.py` hook.
- **Skill, reclaims with approval.** Enumerates and classifies into clean-and-merged,
  clean-with-unmerged-commits, dirty, and dirty-in-a-volatile-directory such as `/tmp`.
  Removes only the first class. Never touches a dirty worktree without explicit approval,
  and surfaces its path instead. This mirrors what Pantheon's own `worktree.rs` already
  does, which refuses to remove a dirty worktree rather than deciding for the operator.

**A constraint that affects sequencing.** `lexicon sync` writes agents, skills and MCP
config, and never `settings.json`; `doctor.py` only reads settings files. So Lexicon can
ship the skill today but cannot install the hook. Either the hook is wired by hand in
`~/.claude/settings.json`, as the `check-tree.py` one already is, or Lexicon grows hook
installation as a separate capability first. Decide which before planning this as "a hook
in Lexicon".

**A substitute for the ownership knowledge.** Before proposing a removal, a generic reaper
can check whether any process holds a working directory inside the worktree, through
`/proc/*/cwd` on Linux. That is the concrete stand-in for the pane ownership only Pantheon
has, and it is what keeps the Lexicon skill safe to run while Pantheon is open.

**Suggested issue split.**

Lexicon:

- A worktree inventory and classifier, usable across enrolled repositories.
- A reclaim skill that removes only the clean-and-merged class, reports what it skipped and
  why, and refuses to act on a worktree a live process occupies.
- A report-only session-start hook, once the delivery question above is settled.

Pantheon:

- A merge path from the UI: show the branch, show the diff, offer to open a pull request.
  Sequence this first. Without it there is nothing a reaper is permitted to collect.
- Surface the preserved path when a dirty worktree blocks a session close, closing the
  existing README gap.
- Reap or re-adopt its own worktrees after a crash or restart, rather than orphaning them
  when `remove` never runs.

Tree:

- A one-time cleanup of the 20 worktrees and 21 merged branches that exist today, starting
  with the three dirty ones under `/tmp`.

## 5. Gaps with no record anywhere

These are stated in the README's own known-gaps section and have never had a task. They are
honest limitations rather than defects, but they should exist as issues so they are decisions
rather than surprises.

- Headless dispatch is Claude only, has no live transcript stream, and needs a cancel plus a
  fresh dispatch to retarget. Review rejection sends the existing pane notice rather than
  using CLI `--resume`.
- A restored conductor gets no composer briefing. `set_conductor` only briefs a pane already
  connected to MCP, and restore runs before any pane has spawned, so the pane discovers its
  own role only when it happens to call `list_sessions`. The fix belongs in the backend at
  connect time.
- The markdown mirror under `.pantheon/context/` is written for humans and never read back.
- Fit layout is intended for at most 6 panes.
- Frontend test coverage is thin. `src-tauri/tests/pty_truncation.rs` is a Windows-only
  measurement harness, ignored by default, not a regression test.
- A pane that is alive but has quietly stopped answering is still indistinguishable from one
  thinking hard. This one is deliberate: silence is not evidence and guessing would discard
  real work. Section 3 narrows it rather than closing it.

## 6. Ideas, not commitments

The first five address one operator complaint recorded 2026-09-09: approving work in every
pane means scrolling through every terminal to find the one that is waiting.

**The enabling fact.** The Rust side deliberately never reads pane content. It stamps
`last_output` on arrival and forwards the bytes untouched. But the frontend receives all of
them and writes them into an xterm.js terminal per pane. So detection of "this pane is
waiting on you" can be built entirely in the renderer, without reversing the backend's
no-content-inspection design. That is by far the cheapest version of this feature and it
should be tried before anything is added to the backend.

1. **An attention rail.** One strip listing only the panes waiting on a keypress, with the
   first line of each prompt. Clicking one focuses that pane. Keep the detection patterns in a
   table beside the `SESSION_TYPES` records, because every CLI phrases its approval prompt
   differently and the wording changes between versions. Treat a match as a hint that badges a
   pane, never as grounds to answer for the human.
2. **A next-approval hotkey.** Once the panes are queued, one key moves to the next one
   waiting and focuses its composer, so the queue is walked rather than hunted for. This is
   the part that actually removes the scrolling; the rail alone still requires aiming.
3. **Listen for the bell rather than for text.** Nothing in the code handles `\x07` today.
   Several CLIs emit BEL or an OSC notification when they finish or need input, which is far
   more robust than matching prompt wording. Worth checking per CLI before investing in
   pattern matching.
4. **Desktop notification when the window is not focused.** Tauri has a notification plugin.
   A badge nobody is looking at helps nobody.
5. **Remove approvals instead of routing them.** The cheapest fix is never seeing the prompt:
   per-project permission allowlists for Claude Code, an approval policy for Codex, and
   extending headless dispatch to more CLIs, since headless has no TUI approval surface at
   all. This raises the value of `y4fz0h` above what its P2 suggests. Pre-approve the routine
   cases and route only what genuinely needs a human.
6. **One pane status line instead of three features.** Context occupancy, model health and
   waiting-for-approval are three backlog entries asking the same question: what is this pane
   actually doing right now. One status line answering busy, idle, waiting on you, degraded or
   dead, with an estimated context fill, is one build rather than three.
7. **An opt-in tool to read the last N lines of a pane.** It unblocks context estimation,
   health detection and approval visibility at once. It also reverses a deliberate
   architectural choice, so it deserves an explicit consent story rather than the backend
   quietly starting to read everything.
8. **Focus layout.** One large pane plus a rail of small ones carrying attention badges,
   instead of six equal tiles to scan. Pairs with idea 1 and sidesteps the 6-pane fit ceiling.

## 7. Not Pantheon's problem

**Task status drift belongs to Lexicon.** Four records in this repository claimed `review`
while their pull requests were merged, and the same failure will recur in every repository
that tracks work this way. The check is generic: a work item that names a branch or pull
request should not stay open once that pull request is merged. It belongs in a Lexicon skill
or workflow, applied across enrolled repositories, and not as another item in Pantheon's own
backlog. Recorded here only so the finding is not lost in the migration.

## Method

`main` at `965dbe4`. Read: `BACKLOG.md` in full, all 19 `.tasks/` records, `README.md`,
`AGENTS.md`, `src/components/TerminalPane.tsx`, the PTY reader loop and worktree API in
`src-tauri/`, and `docs/plans/2026-09-06-managed-project-work-map/pantheon.md`. Ran:
`gh pr list`, `gh issue list` (zero open), `git worktree list`, and a per-worktree
`git status --porcelain`. The claim that the three dirty `/tmp` worktrees are superseded by
PR #49 is inferred from filenames and was not verified against a diff.
