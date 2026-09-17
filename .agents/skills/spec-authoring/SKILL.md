---
name: spec-authoring
description: Write a specification under docs/specs/ in the Repository Documentation Standard's shape, with stable FR identifiers, milestones that ladder, and a design document only where a design argument exists. Use when a change needs writing up before it is built, when someone asks for a spec, SPEC-NNNN, design.md, functional requirements, acceptance criteria or a milestone ladder, or when deciding whether a change needs a specification at all.
---

# Spec Authoring

Produce `docs/specs/NNNN-slug/spec.md` in the shape the Repository Documentation Standard
prescribes: what should exist and why, with requirements a reader can cite from an issue.

The specification is not the deliverable. The thinking is. A document that restates a decision
already made, in more words, has cost the reader time and bought nothing.

## Do not run this skill when

The standard lists the changes that do not earn one: small bug fixes, dependency updates, minor
refactors, simple cleanup, tiny UI changes, spelling or documentation corrections, and obvious
maintenance. Say in one sentence that you are skipping it and why, then do the work.

## 1. Decide whether a specification is required

Write one when the change does any of these:

- introduces meaningful user-visible behavior
- modifies an external API or contract
- affects multiple components
- changes persistence or data structures significantly
- introduces meaningful security behavior
- requires architectural choices
- contains ambiguous requirements
- is expected to generate multiple implementation issues
- benefits from explicit acceptance criteria

One of these is enough. None of them, and the work goes straight to an issue.

## 2. Allocate the identifier and the directory

Numbering is per repository, zero-padded, allocated at commit time rather than reserved. Read
`docs/specs/` for the highest number in use and take the next.

```text
docs/specs/0012-authentication/spec.md   ->   id: SPEC-0012
```

The directory slug may change later. The identifier never does, which is why issues, tests and
other documents cite the identifier rather than the path.

Check whether the repository is an umbrella. A specification that places an obligation on more
than one repository belongs in the umbrella's `docs/specs/`, not in any one member's, and its
requirements route outward to issues in the repositories that implement them.

## 3. Write the frontmatter

Five fields are required. See [frontmatter.md](references/frontmatter.md) for the optional fields
worth carrying and the ones that only look useful.

```yaml
---
id: SPEC-0012
title: Authentication
type: spec
status: draft
created: 2026-09-16
---
```

`status` comes from the specification vocabulary and nowhere else: `draft`, `accepted`,
`implementing`, `implemented`, `superseded`, `abandoned`. Do not invent a value, and do not write
a prose `Status:` line as well. Two sources of truth for one fact is the failure the frontmatter
exists to prevent.

## 4. Write the body in this order

Use [spec-template.md](references/spec-template.md) as the starting point. The sections and their
names are the standard's:

`Problem`, `Goals`, `Non-goals`, `User Stories`, `Functional Requirements`, `Acceptance Criteria`,
`Constraints`, `Out of Scope`.

Three rules decide whether the document is worth reading:

- **Requirements carry stable identifiers.** `FR-001`, `FR-002`, one requirement each, each with a
  MUST, MUST NOT or MAY that a reader can fail. An identifier exists so an issue, a pull request, a
  test or a verification document can cite it, so one identifier per unit of routable work.
- **Acceptance criteria are observable.** A condition someone can check without asking the author
  what was meant. "The checker reports zero failures" is one; "authentication is robust" is not.
- **Avoid implementation minutiae.** Framework, library and database choices belong in the design
  or in an ADR, unless they are genuine project constraints, in which case say why.

Write `Out of Scope` even when it feels obvious. An unstated exclusion reads as an oversight, and
the reviewer who spots the gap cannot tell which it was.

## 5. Decide whether a design document is warranted

`spec.md` says what and why. `design.md` says how. Not every specification needs the second, and a
small obvious change stays entirely in the first.

Where a design is needed, use [design-template.md](references/design-template.md). Its sections are
also fixed by the standard, and it is a child document: it carries `type: design` and a `spec:`
naming its parent, and it carries no `id` and no `status` of its own. Its lifecycle is its parent's.
The same is true of `research.md` and `verification.md`.

**A `design/` directory instead of `design.md`, never both.** Permitted only when the design
genuinely covers several subjects, one file per subject, each a child on the same terms. A long
argument about one subject is one `design.md`. Carrying both means neither is the design.

## 6. Add milestones only when the work ladders

`status` says where the document is in its lifecycle. Milestones say how the work is staged. They
are different axes, and a specification delivered in one piece needs only the acceptance criteria.

Where it does ladder, milestones live inside `spec.md`, never as separate files and never as
`SPEC-0012a`:

```markdown
## Milestones

### v0

What ships first, and why it is independently valuable on its own.

Promotion bar: the observable conditions that must hold before v1 starts.
```

Every milestone names what it delivers and the bar cleared before the next begins. A milestone with
no promotion bar is a heading.

**Identifiers are earned, not assigned.** Plain `v0` and `v1` are enough until something outside the
document needs to point at one milestone, at which point they become `M1`, `M2` and are cited as
`SPEC-0012 / M2 / FR-007`. Numbering what nobody cites is the cheapest ceremony to add and the
hardest to remove.

Check the ladder against the criteria before finishing: if every acceptance criterion can hold while
a requirement is still open, either the criteria are incomplete or that requirement belongs to a
later specification.

## 7. Route the requirements and register the document

Every requirement gets an issue in the repository that implements it, and the issue names the
specification and the requirement identifier. Where requirements route to more than one repository,
keep the routing table inside the specification: one row per requirement and repository, because
that table is the only place the cross-repository dependency is visible.

Add a row to `docs/specs/README.md` with the identifier, the status and the subject. An index that
does not list a specification is an index a reader cannot trust.

## 8. Check before proposing

- Every required section present, named as the standard names it.
- `status` from the specification vocabulary, and no prose `Status:` line.
- Every `FR-` unique, citable and failable.
- Every acceptance criterion observable, and between them they cover every requirement.
- `design.md` or `design/`, never both; children carry `spec:` and no `id` or `status`.
- Every milestone has a promotion bar.
- `docs/specs/README.md` updated.

## Reference Files

- [spec-template.md](references/spec-template.md) the `spec.md` skeleton with each section's job.
- [design-template.md](references/design-template.md) the `design.md` skeleton and the `design/`
  directory rule.
- [frontmatter.md](references/frontmatter.md) required and optional fields, the status vocabulary
  and its transitions, and how child documents inherit identity.
