---
name: scaffold-repository
description: Write a new repository's day-one documentation surface from the Repository Documentation Standard's skeleton, fill in the prose the skeleton only shapes, and prove the result by running the tree checker and showing its output. Use when working inside an existing checkout (the repository must already exist; creating it, its rulesets and its hooks are separate work) and a new repository needs its first documentation, an existing repository is being brought onto the Repository Documentation Standard, or a repository is missing AGENTS.md, CLAUDE.md, docs/ indexes, CODEOWNERS or the required issue templates.
---

# Scaffold Repository

Land the documentation surface a repository needs on its first day, taken from the Repository
Documentation Standard rather than improvised, and end by showing a checker's output instead of
claiming the result is conformant.

The day-one surface is smaller than the standard. The standard's rule is that a directory
establishes where a kind of knowledge belongs, and a file is created when useful information
exists for it. So this skill writes the indexes and the instruction files, and does not write an
architecture document for a system that does not exist yet.

## Do not run this skill when

- The repository already carries a conforming surface. Run steps 5 and 6 alone to find out.
- One file is missing from an otherwise conforming repository. Write that file; a scaffold pass
  will refuse to overwrite the rest anyway.
- The target is read-only third-party material, for instance anything under `reference/`.

## 0. Check the installed Lexicon items are current

Before anything else, run the check in [catalog-freshness.md](references/catalog-freshness.md):
`lexicon update --check` in the project this skill is installed in, then the catalog checkout
against its upstream. When a life-cycle skill (this one, `plan-repository`, `shape-ideas`,
`write-spec`, `issue-planning` or `reconcile-project`) is behind the catalog, stop: name the
outdated items and the command that updates them, and write nothing until the owner has updated
or said to proceed on the installed versions. When neither the CLI nor the install manifest is
available, say so and continue. A stale copy of this skill can scaffold without the Life Cycle
opt-in at step 4, and that decision is not offered again once the surface exists.

## 1. Gather the four inputs

- **Target directory.** An existing checkout, usually empty of documentation.
- **Owner.** The GitHub account or team that reviews the instruction files, without the `@`.
- **Repository name.** As GitHub spells it, because it lands in advisory URLs.
- **Project name.** Human readable, for the README heading.

Ask only for what you cannot read off the checkout or the remote.

## 2. Get the skeleton

The copy source is `docs/templates/repo-docs-template/skeleton/` in `G-Hensley/projects`. Prefer a
local checkout of that tree, typically `~/Projects`. Otherwise clone it somewhere disposable.

This skill was verified against that skeleton at revision `3c20137` (2026-09-16), which was on the
branch `docs/adopt-documentation-standard` and not yet on `main`. Read
[day-one-surface.md](references/day-one-surface.md) before assuming the file set still matches; the
copy script fails loudly rather than silently if the skeleton has moved.

## 3. Copy and substitute

Both scripts below ship with this skill; run them from the skill's own directory.

```sh
scripts/copy-skeleton.sh \
  --skeleton /path/to/projects/docs/templates/repo-docs-template/skeleton \
  --target /path/to/new-repo \
  --owner G-Hensley --repo new-repo --project-name "New Repo"
```

It writes the day-one file set and substitutes every placeholder the skeleton carries. It refuses
to overwrite: if any target file already exists it names all of them and writes nothing, because a
half-scaffolded repository is harder to reason about than an unscaffolded one.

[day-one-surface.md](references/day-one-surface.md) lists what it writes, what it deliberately
leaves out, and every placeholder it substitutes.

## 4. Write the prose the skeleton only shapes

The copied files are correct in shape and empty of content. A section left as the skeleton wrote it
reads plausibly and says nothing about this repository, which is the failure mode to watch for.

**`README.md`** is for humans. Say what this is, who it is for, and what it is not. Give the one
command that proves a checkout works. Keep its build, test and run commands identical to the
`Commands` section of `AGENTS.md`, since two copies that drift are worse than one.

**`AGENTS.md`** is the cross-agent working agreement for this repository only. Read
[agents-md.md](references/agents-md.md) before writing it. Two rules decide whether it is any good:

- Every section is repo-specific by construction. If a section would read identically in another
  repository, it belongs in the host-loaded global instruction file and not here. That test rules
  out a standing "global rules" section.
- At or under 150 lines. Splitting into `@path` imports buys nothing, because the host concatenates
  what it loads; the budget is the total.

**`CLAUDE.md`** holds exactly `@AGENTS.md` and nothing else. A symlink to its sibling `AGENTS.md`
also conforms and is the form that survives a session started in a subdirectory.

**`.github/CODEOWNERS`** already owns the instruction files and itself. Replace the owner only if
review should route somewhere other than the repository owner.

Delete `docs/specs/0000-template/` and `docs/decisions/0000-template.md` once the `write-spec`
and `record-decision` skills are installed here, and delete the sentence in each index that names
them. Until then they are the shape a first specification or decision is copied from.

**Opt in to the AI Development Life Cycle.** Ask whether this project adopts Lexicon's operating
model (SPEC-0010), unless the caller already gave an explicit instruction either way. On yes,
append [lifecycle-block.md](references/lifecycle-block.md) verbatim to `AGENTS.md` as its own
section; it is written to work with no other Lexicon file present. On no or no answer, write
nothing. This is the one section exempt from "repo-specific by construction" in
[agents-md.md](references/agents-md.md): adoption is a per-project decision, not a global one, and
the block still counts toward the 150-line budget.

## 5. Check what can be checked about the repository

```sh
scripts/verify-day-one.sh /path/to/new-repo [--lifecycle]
```

Pass `--lifecycle` when the project opted in at step 4; it then asserts the Life Cycle section
landed. Omit it otherwise, since the script has no other way to know the decision was made.

It covers what the tree checker does not assert: the `.github/` set, the four `docs/` indexes,
CODEOWNERS routing the instruction files and itself, the `AGENTS.md` line budget, surviving
placeholders, em dashes, and sections still carrying skeleton guidance. Step 6 is the authority on
everything the two both touch.

Fix what it reports before going on. A failure here is a real defect, not a formatting preference.

## 6. Run the tree checker and show its output

```sh
python3 /path/to/projects/scripts/check-tree.py --repo /path/to/new-repo --verbose
```

`--repo` runs the per-repository assertions against one directory, outside any tree: the
instruction files and the documentation standard's structural contract. No `PROJECT-INDEX.md` row
is needed, and the tree's accepted gaps are not applied, so every error counts. A conformant
scaffold exits 0.

It needs the checker at `be7decd` (2026-09-16) or later. An older copy has no `--repo` and will
reject the flag rather than silently ignore it.

**Show the output verbatim, including its exit code, and do not summarise it as success.** The
whole point of this step is that the skill cannot report a result the checker contradicts. If it
fails, fix the repository; do not reword the report.

Read [tree-checker.md](references/tree-checker.md) before interpreting it. It names what `--repo`
does not assert, and what to do when the repository lives inside the Projects tree instead.

## 7. Register and propose

- Add the repository to `PROJECT-INDEX.md` if it lives in the Projects tree, with a
  repository claim that starts `Own repo`. A row claiming `None` is indexed and then never
  assessed by the checker, which reads like a pass and is not one.
- Branch, commit and open a pull request. Do not commit the scaffold to the default branch.
- In the pull request, name the skeleton revision the copy came from and paste the checker output.

## Reference Files

- [catalog-freshness.md](references/catalog-freshness.md) the step 0 check: which items are
  life-cycle skills, the commands, the fallback without the CLI, and what to report on a stop.
- [day-one-surface.md](references/day-one-surface.md) the exact file set, the omissions and why
  each is an omission, and the placeholder substitutions.
- [agents-md.md](references/agents-md.md) how to fill each `AGENTS.md` section, the 150-line
  budget, and the test for a section that does not belong.
- [tree-checker.md](references/tree-checker.md) what the checker asserts today, what it does not,
  and which invocation to use.
- [lifecycle-block.md](references/lifecycle-block.md) the opt-in AI Development Life Cycle section
  for `AGENTS.md`, self-contained and citing SPEC-0010 by URL.
