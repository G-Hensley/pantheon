# Superseding a decision

An accepted ADR's decision and reasoning are immutable. Reversing a decision is an append, never an
edit.

## The procedure

1. Write a new ADR stating the new decision and why the earlier one no longer holds.
2. Set the old ADR's `status` to `superseded` and its `superseded_by` to the new identifier.
3. Set the new ADR's `supersedes` to the old identifier. The references are reciprocal, so either
   record leads to the other.
4. Leave the old ADR's reasoning exactly as written. It is a dated record of what was true then.
5. Update `docs/decisions/README.md` so the index points at the current decision, keeping the
   superseded row.

Step 3 is the one most often skipped. A one-way pointer means a reader arriving at the new record
cannot tell what it replaced, and the log stops answering the question people bring to it.

## Edit or supersede

| Change | Which |
| --- | --- |
| Fixing a typo or a broken link | Edit |
| Setting `status`, `superseded_by` or `supersedes` | Edit |
| Adding the pointer to the living document | Edit |
| Changing what the ADR decided | Supersession, always |
| Changing the reasoning it gives | Supersession, always |
| Softening a consequence that turned out worse than written | Supersession |

The last row is the tempting one. A consequence that proved wrong is evidence about the decision,
and deleting it removes the record of what was misjudged.

## `deprecated` and `rejected`

Both are in the vocabulary and neither is supersession:

- **`rejected`.** The decision was proposed and not taken. Keep the record: the argument for
  something not done is what stops it being re-proposed every six months with the same reasoning.
- **`deprecated`.** The decision still describes what exists, but the approach is being moved away
  from and no new work should follow it. Where a replacement decision exists, that is a
  supersession instead.

## An accepted risk becoming an ADR

A threat model finding treated as `accepted` records a named risk owner, the residual risk and a
review trigger. Write an ADR as well when the acceptance is durable and architecturally
significant.

Do not write one for every accepted finding. An ADR records that a decision was made and by what
reasoning, and it does not by itself establish that the right person made it. The finding's owner
field is what does that, so an ADR added on top of a well-formed finding adds a record and not a
control.

## Numbering after a supersession

The new record takes the next free number. Do not reuse the superseded record's number, do not
append a letter, and do not renumber anything. The identifier is stable precisely so that an
external citation written a year ago still resolves.
