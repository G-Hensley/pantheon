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

## 2. Route the current milestone, then decide the grouping

File issues only for the requirements of the specification's **current milestone**: the first
milestone in its `Milestones` section that is not yet delivered. Later milestones are not decomposed
now. Each one goes into the routing table as an outcome with no issue (`not filed; M3 outcome`,
"Filed when M3 starts"), so nothing is unrouted by accident and nothing is on the board before
anyone can start it. A specification with no milestones, or one delivered in one piece, is one
milestone, and step 1 already gave you its requirements.

If it is unclear which milestone is current, say which you took and why, rather than filing two.
A requirement that spans milestones is filed for the part the current milestone needs, and the
routing table records the rest as a later outcome. See
[milestone-routing.md](references/milestone-routing.md) for a worked routing table.

Within the current milestone, one issue per requirement is the default, and it is right whenever a
requirement is a unit of work somebody would sit down and do.

Two adjustments, both earned rather than assumed:

- **Split a requirement** that routes to more than one repository. One issue per repository, each
  naming the same `FR-`.
- **Group several requirements** only when they are genuinely one change: the same files, the same
  review, and no useful state where one is done and the others are not. Record which requirements
  the issue covers so the trace survives.

Do not group to reduce the issue count. An issue covering four requirements can be half done for a
month and still look like one open item.

## 3. Write the issue body in the receiving repository's contract

The repository that receives the issue decides its shape; this skill's default does not. Before
writing, read that repository's `AGENTS.md` and any contributing or issue guide it points to, and
look in `.github/ISSUE_TEMPLATE/`. If they name a body contract or a checker, that contract is the
body and that checker is a step of this skill, not an optional extra.

- **A contract exists:** write its sections, in its words, and write the body to a file. Run the
  checker on the file and continue only on exit 0. A failure means the body is wrong, so fix the
  body; do not skip the checker, and do not edit the checker to pass. In this repository the
  contract is the seven sections `integration-library/github-tracking/work_item_check.py`
  enforces, and `docs/guides/issue-workflow.md` names it. See
  [repository-contract.md](references/repository-contract.md) for finding a contract, mapping
  the finished condition and traceability into it, and the commands.
- **No contract:** use the five-section body in
  [contractless-body.md](references/contractless-body.md). It is the default only where the
  repository states nothing.

Never write both. An issue carrying the five-section block and the repository's own sections
answers the same questions twice, and the two copies drift.

The section that decides whether the issue is workable is the finished condition, whatever the
contract calls it. Write what will be true, observably, when this can be closed. "Implement
session expiry" is a title; "sessions issued before the change expire on the next request, proven
by the test named below" is a finished condition. [issue-body.md](references/issue-body.md) has
the test for one.

Name the guarded parts explicitly where the work touches an instruction file, permissions,
credentials, a webhook or security tooling, so whoever picks it up knows before they start rather
than halfway through.

Mark every finished-condition or validation step the implementing agent cannot perform as an
`owner step`: attaching images to an issue or pull request, a publish, push or remote change the
host refuses, or a physical or credentialed action. Say what the agent prepares (a file at a
stated path, or the exact command) and what the owner does. Found at planning time it is one
named handover; found mid-run it stalls the work. See [owner-steps.md](references/owner-steps.md)
for the marker and the known limits to check each step against.

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

A specification reaches `implemented` when its routed issues are closed and no milestone is left
as an outcome with no issue. For a cross-repository specification that is the one case where a
document's status depends on state outside its own repository, so the routing table is what makes
that checkable.

## 8. Check before filing

- Issues exist only for the current milestone, and every later milestone is a routing-table outcome
  with no issue.
- Within the current milestone, every requirement appears in exactly one issue, or in one issue per
  implementing repository.
- Every issue names the specification and its requirement identifiers.
- Every issue has an observable finished condition, not a restatement of its title.
- Every step an agent cannot perform is marked `owner step`, with what the agent prepares and what
  the owner does.
- Every issue body is in the receiving repository's contract, and that repository's checker was run
  on the body file and exited 0 with no edit made to get it there. With no contract and no checker,
  say so.
- Cross-repository blockers are qualified with the repository name.
- No checklist anywhere mirrors the issue list.
- The specification's routing table lists every issue that was filed.

## Reference Files

- [milestone-routing.md](references/milestone-routing.md) choosing the current milestone and
  writing the routing table for the rest.
- [repository-contract.md](references/repository-contract.md) finding the receiving repository's
  issue contract and checker, and this repository's seven-section body.
- [contractless-body.md](references/contractless-body.md) the five-section body for a repository
  that states no contract.
- [issue-body.md](references/issue-body.md) the finished condition, traceability, and the required
  issue-template set a repository carries.
- [owner-steps.md](references/owner-steps.md) marking steps only the owner can perform, and the
  known host limits behind them.
- [backlog-triage.md](references/backlog-triage.md) retiring a `BACKLOG.md`, `PLAN.md` or
  `tasks.md`, with the grouping rule and the archive rule.
