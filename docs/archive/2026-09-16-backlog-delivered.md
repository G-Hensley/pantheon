# Backlog sections that shipped

Dated record, 2026-09-16. Fifteen of `BACKLOG.md`'s twenty sections described work Pantheon has
since done, or research it has since finished. They are moved here verbatim, heading and body
unchanged, with the evidence that closed each one added above it.

Historical evidence, never current instruction. Nothing here is a requirement, including the
sentences written in the future tense: they were written before the work landed and are kept as
written, because rewriting them would destroy the record of what was believed at the time.

Three sections carry a note that is now stale in the other direction. "Two unseen notices" says it
is unmerged, and it merged. "Dispatch headlessly" says it is Claude-only, and it still is. Each is
flagged in its evidence line rather than edited in place.

Open issues that cite a section by name will find it here: #55, #57 and #58 cite three of these
fifteen. `BACKLOG.md` keeps a table mapping all twenty sections to where each one now lives, so a
citation still resolves from where it was written.

| Section | Closed by |
| --- | --- |
| [Prior art: nobody has solved the OpenCode local-model timeout](#prior-art-nobody-has-solved-the-opencode-local-model-timeout) | Completed research, not a capability. The upstream issue it turns on is closed as not planned. |
| [Dispatch headlessly (`opencode run`) instead of typing into the TUI](#dispatch-headlessly-opencode-run-instead-of-typing-into-the-tui) | Shipped for Claude in PR #40 and PR #45 (`6d6994b`). |
| [Local models: pick one that survives a cold start](#local-models-pick-one-that-survives-a-cold-start) | Solved 2026-08-11 by measurement. OpenCode's 30 second budget on a local model is not configurable. |
| [Liveness: tell a slow agent apart from a dead one](#liveness-tell-a-slow-agent-apart-from-a-dead-one) | Shipped, task e18r66: `abandon_lost` and `Shared::reconcile_abandoned` in `src-tauri/src/mcp.rs`. |
| [`get_task_result` with no id outgrows its own response limit](#get_task_result-with-no-id-outgrows-its-own-response-limit) | Shipped, task d4nhzh: `select_tasks` in `src-tauri/src/mcp.rs` and its window test. |
| [Dispatch loses whole 1 KiB chunks from the head of a long prompt](#dispatch-loses-whole-1-kib-chunks-from-the-head-of-a-long-prompt) | Fixed by bounding the payload 2026-08-12, then raised to 8192 bytes for Linux Codex in `4f7d894`. Issue #51 is closed. |
| [The limit, checked against the dispatches that had already gone silent](#the-limit-checked-against-the-dispatches-that-had-already-gone-silent) | The same fix. This section is the retrospective check that found the rule held. |
| [A conductor cannot wait for a dispatch, only re-ask whether it landed](#a-conductor-cannot-wait-for-a-dispatch-only-re-ask-whether-it-landed) | Shipped, task dq3s0j, PR #17: `wait_for_tasks` in `src-tauri/src/mcp.rs`. |
| [A blocked agent can only ask the human, never the conductor](#a-blocked-agent-can-only-ask-the-human-never-the-conductor) | Shipped, task 6v3ebz: `ask_conductor` and `answer_question` in `src-tauri/src/mcp.rs`. |
| [Nothing enforced cross-model review, so "done" meant self-certified](#nothing-enforced-cross-model-review-so-done-meant-self-certified) | Shipped, task d575h4, PR #16 (`3efebac`): a task carries a reviewer, `complete_task` moves it to `in_review`, and `review_task` closes it or returns it as rework. |
| [Dispatch to a busy pane typed the new brief over the running one](#dispatch-to-a-busy-pane-typed-the-new-brief-over-the-running-one) | Shipped in Phase 2 of the repair plan: a dispatch to an occupied pane is `queued` rather than typed. |
| [Two unseen notices for one pane deadlocked each other](#two-unseen-notices-for-one-pane-deadlocked-each-other) | Shipped in PR #47 (`cea236f`). The section's own note that it was unmerged is stale; it merged on 2026-09-08. |
| [The conductor's five-pill task strip hides the work it is meant to coordinate](#the-conductors-five-pill-task-strip-hides-the-work-it-is-meant-to-coordinate) | Shipped, task 7c7f3j, PR #25 (`8d07262`): the task drawer replaced the five-pill feed. |
| [Orchestrator may open the sessions the work needs](#orchestrator-may-open-the-sessions-the-work-needs) | Shipped in PR #49 (`7193ee2`), with human approval on the escalation this section said to treat as one. |
| [Dispatch allowance is invisible and reset is coupled to Stop](#dispatch-allowance-is-invisible-and-reset-is-coupled-to-stop) | Shipped in PR #46 (`203c523`): the allowance is shown and resets without cancelling tasks. |

## Prior art: nobody has solved the OpenCode local-model timeout

**Closed by:** Completed research, not a capability. The upstream issue it turns on is closed as not planned. Cited as grounding by #57 and #58.

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

**Closed by:** Shipped for Claude in PR #40 and PR #45 (`6d6994b`). Extending it to the other CLIs is #58.

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

**Closed by:** Solved 2026-08-11 by measurement. OpenCode's 30 second budget on a local model is not configurable. The behaviour this measured is explained under #57.

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

**Closed by:** Shipped, task e18r66: `abandon_lost` and `Shared::reconcile_abandoned` in `src-tauri/src/mcp.rs`. The case it deliberately left open, a pane alive but failing, is #60.

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

**Closed by:** Shipped, task d4nhzh: `select_tasks` in `src-tauri/src/mcp.rs` and its window test.

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

**Closed by:** Fixed by bounding the payload 2026-08-12, then raised to 8192 bytes for Linux Codex in `4f7d894`. Issue #51 is closed. The measured evidence now lives in `docs/dispatch-composer-evidence.md`.

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

**Raised for Linux Codex, 2026-09-10.** A real Codex CLI composer on Linux
holds the complete prompt intact through at least 64 KiB
(`docs/dispatch-composer-evidence.md`), so `pane_input::max_prompt_bytes`
(`src-tauri/src/pane_input.rs:25`) now grants a Linux Codex pane 8192 bytes
(`CODEX_LINUX_MAX_BYTES`, `pane_input.rs:7`) instead of the shared
1023-byte bound, which every other target/platform combination
(`LEGACY_MAX_BYTES`, `pane_input.rs:6`), including Windows and OpenCode,
keeps unconditionally: this host and this CLI are the only pair with a
real composer-export proof behind them. "Raising that limit" above was
speculative when written; it is now measured for exactly that one pair,
not lifted for all of them, and the 1 KiB chunking loss this entry
describes is not reproducing on this host and CLI at any size tested.

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

**Closed by:** The same fix. This section is the retrospective check that found the rule held.

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

**Closed by:** Shipped, task dq3s0j, PR #17: `wait_for_tasks` in `src-tauri/src/mcp.rs`.

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

**Closed by:** Shipped, task 6v3ebz: `ask_conductor` and `answer_question` in `src-tauri/src/mcp.rs`. When an agent should escalate rather than decide is #66. #55 cites this section for the branch-split history it records.

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

**Closed by:** Shipped, task d575h4, PR #16 (`3efebac`): a task carries a reviewer, `complete_task` moves it to `in_review`, and `review_task` closes it or returns it as rework. Whether a review packet should mask the implementer is #52.

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

**Closed by:** Shipped in Phase 2 of the repair plan: a dispatch to an occupied pane is `queued` rather than typed.

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

## Two unseen notices for one pane deadlocked each other

**Closed by:** Shipped in PR #47 (`cea236f`). The section's own note that it was unmerged is stale; it merged on 2026-09-08.

**Fixed on `fix/undelivered-review-queue`, not yet reviewed or merged.** A
follow-on defect in the delivery path above. `next_delivery_for` picked a
candidate notice and then excluded only that one id from the occupancy test,
but `occupying_task` counts every other `in_review` task assigned to the same
reviewer, delivered or not. So with two undelivered review requests for one
pane, each cited the other as occupancy: neither was selected, nothing was
typed, `notice_delivered` stayed false on both, and every later drain repeated
the same choice. The pane held review requests it had never been told about,
with no timeout and no path out short of cancelling a task. Two undelivered
rework notices for one target deadlocked identically.

Reproduced at the unit level before the fix, not from a live pane:
`two_undelivered_review_requests_do_not_block_each_other` and
`two_undelivered_rework_notices_do_not_block_each_other` in `mcp.rs` both
failed on `6111e0c` with "neither notice has been seen, so one must be
delivered". The fix discounts *other undelivered notices for the same pane*
from that occupancy test and nothing else: a notice the agent has not received
is not work the pane is engaged in, so it cannot be what blocks another.

Acceptance is that unseen notices stop blocking each other without loosening
occupancy anywhere else. Three tests written against the old behaviour pass
both before and after, which is what shows the guard rather than the fix:
a delivered review still blocks the next notice, completing a review releases
the notice behind it, and a genuine pending task on the target still blocks an
undelivered review request. `occupying_task`, `is_occupied`, `queue_predecessor`
and dispatch admission are unchanged; `occupies_pane` is that predicate lifted
out verbatim so the selector cannot restate it and drift.

## The conductor's five-pill task strip hides the work it is meant to coordinate

**Closed by:** Shipped, task 7c7f3j, PR #25 (`8d07262`): the task drawer replaced the five-pill feed. What the drawer still does not recover is #65.

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

## Orchestrator may open the sessions the work needs

**Closed by:** Shipped in PR #49 (`7193ee2`), with human approval on the escalation this section said to treat as one. Verifying approval and first-task acknowledgement end to end is #73.

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

**Built on branch `work/session-requests`, not yet merged and not yet accepted
live.** Every question above was answered the safer way. It is a request, never
an autonomous action: `request_session` records an intent and starts nothing,
and only a human `approve_session_request` reaches a spawn. The ceiling is two
numbers rather than one, because they bound different failures: at most three
requests may await a decision at once, and at most ten may be admitted in an
app run. Validation runs before either is charged, so a malformed request costs
nothing, and a denial does not refund the run total. The host is not
caller-supplied: an approved request launches one of `claude`, `codex` or
`opencode` from a table in the backend, pinned against the frontend's
`SESSION_TYPES` by a test, so a request cannot become arbitrary process
execution. The existing free-model guard applies at admission as well as at
launch.

The id fight with restore is settled by a barrier rather than by a convention.
No id is issued until the frontend reports what it restored; that report raises
the allocator's floor past every pane on screen and can never lower it, and a
reserved id is never returned to the pool even when its launch fails, so no
pane ever wears a dead pane's number.

The part that took the most care is the gap between asking and approving, since
a human decision is slow and everything can move while it is open. The world a
request was made in is captured when it is made and rechecked at the moment of
commitment: a Stop, a project switch (including a switch away and back, which
leaves the path identical and everything else different), a conductor change, a
requester that is no longer the conductor, a requester that has moved out of the
`main` brain (including out and back), a respawned requester or a dead requester
each settle the request as `Stale` rather than launching into a world it was not
made for. Stop also settles every pending request as it lands, so resuming for
an unrelated reason cannot revive one.

An independent review of the first implementation found five defects in exactly
this area, and they are worth recording because four of them were invisible from
the tests that existed:

- **Commitment was not ordered against the changes it checked for.** The epochs
  were individually atomic, which is not the same property. A claim could read
  `halted == false` and matching epochs, have Stop or a project switch complete,
  and still reserve an id and write `Launching`. Admission had the mirror
  problem, reading project A and then stamping B's epoch. There is now one
  lifecycle gate, taken for write by every mutation that changes the world a
  request was made in and for read across the coherent capture and the short
  commit transition. Nothing slow runs under it.
- **The captured project was the journal directory, not the project.** For
  `/repo` the brain stores its markdown in `/repo/.pantheon/context`, and that
  is the path a request captured and an approved agent was launched in. With no
  project selected it captured the application's own data folder as though it
  were a project. The selected project is now stored separately from its
  storage, and the two move together; it cannot be recovered by stripping path
  segments, because the legacy and default layouts differ.
- **A repeat approval could mark a running session as failed.** The claim
  returned the record, so a second Approve read back `Launching` and the command
  could not tell that from having just won. It called the spawn helper again for
  the same reserved id, the duplicate check refused, and that observed failure
  settled a live launch as `Failed`. Exactly one caller now receives the launch
  capability, and only that caller may spawn or settle.
- **`Started` was recorded when the session ended.** Approval awaited the whole
  spawn helper, which returns at EOF on the agent's output, so a healthy agent
  sat in `Launching` for its entire life, three of them exhausted the
  outstanding cap permanently, and the pane's startup record was written after
  its handle had been removed, which let a running agent skip the first-dispatch
  gate as though it were a manual pane. The child's existence and its output
  forwarding are now separate: the request settles as soon as the process is
  inserted, the pane is registered at the claim, and forwarding outlives the
  command.
- **Requester authority was proved before a slow probe and never rechecked.** A
  conductor demoted during the model catalogue lookup still had its request
  admitted with the new conductor's epoch, and every later epoch check matched.
  Identity, liveness and brain membership are now rechecked at admission and at
  commitment, under the gate.

A second review of those corrections found two more, both in the same seam
between claiming a request and starting its process:

- **The spawn erased the startup gate it had just been given.** `spawn_session_
  inner` calls `note_session` on its way to creating the PTY, and `note_session`
  deleted the pane's startup record outright. That deletion was written for a
  respawn, where a new incarnation must not inherit what the last one earned,
  and it was correct about that and wrong about how to get it: a pane with no
  record is read as a manual pane and admitted without any check at all. So it
  ran on the first spawn too, between the claim that registered the pane and the
  process existing, and every approved session was dispatchable from the instant
  it was claimed. The record is now restarted rather than removed: `Starting`
  again, unadmitted again, its clock reset, and still a pane a request created.
  A test asserted the deletion, which is how it survived the first review; the
  comment above that assertion described a restart.
- **Session removal was not ordered against commitment.** The gate ordered
  Stop, the project, the conductor and brain movement, but not the thing that
  makes a pane stop being live. `claim_session_request` copied liveness,
  released the engine's lock, and committed from that copy, so an explicit
  close or an agent exit could complete in between and the claim would still
  authorize a launch for a pane that was already gone. A first attempt at this
  read liveness a second time just before committing, which narrows the window
  and does not order anything; the review was explicit that it would not do.
  The gate now lives on `SessionManager`, where both halves can reach it:
  `Shared` borrows it through `gate()`, and both removal sites, explicit close
  and observed exit, take it for write around the map removal. The lock order
  is one line and documented on the field: lifecycle before sessions, before
  requests, before every other state lock, never the reverse. A removal
  therefore lands entirely before the claim's snapshot or entirely after it has
  committed, and a commitment that won is not retroactively undone.

  What that still cannot order is a process that simply dies, because nothing
  holds a lock at the instant an OS reaps a child and the death is only noticed
  the next time `liveness` reaches `try_wait`. Liveness is read once more for
  that case, now inside the gate rather than after it, so the answer it acts on
  is one no gated removal could have produced. The comment claiming the gate
  excluded every such change has been corrected to say which ones it does.

Still open: live acceptance. Nothing here has been run in the app. A
request-created pane is reported `Starting` until its endpoint connects and
refuses its first dispatch until then, with a 120-second deadline that records
`ready_timeout` and deliberately kills nothing and starts no replacement, but
whether an agent is genuinely ready to work is only knowable from an
acknowledged first task, which needs a human at the keyboard.

## Dispatch allowance is invisible and reset is coupled to Stop

**Closed by:** Shipped in PR #46 (`203c523`): the allowance is shown and resets without cancelling tasks. Verifying the reset in an isolated fixture is #71.

Tracked by task `s9xc2s`. Observed 2026-09-07 during the Lexicon tracker pilot:
legitimate multi-project work reached the 40-dispatch cap, and subsequent turns
continued to receive "dispatch budget exhausted for this run." Independent review
and fresh-session task discovery could no longer be dispatched.

This is a dispatch-count guardrail, separate from headless dollar budgets. The
counter is shared by the running app, starts at zero, increments on accepted task
admission and is cleared by `set_halted(false)`. Completing tasks or starting a
new conversation turn does not refresh it. The conductor bar instead displays
the task ledger's total, which cannot tell a user how much allowance remains.

The existing human recovery is Stop then Resume. Resume clears the counter, but
Stop can cancel pending and queued work and terminate headless attempts. Requiring
that side effect just to renew an allowance is the defect. The agent-facing error
also omits the limit, scope and available recovery action. No MCP reset is exposed.

Correction in progress: show used, maximum and remaining dispatches in the conductor
bar and MCP roster; provide a dedicated human Reset dispatch budget action which
preserves task records and halted state; name that recovery in an exhausted
response. Keep the finite default and atomic charging at admission. Keep reset
outside agent-controlled MCP mutations: the agent whose looping is bounded must
not be able to replenish its own allowance. Stop and Resume retain their existing
behavior. App restart persistence and per-brain budgets are separate decisions.

Acceptance requires evidence that exhaustion blocks admission, a human reset
permits subsequent admission, and reset leaves both task records and halt state
unchanged. Refused dispatches must still cost nothing; concurrent admission must
respect the cap. UI tests must exercise the backend count, explicit reset action
and failure reporting. Run the repository's Rust and frontend checks and obtain
a different-model review before commit.

The user performed the existing Stop/Resume recovery on 2026-09-07; subsequent
Pantheon dispatches succeeded. The correction is being implemented in isolation
and is not installed in the running app.
