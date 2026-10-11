# Routing only the current milestone

A specification with milestones is delivered in order. Issues are filed for the milestone being
started, and the routing table records the rest so they are neither lost nor already on a board.

## Which milestone is current

The first milestone in the specification's `Milestones` section that is not yet delivered. Delivery
is decided by the milestone's own promotion bar or exit statement and by the routing table's
states; a milestone whose issues are all closed is delivered. Where that leaves two candidates, or
the specification says milestones may run in parallel, take the one the request names; if none is
named, say which one you took and why, and file for that one only.

A specification with no `Milestones` section, or one the text says is delivered in one piece, is one
milestone.

## What the routing table says afterwards

Current-milestone requirements carry their issue. Every later milestone's requirements carry an
outcome with no issue, and the trigger for filing it.

Example, a specification for a static-site image pipeline with three milestones, M1 current:

| Requirement | Issue | State |
| --- | --- | --- |
| FR-001, FR-002 (M1: resize on build) | pipeline#14, pipeline#15 | Open |
| FR-003 (M2: responsive formats) | not filed; M2 outcome | Filed when M2 starts |
| FR-004, FR-005 (M3: cache purge on publish) | not filed; M3 outcome | Filed when M3 starts |

The later rows keep their requirement identifiers, so nothing is unrouted; they name no issue
number, so nothing is on the board before it can be started. When M1 is delivered, M2's row is
replaced by its issues, and the same rule applies to M3 after it.

## Requirements that span milestones

If one requirement is met in stages, file the issue for the stage the current milestone needs,
naming the same `FR-`, and add a later-outcome row for the remainder. Do not file the remainder
"for later": it is the next milestone's outcome, and it is filed when that milestone starts.
