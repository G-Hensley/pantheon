# Plan template

The decision table and the handoff block are fixed: `scripts/check-plan.py` reads them. Everything
else adapts to the workspace. If the workspace's plans carry frontmatter or a `Status:` line, follow
that convention; the template below assumes a dated plan with a status line.

````markdown
# Repository plan: <repository>

Status: Proposed
Date: <YYYY-MM-DD>

<Two or three sentences: what the repository is for, and that this plan creates nothing.>

## Inputs

| Input | Source read |
| --- | --- |
| Brief | <path, revision or date> |
| Location | <target directory, owner, repository name, visibility> |
| Governance catalog | <path and revision, or "none in this workspace"> |
| Documentation standard | <path and revision, or "none"> |
| Contributor hosts | <hosts, and where the set came from> |
| Stack defaults | <where they are set, or "none"> |

## Governance profile

<The catalog's base profile and capability flags, a reason for each flag set and left out, or why
none fits and the request that records it. "No catalog" if there is none.>

## Decisions

| Item | Outcome | Decision or reason | Enforced by | Revisit when | Source |
| --- | --- | --- | --- | --- | --- |
| `rulesets` | decided | <the rules, the required check names> | ruleset | | <catalog control, if any> |
| `secret-scanning` | decided | <tool, what it catches, where it runs> | server check | | <catalog requirement> |
| `dependency-scanning` | | <tool, what it catches, the lockfiles covered> | | | |
| `sast` | | <tool per language, what it catches> | | | |
| `dast` | | <tool and what it catches, or why nothing is running to probe> | | | |
| `docs-scaffold` | | | | | |
| `agent-host-hooks` | | | | | |
| `local-hooks` | | | | | |
| `server-checks` | | | | | |
| `test-layout` | | | | | |
| `frontmatter` | | | | | |
| `provenance` | | | | | |

## Notes by item

<Only where a row needs more than its cell: the full ruleset, the test tree, the hook list per host.
Use the item name as the heading.>

## Scaffold handoff

```text
target: <directory the repository will be cloned into>
owner: <GitHub account or team, without @>
repository: <name as GitHub spells it>
project-name: <human-readable name>
lifecycle: <yes | no | owner-decides>
```

## Guarded actions this plan requires

None of these has been taken. Each needs the owner's approval.

1. <Create the repository and its remote.>
2. <Apply the ruleset in `rulesets`.>
3. <...>

## Questions for the first specification

<Product decisions this plan depends on and did not make, each with the plan item it affects.>
````

## Cell rules

- **Item** is the name in backticks exactly as in [checklist.md](checklist.md). Each appears once.
- **Outcome** is one of `decided`, `deferred`, `not-needed`. An item awaiting the owner is
  `decided` with the recommendation in the cell and "awaiting owner" said there.
- **Decision or reason** is never empty. For `not-needed` and `deferred` it is the reason.
  For the four scanner items it says what the scanner is expected to catch, not only which tool
  runs; [checklist.md](checklist.md) states the expectation for each.
- **Enforced by** names a layer from [enforcement-layers.md](enforcement-layers.md) for `decided`
  items that enforce something, or `none`.
- **Revisit when** is required for `deferred`: the event that reopens it.
- A literal `|` inside any cell is written `\|`, or the row reads as having an extra column.
- **Source** cites the catalog or standard requirement the row follows, or is empty when the plan
  decided it alone.
