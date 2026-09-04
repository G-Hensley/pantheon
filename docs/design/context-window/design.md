# Context window accounting, clear, and model switch

Source: `BACKLOG.md`, "The conductor cannot see or reclaim a pane's context
window" and "A pane whose model is failing looks exactly like a healthy idle
one". This design follows model-aware launch (Phase 3) and headless dispatch
(Phase 4). It does not scrape a CLI status line or treat quiet time as proof of
failure.

## Signal model

Pantheon keeps one `ContextWindowSample` per pane: used tokens, model limit,
source, observation time, and the reset generation it belongs to. The source is
one of these:

- **Measured:** a Phase 4 adapter normalized the last headless request's current
  window usage from structured CLI output. An adapter must report current-window
  tokens, not cumulative billing totals. If its format cannot make that
  distinction, usage is unavailable rather than "measured".
- **Estimated:** for an interactive pane, count printable UTF-8 bytes Pantheon
  writes and receives since spawn or a verified reset, then convert with an
  explicitly named heuristic. Control sequences are excluded. Tool results and
  files read outside the PTY remain invisible, while redraws can still inflate
  the count, so this signal ranks panes only. It never triggers a refusal,
  clear, or switch by itself.
- **Unknown:** the model limit or a trustworthy sample is unavailable.

A measured sample becomes an estimated sample as soon as later interactive I/O
is added to it. A verified clear or model relaunch advances the reset generation
and starts again at zero. An unverified action does not reset accounting.

Roster examples make provenance part of the value:

```text
- sess-2 (codex, gpt-5.6-sol) brain=main [context window: 61k/200k, measured 18s ago]
- sess-4 (opencode, local/lfm2.5:8b) brain=main [context window: ≈23k/32k, estimate from pane I/O]
- sess-6 (claude) brain=main [context window: unknown]
```

The words "context window" stay intact in fields, UI labels, documentation, and
tool names. "Context" alone continues to mean the shared brain.

## Clearing a context window

`clear_context_window(target, reason, task_id?)` is conductor-only and accepts
only a live agent pane in the same workspace. It refuses when the target owns
any open task (`pending`, `overdue`, `blocked`, `in_review`, or `rework`) and
rechecks that condition after approval, immediately before writing.

The call stages a destructive action in the UI and returns its action id. Human
approval is the default and cannot be forged by a tool argument. A user setting
may allow approval for the current app run only; persisted autonomous approval
is out of scope. A pane may be cleared at most three times per app run. Refused,
cancelled, and unverified attempts count, so a failing command cannot loop.

Before approval, Pantheon tells the target to preserve only durable material in
the shared brain: decisions with rationale, verified facts, relevant paths and
commit ids, ownership, unresolved blockers, and the next safe step. It says not
to copy a transcript, secrets, credentials, or tentative guesses. The target
uses `record_decision` and `record_fact`, then marks the action ready. If it
cannot do so, the UI lets the human cancel or approve the loss explicitly.

After approval, the backend uses the existing submit path and a per-CLI
`ClearSpec`:

| CLI | command |
| --- | --- |
| Claude Code | `/clear` |
| Codex | `/clear` |
| OpenCode | `/new` (`/clear` is its documented alias) |

Delivery is not success. Each `ClearSpec` must also provide a deterministic
new-conversation acknowledgement captured from an action-scoped, bounded copy
of PTY output. The normal reader still streams bytes to the renderer and stores
no transcript. The tool reports `verified` only after that acknowledgement and
a live-process check. A timeout reports `unverified`, leaves accounting intact,
and disables further automated clears for that pane. Implementing a CLI adapter
is gated on checked-in output fixtures from the supported CLI version. A mere
post-command output burst is not a verifier.

Command check, retrieved 2026-09-03: installed `claude 2.1.259`, `codex-cli
0.153.0`, and `opencode 1.18.27` were checked with `--help`; the destructive
commands were confirmed in the vendor command references ([Claude Code
commands](https://code.claude.com/docs/en/commands), normative, published date
unknown, high confidence; [Codex developer
commands](https://developers.openai.com/codex/cli/slash-commands), normative,
published date unknown, high confidence; [OpenCode
TUI](https://opencode.ai/docs/tui/), normative, last updated 2026-09-03, high
confidence).

## Failure signal and recovery

For an interactive task, Pantheon records dispatch time, first output time, last
output time, and bytes in each burst. No first output is "waiting for first
output". A short early burst followed by silence while the task remains open is
"suspected provider failure". Neither becomes `error` from timing alone: a cold
model can be quiet, and a terse success can look like an error. The signal may
raise a visible recovery suggestion, never take a destructive action.

Phase 4 structured output and process exit are authoritative signals. A
structured 429 opens a circuit breaker for the configured OpenRouter free-tier
account, stops new dispatch to that tier until its retry time or human release,
and tells the human. Pantheon does not rotate to another free model because the
quota is account-wide. Local or paid fallback requires a separate approved
switch and must still satisfy Phase 3 policy.

A switch first settles the failed attempt as `error` or `abandoned`; otherwise
the open-task interlock would correctly refuse it. It then uses the same
destructive-action approval flow as clear, rechecks that the pane has no open
work, and relaunches the pane with Phase 3's model flag. The pane id, brain,
working directory, and existing worktree are preserved; the child process and
conversation are not. Relaunch success and the reported active model verify the
switch.

Fallback candidates come from `MODEL-GUIDE.md` at a configured path and are
loaded into Phase 3's model profiles. Pantheon does not vendor a snapshot. A
missing, stale, or malformed guide disables automatic selection and asks the
human for an explicit model. A recovery chain gets two attempts per root task,
including redispatch, clear, or switch, across linked retry tasks. Exhaustion is
a visible terminal refusal.

## Task and action record

Each root task gains an append-only recovery log. Every detection, redispatch,
clear, and switch records: action id, source signal, target, old and new model,
context window sample before and after, approval state and actor, requested,
started, and verified times, linked retry task id, and outcome (`verified`,
`unverified`, `refused`, `cancelled`, or `failed`) with the reason. Maintenance
clears without a task live in the same session-action ledger; when `task_id` is
present, the task stores the action id. The drawer shows the sequence so a human
can see both what failed and what Pantheon tried.
