---
name: architecture-docs
description: Write or correct a document under docs/architecture/ in the Repository Documentation Standard's shape, declaring every element's state as observed, proposed or unbuilt, and keeping document status separate from element state. Use when describing a system as a whole, when someone asks for architecture docs, a context, building-blocks, runtime, deployment, data or risks document, an ARCH- identifier, a C4 view or a system map, or when an existing architecture document no longer matches the system.
---

# Architecture Docs

Produce a document under `docs/architecture/` that describes the system as a whole and says, for
every element in it, whether that element exists.

Most projects in this tree are partly built. A document restricted to what exists would be empty;
one that describes an intention as though it were running is worse than no document at all. The
state labels are what make a document about an unbuilt system honest rather than fictional.

## Do not run this skill when

The subject is one feature rather than the system. A proposal for a single change is a
specification, and its design argument is that specification's `design.md`. Architecture
documentation is the standing map, not the proposal.

Nor does a contract table belong here. An API, a schema or a configuration surface is a contract,
and it lives where the repository keeps those.

## 1. Choose the document

The recommended set, each with its own subject. Write the one the question needs; a repository
rarely needs all seven at once.

| File | Subject |
| --- | --- |
| `context.md` | System boundaries, users and actors, external dependencies, upstream and downstream systems, major trust boundaries |
| `building-blocks.md` | Major services, modules, packages, their responsibilities, and the important dependencies between them |
| `runtime.md` | Important runtime interactions and sequences: request processing, authentication, asynchronous jobs, event processing, agent workflows |
| `deployment.md` | Environments, infrastructure, topology, networks, containers, cloud resources, runtime dependencies |
| `data.md` | Persistent stores, schemas, ownership, lifecycle, consistency requirements, important data flows. Only where data architecture is substantial |
| `security.md` | Trust boundaries, authentication, authorization, secrets, privileged components, sensitive data, threat assumptions |
| `risks.md` | Known architectural risks and meaningful technical debt |

Two of those have neighbours that are easy to confuse them with:

- `architecture/security.md` **describes** the security structure in the same descriptive voice as
  every other document here. The analysis of what an attacker could do with that structure is a
  threat model, it has a different lifecycle, and it lives under `docs/security/`.
- `risks.md` is architectural risk and debt. It is not a bug backlog, and using it as one is how it
  stops being read.

## 2. Write the frontmatter

```yaml
---
id: ARCH-SECURITY
title: Security Architecture
type: architecture
status: current
created: 2026-09-16
---
```

`id` is stable and does not change when the title or the filename does. `type` is `architecture`.
`status` comes from the architecture vocabulary and nowhere else:

| Status | Meaning |
| --- | --- |
| `current` | The document is valid and matches the system |
| `stale` | Known not to match the system, and it cannot be corrected in this change |
| `superseded` | Replaced by another document |

`stale` is set deliberately, as a signal to the next reader. A document left at `current` while
known to be wrong is the actual failure; marking it `stale` is not one.

## 3. Declare the state of every element

Every material element declares one of three states:

| State | Meaning |
| --- | --- |
| `observed` | Verified to exist and behave this way. Say how it was verified |
| `proposed` | Accepted in a specification or an ADR, not yet built |
| `unbuilt` | Named so the map is complete. No accepted specification yet |

Two obligations ride on those labels: a `proposed` element owes `governed_by` naming the
specification or ADR that accepted it, and an `observed` element owes its source evidence wherever
the format can carry it. Neither is decoration. `proposed` without a governing document means
somebody's intention, not an accepted one; `observed` without evidence is an assertion.

## 4. Put the label at the element, not in the sentence

The rule, stated so it can be checked: every material element has an unambiguous state, declared at
the smallest structure that contains it, which is a document, a section, a table row, a component,
or an edge.

```markdown
## Policy Service

**State:** proposed
**Governing spec:** SPEC-0017

The policy service receives authorization requests from the gateway and ...
```

rather than:

```markdown
The API gateway [observed] forwards requests to the policy service [proposed], which will
eventually communicate with the event bus [unbuilt].
```

Inline labels are allowed where one sentence genuinely mixes states and nothing smaller contains
them. That should be rare, and a section full of them is a section that wanted to be a table.

## 5. Keep document status and element state apart

Two axes. Conflating them produces wrong conclusions in both directions.

| Axis | Question it answers | Values |
| --- | --- | --- |
| Document `status` | Is this document itself valid and current? | `current`, `stale`, `superseded` |
| Element state | Does the thing being described exist? | `observed`, `proposed`, `unbuilt` |

A `status: current` document can be full of `proposed` and `unbuilt` elements, and in an early
repository most of them will be. It is current precisely because it accurately says those elements
do not exist yet. Do not reason "it contains proposed elements, so its status should be proposed":
there is no such status, and the document is not a proposal. It is an accurate description that
contains proposals.

The inverse holds too. A document describing only `observed` elements goes `stale` the moment one
of them is deleted, even though every label in it was right when written.

## 6. Carry the state into any diagram

Where the document has a diagram, the state goes in the intermediate representation wherever the
format can hold it, and in the prose beside the diagram everywhere else, under the same
smallest-containing-structure rule. The gaps are on the things between the nodes: edges and
sequence participants have no field for state, so a diagram whose edges are all `proposed` says so
in one sentence above the image.

See [diagram-state.md](references/diagram-state.md) for which field holds what.

## 7. Register the document

Add it to `docs/README.md`, which names every entry under `docs/`. A document nobody can find from
the index is a document that will be rewritten from scratch by the next person.

## 8. Correct rather than accumulate

An architecture document's failure mode is silent staleness, and the cheapest moment to fix it is
while looking at it. When you find an element whose state is now wrong:

1. Correct it in place where you can verify the new state.
2. Where you cannot verify it in this change, set the document `status: stale` and say in one line
   which element is in doubt.
3. Where the element has been replaced rather than changed, mark the document `superseded` and name
   the replacement.

Do not leave a half-corrected document at `current`.

## 9. Check before proposing

- `type: architecture`, and `status` one of `current`, `stale`, `superseded`.
- Every material element has exactly one state, at the smallest structure containing it.
- Every `proposed` element names its `governed_by`. Every `observed` element cites its evidence.
- No inline state markers except where one sentence genuinely mixes states.
- `architecture/security.md` describes structure; no attacker analysis has leaked into it.
- The document is listed in `docs/README.md`.

## Reference Files

- [document-set.md](references/document-set.md) a skeleton for each of the seven documents, with
  the state table and what belongs in each.
- [diagram-state.md](references/diagram-state.md) where a state label goes in diagram IR, which
  elements cannot carry one, and what the prose beside the diagram has to say.
