# Pantheon

A desktop cockpit for running several AI coding agents side by side, coordinated
rather than siloed. Each agent gets a live terminal pane; panes can share a
context store, and one can be promoted to conductor to fan work out and collect
results.

Runs on Windows and Linux: the terminal layer is `portable-pty`, which is ConPTY on
Windows and a Unix PTY elsewhere. macOS is unexercised rather than ruled out.
Status is working prototype, so read the Known gaps section of `README.md`
before relying on any part of it.

This file is orientation for this project only. The shared working-style rules, the
guarded-change policy, and the greenfield stack defaults live in
`~/Projects/global-llm-configs/AGENTS.md`, which every agent host loads. They are not
restated here on purpose; if they seem to be missing, the host's symlink to that file is
broken, so fix the symlink rather than copying the rules back in.

## Where things are

- `src/` : the React and TypeScript front end, built with Vite.
- `src-tauri/src/` : the Rust side. `mcp.rs` is the in-process MCP server the
  agents talk to, `worktree.rs` is the git worktree isolation, `lib.rs` and
  `main.rs` are the app.
- `ui-gallery/` : component previews.
- `README.md` : what Pantheon is and what it cannot yet do.
- `CONTRIBUTING.md` : setup, building, testing, branching, review, and pull
  requests. It is the authority on all of those; do not duplicate it here.
- `BACKLOG.md` : an index, one line per open item, pointing at the issue that
  owns it and the document under `docs/design/` that holds its reasoning. The
  reasoning is what was worth keeping: what was measured, which hypotheses were
  refuted, what was tried and did not work. Sections whose work has shipped are
  in `docs/archive/2026-09-16-backlog-delivered.md`.

Build with `pnpm`. `dev.cmd` and `build.cmd` on Windows, or `dev.sh` and
`build.sh` on Linux, wrap the usual commands.

## Documentation

Orion member. Cross-repository specifications and decisions that bind this repository live in
`orion/docs/`, and are cited here by identifier rather than restated.

[`docs/README.md`](docs/README.md) maps every document in this repository and records every place
this repository departs from the layout, with the condition that retires each one.

## Tasks

Read `docs/github-issues.md` before picking up work. It records a staged
handoff, so first verify its activation checkpoint:

- Before verified activation, `.tasks/` remains authoritative. Use
  `lexicon task` for an authorized update, except while the conductor has
  explicitly paused legacy writers for the handoff.
- After verified activation, GitHub Issues owns deliverable identity,
  acceptance and resolution, and the repository's selected GitHub Project
  owns stage and priority. Never run `lexicon task` or edit `.tasks/`; the
  files become a read-only historical ledger.
- A staged issue or Ready field is not permission to start. Read its full
  body, dependencies, assignee, comments and latest conductor/session claim.

`docs/github-tracking-migration.md` maps every reviewed source identity to
its owning issue or historical disposition. It is provenance, not another
tracker: the active issue wins if the map and issue later disagree.
Reasoning stays in `BACKLOG.md`; an issue links to it rather than restating
it.

This manual tracker handoff installs no MCP tool, dispatch behavior, or
runtime change. Pantheon's own dispatch/session journal (`brain.jsonl`) is
unrelated and untouched by it.

## Lifecycle setup

[Installed sources and local setup](docs/guides/lexicon-lifecycle.md) describe the
Lexicon catalog revision and host rendering. The Tasks section above and
`docs/github-issues.md` take precedence over the generic lifecycle block below
for tracker authority, activation, and pickup. Lifecycle adoption grants no board
mutation or execution authority beyond the owner's existing authorization.

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
