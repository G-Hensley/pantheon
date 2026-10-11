# Producing the receiving repository's issue contract

The issue goes to the repository that implements the requirement, and that repository decides what
its body looks like. This skill's own shape is a fallback, not a default to overwrite it with.

## Find the contract

Read, in this order, in the repository that will receive the issue:

1. `AGENTS.md` (and `CLAUDE.md` where it only imports `AGENTS.md`), for body sections it names and
   any guide it points to.
2. The contributing or issue guide it points to (`CONTRIBUTING.md`, `docs/guides/`).
3. `.github/ISSUE_TEMPLATE/`: a `work-item.yml` form lists the headings a browser filer sees.
4. Any script or command those files name for checking a body before publication.

Stop at the first source that names the sections. A form is one producer of the contract, not the
contract itself; an agent writes the body directly, so use the form's headings and the checker's
rules, not the form's syntax. If none of the four says anything about a body, the repository has no
contract: use [contractless-body.md](contractless-body.md) and report that no checker was run.

## Write to a file, check, then file

Write the body to a file first. Run the checker on the file. Only on exit 0 is the file ready to
publish, and it is published as it stands. If the checker names a missing or empty section, fix
that section in the body. The checker's exit code says the sections are present, not that the
answers are good; reading the content is still the author's job.

## This repository (`G-Hensley/lexicon`)

`AGENTS.md` points at `docs/guides/issue-workflow.md`, which names the checker. The contract is
seven sections, each a `### ` heading, each with an answer:

| Heading | What goes under it |
| --- | --- |
| `### What` | The behavior or artifact that exists once this is done |
| `### Why` | The problem and who it affects; cite the specification and `FR-` identifiers here |
| `### Scope` | What is in, and what is deliberately out |
| `### Done when` | The finished condition: observable, testable, decided without asking the author |
| `### Validation` | The commands or observations that will produce the evidence, not a claim it passed |
| `### Dependencies and decisions` | Blockers, qualified by repository, and open decisions, or `None` |
| `### Links` | The specification and requirement, related issues, or `self-contained` |

Check with the file, from the repository root:

```sh
uv run python integration-library/github-tracking/work_item_check.py --body-file /tmp/issue-body.md
```

`Dependencies and decisions` and `Links` may say an explicit `None` or `self-contained`; the other
five may not be empty or a placeholder. Do not edit `work_item_check.py` or its scaffold to make a
body pass. `integration-library/github-tracking/body-contract.md` says what the checker assumes.

Where this skill would put a fact, put it here:

| This skill wants | Lexicon body section |
| --- | --- |
| The specification and its `FR-` identifiers | `### Why` and `### Links` |
| The finished condition | `### Done when` |
| How it will be shown to hold | `### Validation` |
| The design document, if there is one | `### Links` |
| A blocking issue | `### Dependencies and decisions`, qualified (`projects#21`, not `#21`) |
| Guarded parts of the work | `### Scope`, in the out or in list |

### A body in the shape

```markdown
### What

`lexicon doctor` reports a project manifest entry that names a skill absent from the catalog.

### Why

SPEC-0031 FR-003. A manifest naming a removed skill installs nothing and says nothing, so the
project runs without a capability its owner believes it has.

### Scope

In: the manifest read in `doctor`, and one new report row. Out: repairing the manifest, and any
change to `lexicon sync`.

### Done when

`lexicon doctor --project <fixture>` exits non-zero and names the missing skill when the manifest
lists one the catalog lacks, and exits zero on a fixture whose manifest is complete.

### Validation

`uv run --all-extras --dev pytest -q tests/test_doctor.py`, with a negative and a positive
fixture, then the two `lexicon doctor` runs above.

### Dependencies and decisions

None.

### Links

`docs/specs/0031-example/spec.md`, FR-003.
```
