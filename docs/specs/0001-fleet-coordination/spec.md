---
id: SPEC-0001
title: Factual fleet coordination
type: spec
status: implementing
created: 2026-10-10
---

# Factual fleet coordination

The first fleet-coordination increment makes interrupted ownership, quiet tasks,
review feedback and known waits visible. The user authorized this work and confirmed
no other Pantheon development sessions were active on 2026-10-10.

## Problem

A restored pane can inherit a task its new process never received (#108). Quiet
workers can hold the dispatch queue indefinitely (#100). Rework findings are
silently shortened before reaching the worker (#109). The task drawer omits queued
work and describes conductor questions as waiting on the human.

## Goals

1. A new pane process does not inherit work from the previous app run.
2. Humans and agents can identify open work and its responsible actor.
3. Every shortened feedback notice provides access to the complete findings.

## Non-goals

Quiet time does not establish completion, failure, or a native CLI approval wait.
This increment does not automatically type reminders into an unknown terminal
state: that could answer an approval prompt. The safe-reminder part of #100 remains
open until there is evidence that a CLI is ready to receive a prompt.

## User Stories

As the human coordinating several panes, I need to see which tasks are active,
queued, awaiting a conductor answer, or waiting for review, and reach the pane
responsible for the next action. As a conductor, I need to distinguish an old
assignment interrupted by restart from a quiet task that is still open.

## Functional Requirements

### FR-001

On first loading a project's journal in an app run, Pantheon MUST settle prior
pane assignments as interrupted, including queued assignments, without replaying
them into restored IDs. Submitted reviews MUST remain available, with their old
reviewer assignment suspended until explicit reassignment. Results and history
MUST survive. Reloading a project already visited in this run MUST preserve live
work. Existing headless restart policy is unchanged.

### FR-002

A rework notice MUST deliver complete findings when its sanitized text fits the
pane budget. Otherwise it MUST explicitly report truncation and name the exact
`get_task_result` task ID before the worker acts on the findings. Stored findings
MUST remain complete.

### FR-003

Roster and task observations MUST name the actor and task, and expose a quiet
warning after five minutes without observed terminal output or agent MCP activity.
Unknown observations MUST remain unknown. Quiet warnings MUST NOT change task
status, resolve work, or inject terminal input. Wait responses MUST surface these
observations for requested tasks.

### FR-004

The task view MUST retain all queued work, show the responsible conductor for a
question, focus the reviewer for active review work, and display interruption reasons
and retained results. An interrupted review with a suspended reviewer assignment
MUST focus its responsible conductor for reassignment. It MUST provide a fleet overview and a route to pending Pantheon
session approvals. Native CLI approval state MUST NOT be inferred from these facts.

## Acceptance Criteria

- Disposable-journal tests reload open tasks under reused pane IDs, preserve
  completed work and review submissions, suppress old deliveries, permit explicit
  reassignment, and prove repeat loads and project round-trips are idempotent.
- Feedback tests cover complete short and medium findings, over-budget UTF-8 text,
  CR/LF sanitization, exact retrieval IDs, and unchanged stored findings.
- Activity tests cover the five-minute boundary, recent MCP activity, unknown
  output observations, headless exclusion, and worker versus reviewer attribution.
- UI tests show queued work, correct question ownership, review navigation after
  target closure, interrupted results, fleet observations, and pending approvals.
- Rust tests, frontend tests and production frontend build pass. Live desktop
  acceptance is recorded separately and is not implied by mocked UI tests.

## Constraints

Use additive journal fields with defaults for older records. Preserve existing
task access rules, brain isolation, dispatch budget and Stop behavior. Verification
must not use the running application's journal, panes, or real dispatch allowance.

## Out of Scope

Native host approval hooks, automatic reminders, automatic redispatch, model
switching, context reset, and tracker activation (#70) are separate work. This
proposal does not install or restart Pantheon.

## Routing

| Requirement | Issue |
| --- | --- |
| FR-001 | [#108](https://github.com/G-Hensley/pantheon/issues/108) |
| FR-002 | [#109](https://github.com/G-Hensley/pantheon/issues/109) |
| FR-003 | [#114](https://github.com/G-Hensley/pantheon/issues/114), visibility portion of [#100](https://github.com/G-Hensley/pantheon/issues/100) |
| FR-004 | [#114](https://github.com/G-Hensley/pantheon/issues/114) |
