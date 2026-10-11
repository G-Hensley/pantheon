---
name: plan-repository
description: Plan a repository before it is created, reaching a recorded decision, a reasoned deferral or a reasoned "not needed" on branch protection and rulesets, secret, dependency, static and dynamic scanning, the documentation scaffold, hooks at the agent-host, local-commit and server layers, test layout, frontmatter and artifact provenance, and write that plan as one document while creating nothing else. Use when someone is about to create or asks for a new repository ("plan the repo", "before I create the repository", "what should we decide up front", "set up a repository for this project"), or when a new project's enforcement and layout have not been decided yet.
---

# Plan Repository

Decide, before the first commit, everything that is nearly free now and costly later, and write
the decisions down as one plan that a scaffolding step and the owner can act on. The plan is the
only thing this skill produces.

Every item is cheap before the first commit and expensive after it: branch protection added after
forty commits has forty unreviewed commits behind it, a test layout changed after two hundred tests
is a migration, and provenance cannot be retrofitted onto releases already made.

## Standing rules

These hold for the whole task, not only at the step that introduces them.

- **Create nothing but the plan.** No repository, remote, ruleset, scanner, workflow, hook, host
  configuration or scaffolded file. Those are guarded changes, and each needs the owner's own
  approval. The plan lists them as actions; it does not take them. Do not edit existing documents
  either, beyond one index row pointing at the plan where the workspace keeps a plans index.
- **Plan the repository, not the product.** Language, interfaces, data model, backend choice and
  milestones belong to the project's first specification, written after the repository exists.
  Where a repository decision depends on one of them, record the dependency and the question, and
  decide nothing about the product itself. A plan that grows specifications and decision records is
  the most common way this task goes wrong.
- **Every checklist item gets an outcome.** `decided`, `deferred` or `not-needed`, each with its
  reason. Omission is not an outcome, and neither is "TBD" or "the owner's call" without a
  recommendation. Where the decision genuinely belongs to the owner, recommend one and mark it as
  awaiting the owner.
- **Consume the governance catalog; do not re-decide it.** If the workspace has a repository
  governance policy (profiles, required controls, scanner classes), cite it wherever it already
  mandates something, and decide only where it is silent. A catalog that names a decision as a
  non-goal, open, or reserved for a later approval has not been silent: the plan follows it as
  written and, if this repository needs more, records a request to the catalog's owner. The same
  holds for the documentation standard's assignment of ownership, such as which system validates
  documents. If none of the catalog's profiles fits, say so and record a request to the catalog's
  owner rather than inventing the profile's policy.
- **A required control must be enforced on the server.** Local and agent-host hooks are fast
  feedback. They bind only the contributors who have them, so they never carry a control the plan
  calls required.

## Workflow

### 0. Check the installed Lexicon items are current

Before anything else, run the check in [catalog-freshness.md](references/catalog-freshness.md):
`lexicon update --check` in the project this skill is installed in, then the catalog checkout
against its upstream. When a life-cycle skill (this one, `scaffold-repository`, `shape-ideas`,
`write-spec`, `issue-planning` or `reconcile-project`) is behind the catalog, stop: name the
outdated items and the command that updates them, and write nothing until the owner has updated
or said to proceed on the installed versions. When neither the CLI nor the install manifest is
available, say so and continue. A stale copy of this skill plans against a checklist the catalog
has since corrected, and the Life Cycle opt-in cannot be offered again once the repository exists.

### 1. Gather the inputs

Read, and cite in the plan's Inputs section with the revision or date read:

- **The brief**: what the project is, who uses it, how it is distributed.
- **Where the repository will live**: target directory, owner, repository name, visibility.
- **The governance catalog**, if any, for example an auditing project's specifications. Read its
  current text as an input; it may be a draft, so do not copy it into the plan.
- **The documentation standard**, if the workspace has one, and its scaffolding skill's inputs.
- **The contributor host set**: which people contribute and which agent hosts and editors they use.
  Take it from the owner or the workspace's instruction files. A catalog's list of supported hosts
  is what tooling can generate for, not who contributes; a contributor host outside it is a
  legitimate plan item, not an error.
- **Stack defaults**, if the workspace sets them. Cite them; choosing the stack is not this task.

Ask only for an input you cannot read from the workspace. Otherwise make the reasonable choice and
record it as a decision awaiting the owner.

### 2. Walk the checklist

Read [checklist.md](references/checklist.md) and answer all twelve items in its order. It says, per
item, what the decision is, what a `not-needed` has to show, and the answers that look complete
and are not. Read [enforcement-layers.md](references/enforcement-layers.md) before answering the
three hook items and branch protection; they are separate interfaces with different enforcement
properties, and a plan that treats them as interchangeable leaves most of its controls advisory.

For each of the four scanner items, the row says what that scanner is expected to catch, in this
repository's terms, as well as the tool and where it runs. A tool name records a choice; the
expectation is what a later reviewer checks the scanner's findings against.

### 3. Place the governance profile

Name the catalog's base profile and capability flags that describe this repository, with a reason
for each flag set and each flag left out. For every scanner class or control the catalog makes
mandatory for this shape of repository, cite the requirement in the item's row instead of deciding
it again. With no catalog in the workspace, say so in Inputs and decide every item on its merits.

### 4. Write the plan

Use [plan-template.md](references/plan-template.md). It fixes the decision table's item names and
columns and the handoff block, which is what makes the plan checkable.

Put the plan where the workspace keeps plans, following its naming and status conventions. With
no convention, use `docs/plans/<YYYY-MM-DD>-<repository>-repository-plan.md` in the workspace that
will hold the repository. One file.

Close with two lists: the guarded actions the plan requires, each for the owner to approve and
none taken; and the questions the project's first specification must answer.

### 5. Hand off to scaffolding

The plan's handoff block carries exactly the inputs the scaffolding step takes: target directory,
owner, repository name, project name, and whether the project adopts the AI Development Life Cycle
(`yes`, `no`, or `owner-decides`). With the `scaffold-repository` skill available, those are its
inputs; the rest of the plan decides what the repository enforces, which that skill does not write.

The Life Cycle is Lexicon's operating model for AI-built projects, specified in Lexicon's SPEC-0010
(https://github.com/G-Hensley/lexicon/blob/main/docs/specs/0010-ai-development-life-cycle/spec.md).
On `yes`, `scaffold-repository` appends its Life Cycle section to the new `AGENTS.md`. Adopting it is
the owner's decision: write `yes` or `no` only when the owner has said so, and `owner-decides`
otherwise. Do not infer it from a similarly named process in the workspace.

### 6. Check the plan and the workspace

Run the checker that ships with this skill, from the skill's directory:

```sh
python3 scripts/check-plan.py path/to/plan.md
```

It fails on a missing, duplicated or unknown item, an outcome outside the three, a `deferred` or
`not-needed` with no reason, a `deferred` with no revisit trigger, and an incomplete handoff block.
Show its output. It proves the plan is complete in shape, never that a decision is right.

Then confirm the workspace changed only by the plan and at most one index row (`git status` where
the workspace is a Git repository). Undo anything else this run wrote before reporting.

### 7. Report

Give the plan's path, the checker's output, the items awaiting the owner, and the guarded actions
listed for approval. Do not describe the repository as created, protected or scanned: none of that
has happened.

## Reference Files

- [catalog-freshness.md](references/catalog-freshness.md) the step 0 check: which items are
  life-cycle skills, the commands, the fallback without the CLI, and what to report on a stop.
- [checklist.md](references/checklist.md) the twelve items: the decision each needs, what counts as
  `not-needed`, and the answers that look complete and are not.
- [enforcement-layers.md](references/enforcement-layers.md) agent-host hooks, local commit hooks and
  server-side checks: whom each binds and how each is bypassed.
- [plan-template.md](references/plan-template.md) the plan's shape: Inputs, the decision table, the
  per-item notes, the handoff block, guarded actions and open questions.
- `scripts/check-plan.py` the completeness check for a written plan; standard library only.
