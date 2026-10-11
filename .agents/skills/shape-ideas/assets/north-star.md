---
id: IDEA-NNNN
title: <Idea name>
type: idea
status: candidate
created: YYYY-MM-DD
---

# <Idea name>

Shaping record: discovery, rationale, hypotheses and tentative direction. This is
not a specification or authority to implement. Use one file under `docs/ideas/`;
fill its increment section from `increment.md`. Remove template guidance before
handing it over, and omit sections that do not apply.

## North Star

**Goal:** what should ultimately become possible, in the user's language.

**Who it serves:** the concrete user, their situation and current workaround.

**Success evidence:** the observation that would show the direction was useful.
This is a hypothesis, not accepted requirements or claimed verification.

**Constraints:** existing systems, stack, data boundaries and investment limits.

**Non-goals:** what a reasonable reader might expect but this direction excludes.

## Direction and alternatives

Chosen approach and discriminating criteria, including reversibility. State why
it won and what would change the recommendation. Keep implementation detail out
unless it changes the direction.

| Alternative | Why not chosen | Revisit if |
| --- | --- | --- |
| <option> | <reason> | <evidence> |

## Appetite and delivery strategy

**Appetite:** <agreed number> agent sessions for <bounded work>, optionally capped
at <$amount> when the host reports cost. One session is one bounded worker session.

**Delivery strategy:** <Direct Build / Incremental Build / Experimental Build>,
because <scope, clarity, uncertainty, failure consequence, reversibility,
integration breadth and verification difficulty that discriminate here>.

Direct Build has exactly one increment, the complete bounded solution. Incremental
Build has a smallest useful outcome and later outcome hypotheses. Experimental
Build begins with an evidence-producing increment; further product work depends on
its result. Each increment carries its own appetite without silently enlarging
the agreed bound. If it does not fit, state what needs a smaller cut and what
scope decision remains open.

## Assumptions and risks

| Assumption | Load-bearing | If wrong |
| --- | --- | --- |
| <assumption> | Yes / No | <what changes> |

| Risk | Treatment and reason |
| --- | --- |
| <risk> | <accepted / mitigated / investigated by a bounded experiment> |

## Increments

Insert the next increment from `increment.md`. Later outcomes, only where the
strategy warrants them, stay a paragraph each: intended benefit, appetite,
assumptions, dependencies and evidence that would justify pursuing them. No
executable backlog or duplicated progress status belongs here.

## Open questions

What is still unknown, who can answer it, and which decision needs the answer.

## Handoff

Next step: `write-spec` promotes accepted requirements into a specification
when the work warrants one; obvious bounded work may go directly to an issue.
This record's definitions of good remain hypotheses until promoted. On consumption
by `write-spec`, its status becomes `promoted`, it links to the specification,
and the record remains history, not a competing source of requirements.
