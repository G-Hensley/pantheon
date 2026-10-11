# `spec.md` template

Copy the skeleton, keep the section names, delete the guidance under each heading as you fill it.
Every section is required. A section with nothing to say says so in one sentence rather than being
removed, because an absent section and an empty one look identical to a reader.

```markdown
---
id: SPEC-0012
title: Authentication
type: spec
status: draft
created: 2026-09-16
owners:
  - Gavin
---

# Authentication

One paragraph: what this document is, what it replaces, and what it does not cover. A reader who
stops here should know whether to keep reading.

## Problem

What is wrong now, stated as observable facts rather than as the absence of the solution. "There is
no rate limiting" is the solution wearing a problem's clothes; "one client made 40,000 requests in
an hour and the queue never drained" is the problem.

## Goals

What is true when this is done. Numbered, so a reviewer can say which one is not met.

## Non-goals

What this deliberately does not solve, and why. This is where a reader's first objection is
answered before they raise it.

## User Stories

Who needs the behavior and why. Written as the situation somebody is in, not as a feature request.
Where the users are agents rather than people, say so; the shape is the same.

## Functional Requirements

### FR-001

Users MUST be able to authenticate using ...

### FR-002

Sessions MUST expire after ...

## Acceptance Criteria

Observable conditions that must all hold for the specification to be satisfied. Each one checkable
without asking the author what was meant. Between them they cover every requirement above.

## Constraints

Hard requirements and restrictions: guarded areas, compatibility that cannot break, a platform
limit, an ordering that has to be respected. Not preferences.

## Out of Scope

Related work explicitly excluded, with a reason each, and where it lives instead.
```

## Optional sections, and when they earn their place

- **Milestones.** Only when the work ships in stages. One heading per milestone, each with a
  promotion bar. See the main skill file.
- **Routing.** Required when requirements route to more than one repository. One row per
  requirement and repository, with the issue and its state.
- **Appendix.** For an inventory or a table too large to sit inline. An appendix that says
  "pending" makes every requirement depending on it unverifiable, so either write it or move the
  requirement it supports to a later milestone.

## The three failures worth checking for

**A requirement that cannot be failed.** "The system MUST be performant" cannot be violated, so it
constrains nothing. Rewrite it with a threshold, or delete it.

**Acceptance criteria that skip a requirement.** Walk the requirement list against the criteria
before proposing. A requirement no criterion covers can be quietly abandoned without the document
noticing.

**A goal that is really a requirement.** Goals describe the state afterwards. Requirements describe
what something must do. If a goal has a MUST in it, it belongs below.
