# Findings: treatment, response state, and acceptance

## The entry

```markdown
### THREAT-0007 Worker scratch directory is shared with the renderer

**Treatment:** mitigated
**Response state:** in-progress
**state_updated:** 2026-09-16
**target_date:** 2026-10-01
**Control:** per-job scratch directory created with mode 0700
**Evidence:** SPEC-0014 FR-003, issue #212
```

A stable identifier, exactly one treatment, one response state, and a date on the state. Everything
else depends on the treatment.

## Treatment is the decision

Every finding has exactly one. Four values, and each one owes something:

| Treatment | What it means | What it requires |
| --- | --- | --- |
| `mitigated` | A control reduces the risk to an acceptable level | The control named, and evidence it is in place: a test, a configuration, a code path, or a documented procedure. A control nobody can point at is not a control |
| `avoided` | The activity or component creating the risk is removed, or never built | What was removed or declined, and where that is recorded. Avoidance is the only treatment that ends with less system, which is why a reader will otherwise mistake it for a scope exclusion |
| `transferred` | Another party's control handles it | The party named, and the contract or agreement that makes it their obligation. Transfer does not remove the vulnerability, so it never implies the detail is safe to publish |
| `accepted` | The risk is knowingly carried | A named risk owner, the residual risk stated, and a review trigger |

Two words deliberately absent. Sharing a risk is folded into `transferred` rather than asking an
author to tell the two apart. `out-of-scope` is not a treatment at all: it describes where the
model's boundary is, not what was decided about a risk, so it belongs in the Scope section. Passive
non-treatment is not one of the choices; a risk being carried on purpose is `accepted`, which has a
name attached to it.

## Response state is progress

```text
open
in-progress
done
```

It applies to all four treatments, because every one of them can be decided before it is done: a
control can be specified and unbuilt, a component can be scheduled for removal, a contract can be
drafted and unsigned, and an acceptance can be waiting on its owner.

`state_updated` is required and records when the response state last changed. It is the field that
makes a finding which has been `in-progress` for eleven months visible as one. `target_date` is
optional. Neither is the document's `reviewed` date: that is about the analysis, these are about one
finding.

A `current` threat model may contain `open` findings. That is the normal condition of a system being
worked on, and a model that cannot express it forces the author to choose between marking the
analysis stale and pretending the work is finished.

Where remediation work exists, it is an issue and the finding cites it. Where the control already
existed, there is nothing to cite and the evidence is the control itself.

## Accepting a risk

`accepted` is the treatment worth being strict about, because the document is the only thing
standing between a known risk and nobody ever looking at it again.

Three things, all three required:

1. **A named risk owner.** Without one, acceptance means the person who wrote the threat model
   accepted it on the project's behalf, which is not how that decision should be made.
2. **The residual risk, in plain terms.** What is still true after the acceptance.
3. **A review trigger.** Without one, a risk accepted under one set of conditions stays accepted
   after those conditions change.

The trigger may be time-driven or event-driven, and both are first class. A date is the easy case.
An event is the better one wherever a date would be arbitrary, and it carries one extra obligation:
it has to be **observable**, so somebody can tell whether it has fired.

| Not a trigger | A trigger |
| --- | --- |
| When the service gets popular | When this endpoint becomes reachable without authentication |
| If the risk increases | When token lifetime is raised above 15 minutes |
| Next year sometime | 2027-03-01, or when the worker pool scales past one instance |

## When an acceptance becomes a decision record

Write an ADR when the acceptance is durable and architecturally significant, which is what a
decision record is for.

Do not write one for every accepted finding. An ADR records that a decision was made and by what
reasoning; it does not by itself establish that the right person made it. The owner field is what
does that.
