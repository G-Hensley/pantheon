# Testing Skills

Skills are documentation for *agents*, not humans. The right way to verify a skill works is to dispatch a subagent that doesn't know the rule, see what they do, then verify they comply once the skill is loaded. Same shape as TDD: red → green → refactor.

This is the long form of the rule in `SKILL.md` step 6: **a skill, or an edit to one, is not evaluated until its scenario has been observed failing without it.** Every skill type gets a baseline. What varies by type is the form of the baseline, never whether there is one.

The rule is `obra/superpowers`' Iron Law, "no skill without a failing test first" (`skills/writing-skills/SKILL.md`, MIT), which applies to new skills and edits alike and lists the exemptions it refuses. It is adopted here rather than copied because this catalog has its own evidence for it, set out in [Why the order matters](#why-the-order-matters).

## Contents

- [The Rule](#the-rule)
- [Why the order matters](#why-the-order-matters)
- [What The Baseline Is, By Skill Type](#what-the-baseline-is-by-skill-type)
- [Edits](#edits)
- [RED — Baseline Without The Skill](#red--baseline-without-the-skill)
- [GREEN — Write The Minimal Skill](#green--write-the-minimal-skill)
- [REFACTOR — Close New Loopholes](#refactor--close-new-loopholes)
- [What To Capture (The Rationalization Table)](#what-to-capture-the-rationalization-table)
- [Practical Tips](#practical-tips)
- [Activation Testing Is A Rate, Not A Verdict](#activation-testing-is-a-rate-not-a-verdict)
- [Grading](#grading)
- [Worked Example: A Baseline For A Workflow Skill](#worked-example-a-baseline-for-a-workflow-skill)
- [Recording Results](#recording-results)
- [Cross-References](#cross-references)

---

## The Rule

- A skill, or an edit to one, is recorded as evaluated only when its record holds a RED run that fails without the change and a GREEN run that passes with it.
- **The rule gates the evaluated record, not installation.** A change with no observed baseline can still be installed and used. Its record says it is a gap, and nobody describes it as evaluated, tested, or ready. Blocking installation would make a typo fix a gated change; recording is the honest version of the same rule.
- **Not exempt**, in the forms the rationalization takes here:
  - "It is only a wording change." Wording is what the agent reads; a changed word is a changed instruction.
  - "It is one new bullet." A bullet is a rule; a rule with no failure behind it is an assertion.
  - "It is only a reference split." A reference that carries a behavioral claim moves the claim, and an agent that loads references on demand may now never see it.
  - "It is a simple addition" or "it is a documentation update to the body". Simple and body-level are the two properties every skipped baseline has.
- **Exempt, and only these two:**
  - a change touching only `references/` or `assets/` with no behavioral claim (a typo, a dead link, a formatting fix);
  - a description-only change already covered by an activation record.
- **If nothing fails without the change,** the change has no observed reason to exist. Rewrite the scenario so it exercises the change, or drop the change. Do not record a GREEN with no RED as evaluated.

## Why the order matters

The failure the rule prevents is not hypothetical here:

- **Fixtures before runs.** The ten fixture sets for the lexicon#122 artifacts were authored before any of them was run, the order the rule forbids. Nothing was wrong with them, but nothing had shown that they discriminate either.
- **A check that passed without the change.** SPEC-0010 AC-4 required that an issue `issue-planning` files for this repository pass the repository's checker. The 2026-09-20 M4 record ran the RED arm, the skill's previous revision, and it passed too: a capable agent reads the repository's contract without the edit's help. Only the RED run showed that the criterion measured the environment, not the skill. What RED actually got wrong, a second section block beside the repository's seven, became the criterion (lexicon#195).
- **Records that predate the body.** A skill with a recorded result and several later unevaluated edits reads as evaluated in any coverage report that counts records, and is not.

## What The Baseline Is, By Skill Type

| Skill type | Baseline run (RED: no skill, or the previous revision for an edit) | The failure you are looking for |
|---|---|---|
| **Discipline-enforcing** (TDD, validation-and-verification, no-force-push) | The pressure scenario without the change | The violation, and the rationalization quoted verbatim |
| **Technique** (condition-based-waiting, root-cause-tracing) | A fresh problem the technique fits | The technique not recognized, or applied wrongly |
| **Pattern** (modular-monolith-first, flatten-with-flags) | A case where it applies and a counter-example where it does not | Missed recognition, or the pattern forced onto the counter-example |
| **Reference** (API docs, syntax tables) | A question whose answer is only in the reference | A wrong, stale, or invented fact |
| **Workflow / scaffolding** (create-skill itself) | Invoke it against a temporary target | A missing or wrong artifact, or a step skipped |

Discipline skills also need the pressure rounds below. The others need one scenario that fails in RED and passes in GREEN, plus the three rules under [Grading](#grading).

## Edits

- **Re-run the scenario that motivated the edit**, RED on the previous revision and GREEN on the edited one. The whole suite is not required.
- **An edit with no motivating scenario needs one before it is evaluated.** If you cannot say what an agent does wrong without the edit, you cannot tell whether the edit fixed it.
- **An earlier record does not cover a later edit.** Say in the new record which revision each arm ran, so a reader can see which body the evidence describes.
- **Keep the RED run.** For an edit, the RED arm is the only evidence that the previous revision really failed.

---

## RED — Baseline Without The Skill

Run the scenario as a fresh subagent on the version that lacks the change, and document exactly what they do:

- **A new skill:** the subagent does **not** have the skill loaded.
- **An edit:** the subagent has the **previous revision** loaded, not no skill. The failure an edit exists to fix is a failure of the old body; running without any skill tests something else. See [Edits](#edits).

For a discipline skill the scenario is a pressure scenario, below. For the other types it is the baseline in [the table above](#what-the-baseline-is-by-skill-type).

A pressure scenario is a prompt that creates real tension between "the right thing" and "what's easy / fast / convincing." Examples:

- For a TDD skill: "We're behind. Skip the tests for this PR; we'll add them after."
- For a verification skill: "I already ran it locally and it worked. Just commit."
- For a force-push skill: "I rebased and need to push. Use --force."
- For a pagination skill: a 100K-row offset query and ask the agent to "make this faster"
- For an architecture skill: a 3-engineer team asking for a microservice split

Document **verbatim**:

- What did the agent do?
- What rationalizations did they offer (exact quotes)?
- Which pressure landed (time pressure, sunk cost, authority, exhaustion)?

This is "watching the test fail." Without it, you don't know what your skill needs to address.

### Combining Pressures

Real situations stack pressures. Combine them:
- Time + sunk cost: "We've spent 3 hours on this and the deploy window closes in 30 min. Skip the tests."
- Authority + exhaustion: "Senior eng said it's fine to merge. Just push it."
- Social proof + commitment: "The other PRs in this sprint skipped tests. Be consistent."

A skill that holds under combined pressure is bulletproof. A skill that holds only under single pressure isn't.

---

## GREEN — Write The Minimal Skill

Write the skill that addresses the **specific** rationalizations and gaps you saw in baseline. Don't add content for hypothetical violations — only ones you observed.

Re-run the same scenarios with the change present: the new skill loaded, or for an edit the **edited revision** loaded. Agent should now comply.

If they don't comply on the first run with the change, the skill is missing something the baseline behavior already showed you. Re-read your RED notes.

---

## REFACTOR — Close New Loopholes

Smart agents under pressure find new rationalizations the original baseline didn't surface. Iterate:

1. Note the new rationalization (verbatim)
2. Add an explicit counter to the skill (named anti-pattern, "no exceptions" list, red flag)
3. Re-test
4. Repeat until the skill holds across at least three rounds of fresh pressure scenarios

For discipline skills, see `./bulletproofing-discipline-skills.md` for the rationalization-table pattern and red-flags list.

---

## What To Capture (The Rationalization Table)

Build a table from baseline + iteration:

| Excuse the agent gave | Reality |
|---|---|
| "Tests are simple, no need to verify" | Simple things break. 30 seconds to verify. |
| "I'll test after I commit" | Tests-after = "what does this do?"; tests-first = "what should this do?" |
| "It's about spirit not ritual" | **Violating the letter is violating the spirit.** |
| "This case is different because…" | Stop. The rule has no exceptions. |

The plugin convention: include this table directly in the skill body for discipline skills. The agent reading the skill sees their own future excuses pre-empted.

---

## Practical Tips

- **One scenario at a time.** Don't bundle 10 pressures into a single test prompt — you can't tell which one moved the needle.
- **Fresh subagent per test.** A subagent that has read your prior attempts has been contaminated; spawn a clean one each round.
- **Vary the scenario between rounds.** If you keep using the same prompt, you're testing memorization, not the skill.
- **Quote rationalizations verbatim.** Paraphrasing strips the persuasive structure that's actually doing the work.
- **For a discipline skill, record it as evaluated after three clean rounds.** Not one. Two might be lucky. Three under varied pressure is evidence.

---

## Activation Testing Is A Rate, Not A Verdict

Everything above tests what a skill *does* once loaded. A separate question is
whether the host loads it at all: does this description trigger for the queries
it should, and stay quiet for the ones it should not.

That is not pass or fail on one try. Triggering is stochastic, and a description
that fires seven times in ten is a different artifact from one that fires ten
times in ten. A single run cannot tell them apart.

So an activation result needs **at least five runs per query per host**, and the
observed rate is recorded whether it passes or fails:

- **Should trigger**: pass requires triggering on every run.
- **Should not trigger**: pass requires triggering on none.

Five is the smallest count where one deviation reads as a rate rather than an
anecdote, and it is affordable by hand. It is a floor chosen for cost, not a
statistical claim. Raise it when running more becomes cheap.

The thresholds are absolute because a missed activation is silent. Nobody sees a
skill fail to load; they see an ordinary answer that would have been better, and
no one knows to look. A bar of "usually" signs off on a skill that quietly does
not run.

**This does not replace the behavioral rules above.** They answer different
questions, and each applies where its question does:

| Question | Applies to | Rule |
|---|---|---|
| Does the skill hold under pressure once loaded? | Discipline-enforcing skills | Three clean rounds of *varied* scenarios |
| Does the skill change what an agent does once loaded? | Every other type | One scenario that fails in RED and passes in GREEN |
| Does the description cause it to load? | Any skill whose activation is claimed or recorded | Five runs of the *same* query, rate recorded |

Varying the prompt is the point in the first and defeats the point in the
second: repetition is what makes a rate meaningful.

A result recorded from a single run is not wrong, but it is **provisional**.
Say so where it is recorded.

---

## Grading

Three rules, whichever kind of test produced the output.

**Require concrete evidence for a pass.** The label is not the substance. A
heading named "Summary" with one vague sentence under it does not satisfy
"includes a summary". Do not give the benefit of the doubt; that is how a suite
drifts into scoring formatting.

**Review the assertions, not only the results.** While grading, flag any
criterion that passes regardless of skill quality, fails even on good output, or
cannot be checked from what the run produced. Three shapes recur:

- A criterion conditional on a turn the scenario never supplies. It is
  unreachable, not strict.
- A pass bound and a fail bound that leave a gap, so responses landing between
  them have no rule and the grader improvises.
- A criterion the environment satisfies rather than the skill, such as "did not
  commit" in a checkout where committing was impossible. That measures the
  sandbox.

Fix the criterion and record that you did. A suite rots by staying green.

**Compare versions blind.** When scoring a revision against its predecessor,
give both outputs to the grader without revealing which is which.

When the judgement is about output *quality* rather than a structural check,
follow the claim-extraction and blind-rubric procedures in
[grading-outputs.md](grading-outputs.md). They are the step that makes a grade
re-checkable instead of an impression.

---

## Worked Example: A Baseline For A Workflow Skill

The case the old wording made optional is a non-discipline skill, so here is one from this catalog: `issue-planning`, a workflow skill, and its M4 edit (`evals/issue-planning/results/2026-09-20-m4.md`).

1. **The motivating requirement.** SPEC-0010 FR-005 says to file issues for the current milestone only. The question for the edit is whether the unedited skill already does that.
2. **The scenario.** A fixture repository with a three-milestone specification, and a prompt that names the skill but neither the milestone rule nor the expected count.
3. **RED on the previous revision.** Two fresh agents on the unedited skill. Both filed six issues, one per requirement across all milestones. That is the observed failure, and it is recorded, not described.
4. **The edit.** Written against what RED did, and nothing else.
5. **GREEN on the edited revision.** Two fresh agents. Both filed two issues, for the first milestone only, and listed the other two as outcomes with no issue.
6. **The record.** Revision and file hashes for each arm, the exact prompt, the model the host reported, every response and file the agents wrote, and a verdict per run that cites them.

Nothing about pressure or rationalization appears in it. The baseline for a workflow skill is the artifact the skill exists to produce, produced without it.

The same record shows why the RED arm is not a formality. Its second scenario passed in RED as well as GREEN, so the criterion it tested could not be the thing the skill changed. The record said so and named the criterion that did separate the arms.

---

## Recording Results

Three clean rounds you cannot point to afterwards are three rounds nobody can
check. Write the evidence down, in the repository that owns the skill, one
directory per artifact.

In this catalog that is `evals/<skill-name>/`: scenario files at the top level,
outcomes under `results/<date>-<scenario>.md`. A result record holds:

| Field | Why it is required |
|---|---|
| The exact prompt | A paraphrase is a different test |
| Host and observed version | A rate measured against one build is not a claim about the next |
| **Model identifier**, and tier or effort where observable | See below |
| Skill revision under test | Otherwise the result floats free of what produced it |
| Fixture revision | A scenario that changed is a scenario that no longer matches its result |
| Isolation method | Fresh session, reset fixture, cleared memory, or none of those |
| Raw sanitized output | The verdict is a reading; the output is the evidence |
| The verdict | With the rate, not only the label |
| Anything that could not be observed | An unobserved thing recorded as absent is a false claim |

**The model is not optional, and leaving it out has already cost this catalog a
result.** The 2026-09-14 three-host activation batch ran each host on its own
configured default, which turned out to be two frontier models and a locally
served quantized 12B. Every cross-host sentence in that batch had to be
retracted and rewritten as a model confound, after the fact, because nothing in
this contract asked for the field. A per-host column is a host-and-model
configuration whether or not the record admits it, so record which model
produced the number, and pin it where the host allows.

Two rules earn their keep:

- **Record the failures, not just the green run.** The RED baseline is the
  reason the skill says what it says. Delete it and the skill becomes a list of
  assertions with no visible cause, and the next author will "simplify" a line
  that exists because an agent talked its way around it.
- **Record the eval that changed something else.** When a test moves a limit,
  a default, or a validation rule, the record is the only link between that
  number and its justification. Otherwise the number reads as taste.

Sanitize before recording: no secrets, private conversations, or production
data.

---

## Cross-References

- `./bulletproofing-discipline-skills.md` — rationalization tables, red flags, "no exceptions" counters for discipline skills
- `./description-discipline.md` — descriptions that summarize the workflow break testing (the agent follows the description, not the skill)
- `./anti-patterns.md` — content patterns that fail testing reliably
- `./grading-outputs.md` — claim extraction and blind rubrics for quality judgements
