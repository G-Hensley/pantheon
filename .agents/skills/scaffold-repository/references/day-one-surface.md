# The day-one surface

The exact file set this skill writes, what it leaves out, and every placeholder it substitutes.

Source: `docs/templates/repo-docs-template/skeleton/` in `G-Hensley/projects`, verified at
revision `3c20137` (2026-09-16). That revision was on the branch `docs/adopt-documentation-standard`
and had not reached `main`. If the skeleton has changed shape since, `copy-skeleton.sh` names the
files it expected and could not find, and exits without writing.

## What is written

| Path | Why it is day one |
| --- | --- |
| `README.md` | The human entry point. Every repository has one from the first commit. |
| `AGENTS.md` | The cross-agent working agreement. Its absence is what left three projects in this tree unreadable to agents. |
| `CLAUDE.md` | Exactly `@AGENTS.md`, so Claude Code reaches the same file and no second copy exists. |
| `.github/CODEOWNERS` | Review routing for the instruction files and for itself. Ownership that arrives later arrives after something has already been merged unreviewed. |
| `.github/pull_request_template.md` | The proposal shape, including the risk tick and the provenance block. |
| `.github/ISSUE_TEMPLATE/work-item.yml` | Required. A unit of work with an explicit finished condition. |
| `.github/ISSUE_TEMPLATE/bug.yml` | Required. A specific observed defect. |
| `.github/ISSUE_TEMPLATE/security.yml` | Required. Security work, and the first thing it says is that a vulnerability goes to an advisory instead. |
| `.github/ISSUE_TEMPLATE/config.yml` | Required. Holds `blank_issues_enabled` and the advisory contact link. Its absence is a decision made by default. |
| `docs/README.md` | The documentation map. A reader who cannot find something from it has found a defect in it. |
| `docs/specs/README.md` | The specification index and its lifecycle rules. |
| `docs/decisions/README.md` | The ADR index. |
| `docs/ideas/README.md` | Candidates, and the rule that an idea is not implemented because it is here. |
| `docs/archive/README.md` | Dated records, and the rule that nothing here is current instruction. |
| `docs/specs/0000-template/spec.md` | The shape a first specification is copied from. |
| `docs/specs/0000-template/design.md` | The shape a first design is copied from. |
| `docs/decisions/0000-template.md` | The shape a first decision record is copied from. |

The three `0000-template` files are written although they are templates rather than content,
because `docs/specs/README.md` and `docs/decisions/README.md` name them by path and tell the reader
to copy them. Landing an index that points at a file which is not there is a defect in the index.
Both indexes also say to delete the template once the corresponding authoring skill is installed,
so the removal has an owner and a trigger.

## What is deliberately not written

The standard's rule is that directories establish where a kind of knowledge belongs and a file is
created when useful information exists for it. So none of these are scaffolded:

- `CONTRIBUTING.md`, optional for a solo or internal repository.
- `contracts/`, which exists only where a machine-readable interface does.
- `docs/architecture/`, `docs/guides/`, `docs/reference/`, `docs/security/`, `docs/diagrams/`.
  Each gets a row in `docs/README.md` when it appears.
- `docs/plans/`, which the standard expects to stop existing. A new repository never starts one.
- `.agents/`, `.claude/`, `.codex/`, `.opencode/`. Lexicon writes these when skills are installed.
  `CODEOWNERS` already routes `/.agents/` so ownership is in place before the directory is.
- Empty directories for the above. Git does not track an empty directory, and a `.gitkeep` under a
  heading with nothing to say is the placeholder the standard exists to prevent.

## Placeholder substitutions

| Pattern | Appears in | Becomes |
| --- | --- | --- |
| `<Project name>` | `README.md` heading | the project name |
| `@<owner>` | `.github/CODEOWNERS`, five rules | `@` and the owner |
| `- <owner>` | `owners:` frontmatter in both spec templates | the owner |
| `G-Hensley/<repo>` | `.github/ISSUE_TEMPLATE/security.yml`, the advisory URL | owner and repository |
| `OWNER/REPO` | `.github/ISSUE_TEMPLATE/config.yml`, the advisory contact link | owner and repository |

`copy-skeleton.sh` greps the target afterwards and exits non-zero if any of these survive, so a
skeleton that adds a placeholder is caught rather than shipped.
