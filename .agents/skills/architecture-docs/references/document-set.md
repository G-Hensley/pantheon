# The architecture document set

Seven recommended documents. Write the ones the repository's questions need; an empty
`deployment.md` is worse than an absent one.

Every one of them carries the same frontmatter shape and the same obligation to state each
element's state.

```yaml
---
id: ARCH-CONTEXT
title: System Context
type: architecture
status: current
created: 2026-09-16
---
```

## The state table

Put it once, near the top, so a reader knows what the labels mean before meeting one:

| State | Meaning |
| --- | --- |
| `observed` | Verified to exist and behave this way |
| `proposed` | Accepted in a specification or an ADR, not yet built |
| `unbuilt` | Named so the map is complete. No accepted specification yet |

## `context.md`

System boundaries, users and actors, external dependencies, upstream and downstream systems, major
trust boundaries.

```markdown
## Actors

| Actor | State | Evidence or governing document |
| --- | --- | --- |
| Operator (human, CLI) | observed | `src/cli/main.py` |
| Scheduled agent run | proposed | SPEC-0009 |

## External dependencies

### GitHub API

**State:** observed
**Evidence:** `src/clients/github.py`, exercised by `tests/test_github_client.py`

...
```

A trust boundary named here is the same boundary the threat model works from. Naming it in one
place and describing it differently in the other is the failure this document set exists to
prevent.

## `building-blocks.md`

Major services, modules, packages, their responsibilities, and the important dependencies between
them.

The temptation is to list every directory. Resist it: a file tree is generated in seconds and says
nothing about responsibility. A building block earns a section when a reader needs to know what it
is answerable for.

## `runtime.md`

Important runtime interactions and sequences: request processing, authentication flow, asynchronous
jobs, event processing, agent workflows.

Sequences are where diagram IR cannot carry state at all, so the prose does. One sentence above the
diagram, naming which participants and which steps are not yet built.

## `deployment.md`

Environments, infrastructure, deployment topology, networks, containers, cloud resources, runtime
dependencies.

This document goes stale faster than any other except `security.md`, which is why both are named in
the standard as candidates for a declared review cadence. Where the repository has declared one,
this document carries `reviewed`. Where it has not, leave the field out rather than setting it once.

## `data.md`

Persistent stores, schemas, ownership, lifecycle, consistency requirements, important data flows.
Write it when data architecture is substantial, not by default.

Ownership is the field most often skipped and most often needed: which component is allowed to write
this store.

## `security.md`

Trust boundaries, authentication, authorization, secrets, privileged components, sensitive data,
threat assumptions.

Describe is the operative word. This is the structure, in the same descriptive voice as every other
document here.

| Belongs here | Belongs in the threat model |
| --- | --- |
| The gateway terminates TLS and forwards an internal token | An attacker with the internal token can reach the admin API |
| Secrets come from the host keyring | The keyring is unlocked for the session lifetime, so a local process can read them |
| The worker runs as an unprivileged user | The worker's writable scratch directory is shared with the renderer |

The left column is standing description. The right column is dated analysis with a revision and a
set of findings, and it belongs under `docs/security/`.

## `risks.md`

Known architectural risks and meaningful technical debt.

```markdown
## Single writer assumption in the job table

**State:** observed
**Evidence:** `src/queue/claim.py:41`

Two workers claiming concurrently would double-run a job. The current deployment runs one worker,
so the risk is latent rather than live; it becomes live the first time the deployment scales.
```

A risk entry says what would have to change for it to bite. Without that, it reads as an apology
rather than a risk.

Do not use this as a generic bug backlog. A specific observed defect is an issue.
