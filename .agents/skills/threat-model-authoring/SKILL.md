---
name: threat-model-authoring
description: Write a threat model under docs/security/ with a stated scope, an attacker model, a pinned revision SHA, and findings that each carry one treatment and a response state. Use when a system or a trust boundary needs analysing against an attacker, when someone asks for a threat model, STRIDE, trust boundaries, THREAT-NNNN, risk acceptance or a security review, or when a change to a boundary has made an existing model stale.
---

# Threat Model Authoring

Produce `docs/security/threat-model.md`: an analysis of what an attacker could do to the system,
pinned to the revision it was performed against, ending in findings that each go somewhere.

An architecture document describes what is. A threat model argues about it and produces work. The
two go stale on different triggers, which is why they are separate documents.

## Do not run this skill when

The task is describing the security structure rather than attacking it. Trust boundaries,
authentication, authorization, where secrets live and what data is sensitive are a standing
description, and they belong in `docs/architecture/security.md`.

Nor is this where security behavior is accepted. A threat model analyses a system and produces
findings; a specification commits to behavior. A threat model cannot accept behavior, and a
specification cannot substitute for the analysis.

## 1. Create the directory only when there is something to keep

```text
docs/security/
├── README.md                 what is modelled, what is not, the re-review trigger, and the
│                             artifact-handling option this repository uses
├── threat-model.md           the analysis; split per trust boundary when one file stops being readable
└── data-classification.md    the classification vocabulary this repository uses
```

Most repositories will not have one. A repository promising trust-boundary tests needs one, because
a trust boundary cannot be tested before it is written down.

## 2. Write the frontmatter

```yaml
---
id: THREAT-MODEL-GATEWAY
title: Gateway Threat Model
type: threat-model
status: draft
created: 2026-09-16
reviewed: 2026-09-16
---
```

`reviewed` is required here and optional almost everywhere else, because staleness is this
document's whole failure mode and nothing else detects it. Statuses are `draft`, `current`, `stale`,
`superseded`.

## 3. Write the four required contents

In this order, because each one makes the next meaningful.

**Scope.** What is inside the boundary being modelled and what is outside it, with a reason for each
exclusion. An unstated scope is the most common defect in a threat model, because every omission
then reads as an oversight rather than a decision. A risk outside the boundary is recorded here and
nowhere else: it is not a finding with a treatment, because the model did not analyse it.

**The attacker model.** Who is assumed hostile and what they are assumed to be able to do. A finding
only means something relative to this.

**The revision it was performed against**, as a commit SHA. The pin is provenance, not an expiry: it
says which structure was analysed, so a reader can tell whether the thing in front of them is the
thing that was modelled.

**Findings.** Each with a stable identifier such as `THREAT-0007`, exactly one treatment, and a
response state.

## 4. Give every finding one treatment and one response state

Conflating them is why threat models rot. "What have we decided to do about this risk" and "has it
happened yet" are different questions, and a finding needs both answered.

| Treatment | What it means | What it requires |
| --- | --- | --- |
| `mitigated` | A control reduces the risk to an acceptable level | The control named, and evidence it is in place: a test, a configuration, a code path, or a documented procedure |
| `avoided` | The activity or component creating the risk is removed, or never built | What was removed or declined, and where that is recorded |
| `transferred` | Another party's control handles it | The party named, and the contract that makes it their obligation |
| `accepted` | The risk is knowingly carried | A named risk owner, the residual risk, and a review trigger |

Response state is progress, and it applies to all four, because every one of them can be decided
before it is done: `open`, `in-progress`, `done`. Carry `state_updated` with it, which is what makes
a finding that has been `in-progress` for eleven months visible as one. `target_date` is optional.
Neither is the document's `reviewed` date: that one is about the analysis, these are about one
finding.

A `current` threat model may contain `open` findings. That is the normal condition of a system being
worked on. Where remediation work exists it is an issue and the finding cites it; where the control
already existed there is nothing to cite and the evidence is the control itself.

`out-of-scope` is not a treatment. It describes where the boundary is, so it belongs in Scope.

See [findings.md](references/findings.md) for the finding entry shape and what accepting a risk
requires.

## 5. Move the status the moment an input changes

- `draft` to `current` when scope, attacker model and findings can be relied on.
- `current` to `stale` **the moment a modelled input changes, in the same change that causes it**,
  before any re-review happens. The window between the system changing and somebody re-reading the
  model is exactly when a `current` threat model is lying.
- `stale` back to `current` by re-reviewing against the new revision and updating the pinned SHA. A
  re-review that changes no finding is still a re-review, and `reviewed` records it.
- `current` or `stale` to `superseded` when the boundary itself is redrawn, so the old analysis is
  of a system that no longer exists in any recognizable form. It then supersedes reciprocally, the
  way a decision record does.

The triggers go in `docs/security/README.md`: any change to a trust boundary in
`docs/architecture/security.md`; any new or changed external interface; any new privileged
component, identity or secret consumer; any change to the attacker model; otherwise the repository's
declared interval.

## 6. Decide publication separately from treatment

**Publication is never derived from a finding's treatment.** The tempting shortcut is the dangerous
one: `mitigated` does not mean the detail is harmless, because a control can be incomplete,
misconfigured, or bypassable in a way the write-up reveals. `transferred` leaves the vulnerability
present. `avoided` closes one path and says nothing about the others the write-up maps.

Two separate questions:

- **Artifact handling**, a standing policy set once per repository in `docs/security/README.md`:
  private analysis, reduced analysis, or reviewed publication. A repository that has not declared
  one uses private analysis, because that is the only option that is safe while undecided.
- **Vulnerability disclosure**, decided per finding: no disclosure, private, limited, or full.

[disclosure.md](references/disclosure.md) has both vocabularies, when silence stops being
defensible, and where an unfixed undisclosed finding is allowed to live.

## 7. Use two diagrams, not one

Trust boundaries exist only in the architecture diagram schema, so a threat model that wants
boundaries and data classes needs both:

| What it shows | How |
| --- | --- |
| Trust boundaries and privileged components | `diagram_type: architecture`, `boundaries` entries of `kind: "security-group"`, and `type: "security"` on the controls |
| What data moves where, and of what class | `diagram_type: dataflow`, with `classification` on each flow |

`classification` is an unconstrained string, so it drifts into six spellings of one idea unless the
repository fixes a vocabulary. Declare it in `data-classification.md` and use nothing outside it.

An architecture connection's `variant` holds one value, so an edge cannot be both `security` and
`dashed`. Where an edge is both security-relevant and not built yet, the prose beside the diagram
carries the second.

## 8. Check before proposing

- `type: threat-model`, `status` from its vocabulary, and `reviewed` present.
- Scope states exclusions with a reason each.
- The attacker model is stated, not implied.
- A commit SHA pins the revision analysed.
- Every finding has an identifier, exactly one treatment, a response state, and `state_updated`.
- Every `mitigated` finding names a control somebody can point at.
- Every `accepted` finding names an owner, the residual risk, and an observable review trigger.
- No exploitable detail for an unfixed undisclosed finding is reachable from a public repository.
- `docs/security/README.md` declares the artifact-handling option and the re-review trigger.

## Reference Files

- [threat-model-template.md](references/threat-model-template.md) the document skeleton, with the
  README that has to sit beside it.
- [findings.md](references/findings.md) the finding entry, the treatment and response-state split,
  and what accepting a risk requires.
- [disclosure.md](references/disclosure.md) artifact handling, per-finding disclosure, when to
  disclose anyway, and where an undisclosed finding lives.
