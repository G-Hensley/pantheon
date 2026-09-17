---
name: issue-planning
description: Derive GitHub Issues from an accepted specification's requirements, or triage a Markdown backlog into them, with an explicit finished condition and traceability back to the FR identifier. Use when work needs breaking into issues, when someone asks to file, plan or shape issues from a spec, to retire a BACKLOG.md or a tasks.md, or when deciding what belongs in the tracker rather than in a document.
---

# Issue Planning

Turn requirements into tracked work. GitHub Issues are the authoritative source of truth for
executable work; specifications define requirements, designs define strategy, and issues define
what has to be done.

The failure this skill exists to prevent is two authoritative task lists. When a Markdown checklist
and the tracker disagree, the Markdown one loses silently: `tasks.md` incomplete while the issue is
closed.

## Do not run this skill when

- The work is one obvious change with a clear finished condition. File the issue and move on.
- Somebody wants a plan document. A plan is prose; this produces tracker entries.
- The specification is still `draft` and its requirements are moving. Filing issues against
  requirements that change produces issues nobody can close.

## 1. Read the requirements, not the prose

Work comes from the `FR-` identifiers. Each requirement is a unit that routes, so start from the
requirement list and the routing table rather than from the narrative.

Where a specification routes to more than one repository, the issue goes in the repository that
implements it, not in the one that holds the specification.

## 2. Decide the grouping

One issue per requirement is the default, and it is right whenever a requirement is a unit of work
somebody would sit down and do.

Two adjustments, both earned rather than assumed:

- **Split a requirement** that routes to more than one repository. One issue per repository, each
  naming the same `FR-`.
- **Group several requirements** only when they are genuinely one change: the same files, the same
  review, and no useful state where one is done and the others are not. Record which requirements
  the issue covers so the trace survives.

Do not group to reduce the issue count. An issue covering four requirements can be half done for a
month and still look like one open item.

## 3. Write the issue body

Use [issue-body.md](references/issue-body.md). The shape the standard expects:

```markdown
## Specification

SPEC-0012

## Requirements

- FR-002
- FR-004

## Design

docs/specs/0012-authentication/design.md

## Acceptance Criteria

...

## Verification

...
```

The section that decides whether the issue is workable is the finished condition. Write what will
be true, observably, when this can be closed. "Implement session expiry" is a title; "sessions
issued before the change expire on the next request, proven by the test named below" is a finished
condition.

Name the guarded parts explicitly where the work touches an instruction file, permissions,
credentials, a webhook or security tooling, so whoever picks it up knows before they start rather
than halfway through.

## 4. Record dependencies where they exist

An issue blocked on another says so, and says which one. A dependency that lives only in somebody's
head reorders the work the first time two people pick up in parallel.

Where the blocker is in another repository, qualify it: `projects#21`, not `#21`. A bare number
resolves to the wrong repository silently.

## 5. Do not duplicate task state

Once issues exist, the document does not track their progress. No checklist in the specification
mirroring the issue list, no `tasks.md`, no status column that has to be hand-updated.

The specification carries the routing table, which says where each requirement went. The tracker
carries whether it is done. Those are different facts and only the second changes daily.

If a planning checklist was used while shaping the work, it becomes informational once the issues
exist, and says so at the top.

## 6. Triage an existing backlog rather than keeping it

A backlog is not a document kind in this standard. It holds work accepted in principle and not
scheduled, which belongs in the tracker where it can carry an owner, an acceptance condition and a
stage. A Markdown backlog cannot, which is why the ones that exist drift into a list nobody can
tell apart from a list nobody has looked at.

[backlog-triage.md](references/backlog-triage.md) has the procedure. Three rules decide most of it:

- **Group by coherence, not one issue per line.** Several related items held on the same condition
  are one issue recording that condition, not four issues each restating it.
- **An item describing delivered work is a dated record.** It goes to `docs/archive/`, because an
  issue is the wrong shape for something that already happened.
- **The file stays until it is empty**, `docs/README.md` records it as a temporary exception while
  it exists, and it is then deleted. Like `docs/plans/`, it is expected to stop existing.

This is the most expensive part of adopting the standard for a repository that has accumulated
one, and it is worth saying so up front rather than discovering it midway.

## 7. Close the loop back to the specification

Every issue names its specification and its requirement identifiers, and the specification's
routing table names the issue. Both directions, because each answers a different question: the
issue answers "why am I doing this", and the table answers "is anything unrouted".

A specification reaches `implemented` when its routed issues are closed. For a cross-repository
specification that is the one case where a document's status depends on state outside its own
repository, so the routing table is what makes that checkable.

## 8. Check before filing

- Every requirement appears in exactly one issue, or in one issue per implementing repository.
- Every issue names the specification and its requirement identifiers.
- Every issue has an observable finished condition, not a restatement of its title.
- Cross-repository blockers are qualified with the repository name.
- No checklist anywhere mirrors the issue list.
- The specification's routing table lists every issue that was filed.

## Reference Files

- [issue-body.md](references/issue-body.md) the issue body shape, the finished condition, and the
  required issue-template set a repository carries.
- [backlog-triage.md](references/backlog-triage.md) retiring a `BACKLOG.md`, `PLAN.md` or
  `tasks.md`, with the grouping rule and the archive rule.
