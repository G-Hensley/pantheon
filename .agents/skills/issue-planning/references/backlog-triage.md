# Retiring a backlog

A repository adopting the standard with an existing `BACKLOG.md`, `PLAN.md`, `CONTINUE-HERE.md` or
`tasks.md` triages it into issues rather than keeping it. The file stays until it is empty,
`docs/README.md` records it as a temporary exception while it exists, and it is then deleted.

Expect this to be the most expensive part of adoption. Lexicon's backlog was 35 items, of which 6
already referenced an issue.

## Procedure

1. **Read the whole file before filing anything.** Grouping is the decision that makes this cheap
   or expensive, and it cannot be made one line at a time.
2. **Sort every item into one of four outcomes**, below.
3. **File the issues**, grouped, each with a finished condition.
4. **Move the delivered items** to `docs/archive/` as a dated record, verbatim.
5. **Delete the lines that are gone**, leaving the file smaller after every pass so progress is
   visible.
6. **Record the file in `docs/README.md`** as a temporary exception for as long as it exists, and
   delete both when it empties.
7. **Correct any instruction file** that names the file as where something lives. That edit is
   guarded where the file is host-loaded, so it is shown before it is applied.

## The four outcomes

| The item is | Outcome |
| --- | --- |
| Work accepted in principle and not scheduled | An issue, grouped with related items held on the same condition |
| Already delivered | A dated record in `docs/archive/`. An issue is the wrong shape for something that happened |
| Already an issue | Delete the line. It was a second copy of tracker state |
| A candidate nobody has accepted | An idea under `docs/ideas/`, numbered, with `status: candidate` |

The fourth is the one people skip, and it matters: an idea is a candidate, a backlog item is
accepted work, and collapsing the two is how a backlog becomes a list nobody trusts. An agent must
not implement an idea solely because it appears under `docs/ideas/`; it is promoted into a
specification or an issue first.

## Grouping by coherence

Several related items held on the same condition are one issue that records the condition, not four
issues that each restate it.

```text
- Rate limit the public endpoint
- Add a per-key quota
- Return 429 with a Retry-After
- Document the limits
```

That is one issue about rate limiting with four acceptance criteria, not four issues that will be
worked in one sitting and closed in one pull request. The test is whether any of them is separately
valuable to finish; where none is, they are one issue.

## Where the reasoning goes

The argument for doing the work does not disappear with the file. It lives in the issue body, which
is where it belongs, and in the specification that supersedes the item once the work is large
enough to need one.

Check the instruction files before deleting. Where `AGENTS.md` says reasoning lives in the backlog,
that sentence becomes untrue the moment the file is deleted, and correcting it is part of the same
work rather than a follow-up somebody remembers.

## `docs/plans/` has the same property

The standard names two things expected to stop existing: `docs/plans/` and a `BACKLOG.md`. Plans
differ in one way, so do not triage them the same:

- A **live plan** converts to a specification the next time it is substantively revised. Converting
  a plan nobody is reading is busywork.
- A **finished plan** is a dated record and moves to `docs/archive/`, not converted.
- No new plan is written. New work is a specification.

While the directory exists, `docs/README.md` says which plans are still live, so it is never an
undifferentiated pile.
