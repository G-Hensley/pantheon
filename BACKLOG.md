# Backlog

Ideas are not commitments. Promote an item only once its trigger, ownership,
security boundary, and validation method are understood.

This file holds capabilities Pantheon does not have yet. Findings from
point-in-time reviews of existing code go to `IMPROVEMENT-AUDIT.md`, which is
deliberately untracked (see `.gitignore`), so a fresh clone will not have it.

---

## Prior art: nobody has solved the OpenCode local-model timeout

Checked before investing further. Short answer: it is a known, unfixed problem,
and the one team that documented it in depth gave up and moved to hosted models.

Note the repo moved: `sst/opencode` now redirects to `anomalyco/opencode`.

| Issue | Substance | Status |
|---|---|---|
| [#29420](https://github.com/anomalyco/opencode/issues/29420) | Names the cause: the timeout mechanism used `AbortSignal.timeout()`, "which does not work correctly in Bun's runtime", so provider requests had no effective timeout. Proposes a stream watchdog with a **30s first-byte** and 120s idle timeout | **Closed as not planned** |
| [#2974](https://github.com/anomalyco/opencode/issues/2974) | Config `timeout` "totally ignored" for local providers. The reporter used 900000, the same value I tried | Closed |
| [#3708](https://github.com/anomalyco/opencode/issues/3708) | Timeouts persist on larger models despite config | Open |
| [#20466](https://github.com/anomalyco/opencode/issues/20466) | "SSE read timed out" is thrown but the session retry never retries it | Open |
| [#22132](https://github.com/anomalyco/opencode/issues/22132) | Our exact hang: local Ollama hangs while `/v1/chat/completions` works directly | Open, no root cause, no workaround |
| [#18428](https://github.com/anomalyco/opencode/issues/18428) | Ollama takes 60-90s via OpenCode vs 3s direct; ~75s of OpenCode-side overhead suspected in streaming logic | **Closed as not planned** |

The 30s first-byte figure in #29420 matches the observed abort exactly, and the
broken `AbortSignal.timeout()` explains why provider `timeout` values have no
effect: the binary calls `AbortSignal.timeout(V.timeout)`, which is the
primitive that issue says does not work under Bun.

An [independent write-up](https://zenn.dev/masafumi_heijo/articles/opencode-ollama-timeout-tui-hang)
reached the same conclusion by a different route, including the distinction
that matters here: timeouts took effect under `opencode run` but not in the
interactive TUI. Their phrasing is worth keeping, since it describes most of
today: *"added and effective are two different problems."* They abandoned local
Ollama and standardised on Claude.

**The only workaround anyone confirms** is a third-party plugin,
[Mte90/opencode-auto-resume](https://github.com/Mte90/opencode-auto-resume),
which auto-resumes on timeout or error and exposes `chunkTimeoutMs`
(default 45000). Worth evaluating, but it resumes after a failure rather than
preventing one; pre-warming avoids the failure altogether and needs no plugin.

**What appears to be new signal:** no issue documents the TUI-versus-headless
asymmetry. Everything upstream reports the hang without noticing that
`opencode run` survives the same request. Measured here at 33.2s and 32.7s
headless against 30.4s and 30.5s cancelled in a pane. That is worth filing
upstream, since it localises the bug to the interactive path and is cheap for a
maintainer to reproduce.

## Dispatch headlessly (`opencode run`) instead of typing into the TUI

**Shipped for Claude, task y4fz0h, in `feat/headless-dispatch-tool`.** MCP `dispatch(headless: true)` now calls
the PR #40 process owner through the per-pane queue, waits for 30 seconds of
quiet, and maps process exit into the result/review lifecycle without requiring
`complete_task`. Multiline briefs bypass the pane byte limit. Mode, exit code,
budget, CLI UUID, and usage (including any tool the child was refused) are
persisted; cancellation reaches the process tree. The budget is
conductor-settable per dispatch (`budget_usd`, $5.00 default, $25.00 ceiling,
refused rather than clamped when out of range), after a rework brief found the
first cut's fixed $1.00 failed any real task against the measurement below.
Evidence: 335 Rust tests and 29 Vitest tests pass, including lifecycle, queue,
parser, process-tree cancellation, and TaskDrawer cases; `pnpm build` passes,
and Clippy reports only the existing `spawn_session` argument-count warning.
Claude 2.1.261 measurement observed success exit 0 and budget
failure exit 1 with `is_error`, `errors`, `subtype`, and `terminal_reason`.
OpenCode support, live transcript streaming, and headless resume remain open;
rework notices still go to the pane. The original OpenCode cold-start evidence
below motivates that remaining host work.

The 30 second ceiling on local-model requests is **not** an OpenCode-wide
limit. It belongs to the interactive session path only.

Measured both ways against a deliberately cold 20 GB model, which needs about
33s:

| Path | Ollama request | Outcome |
|---|---:|---|
| Pantheon pane (interactive TUI) | 30.4s, 30.5s | **cancelled both times** |
| `opencode run` (headless) | 33.2s, 32.7s | **completed both times** |

Two headless runs comfortably exceeded the limit that kills every pane request.
`BUN_CONFIG_HTTP_IDLE_TIMEOUT` made no difference and is not the cause; it was
tested and refuted rather than assumed.

Provider options cannot raise the pane ceiling either, and the compiled binary
shows why: the fetch wrapper collects abort signals and calls
`AbortSignal.any()`, which fires on the earliest. A signal is already attached
upstream (`t.signal`) before `timeout` / `headerTimeout` / `chunkTimeout` are
appended, so a longer value can never win.

So Pantheon has an architectural option worth weighing. It currently drives agents
by typing into an interactive CLI, which is what makes dispatch fragile in two
separate ways already documented here: the 1024-byte head truncation and this
30s ceiling. Running non-interactive work through `opencode run -m
provider/model "..."` would sidestep both, and would make large local models
usable, since laguna answers in 1.1s warm and only ever fails on a cold start it
is not allowed to finish.

The tradeoff is real and should not be waved away: the interactive pane is the
product. A user watching an agent work in a terminal is the point of Pantheon, and
a headless dispatch is invisible. A hybrid, where interactive panes stay as they
are and dispatched tasks run headlessly against the same session, is the shape
worth exploring, not a wholesale change.

## Local models: pick one that survives a cold start

Solved 2026-08-11, by measurement rather than tuning.

OpenCode enforces a hard **30 second** budget on a request to a local model.
It is not configurable: `timeout`, `headerTimeout` and `chunkTimeout` exist in
OpenCode's schema under `provider.<name>.options`, but setting them changed
nothing on the `/v1/chat/completions` path. Two requests either side of the
config change took 30.4172622s and 30.5398321s, so the option is simply not
honored there.

Measured against that budget, with OpenCode's ~13.2k-token prompt:

| Model | Size | Warm | Cold |
|---|---:|---:|---:|
| `lfm2.5:8b` | 5.6 GB | 9.6s | **13.9s** |
| `laguna-xs-2.1` | 20 GB | 11.2s | **32.7s** |

So a 20 GB model is 2.7 seconds too slow on a cold start, every time, and a
5.6 GB one has 16 seconds of headroom. Nothing here is flaky: a large local
model works until it idles out of memory, then fails deterministically on the
next request. That is exactly the "sometimes just stops working" symptom.

**Use small local models for OpenCode panes.** Large ones are viable only if
they never go cold, which is a guarantee nothing currently makes.

Supporting fixes already applied, both server-side because the client cannot be
configured:

- `OLLAMA_CONTEXT_LENGTH=32768`. The model's own context length is 262144;
  Ollama sized the KV cache from it, predicted 27.1 GiB, and evicted the model
  mid-session. Real usage was 1.2k-14k tokens. Note that `limit.context` in
  `opencode.json` does **not** control this: it caps what OpenCode will build
  into a prompt, not what Ollama allocates. Those are different things and
  conflating them wasted an afternoon.
- `OLLAMA_KEEP_ALIVE=30m`, up from the 5 minute default, so a thinking agent
  does not idle its model out and then pay a cold start it cannot afford.

Both are set in the user environment, but Windows only propagates that to
processes started after a fresh login, so Ollama must be launched from a shell
that already has them until you log out and back in.

Remaining lever if a bigger local model is ever wanted: shrink the 13.2k-token
prompt. Unverified whether disabling tools removes their definitions from the
request or only blocks execution (see sst/opencode#1320); it needs measuring
with a logging proxy, not assuming.

## Liveness: tell a slow agent apart from a dead one

**Shipped, task e18r66.** A task whose target pane's process has exited now
reaches a terminal `abandoned` status, distinct from `cancelled` and from
`overdue`, and `list_sessions` marks the pane `DEAD` so dispatch to it is
refused. Evidence: `abandon_lost` and `Shared::reconcile_abandoned` in
`src-tauri/src/mcp.rs` with their tests, `SessionManager::liveness` in
`src-tauri/src/lib.rs`, and `README.md`, which describes the `abandoned`
status and the `DEAD` marker under "Guardrails" and "Known gaps". A pane that is alive but silent is deliberately still
indistinguishable from one thinking; that case is "A pane whose model is failing
looks exactly like a healthy idle one", at the end of this file.

Found while testing the overdue fix, and partly caused by it.

Before, a task past the threshold flipped to "timeout" and its result was
refused. That was wrong, and it is fixed. But the fix traded one failure for
another: a task is now **never** terminal on its own. If the agent process dies,
its task sits at "overdue" forever and the conductor waits on a result that can
never arrive.

Observed directly: three OpenCode panes, only two `opencode` processes alive,
and the third pane's task stuck at "overdue" with `complete_task` never called.
Nothing in Pantheon noticed the process was gone.

That pane was running **Laguna XS 2.1 locally through Ollama**, not a hosted
model, and the operator reports local models stopping like this is recurring
rather than a one-off. So this is not an exotic edge case to design around
loosely: on this machine it is the expected failure mode of an entire class of
session, and the pane most likely to die silently is the one whose work is
cheapest to hand out.

Two consequences worth separating:

- **Detection** is the item below: a dead pane's task must reach a terminal
  state.
- **Routing** belongs with model-aware dispatch: local panes are the wrong
  target for long, unattended, or on-the-critical-path work, however cheap they
  are. Cost is not the only axis; delivery probability is one too.

"overdue" is honest about not knowing, which is better than a false "timeout".
But Pantheon does know something it is not using: it spawned the process and can
see whether it is still running.

- Mark a task `abandoned` when its target session's process is gone. That is a
  real terminal state, distinct from "cancelled" (deliberate) and from
  "overdue" (still working).
- Surface pane liveness in `list_sessions`, so a conductor does not dispatch
  into a dead pane in the first place.

Confirmed again 2026-08-12, in the shape that matters most. Two of five panes
took a task at the start of a long session and never returned anything at all.
Both were still listed by `list_sessions` as ordinary dispatch targets for the
entire session, with nothing distinguishing them from the three panes doing
real work. The conductor's only signal was the absence of a result, which is
indistinguishable from slowness, so it kept the tasks open and eventually
re-dispatched the same work to a pane that was answering. `list_sessions` is
the natural place to fix this precisely because it is the call a conductor makes
*before* choosing a target, and it is currently the one call that cannot be
wrong in a useful way: it reports presence, and presence is not readiness.
- Consider whether a dead pane's task should be re-dispatchable to another
  session, and whether that should be automatic or offered.

## `get_task_result` with no id outgrows its own response limit

**Shipped, task d4nhzh.** With no id, `get_task_result` now returns every open
task plus the `RECENT_FINISHED` most recently finished ones and says how many
older ones it left out; `status` filters by state and `include_all` returns the
whole history. Evidence: `select_tasks` in `src-tauri/src/mcp.rs` and its
`include_all_and_status_bypass_the_window` test.

It returns every task ever dispatched. At 28 tasks that is already ~67k
characters, which exceeds the tool response limit and fails outright, so the
documented way to collect a fan-out breaks exactly when a workspace has been
used for a while.

Second data point, 2026-08-12: **39 tasks, 111,770 characters**, still growing
roughly linearly at ~2.8k per task. The failure is worse than "it errors",
because the harness spills the payload to a file and instructs the caller to
read all of it back in chunks. So the documented collection path now costs more
context than the results are worth, and a conductor that follows the tool's own
advice burns its window on prompts it already sent. Every collection in that
session had to fall back to polling ids one at a time, which is exactly what the
tool description tells you not to do.

Wants a default window: open tasks plus recently finished ones, with older
history behind an explicit flag. Filtering by status would also let a conductor
ask the question it actually has, which is "what am I still waiting on".

## Dispatch loses whole 1 KiB chunks from the head of a long prompt

**Measured, not guessed.** Three Codex dispatches arrived beginning mid-word.
Locating the survival point in the original text and adding the wrapper that
`dispatch_prompt` prepends gives the same answer twice:

| Dispatch | target | payload | task chars lost | + prefix | total lost |
|---|---|---:|---:|---:|---:|
| OpenRouter guardrails | codex | - | 942 | 82 | **1024** |
| OpenCode timeouts | codex | - | 942 | 82 | **1024** |
| agent-toolkit review | **opencode** | 2645 | 1966 | 82 | **2048** |

Two different prompts of different lengths, both losing exactly 1024 bytes from
the head. That is a 1 KiB buffer, not a race and not contention, and it kills
the earlier hypothesis that a concurrent fan-out was to blame.

**Two corrections from a third measurement, 2026-08-12.**

*It is not a fixed 1024 bytes.* A 2645-byte payload lost exactly 2048, which is
two whole chunks. So the loss scales with size: every complete leading 1 KiB
chunk is dropped and only the trailing partial chunk arrives. Check the arithmetic
against all three rows: 2645 = 2x1024 + 597 survived; the codex rows lost one
chunk each and kept their remainders. "Loses exactly 1024" was true of the sample,
not of the mechanism, and a fix validated only against ~2 KiB prompts would look
correct while still corrupting longer ones.

*It is not codex-specific.* This row is an **opencode** pane. The entry below
points at `submit_to`'s `PASTE_START`/`PASTE_END` framing as the place to look
first, and that framing is codex-only, so it cannot be the whole cause.

**The write path is not the cause either. Measured and refuted, 2026-08-12.**

I concluded from the above that "whatever drops the chunks sits in the shared
write path". That was wrong, and a chunking-and-pacing fix in `write_to` would
have been a speculative change to code that is not broken.

`src-tauri/tests/pty_truncation.rs` opens a PTY exactly as `spawn_session` does,
writes offset-labelled payloads through the same `portable-pty` 0.9 master
writer, and reads back what arrives:

| requested | received | lost |
|---:|---:|---:|
| 512 | 512 | 0 |
| 1024 | 1024 | 0 |
| 2048 | 2048 | 0 |
| 4096 | 4096 | 0 |
| 8192 | 8192 | 0 |
| 65536 | 65536 | 0 |

Zero loss through 64 KiB, and a second case writes 64 KiB while the child is
deliberately not draining for a second: still zero. A single `write_all`
returns `Ok` and every byte arrives. Supporting reads: portable-pty's
`take_writer` hands back the ConPTY stdin pipe's descriptor with no buffering of
its own, and `filedescriptor` calls synchronous `WriteFile` and reports the real
byte count, which `write_all` loops on.

So Pantheon's one-call write, portable-pty's writer, the ConPTY input buffer, and
a child that is not yet reading are all eliminated as causes.

**What is left** is the target application's own terminal input handling. The
1 KiB replacement pattern fits a reader or editor that keeps only its most
recent input batch. Note the harness drained with `cmd /c more`, which reads
stdin as a stream; a TUI reading console input events is a different path
entirely, and that difference is the next thing to test. The decisive
reproduction spawns the real agent CLIs with deterministic startup and inspects
the editor buffer before submission, comparing one write against paced chunks.

**Fixed by bounding the payload, 2026-08-12.** `dispatch` now refuses any
injection of 1024 bytes or more (`MAX_INJECTION_BYTES` in `src-tauri/src/mcp.rs`)
instead of sending it and hoping. The mechanism drops *complete* leading chunks
only, so
an injection under one chunk has no complete leading chunk and cannot lose a
byte. That is a guarantee rather than a mitigation, and it does not depend on
ever finding the cause.

The refusal carries the move, not just the verdict: how many bytes over, and
that the fix is to split the task or point the agent at a file. It runs before
the dispatch budget is charged and before the task is recorded, so a refused
dispatch leaves no phantom `pending` id to poll.

The cost is real and is the reason to keep looking for the cause: the wrapper
takes 82 bytes of header and 111 of completion contract, leaving about **830
bytes of brief**. Raising that limit is what fixing this entry buys.

*An approach that did not work, recorded so it is not retried.* The first
attempt appended an integrity footer to the prompt: its own length and opening
quoted back, for the agent to check. Two flaws, both found by its own tests. The
surviving tail is `len % 1024` bytes, which can be a handful, so no footer of
any length is guaranteed to arrive; and asking a model to verify a character
count is asking it to do the one thing it is worst at. Delegating delivery
integrity to the receiver cannot work when delivery is what is broken.

The same session lost three dispatches to the same opencode pane this way. Each
time the agent noticed and said so, which is luck: it answered the questions it
received and never knew the earlier ones existed. Two of three reviews came back
half-answered for this reason, and the missing half contained a real bug when it
was finally asked in a shorter prompt.

Short dispatches are unaffected, which is the constraint that makes this
interesting: if the first 1024 bytes were always dropped, a 200-byte dispatch
would arrive empty, and those work fine. So the loss appears only once the
payload exceeds one chunk. A chunked writer whose first chunk is overwritten,
or lost to a redraw before the target's input is ready, fits the evidence.
`submit_to` frames Codex payloads with `PASTE_START`/`PASTE_END`, so the codex
path is the one to inspect first.

**A correction worth keeping.** An earlier entry here claimed a length-matched
test arrived intact and concluded this was not length-related. That test only
asked the agent to echo the *final* words and an end token, so it could not have
detected head truncation and almost certainly lost its own first 1024 bytes into
the filler. Testing only the end of a message cannot prove the beginning
arrived.

Silent corruption is worse than a failed dispatch: the agent does competent work
on the wrong brief, and in two of three cases said so only because it happened
to notice. Dispatch should verify what landed, or fail loudly.

Distinct from the submit race in `IMPROVEMENT-AUDIT.md` #1, which drops the
Enter rather than the text.

## The limit, checked against the dispatches that had already gone silent

Three review dispatches from 2026-08-13, measured after the fact against the
1024-byte rule:

| task | injection | would lose | outcome |
|---|---:|---:|---|
| sess-2, `mcp.rs` review | 967 B | 0 | **done**, 2161-char result |
| sess-3, auth review | 975 B | 0 | pending at 30m |
| sess-5, auth review | 2366 B | **2048** | pending at 341m |

The sess-5 dispatch lost two whole chunks and arrived as roughly 318 bytes of
tail. It was never going to come back, and the pane was not at fault: it was
handed a fragment. That dispatch is exactly what `MAX_INJECTION_BYTES` now
refuses, so the case the limit was built for had already happened and had
already been misread as a slow agent.

Worth stating plainly because the wrong lesson was available and tempting: for
five hours the visible evidence was "opencode panes do not finish reviews".
Two of the three panes were fine. The conductor was sending briefs that could
not arrive.

**What it does not explain.** The sess-3 dispatch fits in one chunk, arrived
whole, and still produced nothing after 30 minutes while its pane stayed live.
That is a genuinely slow or stuck agent, and it is the case `#22` and the
`wait_for_tasks` entry below are about. Two different failures wearing the same
symptom, `status: pending` forever, is the reason both need fixing: without a
size guarantee there is no way to tell them apart, and every silent pane looks
like a bad model.

**Consequence for routing.** Until a stuck pane can be told from a slow one,
the only honest signal is demonstrated completion. sess-2 has returned
substantive reviews twice; the auth review was re-dispatched there rather than
retried on a pane that had already gone quiet. That is a workaround, not
routing, and it is what `#24` should replace.

## A conductor cannot wait for a dispatch, only re-ask whether it landed

**Shipped, task dq3s0j.** `wait_for_tasks` blocks until the named ids reach a
terminal status or the timeout fires (45 s default, 55 s ceiling), returns
the same rendering `get_task_result` would, words a timeout distinctly from
completion, cancels nothing, and returns early when a task becomes `blocked`.
Evidence: `wait_for_tasks` in `src-tauri/src/mcp.rs` with its tests, and the
tool list under "How agents connect" in `README.md`. One caveat, measured
2026-09-03: from a Claude Code pane a wait of 110 s or more fails at the MCP
transport with "The operation timed out" while 45 s returns, so the shipped
default of 45 s and cap of 55 s are deliberate (`WAIT_DEFAULT_SECS` and
`WAIT_MAX_SECS` in mcp.rs), not the 600 s default discussed below.

`get_task_result` is a poll. There is no call that blocks until a task
finishes, and no notification when one does. So a conductor that dispatches
work and has nothing else queued has exactly two options: guess an interval and
poll, or say "I'll report when it lands" and then never actually look again.

The second is what kept happening, and it is worse than it sounds, because the
sentence reads like a commitment. The conductor is not lying; it has no
mechanism behind the promise. Observed directly on 2026-08-13: a review was
dispatched to sess-2, reported as "still pending", and the turn ended. The
review had in fact completed. The only reason it was ever read is that the user
asked why nothing was waiting on it.

The workaround that does work, and what it shows: the task store is append-only
JSONL at `<project>/.pantheon/context/brain.jsonl`, so a shell loop can poll the
last record for a task id until its status leaves `pending`, and the harness
notifies on exit. That works, and needing to reach around the MCP server into
its own storage to find out whether a task finished is the argument for putting
it in the server.

**The shape to aim for.** A `wait_for_tasks` call that blocks until given task
ids reach a terminal status or a timeout expires, returning the same payload
`get_task_result` would. Blocking is the point: it collapses "dispatch, guess,
poll, guess again" into one call, and it makes "I'll report when it lands" a
thing the conductor can actually do.

Worth settling before building: a maximum wait, since a hung pane must not hold
a conductor forever, and it should interact with the overdue threshold rather
than duplicate it; whether it returns on the first completion or all of them,
with first being more useful for a fan-out where any result unblocks the next
step; and whether the timeout return is distinguishable from completion, which
it must be, or the conductor cannot tell "finished" from "gave up waiting".

Related: the roster's busy and OVERDUE markers already tell a conductor a pane
is slow. This is the other half, letting it act on that without spinning.

## A blocked agent can only ask the human, never the conductor

**Shipped, task 6v3ebz.** `ask_conductor` lets a dispatched agent put a question
against its `task_id` and wait; the task holds a distinct open `blocked` status
until `answer_question` (conductor only) delivers the answer and sets it running
again. Exchanges are kept on the task, capped at `MAX_QUESTIONS_PER_TASK`; a
900 s ask timeout tells the agent to use its own judgement; no conductor, or a
halted workspace, is a stated fallback to the human rather than a silent one;
and `wait_for_tasks` returns early on a blocked task, the shared mechanism the
entry above asked for. Evidence: `ask_conductor` and `answer_question` in
`src-tauri/src/mcp.rs` with their tests, and the tool list under "How agents
connect" in `README.md`.

Dispatch is one-way. `dispatch` hands a brief to a pane and returns; the only
path back is `complete_task` at the end. So an agent that hits a genuine
ambiguity mid-task has three options, and all of them are bad: guess and risk
doing competent work on the wrong decision, stall until someone notices, or ask
the human in its own terminal.

Asking the human is the least bad of the three, and it is what happens. It is
also the one that scales worst. Observed 2026-08-13: sess-4, working a dispatched
task, put its question to the user rather than to the conductor that briefed it.
The user's own words were that it "probably should've routed to you". With five
panes working in parallel, every one of them holds this option, so the human
becomes the synchronisation point for questions they did not ask and lack the
context to answer, which is the exact cost conducting was supposed to remove.

The conductor is usually the *better* answerer, not merely the more appropriate
one. It wrote the brief, it holds the reasoning the brief compressed away, and
it can see the other tasks in flight. In this case the conductor had already
made and recorded the relevant decisions; the agent simply had no way to reach
them.

**The shape to aim for.** An `ask_conductor` call: a blocked agent poses a
question against its `task_id`, the question surfaces in the conductor's pane
the way a task result does, and the answer is delivered back to the waiting
agent. The task stays open and distinguishable throughout: `blocked` is a
different thing from `pending`, and a conductor collecting results needs to see
the difference.

Open questions worth settling before building:

- **What happens when no conductor is live, or it is halted.** Falling back to
  the human is right, but it must be a deliberate fallback rather than the
  silent default it is today.
- **Whether the agent blocks or continues.** Blocking is simpler and matches
  what an agent does now when it asks; continuing on the parts that do not
  depend on the answer is better use of the pane but much harder to get right.
- **A ceiling.** A pane that can interrupt the conductor can do so in a loop,
  and the conductor's context is the scarce resource in a long session. The
  `MAX_DISPATCHES` precedent applies.
- **Whether the answer is recorded on the task.** It is a decision made
  mid-task, and `record_decision` already exists for exactly this class of fact.
  A question answered and then lost is one the next agent asks again.

Interacts with the `wait_for_tasks` entry above: both are about a conductor and
a pane needing to communicate between dispatch and completion, and they should
share one mechanism rather than grow two.

## Nothing enforced cross-model review, so "done" meant self-certified

**Shipped, task d575h4, in PR #16 (`3efebac`); the cross-model rule and the
delivery gap this section named both shipped later in Phase 2 of
`docs/plans/2026-09-03-pantheon-repair.md`.** A task now carries a
`reviewer`, `complete_task` moves it to `in_review` rather than `done`, and only
`review_task` from that reviewer closes it or sends it back as `rework`.
`dispatch` picks a reviewer unless one is named or review is explicitly waived,
and `get_task_result` says which. `choose_reviewer` now prefers a live session
running a different CLI kind than the target before falling back to any other
live session, closing the cross-model gap this heading named; a reviewer of
the same kind is still possible only when no other kind is live. Both halves
of the review result are also delivered rather than left for the conductor to
relay: `complete_task` types a review request straight into the reviewer's
pane, and a rejected `review_task` types a rework notice into the target's,
each readable in full through `get_task_result`, which the task's target and
reviewer may now call by id and not only its dispatcher. Evidence:
`choose_reviewer`, `review_request_notice`, `rework_notice`, and
`task_for_reader` in `src-tauri/src/mcp.rs`, and their tests.

The policy already exists and is specific. `CONTRIBUTING.md` ("Review before
you commit") lays out six steps: implement, route to a **different-model**
reviewer, reviewer reports findings, implementer fixes and asks for a recheck,
reviewer approves, then commit. It names the areas where this is never
optional: `mcp.rs`, `worktree.rs`, session identity, dispatch, anything in
`SECURITY.md`. `.github/pull_request_template.md` carries the matching
checklist, including "Reviewer: model and session, different from the
implementer".

**Nothing in Pantheon implements any of it.** `complete_task` takes a result
string, writes `status = "done"`, and that is the end of the task's life. There
is no reviewer field, no review state between pending and done, and no way for
the conductor to ask "who checked this?" because the answer was never recorded.
The workspace is built out of different models sitting side by side, which is
exactly the thing that makes cross-model review cheap, and it is the one thing
the task model does not represent.

So the policy holds only as long as a conductor remembers it, which is the
failure mode of every convention that lives in a document and nowhere else.

**Evidence, from this session.** Commit `ba2fc87` changes `mcp.rs` and the
dispatch path, two of the areas `CONTRIBUTING.md` says *always* require a
cross-model review. It was implemented, tested, committed and pushed by one
model with no reviewer. Not because the rule was rejected, but because nothing
asked. The review was dispatched to sess-2 only after the user asked which item
owned this, which is the wrong order and the point of the entry.

**The shape to aim for.** `complete_task` moves a task to a `review` state
rather than `done`, naming a reviewer session that is not the implementer and
not the conductor. Only a `review_task` call from that session closes it, with
a verdict and findings recorded on the task. `get_task_result` then reports
"done, reviewed by sess-2" or "awaiting review", so a conductor cannot mistake
one for the other, and the roster shows review debt the way it now shows busy
panes.

Open questions worth settling before building: whether the conductor may waive
review for trivial work (a typo fix should not need a round trip) and how that
waiver is recorded; whether a rejected review reopens the original task or
creates a linked one; and how this relates to the board sketch in
`IMPROVEMENT-AUDIT.md`, whose `Review` column is the same idea with a UI
attached. The task-model change is the load-bearing half and does not need the
board to be useful.

Related: `IMPROVEMENT-AUDIT.md`'s task-board sketch (explicitly marked "the
design is still open") is the only other place this appears, and it is a
drawing rather than a plan.

## Dispatch to a busy pane typed the new brief over the running one

**Shipped, Phase 2 of `docs/plans/2026-09-03-pantheon-repair.md`.** Measured
2026-09-03: `dispatch_precheck` checked halted, self-dispatch, target
liveness, and injection size, never whether the target already had open work,
so a second brief landed in the pane mid-task and both instructions ended up
in whatever the running agent was reading. A dispatch to an occupied pane
(target of a pending, overdue, rework, or blocked task, or reviewer of an
in_review one) now creates the task with status `queued` instead of typing it,
naming which task it is queued behind and at what position; each pane holds
at most `QUEUE_CAP` (3) queued briefs, a fourth refused with the reason and
the queued ids, nothing journaled for the refusal. Whenever a pane stops being
occupied, whatever is next for it (an undelivered review request or rework
notice first, then the oldest queued brief, FIFO) is delivered through the
same path automatically, gated against halted and against typing into a pane
that is still occupied. `cancel_task` and `reassign_task` both work on a
queued task. Evidence: `queue_predecessor`, `queue_cap_refusal`,
`next_delivery_for`, `occupying_task`, and `Shared::drain_pane` in
`src-tauri/src/mcp.rs`, and their tests.

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

## Orchestrator may open the sessions the work needs

Currently the human opens panes with Ctrl+K and the conductor dispatches to
whatever exists. The conductor can see that a task wants a different model and
can do nothing about it.

Worth exploring: let the conductor request a new session of a named kind, so a
plan that needs a second opinion from a different vendor can arrange one.

This is a privilege escalation and needs treating as such:

- Spawning a session starts a real process, may create a git worktree, and
  consumes quota. `MAX_DISPATCHES` exists for a reason; an equivalent ceiling
  is needed here.
- Decide whether it is a request the human approves or an autonomous action.
  Approval by default is the safer starting point.
- A conductor that can create sessions can create them in a loop. Bound it
  structurally, the way dispatch depth is already bounded by only the conductor
  being able to dispatch.
- Interaction with session restore, below: restored panes and
  conductor-created panes should not fight over ids.

## Guardrail: OpenCode sessions must stay on free OpenRouter models

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

## The conductor's five-pill task strip hides the work it is meant to coordinate

**Shipped, task 7c7f3j, in PR #25 (`8d07262`).** The five-pill feed is replaced
by a task drawer that shows every open task without truncating its brief, gives
`blocked`, `pending`, `overdue`, `in_review`, `rework`, and terminal work their
own treatment, focuses the target pane from a task, and bounds finished history.
Evidence: `src/components/TaskDrawer.tsx` and `TaskDrawer.test.tsx`, whose tests
prove a sixth concurrent task stays visible and that a blocked task renders
first. The drawer renders `blocked` from `task.status` alone; that the status
is set only for a task holding an open question is a backend invariant, covered
by the lifecycle tests in `mcp.rs`, not by the component test. Design in
`docs/design/conductor-task-surface/increment-1.md`.

The conductor bar is an at-a-glance view only while the fan-out is small.
`src/components/ConductorBar.tsx:42-60` reverses the task list, keeps five
entries, truncates every brief after 38 characters, and leaves the full text
behind a hover title. At six concurrent tasks the sixth is not merely compressed:
it is absent from the feed. Similar briefs lose the parameters that distinguish
them at exactly the point where a conductor needs to tell several branches of
work apart.

The backend has already outgrown the three-state board originally sketched for
this. A task may be `pending`, `overdue`, `in_review`, `rework`, `done`,
`error`, or `cancelled`; `src-tauri/src/mcp.rs:1085-1090` deliberately counts
the first four as open because submitted work awaiting review is not finished.
Reducing that lifecycle to Blocked, In Progress, and Completed would hide review
debt and repeat the same mistake in a larger surface.

**One premise has been tested by the code and refuted: PTY quiet time cannot
identify a blocked agent.** The earlier proposal reused the submit-timing signal
as a blocked detector. But `src-tauri/src/lib.rs:173-212` records why silence is
ambiguous even for the narrower problem that signal actually solves. A target
may stay silent while it buffers input, and an output gap may mean "has not
started yet" rather than "has finished". Once a prompt is submitted, the same
silence can be a model thinking, a tool waiting for approval, a cold local model,
or a dead process. Naming any of those `blocked` from timing alone produces a
confident state Pantheon did not observe.

That does not make the scaling problem less real. It separates two pieces that
should not be coupled:

- The task surface can show every recorded state Pantheon actually knows today,
  grouped into open, awaiting review, and terminal work without inventing a
  diagnosis.
- A future blocked state needs an explicit signal from the agent or tool
  protocol. Process liveness can prove that a pane is dead, but neither process
  liveness nor terminal silence can prove that a live pane wants human input.

**The shape to aim for.** Replace the five-pill feed with a task board or drawer
that shows every open task, gives `in_review` and `rework` their own visible
treatment, keeps recent terminal work available without letting it crowd out
live work, and focuses the target pane from each item. Build it from the task
states already returned by `conductor_state` at
`src-tauri/src/lib.rs:1150-1163`. Add `blocked` only when an agent-facing
question or approval path can set and clear it explicitly. The board is a view
of the task ledger, not a second task model.

Open questions worth settling before building:

- **Board versus drawer.** A permanent multi-column board makes parallel state
  legible but takes space from the terminals, which are still the product. A
  drawer preserves the cockpit until the conductor needs detail.
- **How much terminal history stays visible.** Open work must never be dropped.
  Recent terminal tasks are useful confirmation, but unbounded history recreates
  the response-growth problem already fixed in `get_task_result`.
- **Whether cards move tasks.** Status should come from the task lifecycle, not
  drag-and-drop cosmetics. Reassignment and retry need server semantics before
  the UI offers them.
- **What earns `blocked`.** An explicit `ask_conductor` or approval event is
  honest. Quiet-time inference is not.
- **How results are inspected.** A card needs the full brief, result, reviewer,
  and findings without forcing all of that text into the overview.

## Restoring panes does not restore the agents that occupied them

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

Conductor identity is the remaining obvious hole. The frontend initializes it
to `null` at `src/App.tsx:97-100`. The Tauri command at
`src-tauri/src/lib.rs:1166-1171` only forwards the new value, and
`src-tauri/src/mcp.rs:551-576` keeps it in memory. Reopened panes retain their
ids and brains, but the user must promote the conductor again. Persisting the id
is truthful because it restores a role assignment, not agent state. If that
pane fails to restore, Pantheon must clear the saved conductor and say why rather
than transferring authority to another pane.

**The shape to aim for.** Keep the existing frontend-owned restore path, key
roster and conductor state by a stable project identity, and restore the
conductor only after its pane has spawned successfully. Continue treating each
pane as a new process, preserve saved worktree references, and make partial
restore failures visible without discarding the panes that remain valid. Layout
settings may stay machine-local; the question is which settings are genuinely
project-specific, not whether every setting can be put into one file.

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
- **Conductor failure.** The role must be restored only to the recorded pane.
  A missing or failed pane clears it; Pantheon must never silently promote a
  substitute.
- **What layout belongs to the project.** Pane membership and worktrees clearly
  do. Window height and column preference may be user and machine preferences
  rather than repository state.
- **Dirty worktree recovery.** Reuse is already safer than replacement. If
  reattachment fails, the UI still needs to lead the user to the preserved path
  and explain why the pane was not reopened.

## The conductor cannot see or reclaim a pane's context window

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

## A pane whose model is failing looks exactly like a healthy idle one

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
