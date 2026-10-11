# Filling in `AGENTS.md`

The skeleton gives the sections. This is what each one is for and what makes a bad answer.

## The two rules that matter

**Repo-specific by construction.** Rules that hold everywhere already live in the host-loaded
global instruction file, `~/Projects/global-llm-configs/AGENTS.md`. Writing style, guarded-change
policy, version-control rules and the greenfield stack defaults are global. A project file that
restates one creates a second copy, the copy drifts, and Claude Code concatenates instruction files
rather than overriding them, so a contradiction resolves arbitrarily rather than in the nearer
file's favour. That is why there is no "global rules" section to write.

The test: if a section would read identically in another repository, delete it.

Two exceptions. A nested repository whose host may not load the ancestor file points at the
global file in a single repo-specific sentence. And the opt-in Life Cycle section
([lifecycle-block.md](lifecycle-block.md)) reads identically wherever it appears, yet stays local
rather than moving to the global file: SPEC-0010's model is adopted per project by owner decision,
not assumed for every repository the way writing style or version-control rules are, so its
presence is itself the repo-specific fact. Write it only when the project has opted in, and count
it toward the 150-line budget like any other section.

**At or under 150 lines.** Claude Code's own documented target is under 200 lines, so 150 is the
tighter budget this tree keeps below the host's ceiling. An `AGENTS.md` that grows past it is
carrying something that belongs under `docs/` or in a skill. Splitting it into `@path` imports
saves nothing: the imports load at launch, so the budget is what the host concatenates in total,
not the longest file.

## Section by section

**Project.** What this is, its current status, what it is not. The last clause does the most work:
it is how an agent avoids solving a problem this repository has deliberately declined.

**Commands.** Build, test, lint, typecheck, as literal commands that work from a fresh checkout.
Keep them identical to the README's Development section. A command that is wrong here is worse than
absent, because it will be run.

**Repository Map.** Only the paths an agent would get wrong by looking: what is generated, what is
not editable, what a name misleads about. Not an inventory of directories, which an agent can
derive and a host's own trimming guidance removes.

**Engineering Rules.** Invariants that hold here and would be wrong elsewhere. If a rule is good
engineering everywhere, it is not one of these.

**Documentation.** Where documents live and which wins on a disagreement. The skeleton's text
already names `docs/README.md`, the specification and decision paths, and that execution state
lives in GitHub Issues. Keep the sentence naming the Repository Documentation Standard: it is the
only pointer a reader has to the rules this layout follows.

**Testing.** The minimum verification expected before proposing a change here. A number or a
command, not an aspiration.

**Security.** Trust assumptions specific to this repository: its boundaries, its privileged
components, how it handles its own secrets. Not the global security policy.

**Git and Pull Requests.** Only what differs here: branch naming, required checks, and whether any
standing delivery authority is narrowed for this repository. If nothing differs, delete the
section rather than restating the global rules.

**Skills.** Which agent skills apply here and when. A skill nobody knows about is a skill nobody
invokes.

**Do Not.** Recurring failure modes actually observed in this repository. Each entry should be
traceable to something that happened. A speculative list reads as noise and gets skimmed past,
which costs the entries that were earned.

**Life Cycle.** Present only when the project opts into SPEC-0010 at step 4. Not filled in, not
summarised: [lifecycle-block.md](lifecycle-block.md) is appended verbatim, because it is already
proportioned to the budget.

## Delete rather than pad

A section with nothing repo-specific to say is deleted, not filled with something generic. The
skeleton is a checklist of questions, not a form that must be completed. `verify-day-one.sh` fails
a file that still carries the skeleton's guidance text verbatim, for exactly this reason.
