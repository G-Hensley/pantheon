# Increment Slicing

How to cut a design into versions that ship one at a time. The test for every cut:
if only this landed, would someone gain a useful capability or decision evidence?

## The two rules that decide most cuts

**Independently deliverable.** A product increment can go to a real user without
waiting on a later increment. An experiment delivers evidence that stands alone.

**Independently valuable.** Someone is better off once it lands. If v1 is useless
without v2, they are one increment and you have cut in the wrong place.

Everything below is a consequence of these two.

## Slice by outcome, not by layer

The most common bad cut follows the architecture:

```text
BAD
v0: database schema and migrations
v1: API endpoints
v2: UI
```

Nothing is usable until v2. Every integration problem stays hidden until the
expensive end, feedback arrives after all the decisions are locked, and there is no
point at which you can stop and still have something.

The same work, cut by outcome:

```text
GOOD
v0: one user shares one saved report with one teammate by link, view-only
v1: shares go to named teammates with revocable access, and the owner sees who viewed
v2: team-wide shared folders with role-based permissions
```

Each of these is a thing a person can do. Each can ship and stop.

## A worked ladder

The design: let users share a saved report with their team.

**North-star (the durable version):** any team member can find, open, and subscribe
to reports their colleagues have built, with permissions that match the underlying
data access, and without asking the report author for anything.

**Riskiest assumption:** that permissions on the report can be derived from
permissions on the underlying data, rather than managed separately. If that is
false, the entire model changes, so it goes first.

| Increment | Outcome | Why here |
|---|---|---|
| v0 | Owner generates a view-only link; anyone with the link and existing data access can open the report | Tests the risky permission assumption on the smallest possible surface, one real user, one real report |
| v1 | Owner shares to named teammates, can revoke, and sees view counts | Adds the access control the link version deliberately skipped, now that the model is proven |
| v2 | Team-wide shared folders with roles | Only worth building once sharing is used enough to need organizing |

Note what v0 is not. It is not "the sharing infrastructure." It is a thing one
person can do end to end, chosen because it puts the riskiest assumption in front
of a real user in the least work.

## Experiments are increments whose good is evidence

A throwaway spike, a load test, interviews or a disposable prototype can be a valid
increment. It earns its place by retiring a load-bearing uncertainty, not by
pretending to ship a product. Its definition of good names the question, repeatable
evidence, threshold, and the decision each result enables. Include quality,
exclusions and a disposal or rollback path, just as for a product increment.

Choose **Experimental Build** when that uncertainty could invalidate the direction.
For example: within one agent session, measure local transcription on ten agreed
recordings against reference transcripts. Good might be error below the agreed
threshold at the required speed. A pass permits shaping a prototype; a failure
reopens engine choice or stops investment. Neither outcome authorizes building it.

An experiment has an appetite in agent sessions, optionally a cost ceiling. If it
cannot answer the question within that bound, report that it needs a smaller cut;
do not stretch the estimate. An experiment with no decision it could change is
not a useful increment.

## Direct Build does not need a ladder

Use **Direct Build** when the whole solution is one bounded, reversible, verifiable
increment. A JSON-to-CSV CLI command using an existing parser, with no integrations,
can be the complete first delivery within one agent session. Its quoting fixtures
and refusal to overwrite a destination on invalid input are its evidence. Do not
invent a partial exporter followed by polish to make an MVP ladder.

Use **Incremental Build** when one bound cannot contain the full understood work.
The report-sharing ladder above is an example, not a required shape for every idea.
Record the strategy and reason before slicing, considering scope, clarity,
uncertainty, consequence of failure, reversibility, integration and verification.

## Sequence by risk, not by ease

The natural pull is to build the easy, well-understood parts first because progress
feels good. That defers the decision that could invalidate everything.

Ask: which assumption, if wrong, means rebuilding? Put the increment that tests it
first, even when it is the awkward one. A v0 that proves the hard thing and looks
unimpressive beats a v0 that looks finished and proves nothing.

## Specify the next one, sketch the rest

Write the next increment in full detail. Keep every later increment to a paragraph
of intent.

Detail written for v2 today is mostly wrong by the time v1 ships, because v1 is
what teaches you what v2 should be. Worse, detailed specs get defended: people
treat rewriting them as waste and steer toward the plan instead of the goal. A
paragraph is cheap to throw away, which is exactly why it survives contact with
reality.

Reshape the remaining ladder after each increment ships. That reshaping is the
process working, not the plan failing.

## One-way doors

Flag anything expensive to reverse, and place it as late as the design allows:

- Database schema changes that require a migration to undo
- Public API shapes, URL structures, and webhook payloads other people build on
- Data retention and deletion behavior
- Anything that writes data you cannot recompute
- Pricing, billing, and anything a customer signs

When a one-way door has to come early, say so in the increment section's reversibility
section and name the cost of getting it wrong. When it can wait, keep the early
increments behind a flag, a manual step, or a smaller surface until the shape is
proven.

## Sizing against appetite

Resolve the appetite with the owner before slicing. Use bounded sessions of one
agent worker, with an optional dollar ceiling where cost is reported. Reuse a
stated bound; if none exists, ask before cutting. Record it on every increment,
including evidence work and later sketches; mark tentative later bounds as such.
Do not substitute wall-clock availability, tokens or model tiers for this unit.

Compare scope with the bound. If preview and confirmation consume the one agreed
session while presets and rollback each need another, the cut does not fit.
Say which behaviors need a smaller cut. Offer a separately useful read-only preview
or a decision experiment, clearly awaiting agreement to the changed scope. Preserve
the excluded behaviors as unaccepted later possibilities, not hidden omissions from
an otherwise unchanged definition of good. Do not recommend more sessions as the
way to fit. If no safe useful cut remains, report that and reconsider the direction.

Feedback remains the test of the cut: if it arrives only after unrelated
capabilities finish, return to Shape. More than four or five increments may mean
two projects; keep the second as a named idea.

## Bad cuts, and what they usually mean

| Bad cut | What it signals | Fix |
|---|---|---|
| Layer by layer (schema, API, UI) | Slicing by implementation structure | Cut by what a user can do |
| "v0: the framework, v1: the features" | Building infrastructure ahead of a proven need | Make v0 one real feature end to end; let the framework emerge |
| Every increment is "phase 1 of the real thing" | No increment is independently valuable | Find the one thing that helps someone alone, and start there |
| v0 contains everything, v1 is polish | Fear of shipping something incomplete | Move anything not needed for the core outcome to v1 |
| Increments split by team or component | Organizational structure, not user value | Cut by outcome and let teams coordinate within a slice |
| The risky part is in v3 | Sequencing by comfort | Move it to v0, even partially, as an experiment |
