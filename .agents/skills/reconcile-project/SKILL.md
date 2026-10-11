---
name: reconcile-project
description: Reconcile project plans, documentation, branches, code, tests, PRs, and issues into an evidence-linked account of remaining work. Use when asked what is left in a project, whether a feature is built or shipped, or whether planning records still match the implementation. Also produces the Reckon inputs block (board counts, flow figures, merged and closed work, per-criterion hold state, staleness flags) a session gathers before writing a Reckon record; use when a session is closing an increment and needs those supporting facts.
---

# Reconcile Project

Answer "what is left" by comparing sources against the claims each can support.
Report implementation work and stale records separately so a documentation repair
does not become an instruction to rebuild a completed feature.

## Workflow

### 1. Establish scope and intended completion

Read the project's instructions and identify the integration branch, inspected
revision, and relevant acceptance criteria. Derive claims from the user's request
and the project's planning sources; do not invent features. Record available
branches and sources, and identify any unavailable remote, release, or runtime
evidence. Do not fetch, switch shared checkouts, or mutate trackers implicitly.

### 2. Assign authority for each kind of claim

Use the source that can establish the particular fact:

- **Intent and acceptance:** the user's accepted scope and governing project
  requirements define the behavior and what counts as complete. Plans and design
  documents explain intent; resolve conflicting requirements explicitly.
- **Implementation at a revision:** inspect relevant code and its limitations.
  Tests describe checks; their presence alone does not establish a passing run.
  Report observed test commands, revision, results, and coverage limits. Neither
  a narrow test nor code presence proves every acceptance criterion works.
- **Integration:** inspect commits, ancestry, and diffs against the intended
  branch. Branch names and closed issues do not establish that code landed.
  PR merge metadata can establish a merge; verify its target and included work.
- **Tracking and review:** issues, Project fields, and PR reviews establish their
  recorded status. They may lag implementation or use different completion rules.
- **Release and operation:** release artifacts and observed runtime evidence
  establish what was released or deployed. A merge or local test cannot do so.

Name the evidence and its authority per claim in the report. Conflicting prose
about implementation can be stale while still defining requirements the code has
not met. Do not rank all sources with a universal "code wins" rule.

### 3. Classify implementation, then record documentation health

For each claim, use these implementation states within the inspected scope:

- **Not built in inspected revisions:** no implementation found after checking
  relevant paths and available branches. Name that search boundary; unqueried
  branches or unavailable sources remain unknown.
- **Built on another branch:** implementation exists outside the integration
  branch. Cite the relevant code and branch diff or ancestry evidence, and state
  validation limits. Remaining work includes review and integration as applicable.
- **Implemented on the integration branch:** code implements the scoped claim.
  State observed validation and any unmet acceptance, release, or runtime checks;
  do not equate implementation with complete delivery.
- **Unknown or partial:** insufficient evidence or incomplete implementation.
  Name the missing evidence or behavior instead of forcing a complete state.

Independently flag **built and undocumented** when implementation exists but
relevant documentation is missing or stale. This can coexist with implementation
completion. A feature need not be rebuilt because a broad design document is
stale; record the documentation repair. Use **complete for the checked scope**
only when that scope's acceptance evidence is available, and name its limits.

### 4. Produce a concise reconciled report

Start with the inspected revision, branch, and evidence limits. Then provide:

1. **Remaining work:** one evidence-linked list of concrete implementation,
   validation, integration, release, and documentation actions. Label each with
   its implementation state and supporting source. Avoid duplicate build work
   for completed functionality; documentation repairs remain explicit tasks.
2. **Staleness and contradictions:** cite both conflicting sources, explain which
   establishes the disputed fact and why, and distinguish stale status from an
   unresolved requirement. Cross-reference repairs already in the work list.

Briefly identify completed claims when useful to explain their removal from the
build backlog. Cite paths and lines, revisions, actual test results, and PR or
issue identifiers where available. Unknown evidence stays unknown.

## Reckon inputs

A session closing an increment needs the supporting facts a Reckon record
cites (SPEC-0010 section 3.3). This skill gathers those facts: board counts
per Status column, or "no board" for a repository on none, the interval's
flow figures, PRs merged and issues closed
since the previous record, one `hold` / `not hold` / `unknown` line per
acceptance criterion, and staleness flags. **It stops there.** It does not
write the Reckon record and does not produce any of the eight Reckon
judgment fields (confirmed or disproven assumptions, new constraints, new
risks, opportunities, whether the outcome was achieved, implications, or a
disposition): those are judgment, decided at M3 close on lexicon#140. The
record itself goes in a comment headed `### Reckon record` on the tracking
issue; this skill does not put it there. See
[reckon-inputs.md](references/reckon-inputs.md) for the full recipe.

## Boundaries

This is a read-only reconciliation workflow. It does not implement the work,
rewrite documentation, install host configuration, or update external trackers.
Any such action belongs to a separately authorized step. The Reckon inputs
shape above is read-only in the same way: it reads the board and the API and
writes nothing back, including the Reckon record itself.

Do not use it for a single-diff review or GitHub closure eligibility alone. For
required-PR and merge-policy checks, use the repository's existing read-only
GitHub tracking checker where available; do not reproduce its logic here.

## Resources

- [reconciliation-examples.md](references/reconciliation-examples.md): a bounded
  example of implementation, release, and stale-record claims.
- [reckon-inputs.md](references/reckon-inputs.md): the full recipe for the
  five Reckon inputs facts and the boundary that stops at them.
