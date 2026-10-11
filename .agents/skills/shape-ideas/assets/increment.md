### <Increment name>

A section of the shaping record, not a separate specification. For Direct Build,
this is the only increment; do not invent later rungs.

**Appetite:** <number> agent sessions, optional <$amount> cost ceiling.
**Strategy:** <the record's strategy>; <why this cut serves it>.
**Fit:** <how the bounded work fits; if it does not, name the smaller-cut decision
still needed, preserving required behavior until the owner agrees a change>.

#### Outcome hypothesis

What someone gains if only this increment lands. For an experiment, state the
question answered and the decision it enables, not a fictional shipped feature.

#### Scope and exclusions

The outcome included, and what a reasonable person might expect but will not get.
Later candidates stay hypotheses rather than accepted requirements.

#### Definition of good (hypothesis)

These five parts become acceptance criteria only when `write-spec` promotes
them; this record does not assert requirements or completed verification.

- **Outcome:** <the concrete benefit or decision-relevant knowledge>.
- **Evidence:** <reproducible observation, named fixtures or measurement; for an
  experiment include the threshold and what either result changes>.
- **Quality bar:** <only relevant dimensions, each with a specific threshold>.
- **Out of scope:** <explicit exclusions, consistent with the scope above>.
- **Reversibility:** <rollback or disposal path, migration cost, one-way doors>.

#### Dependencies and assumptions

What must exist, which assumptions could invalidate the cut, and what remains
manual or deferred. Flag any proposed scope reduction awaiting agreement.
