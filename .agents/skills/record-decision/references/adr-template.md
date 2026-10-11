# ADR template

One decision per file. Copy the skeleton, keep the section names, delete the guidance as you fill
it in.

```markdown
---
id: ADR-0007
title: Use Redis for session storage
type: adr
status: accepted
created: 2026-09-16
decision_makers:
  - Gavin
governs:
  - SPEC-0012
supersedes: null
superseded_by: null
---

# ADR-0007: Use Redis for session storage

**Decided 2026-09-16.**

## Context

What was true when this was decided: the requirement that forced a choice, the constraints that
narrowed it, and what was still unknown. Write it so that a reader later can tell whether these
forces still hold, because that is the question a reversal turns on.

Name the document that raised the choice, for instance "SPEC-0012 requires session storage".

## Decision

The decision in one sentence, then the elaboration.

## Options considered

| Option | Why it was not chosen |
| --- | --- |
| Postgres table | ... |
| Signed cookies only | ... |

## Consequences

What this makes easy, what it makes hard, and what it forecloses. Include the cost, not only the
benefit: a record with only positive consequences was written to justify a choice rather than to
record one.

## Confirmation

How anyone would know the decision is being followed. Name the test, the configuration, the code
path or the procedure. Where nothing enforces it, say so plainly.

## Current behavior

The living document describing how this works now: a specification, a design, an architecture
document or a reference page. This record is frozen; that one is not.
```

## The optional MADR fields

`decision_makers` is required at `accepted`. Two more exist and stay optional, both empty lists
where they apply and nobody was involved:

```yaml
decision_makers:
  - Gavin
consulted: []
informed: []
```

## What belongs in an ADR and what does not

**Belongs.** The forces, the decision, the alternatives and their cost, the consequences, and how
compliance is checked.

**Does not belong.** The current behavior of the system, in detail. An ADR is frozen, so anything
written here that later changes becomes wrong and cannot be corrected. Describe it in the living
document and point at it.

**Does not belong.** A decision that binds only one repository, written in an umbrella. Decisions
move with the code they govern. A decision binding two or more repositories to each other belongs
in the umbrella, and the repositories it constrains cite it by identifier rather than restating it.
