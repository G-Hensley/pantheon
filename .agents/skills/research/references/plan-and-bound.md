# Plan and Bound

How to decompose a multi-part question before retrieving, and when to stop.

Every external research tool compared for this skill in 2026-09 had a stopping
rule; this skill had none. Without one, retrieval ends when the agent feels done,
which is the one criterion nobody can audit.

## When to plan

Plan when the answer has more than one load-bearing part: "is X affected and
what is the fixed version", "compare A and B on C", "what changed between 2.x
and 3.x and does it affect our config". Skip it for a single-fact question.
Planning a one-fact lookup is ceremony, and ceremony gets abandoned.

## The plan

Write it before the first retrieval. One line per sub-question:

| Sub-question | Settling artifact | Currency pointer, if a currency claim |
|---|---|---|
| Is pkg 1.2.3 affected by the advisory? | OSV record, then the maintainer advisory | n/a |
| What is the first fixed version? | The advisory's affected range, and the release list | The release list |

The settling artifact comes from the routing in
[source-policy.md](source-policy.md). Naming it first is the point: an agent
that decides what counts as an answer after seeing results can accept whichever
result arrived first. Memory may choose the sub-questions and where to look;
it is still not evidence for any of them.

Each sub-question is a lane. When records are written to a file, it is the
`lane_id` in [evidence-schema.md](evidence-schema.md).

Keep the plan inside the question's scope. A sub-question inherits the parent's
version, platform, and edition; it never silently broadens them.

## Rounds

The **initial pass** retrieves the settling artifact for every sub-question.

A **follow-up round** is a second batch of retrieval aimed at gaps the initial
pass left. Allow at most **two**. Each must name the gap it fills: a
sub-question still without evidence, a conflict to resolve, or a currency
pointer not yet found. A round that only rephrases an earlier query is not a
round; it is the same search again.

Keep a running list of the queries run and the URLs opened. Do not repeat an
entry. If two queries for a sub-question surface no primary source, change the
angle instead of the wording: go to the repository, its releases and tags, or
its issue tracker, or search in the project's own vocabulary.

## Stop when

Stop at the first of:

1. Every sub-question has its settling evidence.
2. A round finds nothing new: no new origin, no new artifact, no change to any
   answer.
3. Both follow-up rounds are spent.

What is still open is reported as a gap, with what was checked. A spent bound
is a result, not a failure: "I checked A, B, and C and could not establish X"
is an answer a reader can act on, and a guess dressed as a finding is not.

## What the bound never removes

The round cap limits discretionary searching. It never waives a minimum the
skill sets elsewhere:

- A currency claim still needs the artifact **and** its currency pointer.
- A consequential claim still needs its second independent source.
- "Latest" still needs the check for newer non-normative work.

Retrieval needed to meet one of these minimums does not count against the round
cap. If a minimum cannot be met at all, the claim is reported as unverified at
`low` confidence; it is not made anyway.
