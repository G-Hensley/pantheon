# Backlog

Ideas are not commitments. Promote an item only once its trigger, ownership,
security boundary, and validation method are understood.

Findings from point-in-time reviews of existing code go to `IMPROVEMENT-AUDIT.md`,
which is deliberately untracked (see `.gitignore`), so a fresh clone will not have it.

## What this file is now

Triaged 2026-09-16 under `orion:SPEC-0001` FR-009. Twenty sections went in. Fifteen described
work that had shipped or research that had finished, and are archived. Five describe work still
to do, and each is one line below pointing at the issue that owns it and the document that holds
its reasoning. None was untracked, so the triage filed no new issue: the tracker cutover recorded
in [`docs/github-tracking-migration.md`](docs/github-tracking-migration.md) had already given
every open item an owner.

What is left here is an index, not a backlog. It exists so that an issue citing a section by name
still resolves from where the citation was written. It is deleted when nothing cites it, which is
when the seven issues below have closed.

## Still to do

One line per item. The issue owns scope, acceptance and status. The document holds the reasoning
and the measurements the issue links to rather than restates.

| Section | Owned by | Reasoning |
| --- | --- | --- |
| Model-aware dispatch | #56 | [`docs/design/model-routing/design.md`](docs/design/model-routing/design.md) |
| Guardrail: OpenCode sessions must stay on free OpenRouter models | #53 | [`docs/design/openrouter-free-tier/design.md`](docs/design/openrouter-free-tier/design.md) |
| Restoring panes does not restore the agents that occupied them | #72 | [`docs/design/session-restore/design.md`](docs/design/session-restore/design.md) |
| The conductor cannot see or reclaim a pane's context window | #61 and #63 | [`docs/design/context-window/design.md`](docs/design/context-window/design.md), "Clearing a context window" |
| A pane whose model is failing looks exactly like a healthy idle one | #60 and #62 | [`docs/design/context-window/design.md`](docs/design/context-window/design.md), "Failure signal and recovery" |

Seven distinct issues own the five: #53, #56, #60, #61, #62, #63 and #72.

## Where the archived sections went

The fifteen sections whose work has shipped, moved verbatim with the commit or pull request that
closed each one. The text is unchanged; only its location moved.

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
