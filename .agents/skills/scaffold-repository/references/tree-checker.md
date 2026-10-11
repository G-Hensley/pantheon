# The tree checker

`scripts/check-tree.py` in `G-Hensley/projects` is the drift check. This skill ends by running it
and showing its output, so that the skill cannot report a success the checker would contradict.

## What it is

It compares claims in `AGENTS.md`, `PROJECT-INDEX.md` and `docs/README.md` against the filesystem.
It is mechanical: it proves structure and can never prove that a description is still true.

```text
--root PATH   Projects tree to inspect (default: the checkout containing this script)
--repo PATH   one repository to check on its own, outside any tree
--verbose     also list checks that passed
--json        machine-readable output
Exit 0 all clear (known gaps allowed), 1 at least one unexpected failure, 2 --repo is not a directory.
```

## `--repo`, the invocation this skill uses

```sh
python3 scripts/check-tree.py --repo /path/to/new-repo --verbose
```

It runs the two per-repository checks and nothing else, against a directory that need not be in any
tree and need not have a `PROJECT-INDEX.md` row. The tree's `known-gaps.txt` is deliberately not
applied, because an accepted gap in the Projects tree says nothing about a new repository, so under
`--repo` every error counts.

| Check | Asserts |
| --- | --- |
| `check_instruction_files` | `AGENTS.md` exists, and `CLAUDE.md` is either exactly `@AGENTS.md` or a symlink to its sibling `AGENTS.md` |
| `check_repository_contract` | `README.md` at the root; `docs/README.md` wherever `docs/` exists; `NNNN-slug` names under `docs/decisions/` and `docs/specs/`; and `id`, `type`, `status`, `created` frontmatter with a status from the type's vocabulary on every `spec.md` and decision record |

`0000-template` entries are exempt by name, which is why the templates this skill writes do not
fail the frontmatter assertion. Empty directories are ignored.

This mode requires the checker at `be7decd` (2026-09-16, `G-Hensley/projects` PR #47) or later. It
was added for this skill. An older copy rejects `--repo` rather than ignoring it, so a stale
checker fails loudly.

## What `--repo` does not assert

Worth knowing before reading a clean run as full conformance:

- Nothing under `.github/`. The standard requires `CODEOWNERS`, the four issue templates and the
  pull request template; the checker asserts none of them.
- The `docs/specs/`, `docs/decisions/`, `docs/ideas/` and `docs/archive/` index files. Only
  `docs/README.md` is checked.
- The `AGENTS.md` 150-line budget, and whether any section still carries the skeleton's guidance
  text rather than something about this repository.
- Prose accuracy of any kind, which no filesystem check can reach.

`scripts/verify-day-one.sh` covers that list. The two overlap on the instruction files, the root
`README.md` and `docs/README.md`; where they ever disagree, the checker is authoritative and the
script is the thing to fix.

## When the repository is inside the Projects tree

Add its `PROJECT-INDEX.md` row and run the tree instead, which assesses the new project in context
alongside everything else:

```sh
python3 scripts/check-tree.py --root /path/to/projects --verbose
```

**The row's repository column decides whether the project is checked at all.** Only rows whose
claim begins with `Own repo` or contains `tracked subdirectory` are enumerated, and rows with a
status of Archive, Planning, Reference or Remote only are skipped. A new repository written up as
`None` is indexed and then never assessed, which reads like a pass and is not one. Measured on
2026-09-16: with the claim `None` the run reported only `index path exists` and `repo claim
matches`; with `Own repo (G-Hensley/demo-repo, private)` and an origin remote configured it added
`experiments/demo-repo instruction files conform`.

Running `--root` against a repository that is not a Projects tree is the wrong invocation and
reports failures about the missing tree rather than about the repository. Use `--repo` for that.
