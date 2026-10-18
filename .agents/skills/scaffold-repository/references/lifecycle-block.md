## Life Cycle

This project has opted into Lexicon's AI Development Life Cycle. Source of truth: SPEC-0010,
https://github.com/G-Hensley/lexicon/blob/main/docs/specs/0010-ai-development-life-cycle/spec.md

**Before the first increment.** `shape-ideas` establishes the North Star (recorded in the shaping
record under `docs/ideas/`): what the project ultimately accomplishes, for whom, and its major
goals, non-goals and constraints. It then picks the smallest delivery strategy: Direct Build ships
the finished product as the first increment when that already fits one bounded, reversible,
verifiable increment; Incremental Build runs North Star, MVP, then increments when the work is
larger but assumptions hold; Experimental Build retires an uncertain load-bearing assumption
through a question or spike to evidence before any prototype or MVP. Never an MVP just because the
work is called a project. The roadmap past the current increment stays a hypothesis, never a
backlog of executable issues. Before the repository exists, `plan-repository` decides what is
cheap now and costly later: rulesets, scanning, docs structure, test layout, hooks. Its plan ends
in a handoff block carrying the inputs `scaffold-repository` takes to scaffold the day-one file
surface.

**The loop.** Work then runs continuously through Restock, Run, Reckon. Restock refills `Ready`
from the most recent Reckon record on each issue, not just current board state, and creates
executable issues for the current increment only. Run designs, builds and verifies. Reckon writes
what the increment taught. Every increment ends with a Reckon record before the next Restock.
Each issue carries the seven-section contract: What, Why, Scope, Done when, Validation,
Dependencies and decisions, Links. When this repository's issues are on a GitHub Project board,
`Ready`, `Doing` and `Review` are its Status columns, with starting limits per repository, subject
to revision from measured data: `Ready` floor 3, `Review` ceiling 5. When they are on no board, the
repository runs without columns, neither limit applies, and a report that would read the board says
"no board" rather than zero. When every remaining roadmap item waits on outside evidence (a first
user, an engagement, a threshold), Restock creates no issues to fill `Ready`: it records `waiting on
evidence` in a comment headed `### Restock: waiting on evidence` on the last increment's tracking
issue, naming each gated item, the evidence it waits on and the trigger that reopens Restock. The
`Ready` floor does not apply until a trigger arrives, new work needs none of that evidence, or the
owner reshapes the roadmap. The acts below each have their own skill in `skill-library/`; use the
smallest one that answers the open question, and skip the rest.

1. `shape-ideas`: establish the problem, outcome and boundaries, when they are not yet agreed.
2. `research`: resolve a decision-relevant fact, when one is uncertain.
3. `write-spec`: promote agreed shaping into canonical requirements, when the work warrants a
   specification. Skip it for a bounded bug or an obvious small change.
4. `record-decision`: record a durable, consequential decision and its rationale.
5. `issue-planning`: create executable issues for the current increment only, not the whole
   roadmap.
6. `implement-change` for planned work, `fix-bug` for an observed defect that owes no shaping.
7. `ship-change`: report `ready` only after independent review and exact-head checks both hold.
8. `reconcile-project`: produces a general, evidence-linked account of what is built, stale, or
   undocumented, and, before a Reckon record, the Reckon inputs block (board counts or "no board",
   flow figures, merged and closed work since the last record, a hold state per acceptance
   criterion, staleness flags). It gathers inputs only; the Reckon record itself is written
   separately.

Records:

- A shaping record lives under `docs/ideas/`.
- A specification lives under `docs/specs/`.
- A Reckon record is a comment headed `### Reckon record` on the issue tracking the increment. It
  states confirmed and disproven assumptions, newly discovered constraints, new risks, unexpected
  opportunities, whether the outcome was achieved, implications for what follows, and a
  disposition: `continue`, `remediate`, `reshape`, `investigate`, `accept`, `defer` or `stop`.

Fallback rule: when the ideal capability does not exist yet, the conductor or a named person does
the bounded work by hand, with the acceptance and evidence the skill would have required, and
names the substitution in that increment's Reckon record.
