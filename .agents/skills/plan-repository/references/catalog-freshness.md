# Catalog Freshness Check

Run this before anything else in `plan-repository` or `scaffold-repository`. Project start is the
one moment a stale skill cannot be corrected later: the Life Cycle opt-in, the plan and the day-one
surface are all decided from whatever skill text is installed, and a later drift warning arrives
after those decisions are taken. Two outside projects started on stale skills this way (Lexicon
issues #224 and #245), and one never saw the Life Cycle opt-in at all.

The check reads; it never updates. Updating installed files is the owner's decision. It protects
only installs that already carry this step: a copy installed before it existed never runs it, and
catching those is the session-start drift report's job (Lexicon issue #235), not this one's.

## The life-cycle skills

A stale copy of any of these stops the task:

- `plan-repository`
- `scaffold-repository`
- `shape-ideas`
- `write-spec`
- `issue-planning`
- `reconcile-project`

Any other installed item that is behind is named in the report, and the task continues.

## What to run

Run from the project the skill is installed in: the Git repository whose `.agents/` holds it, not
the repository being planned or scaffolded, which has nothing installed yet.

**1. Installed items against the catalog.**

```sh
lexicon update --check
```

It writes nothing. Exit 0 means every installed item matches the catalog checkout. Exit 2 lists
each item as `current`, `outdated`, `modified` (edited here) or `missing` (the catalog no longer
has it). A life-cycle skill reported `outdated` or `missing` stops the task. One reported
`modified` was edited deliberately here: name it, say that its freshness against the catalog is
unknown, and continue. Any other exit status means the command failed: take the "could not check"
path under Without the CLI.

**2. The catalog checkout against its upstream.** `update --check` compares with the local
catalog checkout, so a checkout that is itself behind reports `current` for skills that have since
changed. Find the checkout in the `catalog-root` row of `lexicon doctor` (widen the terminal, for
instance `COLUMNS=200 lexicon doctor`, if the path is cut off), then:

```sh
git -C <catalog-root> fetch --quiet
git -C <catalog-root> diff --name-only HEAD...@{upstream} -- \
  skill-library/plan-repository skill-library/scaffold-repository skill-library/shape-ideas \
  skill-library/write-spec skill-library/issue-planning skill-library/reconcile-project
```

`fetch` changes only remote-tracking refs, never the checkout. The three dots compare with the
point the checkout branched from, so only changes the upstream has and the checkout lacks are
printed; a checkout ahead of its upstream (local commits, a feature branch) is not stale for that.
Any path printed is a life-cycle skill the catalog has not caught up with, and stops the task.

Run both checks before reporting, even when the first already stops the task: the second decides
which command the report names.

## Without the CLI

When `lexicon` is not on the path but `.agents/.agent-toolkit.json` exists, read the manifest. An
item recorded with `catalog.git_commit` was installed from that commit; compare it with the
catalog's `main`:

```sh
gh api repos/G-Hensley/lexicon/compare/<git_commit>...main \
  --jq '[.files[].filename | select(startswith("skill-library/<skill>/"))]'
```

Run it once per installed life-cycle skill. A non-empty list stops the task. The compare lists at
most 300 files, so for an install many commits old treat an empty list as approximate. An item
with no `catalog` entry predates provenance recording: report its freshness as unknown.
`git_dirty: true` means the install came from a checkout with uncommitted changes, so the commit is
approximate: say so beside the result.

When neither the CLI nor the manifest is available, or a command above fails (no network, no
`gh`, a catalog checkout with no upstream branch), say exactly that ("could not check whether
the installed Lexicon items are current: <reason>") and continue. A missing convenience is not a
safety control; the report names the gap so the owner can close it.

## When it stops

Stop before gathering inputs or writing anything. Report:

- each stale life-cycle skill and what showed it stale (`outdated`, `missing`, or the changed
  catalog paths);
- the command that brings it current: `lexicon update --refresh-catalog` when the catalog checkout
  is behind, which fast-forwards it and then updates, otherwise `lexicon update`;
- any other outdated items, for the same command.

Then wait. Resume from the top once the owner has updated, or when the owner says to proceed on
the installed versions; record that choice, and the stale items, in the plan's Inputs or the
scaffold's pull request.
