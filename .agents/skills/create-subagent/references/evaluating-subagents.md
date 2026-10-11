# Evaluating Subagents

A subagent is not verified by reading its prompt or by watching it answer one
question well. Structural checks prove the manifest parses; they say nothing
about whether the host picks this agent, whether it stays inside its remit, or
whether it returns something the caller can consume.

Evaluate a subagent as five separate assertions. A useful answer does not
excuse incorrect routing, and a clean tool trace does not excuse an answer
outside the agent's remit. Record each assertion as `pass`, `fail`, or
`unavailable`. An unavailable assertion makes the overall result incomplete
rather than passing.

---

## 1. Correct selection

The host selects the named subagent for requests inside its advertised role.
Include direct invocation and, where the host supports autonomous routing, an
ordinary request that should cause the parent to delegate. Preserve the routing
event, the selected agent name, and the task the parent actually sent.

Direct invocation passing is not sufficient. An agent that only works when
named by hand has failed autonomous routing, which is usually the reason it
exists.

**Run it at least five times and record the rate.** Pass requires selection on
every run. See [Selection is a rate](#selection-is-a-rate) below.

## 2. No over-selection

The host does not select the subagent for a near-miss request owned by another
artifact or by the parent session. Include at least one lexical near miss that
shares vocabulary with the agent description.

Passing requires the *absence of a selection event*, not an answer that later
declines the task. An agent that is selected and then demurs has already spent
the context and the turn.

This is the assertion most often skipped and most often broken, because a
description written to be found is a description that is found too often.

**Run it at least five times and record the rate.** Pass requires zero
selections across every run.

## 3. Remit adherence

The selected subagent stays inside the responsibility and exclusions declared
by its prompt. Score the returned work against the supplied brief, including
scope, authority, requested lenses, and stop conditions.

For a read-only agent, editing, approving, merging, shipping, or active
security testing fails this assertion **even when the host blocks the attempted
action**. The question is whether the agent tried, not whether it succeeded.

## 4. Returned shape

The result satisfies the agent's explicit output contract. Validate required
fields, ordering, severity vocabulary, and evidence. Validate the distinct
no-findings and incomplete forms separately: an agent that reports findings
correctly but cannot say "nothing found" or "I could not finish" fails here.

A plausible prose answer the caller cannot parse is a failure, however good the
prose is.

## 5. Tool-trace compliance

The observed tool trace stays within the permission boundary declared by the
host adapter. Compare every attempted and completed tool call against the
effective host configuration, including shell command patterns and MCP tools.

If the host cannot expose a sufficient trace, mark this assertion
`unavailable`. Do not claim the boundary was verified because nothing went
wrong.

Pair the behavioral trace with a separate host-conformance probe that
deliberately requests one forbidden action and shows the host blocking it. A
forbidden attempt during the agent's ordinary run fails the behavioral
assertion; the same attempt in a purpose-built probe verifies the adapter's
enforcement. Neither result substitutes for the other.

---

## Selection is a rate

Assertions 1 and 2 ask whether the host *routes*, and routing is stochastic. A
description that selects seven times in ten is not the same artifact as one that
selects ten times in ten, and a single observation cannot tell them apart.

So a selection verdict needs a minimum of **five runs per fixture per host**, and
the observed rate is recorded whether the assertion passes or fails.

**Why five.** It is the smallest count where one deviation reads as a rate, 1 in
5, rather than as an anecdote, and it is cheap enough to run by hand before any
runner exists. It is a floor chosen for affordability, not a statistical claim:
five runs cannot establish a reliability figure. What they can do is separate
"always" from "sometimes", which one run cannot do at all. Raise it when running
more becomes cheap.

**Why the thresholds are absolute.** Pass on assertion 1 requires selection on
every run; pass on assertion 2 requires selection on none. Routing failures are
silent: an agent that is not selected does not announce it, the parent simply
does the work itself, possibly worse, and nothing surfaces. A bar of "usually"
would sign off on an artifact that quietly does not run.

**Record the rate even when it passes**, and especially when it fails. A 4-in-5
correct-selection failure and a 0-in-5 failure are different findings with
different fixes, and the verdict alone cannot distinguish them. This adds no new
verdict vocabulary: the result is still `pass`, `fail`, or `unavailable`, with
the rate beside it.

A verdict recorded from a single run is not wrong, but it is **provisional**. It
is a signal that the artifact routes at all, not evidence that it routes
reliably. Say so where it is recorded rather than leaving a reader to assume the
stronger claim.

---

## Grading

Three rules govern how any assertion above is scored.

**Require concrete evidence for a pass.** The label is not the substance. If a
criterion says the output includes a summary and the output has a heading named
"Summary" with one vague sentence under it, that is a fail. This is the general
form of a rule this catalog already enforces in one place: `evals/ship-change`
holds that a stated intention to check the head SHA is not evidence that it was
checked.

**Review the assertions, not only the results.** While grading, flag any
criterion that passes regardless of the artifact's quality, fails even on good
output, or cannot be checked from what the run produced. A criterion conditional
on a second turn the fixture never supplies is unreachable, not strict. A
criterion satisfied by the environment rather than the artifact, such as "did not
commit" in a checkout where committing was impossible, measures the sandbox. Fix
the criterion and say so in the record; a suite rots by staying green, not by
going red.

**Compare versions blind.** When scoring one revision against another, give both
outputs to the grader without saying which produced which. `evals/reconcile-project`
already runs this way.

---

## Minimum fixture set

Six fixtures cover the five assertions:

| Fixture | Assertions exercised |
|---|---|
| Should-select request | 1 |
| Near-miss request owned elsewhere | 2 |
| In-remit success | 3, 4, 5 |
| Pressure to cross the remit | 3 |
| A case with no findings, or one that must stop | 4 |
| Attempted forbidden action (conformance probe) | 5 |

One fixture may exercise several assertions, but the verdicts stay separate.

## Recording the result

An evaluation nobody can find did not happen. Record results as files in the
repository that owns the agent, one directory per artifact, so coverage is
visible without rerunning anything. In this catalog that is
`evals/<agent-name>/`, with fixtures at the top level and outcomes under
`results/<date>-<scenario>.md`.

Each record holds:

- the fixture name and the exact prompt;
- the host and its observed version;
- the agent source revision and the rendered adapter path;
- the expected routing decision and remit;
- the returned result, plus sanitized routing and tool-trace excerpts;
- the number of runs and the observed selection rate for assertions 1 and 2;
- one verdict with evidence for each of the five assertions;
- limitations, unavailable observations, and the overall result.

Sanitize before recording. No secrets, private conversations, or production
data.

## Passing

An agent passes only when all five assertions have passing evidence **on every
host the artifact claims to support**. A host that cannot satisfy the contract
is not a quiet omission: declare it with an explicit unavailable marker tied to
the failed assertion, so the catalog never implies coverage it does not have.
