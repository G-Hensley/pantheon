---
name: create-skill
description: Create or revise portable Agent Skills with correct structure, triggering metadata, supporting resources, and validation. Use when a user asks to create, scaffold, improve, or audit a SKILL.md-based skill.
---

# Create Skill

Build a portable skill around the open Agent Skills format. Keep the portable core independent of Claude-, Codex-, Agy-, or OpenCode-only behavior.

## Process

### 1. Establish the use cases

Collect or infer:

- the concrete requests that should trigger the skill;
- the requests that should not trigger it;
- the required output or behavior;
- any scripts, references, or assets that should be reusable;
- the intended installation scope and hosts.

Ask only when a missing choice would materially change the result.

### 2. Check existing skills

- Search the target skills root for overlapping names and capabilities.
- Extend an existing skill when the new material shares the same trigger and workflow.
- Create a separate skill when it has a distinct trigger, responsibility, or security boundary.

### 3. Choose the name and location

- Use lowercase letters, digits, and hyphens; keep the name under 64 characters.
- Prefer a short verb-led name such as `create-subagent` or `review-migrations`.
- Match the folder name to the `name` field exactly.
- Install the finished skill as `<skills-root>/<name>/SKILL.md`. Do not place another discoverable skill inside this skill's folder.

Common portable roots include `.agents/skills/` for a project and `~/.agents/skills/` for a user. Host-specific mirrors or adapters are distribution concerns, not part of the portable core.

`scripts/init_skill.py <name> --path <skills-root>` writes the folder, the frontmatter, and a `references/` stub, with `--evals` for an eval scenario. It leaves `TODO` markers deliberately: the scaffold fails validation until they are replaced, because an unfinished skill that validates looks finished.

### 4. Write portable frontmatter

Use only the open-format fields in the portable `SKILL.md`:

```yaml
---
name: example-skill
description: Perform a specific capability. Use when the request includes concrete triggering conditions or artifacts.
---
```

The description must state both what the skill provides and when it applies. Put workflow details in the body. Read [description-discipline.md](references/description-discipline.md) before finalizing it.

All "when to use" information belongs here. A "When to use this skill" section in the body is dead weight, because the body loads only after the skill has already triggered.

Six fields are defined by the open format, and they are not one tier:

| Field | Tier | Use it for |
|---|---|---|
| `name`, `description` | Portable core | Every host reads these. Required. |
| `license`, `compatibility`, `metadata` | Spec-legal | Packaging and upload accept them. Whether a host acts on them varies. |
| `allowed-tools` | Spec-legal, experimental | Never as a permission boundary. |

Spec-legal means the field survives packaging rather than failing it. It does not mean a host acts on it. `allowed-tools` is the trap: it looks like a permission control, it is experimental, and a skill that relies on it for safety is unprotected on every host that ignores it. State a tool restriction in the body as a constraint, and carry the enforcement in a host adapter.

A field outside those six is rejected, not ignored: packaging and upload fail with an error naming it. So `disable-model-invocation`, `context`, `agent`, and `paths` stay out of the portable file and go in a host adapter, and so does any field you invent.

`metadata` is a free-form map of string keys to string values, which is where catalog or freshness data goes if it must travel with the skill. Prefer an external inventory when the data is about the repository rather than about the skill.

### 5. Build the body and resources

- Use imperative instructions and an ordered workflow when sequence matters.
- Include only information an agent cannot reliably infer.
- **Keep the body at or under 200 lines. This is enforced, not advised**: `lexicon validate` fails a skill that exceeds it. The open format permits 500, but the body enters the conversation once and stays there for the rest of the session, so every line is resident cost even for the turns that never touch it, and depth buried in prose is depth an agent skims. Move detail into `references/`, which loads only when needed.
- **Write standing instructions, not one-time steps.** A host does not re-read `SKILL.md` on later turns. Guidance meant to hold for a whole task has to read as a rule that is still true on turn twenty, because "first, do X" is read once and then competes with everything that arrived after it.
- Put deterministic or repeatedly rewritten logic in `scripts/`.
- Put templates or output material in `assets/`.
- Link every supporting resource directly from `SKILL.md` with relative paths.
- Avoid absolute paths, and keep host-specific dynamic-injection syntax out of the portable core. Forms such as `` !`command` ``, `$ARGUMENTS`, and `${CLAUDE_SKILL_DIR}` are documented, and they are documented per host: they do not travel, and a host that does not expand them passes them to the model as literal text.

Use [skill-template.md](references/skill-template.md) as the starting point and [anti-patterns.md](references/anti-patterns.md) during review.

### 6. Validate behavior

**A skill, or an edit to one, is not evaluated until its scenario has been observed failing without it.** Run the scenario the change exists for against the version that lacks it (no skill for a new skill, the previous revision for an edit), record the failure, then run it with the change and record the pass. This is `obra/superpowers`' Iron Law (MIT), restated here against the evidence of the catalog this skill comes from (`G-Hensley/lexicon`): ten fixture sets there were written before any was run (lexicon#122), and an acceptance check it had recorded turned out to pass on the skill's previous revision too, so it measured nothing the edit changed (SPEC-0010 AC-4).

- **The rule gates the evaluated record, not installation.** A change with no observed baseline may still ship. Record it as a gap, and do not call it evaluated, tested, or ready.
- **Edits are covered.** Re-run the scenario that motivated the edit, RED on the previous revision and GREEN on the edited one; the whole suite is not required. A result recorded before an edit does not cover the edit.
- **Not exempt:** a wording change, a new bullet or section, a "simple addition", or a reference file split out of the body that carries a behavioral claim. Each of these changes what an agent does, which is the thing the baseline measures.
- **Exempt, and only these:** a change touching only `references/` or `assets/` with no behavioral claim, and a description-only change already covered by an activation record.
- **If nothing fails without the change,** the change has no observed reason to exist. Rewrite the scenario or drop the change; do not record it as evaluated.

The baseline takes the form of the skill type:

- **Reference:** a question that needs the bundled reference, asked without it. A wrong or missing fact is the failure.
- **Technique or pattern:** a fresh problem the technique fits. Missed recognition or a wrong application is the failure.
- **Task or scaffolding:** invoke it against a temporary target. A missing or wrong artifact is the failure.
- **Discipline-enforcing:** realistic pressure; the violation and its rationalization, verbatim, are the failure. Read [bulletproofing-discipline-skills.md](references/bulletproofing-discipline-skills.md).

[testing-skills.md](references/testing-skills.md) is the long form of this rule, with a worked baseline for a workflow skill.

Use isolated sessions or subagents only when the environment permits them. When the verdict turns on output quality rather than a structural check, grade it with the claim-extraction and blind-rubric procedures in [grading-outputs.md](references/grading-outputs.md). Do not leak the expected diagnosis into an evaluator. Preserve raw prompts, outputs, diffs, and logs for nontrivial tests. See [testing-skills.md](references/testing-skills.md).

**Record the result where the next person will find it.** Write fixtures and
outcomes into the repository that owns the skill, one directory per artifact,
so coverage is visible without rerunning anything. In this catalog that is
`evals/<skill-name>/`, with scenarios at the top level and outcomes under
`results/<date>-<scenario>.md`. A result that changes the skill, or changes the
tooling around it, is the one that most needs a record: an unrecorded eval
leaves a rule nobody can trace back to its evidence.

### 7. Verify structure and discovery

- Confirm `SKILL.md` exists and its YAML parses. Malformed frontmatter usually fails silently: a host loads the body with empty metadata, so direct invocation still works while the description matches nothing and the skill never triggers on its own.
- Confirm every top-level field is one of the six the format defines.
- Confirm the folder name matches `name`.
- Confirm every relative resource link resolves.
- Confirm there are no nested `SKILL.md` files inside the skill.
- Inspect the host's skill list or invoke the skill explicitly after installation.
- Test every host the skill claims to support; successful discovery in one host does not prove another host will load it.

`scripts/validate_skill.py <skill-folder>` performs the first five, and travels with the skill, so it works in a project that has no catalog CLI. In this catalog `lexicon validate` performs the same five over every artifact at once, and a parity test keeps the two from drifting apart. Neither can perform the last two, which need a host.

## Auditing an existing skill

Enter here when the skill already exists and the request is to review, improve, or revise it. Do not run the authoring process to reach this; steps 1 through 5 assume there is nothing to preserve.

Report each of these as a finding with the evidence behind it, and say which are structural and which are behavioral:

1. **Structure**, from step 7 above.
2. **Description**, against [description-discipline.md](references/description-discipline.md). Does it state a trigger a host can route on, and does it claim a capability the body does not contain?
3. **Body**, against [anti-patterns.md](references/anti-patterns.md): standing instructions rather than one-time steps, detail that belongs in `references/`, guidance whose form does not match the failure it addresses.
4. **Trigger evidence.** Is there a recorded activation rate, on which hosts, at how many runs? A description nobody has measured is an untested claim.
5. **Behavior evidence.** Are there fixtures, has any been run, and does the newest RED and GREEN record postdate the last behavioral edit? Fixtures with no recorded result mean the evaluation was designed and never run; a record older than the body it describes covers a different skill.
6. **Staleness.** Are the host claims, versions, and external references still true, and when were they last checked?

Do not report a skill as sound on structural checks alone. Say plainly which of the six you could not establish, rather than leaving the gap for the reader to infer.

## Reference Files

- [skill-template.md](references/skill-template.md): portable templates and host-adapter boundary.
- [description-discipline.md](references/description-discipline.md): capability-and-trigger descriptions and naming guidance.
- [testing-skills.md](references/testing-skills.md): behavioral testing and RED-GREEN-REFACTOR guidance.
- [grading-outputs.md](references/grading-outputs.md): claim extraction and blind rubrics, for verdicts that turn on output quality.
- [anti-patterns.md](references/anti-patterns.md): content, structure, and process mistakes.
- [bulletproofing-discipline-skills.md](references/bulletproofing-discipline-skills.md): additional pressure testing for discipline-enforcing skills.

## Scripts

Both are plain Python with no host or catalog dependency, and both install with
the skill.

- `scripts/init_skill.py`: scaffold a skill folder. Step 3.
- `scripts/validate_skill.py`: check structure. Step 7.
