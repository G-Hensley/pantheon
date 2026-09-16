# Docs

The map of every document in this repository and where each kind lives. A reader who cannot find
something from this file has found a defect in this file.

| Path | Holds |
| --- | --- |
| [`specs/`](specs/README.md) | Specifications: what is required and why, one directory per specification, requirements routed to issues. |
| [`decisions/`](decisions/README.md) | Architecture Decision Records, one per file, superseded rather than edited. |
| [`ideas/`](ideas/README.md) | Candidates, not requirements. Nothing here is implemented until it is promoted. |
| [`archive/`](archive/README.md) | Dated records no longer acted on. Historical evidence, never current instruction. |
| [`design/`](design/) | Design arguments that outgrew a single file, one directory per subject: `conductor-task-surface/`, `context-window/`, `headless-dispatch/`, `model-routing/`, `openrouter-free-tier/`, `session-restore/`. The last three arrived from `BACKLOG.md` in the 2026-09-16 triage. They predate `specs/` and stay here until the work they describe has a specification to sit under. |
| [`plans/`](plans/) | Dated plans routed here from `G-Hensley/projects`. Still to empty: a plan is read, audited into issues, and then archived. One file today. |
| [`dispatch-composer-evidence.md`](dispatch-composer-evidence.md) | Measured evidence for the dispatch prompt limits, kept beside the design it justifies. |
| [`github-issues.md`](github-issues.md) | The issue workflow and the tracker cutover's activation checkpoint. |
| [`github-tracking-migration.md`](github-tracking-migration.md) | Provenance: every reviewed source identity mapped to its owning issue or historical disposition. Not a second tracker. |

`architecture/`, `guides/`, `reference/`, `security/` and `diagrams/` are created when there is
something to put in them, and each gets a row here when it appears. The layout and its rules are the
Repository Documentation Standard in `G-Hensley/projects` at `docs/templates/repo-docs-template/`.

## Recorded exceptions

Things that are not where the standard would put them, each with the reason and the condition that
retires it. An exception is a decision with an end date, not a permanent carve-out.

| What | Why it is here | Retires when |
| --- | --- | --- |
| Root `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `LICENSE` | GitHub reads all four from the repository root and surfaces them in its own interface. Moving them under `docs/` would hide them from the place contributors and reporters actually look. | Never. This is the conventional location and the standard's root-file rules expect it. |
| Root `BACKLOG.md`, 0 sections of prose | An index, not a backlog. Triaged 2026-09-16: of 20 sections, 15 moved verbatim to [`archive/2026-09-16-backlog-delivered.md`](archive/2026-09-16-backlog-delivered.md) and 5 became one line each pointing at the issue that owns them and the design document that holds their reasoning. The file survives only so that an issue citing a section by name still resolves. | Nothing cites it, which is when the seven owning issues (#53, #56, #60, #61, #62, #63, #72) have closed. Deleted then, not before. |
| [`plans/`](plans/) | Routed here by `projects:SPEC-0001`'s design table. A plan is not a specification: it is dated work-in-progress that becomes issues. | Its one file is audited into issues and moved to `archive/`. |
| `.tasks/`, 19 files | The pre-cutover task ledger. `orion:SPEC-0001` FR-009 removes it once the tracker cutover is verified under [projects#7](https://github.com/G-Hensley/projects/issues/7), and not before, because the map in `github-tracking-migration.md` still resolves against these files. | projects#7 records verified activation. |
| `specs/0000-template/`, `decisions/0000-template.md` | The standard's copyable shapes. They stay until the `spec-authoring` and `adr-authoring` skills are installed, which waits on [lexicon#164](https://github.com/G-Hensley/lexicon/pull/164) and [lexicon#166](https://github.com/G-Hensley/lexicon/pull/166). | Those two pull requests merge and the skills install under `.agents/skills/`. |
| No `docs/security/` | Pantheon's security posture is in root `SECURITY.md` and in the Orion toolchain threat model, which analyses this repository from the umbrella because the attack surface crosses repository boundaries. | Pantheon needs a finding that is its own rather than the toolchain's. |
