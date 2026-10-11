# `design.md` template

`design.md` answers HOW the accepted specification will be implemented. It is a child of its
specification: it carries `type: design` and a `spec:` naming the parent, and it carries no `id`
and no `status` of its own. Its lifecycle is the parent's, so a design whose parent is `accepted`
is accepted.

Not every specification needs one. A small, obvious change stays entirely in `spec.md`.

```markdown
---
title: Authentication Design
type: design
spec: SPEC-0012
---

# Authentication Design

## Overview

The implementation approach in a few paragraphs, enough that a reader can predict the shape of the
change before reading the rest.

## Components Affected

Services, modules, packages or systems that change, and what changes in each.

## Data Model

Data structures and persistence changes. Migrations, and whether they are reversible.

## API Changes

New or modified interfaces. Where a machine-readable contract exists under `contracts/`, name it
rather than describing it twice; the contract is the authority and prose that disagrees with it is
a defect.

## Request / Runtime Flows

The runtime behavior that is not obvious from the component list: ordering, retries, concurrency,
what happens on the second call.

## Security Considerations

Trust boundaries crossed, authorization decisions, secrets handled, input validated. Where this
raises something a threat model should analyse, say so and route it there rather than analysing it
here.

## Failure Modes

What fails, how it is detected, and what the system does about it. Include the failure that is
silent, because that is the one the design decides the cost of.

## Observability

Logging, metrics, tracing, alerting, and what a person debugging this at 2am would need.

## Migration

Backwards compatibility, rollout order, and how to reverse it.

## Testing Strategy

How the implementation is verified, and which requirement each test answers to.

## Alternatives Considered

The reasonable alternatives and why they were not selected. An alternatives section with one entry
is a decision that was never really compared.
```

## `design/` instead of `design.md`

A specification whose design genuinely covers several subjects MAY use a `design/` directory, one
file per subject:

```text
docs/specs/0003-learning-ledger/
├── spec.md
├── research.md
└── design/
    ├── memory-model.md
    └── progression.md
```

Three rules, in order of how often they are broken:

1. **Never both.** A specification carrying `design.md` and `design/` has no design, because
   neither is authoritative.
2. **Every file inside is a child on the same terms.** `type: design`, a `spec:` naming the parent,
   no `id`, no `status`.
3. **The test is subjects, not length.** A long argument about one subject is one `design.md`.
   Splitting one argument across files makes it harder to read, not easier.

This is permitted rather than encouraged. Reach for it when the design was already several
documents because it has several subjects, and collapsing it would produce something nobody reads
to the end.

## `research.md` and `verification.md`

Both are children of the specification on exactly the same terms as `design.md`.

- `research.md` when a decision in the specification rests on findings that need their sources
  recorded. Research with no parent specification is a document in its own right, `type: research`,
  and does not live in a specification directory.
- `verification.md` when the evidence that acceptance criteria hold is worth keeping rather than
  living in a pull request.
