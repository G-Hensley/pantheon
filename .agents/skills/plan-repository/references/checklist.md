# The twelve decisions

Answer every item, in this order, with the item name exactly as written here: the plan's checker
matches on it. Each item names the decision, what a `not-needed` has to show, and the answers that
read as complete and are not.

Outcomes:

- `decided`: what will be true on day one, and which layer enforces it.
- `deferred`: why not now, and the observable event that brings it back ("when the first release
  is tagged", "when a second contributor joins"). A date alone is a reminder, not a trigger.
- `not-needed`: why this repository does not need it, in terms of this repository. "Small
  project" is not a reason; "no network listener, so nothing for DAST to probe" is.

## 1. `rulesets`

Branch protection or repository rulesets on the default branch.

Decide: pull request required; which status checks are required, by name (they are the
`server-checks` item's output); whether force-push and deletion are blocked; the merge method and
whether history must be linear; required approvals and whether CODEOWNERS review is required; who
can bypass.

Looks complete and is not: "branch protection is the owner's action". Applying it is the owner's
action; deciding what it contains is this item. Also: requiring approvals on a solo repository,
where the only approver is the author, which either blocks every merge or is bypassed on each one.
Under a personal account the owner can edit the rules, so say this is self-governance.

`not-needed` is rare: a repository nobody merges into, such as a throwaway experiment.

## 2. `secret-scanning`

Expected to catch credentials committed to the tree or introduced in a pull request's commits:
provider tokens, private keys, high-entropy assignments. Decide the tool, where it runs (server
check, platform push protection, local hook for speed), and what happens on a finding.

A governance catalog usually makes this mandatory for every repository. Cite it.

## 3. `dependency-scanning`

Software composition analysis. Expected to catch published advisories against the locked version
of a dependency. It applies wherever there is a lockfile, including a nested one or one belonging
to a pinned third-party build the repository runs. Decide the tool, the lockfiles covered, and
whether update automation is on.

Looks complete and is not: scanning the manifest instead of the lockfile, which reports the range
you asked for rather than the version you run.

## 4. `sast`

Static analysis of first-party source. Expected to catch injection sinks, unsafe deserialization,
unsafe subprocess and shell use, hardcoded credentials and weak cryptography. Decide per language
present, and say which language has no tool yet if that is the case, rather than letting it drop
out silently.

## 5. `dast`

Dynamic testing against a running instance. Expected to catch what only shows at runtime: an
exposed endpoint, missing authentication on a route, injection through a live interface,
misconfigured headers.

It needs something running to probe. `not-needed` is the usual answer for a library, a CLI or a
repository of documents, and the reason is that there is no listener. A service with a network or
local-socket interface needs a real answer, often `deferred` until the interface exists, with that
as the trigger.

Looks complete and is not: leaving it out because the catalog does not name it. A catalog that
lists four scanner classes has decided those four; it has not decided that DAST is unneeded.

## 6. `docs-scaffold`

What the day-one documentation surface contains: `README.md`, `AGENTS.md`, `CLAUDE.md`, the
`docs/` directories, `CONTRIBUTING.md`, `.github/` templates and CODEOWNERS, the host settings and
installed skills under `.agents/`, and whether the generated host mirrors are committed. Follow the
workspace's documentation standard where it has one and name what the scaffold leaves out
deliberately (an architecture document for a system not yet built is usually one).

This item decides the surface. The scaffolding step writes it.

## 7. `agent-host-hooks`

Hooks run by an agent host (session start, before or after a tool call). Decide which hooks, for
which hosts in the contributor host set, and what each does. Say that they bind only a contributor
running that host with the project's configuration loaded. See
[enforcement-layers.md](enforcement-layers.md).

Looks complete and is not: planning for the owner's hosts, or for the catalog's supported hosts,
when the question is which hosts contributors use.

## 8. `local-hooks`

Local Git hooks (pre-commit, commit-msg, pre-push) and how they are installed. Decide which checks
run locally for speed and how a contributor installs them. Say that they bind only those who
installed them and are skipped with `--no-verify`.

## 9. `server-checks`

The CI jobs that run on every pull request and which of them are required through `rulesets`. This
is the only layer that binds every contributor, so every control the plan calls required appears
here, even when a hook also runs it earlier. Name the jobs; the required-check names in `rulesets`
must match them.

Looks complete and is not: requiring a check the governance catalog says is not required yet, or
adding a job for work the documentation standard assigns to another system (an audit the standard
says a repository receives rather than runs). Both re-decide something that was already decided.

## 10. `test-layout`

The exact directories and structure tests are written in: unit, integration, end-to-end and
evaluation tests, where each lives, how fixtures are shared, and naming. Follow the language's
convention and say what it is (for example Rust unit tests in-module under `#[cfg(test)]` and
integration tests in `tests/`; Python tests in `tests/` mirroring the package, run by pytest).

Looks complete and is not: a testing strategy (what will be tested) with no layout (where the tests
go). The layout is the part that becomes a migration later.

## 11. `frontmatter`

Where YAML frontmatter earns its place and where it is noise. It earns it where something reads
it: a validator, an index, an agent routing on it (specifications, decision records, ideas,
architecture documents, skill and agent definitions). It is noise on files nothing parses, and the
workspace's documentation standard may already say which (`README.md`, `AGENTS.md` and `CLAUDE.md`
usually carry none). Decide which document kinds carry it, which schema validates it, and where
that validation runs; where the standard names the validator, that is where it runs.

## 12. `provenance`

Artifact provenance: build attestations, signing, SBOM. Decide from the distribution model:

- Nothing is distributed (the repository is consumed as source by its owner): `not-needed`, and
  say what event would change that, since provenance cannot be added to releases already made.
- Artifacts are released (packages, binaries, images): decide the level, for example platform
  build attestations, and which catalog capability flag it falls under.

Decide whether and at what level. Implementing it is later, separate work.

Looks complete and is not: pinning and recording a third-party dependency's version. That is
supply-chain hygiene for what the repository consumes; provenance is about what it produces.
