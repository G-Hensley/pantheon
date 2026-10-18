# Grading Outputs

Read this when a skill's evaluation depends on judging output quality rather
than on a structural check. [testing-skills.md](testing-skills.md) covers how to
build and run a scenario; this covers how to judge what came back without the
judgement drifting into agreement with whoever wrote the output.

The two procedures below are adapted from the grader and comparator agents in
[anthropics/skills](https://github.com/anthropics/skills), Apache 2.0. Adapted
rather than copied: theirs are subagent prompts dispatched by path, and what is
portable about them is the order of the steps, not the dispatch.

## Grading one output

The step that does the work is claim extraction, and it is the step usually
skipped. An overall impression of an output is not gradable, because nothing in
it can be checked. Break the output into claims first, then verify each by its
kind:

| Claim kind | Verified against |
|---|---|
| Factual: a number, a file, an API, a behavior | The artifact itself, opened and read |
| Process: "I ran the tests", "I checked both hosts" | The transcript or tool trace |
| Quality: "this is clearer", "this is safer" | The stated justification, which must be present |
| Unverifiable | Nothing. Flag it and say so. |

An unverifiable claim is a finding, not a gap in the grading. "The skill
improved routing" with no run behind it is an untested assertion, and a grader
that lets it pass has laundered it into evidence.

Grade against the criteria the scenario recorded before the run, not against
criteria assembled after seeing the output. Criteria written afterwards describe
the output.

## Comparing two outputs

**Generate the rubric before knowing which output is which.** This is the whole
discipline and it is fragile: once you know which side the skill produced, every
later judgement bends toward it, and no amount of care recovers the blindness.
Order of operations:

1. Write down what a good output would contain, from the prompt alone.
2. Score each output against that list, still unlabeled.
3. Only then reveal which was which, and record the verdict.

If the outputs cannot be anonymized, say so in the record and treat the
comparison as weaker evidence. A comparison run with the labels visible is an
opinion with a table around it.

## Recording it

Whatever the verdict, the record carries the fields in
[testing-skills.md](testing-skills.md#recording-results): exact prompt, host and
version, model identifier with tier or effort, skill revision, isolation method,
raw output, verdict, and what the run could not observe. A grade with no raw
output behind it cannot be re-examined when the skill changes, which is exactly
when someone will want to.
