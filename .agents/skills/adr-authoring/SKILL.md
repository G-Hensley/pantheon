---
name: adr-authoring
description: Record a durable decision as an Architecture Decision Record under docs/decisions/, with a Confirmation section, reciprocal supersession, and an updated index. Use when a choice should survive the task that caused it, when someone asks for an ADR, a decision record, ADR-NNNN, or to reverse, supersede or deprecate an earlier decision, or when a design or threat model surfaces a choice worth recording.
---

# ADR Authoring

Produce `docs/decisions/NNNN-slug.md`: one decision per file, recording what was decided, why, and
how anyone would know it is being followed.

The standard uses one term for these, ADR, and one location. An accepted ADR's decision and
reasoning are immutable, so getting the shape right at writing time is cheaper than it looks.

## Do not run this skill when

- The choice is an implementation detail with no consequence past the change that makes it.
- The decision is already recorded in an accepted specification and nothing about it is
  architecturally significant on its own.
- Somebody wants a record of what happened rather than what was decided. That is a dated record
  and belongs in `docs/archive/`.

An ADR for every choice makes the log unreadable, and an unreadable log is not consulted.

## 1. Confirm it is a decision worth recording

Write one for a decision that should survive beyond the feature or task that caused it:

- selecting one technology over another
- choosing an architectural pattern
- selecting an authentication or authorization model
- defining an important trust boundary
- adopting an inter-service communication pattern
- knowingly accepting a durable, architecturally significant risk

The test that catches the rest: could somebody violate this decision without noticing? If not, it
is not constraining anything, and a record adds ceremony without adding a constraint.

## 2. Allocate the identifier

Per repository, zero-padded, allocated at commit time rather than reserved in advance. Two sessions
adding a decision at once collide; that is a rename before merge, which is cheap while the document
is new.

```text
docs/decisions/0007-use-redis-for-session-storage.md   ->   id: ADR-0007
```

The filename says what was decided, not what was considered. `0007-session-storage.md` makes a
reader open the file to learn the answer.

## 3. Write the frontmatter

```yaml
---
id: ADR-0007
title: Use Redis for session storage
type: adr
status: accepted
created: 2026-09-16
decision_makers:
  - Gavin
governs: []
supersedes: null
superseded_by: null
---
```

`type` is `adr`. Not `decision`: that value is outside the standard's closed type set, and a
schema will reject it later in a way nobody can explain from the file itself.

The ADR status vocabulary is `proposed`, `accepted`, `superseded`, `deprecated`, `rejected`.

Two fields become obligations at `accepted`, and they are the reason an ADR is more than a note:

- **`decision_makers`.** An ADR records that a decision was made; without this it does not record
  who made it, and that difference matters at exactly the moment somebody asks whether the right
  person decided.
- **`governs`.** What the decision binds, with `[]` permitted and meaning considered and none. An
  empty value is readable; an absent field is not, which is what makes "which accepted decision has
  no architectural representation" answerable at all.

At `superseded`, `superseded_by` is required and the new record's `supersedes` points back.

## 4. Write the body

Use [adr-template.md](references/adr-template.md). The sections that carry the weight:

**Context.** The forces at the time: what was true, what was constrained, what was unknown. Written
so that a reader two years later can tell whether those forces still hold. This is the part that
makes a reversal legible rather than arbitrary.

**Decision.** One decision, stated in a sentence before it is elaborated.

**Consequences.** What this makes easy, what it makes hard, and what it forecloses. An ADR with
only positive consequences was written to justify a choice rather than to record one.

**Confirmation.** How anyone would know the decision is being followed.

## 5. Write the Confirmation section, or say there is none

This is the section most often missing and the one that decides whether the decision is real.
Three honest answers, and only the third is a problem:

- A check exists and the ADR names it. This turns the decision into a constraint something
  enforces.

  ```markdown
  ## Confirmation

  Session storage is asserted in `tests/integration/test_session_backend.py`, which fails if any
  backend other than Redis is configured.
  ```

- No check exists and the ADR says so plainly. That records a decision resting on people
  remembering, which is a known risk rather than a hidden one.
- The section is absent. Then nobody can tell which of the two it was.

A decision whose Confirmation cannot be written at all deserves a second look before acceptance: it
usually means the decision is not specific enough to be violated.

## 6. Point at the living document

An accepted ADR is a historical record, not a description of the current system, and it is frozen.
So every accepted ADR that changed how something behaves names the living document that describes
the behavior now: a specification, a design, an architecture document or a reference page.

Without that pointer, the only readable account of current behavior is a document nobody is
allowed to update, which is how a correct ADR becomes a misleading one without a word of it
changing.

## 7. Reverse a decision by appending, never by editing

An accepted ADR's decision and reasoning are immutable. Its value is that it records what was
believed and why at the time, so editing either to match a later decision destroys the only thing
it was keeping. Its lifecycle metadata is not immutable, because supersession has to be recordable.

[supersession.md](references/supersession.md) has the five-step procedure and the line between an
edit and a supersession. The short version: a typo or a status field is an edit, and changing what
was decided or the reasoning given is a supersession, always.

## 8. Update the index

`docs/decisions/README.md` exists so a reader can learn the current position on a question without
opening every file and reconstructing the order:

```markdown
| ID | Decision | Status | Decided by | Superseded by |
| --- | --- | --- | --- | --- |
| ADR-0007 | Use Redis for session storage | superseded | Gavin | ADR-0019 |
| ADR-0019 | Use Postgres for session storage | accepted | Gavin | |
```

Superseded rows stay. Removing them turns the log into a snapshot, and the question a reader
usually arrives with is what was decided before and why it changed.

## 9. Check before proposing

- `type: adr`, and `status` from the ADR vocabulary.
- At `accepted`: `decision_makers` named, `governs` present even if empty.
- Confirmation section present, naming a check or saying plainly there is none.
- A pointer to the living document where the decision changed behavior.
- Supersession reciprocal in both records, and the superseded record's reasoning untouched.
- `docs/decisions/README.md` updated, superseded rows kept.

## Reference Files

- [adr-template.md](references/adr-template.md) the record skeleton with each section's job.
- [supersession.md](references/supersession.md) the reversal procedure, the edit-versus-supersede
  line, and how an accepted risk from a threat model becomes an ADR.
