---
title: Factual fleet coordination verification
type: verification
spec: SPEC-0001
created: 2026-10-10
---

# Factual fleet coordination verification

Verified in an isolated checkout, updated to `126ffbe945f683266cc2a376634cf4cff8a791f6`
after the dependency patch in PR #112 merged.
The running Pantheon application, its panes and journal were not used for verification.

## Requirement evidence

| Requirement | Observed evidence |
| --- | --- |
| FR-001 | Disposable-journal tests cover interrupted pane tasks, queued replay suppression, preserved review results, first-load versus same-run project reloads, reused IDs, and explicit review reassignment. Delivery tests exercise available, busy and stopped panes. |
| FR-002 | Tests cover complete findings, over-budget UTF-8 findings, newline normalization, retrieval instructions and retained stored findings. |
| FR-003 | Activity tests cover the quiet threshold, recent MCP contact, unknown observations, clock rollback, blocked and queued tasks, headless exclusion, review attribution and actionable interrupted waits. |
| FR-004 | Component tests cover queued tasks, question ownership, mixed CLI identities, review navigation after worker closure, interrupted results, fleet observations and pending session approvals. |

## Local checks

- Rust tests: 464 unit tests and 2 integration tests passed; 1 measurement harness ignored.
- Frontend tests: 220 tests in 18 files passed.
- `pnpm build`, Rust formatting, Clippy with warnings denied, and `git diff --check` passed.
- `cargo audit` exited successfully with 9 dependency warnings, including unmaintained and unsoundness advisories and a yanked dependency.
- `pnpm audit --audit-level=high` passed after incorporating merged PR #112. The previous failure was the existing `source-map-js` advisory; the patched dependency, frontend tests and production build were verified again. No lockfile changes are included in this fleet increment.

The production frontend build emitted its existing large-chunk warning. No claim of
Windows execution or live desktop acceptance follows from these local Linux checks.

## Visual check

A temporary Vite fixture rendered the real task drawer and fleet overview in cached
Chromium at 1100 by 1600 pixels. Inspection covered blocked, quiet, queued, review and
abandoned tasks, a closed worker, mixed CLI identities, and a pending session request.
The check caught a missing queued-badge color; the badge now shares pending styling.
The fixture is excluded from the change. This is browser component evidence, not
Tauri or native CLI integration evidence.

## Independent review

The implementer was Codex GPT-6 (`/root`). The reviewer was GPT-6 Astra
(`/root/fleet_review`). Review found and rechecked three fixes: recovered-review notice
delivery, worker identity versus navigation target, and interrupted-review wait replies.
The reviewer independently ran 16 component tests and the recovered-review delivery and
interrupted-wait Rust tests, then approved the production diff and final badge styling.

## Remaining acceptance

Exact-commit CI and live desktop acceptance remain open.
A disposable desktop project should exercise restart recovery, reviewer reassignment,
question navigation and session approval navigation before installation into the user's
running application. Native CLI approval detection and automatic reminders remain outside
SPEC-0001; a quiet pane does not identify an approval prompt.

## PR review follow-up

PR #115's five test-quality findings were addressed with two Rust tests and 22
frontend cases. These cover unavailable-reviewer observations and wait replies,
attention indicators, terminal and interrupted ownership exclusions, stopped pane
status, task-scoped quiet notices, recovery-reason precedence, and activity labels
including missing values and unknown states. No production behavior changed.

GPT-6 Astra (`/root/fleet_review`) approved the follow-up and independently ran 58
targeted frontend tests. The implementer reran the full Rust and frontend suites,
production build, and Clippy with warnings denied. All passed; the pre-existing
large-chunk build warning remains.
