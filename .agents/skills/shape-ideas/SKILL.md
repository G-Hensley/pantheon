---
name: shape-ideas
description: Shape a rough idea into a shaping record with a delivery strategy, an appetite and a sequence of independently valuable increments, each with a definition of good held as a hypothesis. Use when someone wants to brainstorm, whiteboard, or think through a feature, product, system, process, or rework before building; wants help scoping an MVP or a v0, v1, v2 ladder; wants to decide what ships first; or asks what "good" or "done" should mean for a slice of work.
---

# Shape Ideas

Turn a rough idea into a design the person actually believes in, then cut it into
increments that ship one at a time, each with a stated definition of good. The
value is in the interview, not the paperwork. The shaping record holds discovery,
rationale and hypotheses; it is never the requirements authority.

## Do not run this skill when

- The change is mechanical and reversible in a single step: a rename, a config
  value, a dependency bump, a copy edit, a one-line fix.
- The person already has a decided design and is asking for execution.
- The request is a question to answer, not work to shape.
- The person explicitly asks to just build it. Offer this once, accept the answer,
  and do not re-offer in the same session.

Say what you are skipping and why in one sentence, then do the work directly.
Running a design ceremony over a trivial change trains people to route around you.

## 1. Set the depth

Pick from consequence, ambiguity, and reversibility. State the depth you chose and
let the person move it.

| Depth | Fits | Interview | Output |
|---|---|---|---|
| Quick | One reversible slice, low blast radius, clear intent | One round of clarification | Options and a recommendation in conversation |
| Standard | A feature with real unknowns or several moving parts | Passes 1, 2, and 4 | Shaping record with the next increment and later outcomes |
| Deep | A new system or product, or several interacting subsystems | All four passes plus risks and validation experiments | Shaping record with North Star, increments and risks |

Consequence is not depth. A guarded area (auth, secrets, personal or production
data, schemas and migrations, deployment) raises the approval bar and makes the
Edges pass mandatory, but a tightly specified change inside one can still be Quick.
Depth can also change mid-session. If pass 1 reveals the idea is three projects
wearing a trench coat, stop and decompose before going deeper.

## 2. Diverge before you converge

Generate before you judge. Offer several genuinely different directions, including
at least one that questions the framing, and hold off on evaluating any of them
until the set is on the table.

Two moves that pay for themselves early:

- **Challenge the premise once, explicitly.** State the problem you believe is
  actually being solved. If the request would not solve it, say so before designing
  anything. This is the highest-value move here, and only works before investment.
- **Check prior art** when the problem is likely already solved. An existing
  library, pattern, or product that covers 80 percent changes the design.

Do not converge on one approach until the person has seen the alternatives.

When a choice turns on layout, spatial arrangement, or competing visual directions,
offer to show it rather than describe it, using whatever the host provides. A
question about a UI topic is not automatically a visual question.

## 3. Interview in passes

Multiple short passes, each with a stated exit condition and a recap. Work them in
order, and go back when later evidence changes an earlier answer: slicing routinely
exposes a constraint that reshapes the frame. The exits are checkpoints, not
one-way gates. Recap in three to six lines before moving on, so the person can
correct a wrong turn cheaply.

1. **Frame.** The outcome, who it is for, what changes when it works, what is
   deliberately excluded. Exits when you can state the goal in one sentence the
   person agrees with.
2. **Shape.** Approaches, constraints, and the decisions that actually matter.
   Exits when one direction is chosen and the reason is written down.
3. **Edges.** Failure modes, data and state, scale, security, operations,
   migration. Exits when the risks are known and each is accepted, mitigated, or
   turned into an experiment. Skip at Quick depth.
4. **Slice.** Increments, sequencing, and the definition of good for each. Exits
   when the first increment is small enough to build and its evidence is agreed.

Use [interview-passes.md](references/interview-passes.md) for question discipline,
question banks, the recap format and the assumption ledger.

## 4. Compare approaches against stated criteria

Name the criteria before the options, so the recommendation is checkable rather
than asserted. Use criteria that actually discriminate here, and always include
reversibility.

Give two to four options, each with what it costs, what it buys, and what it
forecloses. Lead with your recommendation and the reason. Record the rejected
alternatives and why they lost, which is what stops the debate reopening in three
weeks. Never recommend without stating what would change your mind.

## 5. Resolve appetite and choose the delivery strategy before slicing

Resolve how much the work is worth before cutting it: **agent sessions**, each a
bounded session of one worker, with an optional dollar ceiling where the host
reports cost. Reuse a supplied bound; otherwise ask and wait for agreement before
slicing. Wall-clock estimates and model-tier choices are not an appetite.

Record one delivery-strategy line and its reason. Consider scope and complexity,
requirement clarity, technical uncertainty, consequence of failure, reversibility,
integration breadth and verification difficulty:

- **Direct Build:** the complete solution fits one bounded, reversible, verifiable
  increment. Emit that one increment, with no manufactured MVP or later ladder.
- **Incremental Build:** too large for one increment, major assumptions understood.
  Shape a North Star, the smallest useful outcome, then later outcome hypotheses.
- **Experimental Build:** a load-bearing assumption is uncertain. The first
  increment is a bounded experiment whose useful result is reproducible evidence;
  a prototype or MVP follows only if that evidence justifies it.

Every emitted increment carries an appetite, including an experiment or later
outcome sketch. State how each fits the agreed bound; tentative later allocations
are hypotheses, not an expansion of the current appetite. If a cut does not fit,
report **needs a smaller cut**, naming what does not fit. Offer a split, reduction,
investigation or reconsideration for agreement; never silently drop required
behavior or propose a longer estimate to make it fit. Unagreed scope stays pending.

Slice by independently valuable outcome, never by layer. A product increment must
ship usefully alone; an experiment must answer a decision-relevant question alone.
Detail only the next increment; later outcomes stay brief. Sequence by risk and
flag one-way doors. More than four or five increments may mean two projects.
See [increment-slicing.md](references/increment-slicing.md) for examples and bad cuts.

## 6. Define good for every step

**Per pass:** each pass has an exit condition above. That is its definition of
good. Do not move on by declaring it met; state how it is met.

**Per increment:** the record carries a definition of good as a hypothesis with five
parts: outcome, evidence, quality bar, out of scope, and reversibility. Evidence
must be reproducible or independently checkable rather than resting on your word,
and the quality bar names only the dimensions that apply, each with a threshold.

It becomes acceptance criteria only when `write-spec` promotes it. Do not
present the record as a specification or its hypotheses as accepted requirements.
A definition of good that would pass for any increment is not one. See
[definition-of-good.md](references/definition-of-good.md) for the rubric and a
strong and weak example side by side.

## 7. Write the shaping record, proportionally

Write after the person approves the direction and writing location; reuse approval
already given. At Standard and Deep depth, write one `docs/ideas/<slug>.md` in the
standard's idea shape: `id: IDEA-NNNN`, `title`, `type: idea`, `status: candidate`,
`created: YYYY-MM-DD`. Check existing identifiers before choosing the next number.
Project convention wins within the idea surface. At Quick depth the record may
stay in conversation. If files are declined, keep the conversation and report that
the Standard/Deep record remains unwritten.

Before a project repository exists, use `docs/ideas/` of the repository holding its
planning; move the record into the new repository when it is scaffolded.

Use [north-star.md](assets/north-star.md) for the record and its North Star section,
and [increment.md](assets/increment.md) as a section inside it, not separate specs.
Record the strategy, appetite, hypotheses, alternatives, assumptions and risks.
Direct Build has one increment; later outcomes, if warranted, remain brief intent.
Register the record in the repository's ideas index where one exists.

Reread for placeholders, contradictions, ambiguous hypotheses and scope exceeding
the appetite. Drop irrelevant template sections; do not fill them with "N/A".

## 8. Stop at the boundary

Designing something is not permission to build it. This holds from the moment the
skill engages, not only once a shaping record exists.

While the person has not asked you to build:

- Do not implement any of it.
- Do not commit, stage, or branch.
- Do not invoke another workflow, planning skill, or agent.
- Name `write-spec` as the next step to promote accepted requirements into a
  specification when warranted. Offer that handoff; do not invoke it. Obvious
  bounded work can go to an issue instead of manufacturing a specification.

When they do ask you to build, that is authorization. Take it, do not argue, do not
re-offer the design. Three things still hold:

- Preserve the shaping record and name the transition. When a specification is
  warranted, hand off to `write-spec` to consolidate accepted requirements
  before implementation; do not make this record a competing authority.
- Committing is separate from building. Commit when asked, or when project policy
  directs it for completed work. Never commit merely because shaping finished.
- In a guarded area, the project's guarded-change policy still applies. An agreed
  design is not the scoped approval that policy asks for, because the design is
  what you would be seeking approval of.

An instruction to build is authorization. Enthusiasm, a passing remark, and your
own sense that the shaping is finished are not.

## Resources

- [interview-passes.md](references/interview-passes.md): question banks, recap format, assumption ledger.
- [increment-slicing.md](references/increment-slicing.md): worked ladders, slicing heuristics, bad cuts, validation experiments.
- [definition-of-good.md](references/definition-of-good.md): the five-part rubric, quality-bar dimensions, strong versus weak examples.
- [north-star.md](assets/north-star.md): record template with the North Star and strategy.
- [increment.md](assets/increment.md): record section for one increment and its appetite.
