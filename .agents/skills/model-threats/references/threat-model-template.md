# The threat model document, and the README beside it

## `docs/security/threat-model.md`

```markdown
---
id: THREAT-MODEL-GATEWAY
title: Gateway Threat Model
type: threat-model
status: current
created: 2026-09-16
reviewed: 2026-09-16
---

# Gateway Threat Model

## Scope

Inside the boundary:

- the public HTTP listener and its TLS termination
- the request authentication path
- the token cache

Outside the boundary, with reasons:

| Excluded | Why |
| --- | --- |
| The upstream identity provider | Operated by a third party under contract; analysed in their own model |
| Physical access to the host | The deployment target is a managed runtime with no operator shell |
| Supply chain of build dependencies | Covered by the repository's dependency policy, not by this analysis |

A risk outside the boundary is recorded here and nowhere else. It does not become a finding with a
treatment, because the model did not analyse it.

## Attacker model

Assumed hostile:

- an unauthenticated network client reaching the public listener
- an authenticated user of one tenant attempting to reach another tenant's data

Assumed capable of: sending arbitrary requests, replaying observed tokens within their lifetime,
and reading anything published in the public repository.

Assumed not capable of: reading host memory, altering the deployed image, or compelling the identity
provider.

## Revision analysed

`a3f19c4e0b7d2f6a8c15e93b04d7f2a6c8e1b503`

## Findings

### THREAT-0001 Token cache is keyed without the tenant

**Treatment:** mitigated
**Response state:** done
**state_updated:** 2026-09-14
**Control:** the cache key includes the tenant identifier
**Evidence:** `src/gateway/cache.py:58`, covered by `tests/test_cache_isolation.py`

An authenticated user of one tenant could otherwise receive a cached response issued for another.

### THREAT-0002 Replayed token remains valid until expiry

**Treatment:** accepted
**Response state:** open
**state_updated:** 2026-09-16
**Risk owner:** Gavin
**Residual risk:** a token observed in transit is usable for its remaining lifetime, at most 15 minutes
**Review trigger:** when token lifetime is raised above 15 minutes, or when the listener is reachable
from outside the managed network

...
```

Split the file per trust boundary when one file stops being readable. Splitting earlier than that
costs a reader the ability to see the whole boundary at once.

## `docs/security/README.md`

This one is not optional. It carries the three things the model itself does not:

```markdown
# Security Documentation

## What is modelled

`threat-model.md` covers the gateway trust boundary. The worker pool and the scheduled agent runs
are not modelled yet.

## Artifact handling

This repository uses **private analysis**: the threat model stays out of the publishable tree, and
only the structural description in `docs/architecture/security.md` is committed here.

## Re-review trigger

This model is re-reviewed on any of:

- any change to a trust boundary in `docs/architecture/security.md`
- any new or changed external interface
- any new privileged component, identity or secret consumer
- any change to the attacker model
- otherwise, every 180 days
```

A repository that has not declared an artifact-handling option uses private analysis, because that
is the only option that is safe while undecided.

## `docs/security/data-classification.md`

The classification vocabulary, fixed once. The diagram field that carries it is an unconstrained
string, so without this file it becomes six spellings of one idea inside a year.

```markdown
| Class | Meaning | Examples |
| --- | --- | --- |
| `public` | Published or publishable as is | Version numbers, the public API surface |
| `internal` | Not secret, not for publication | Request counts, internal service names |
| `sensitive` | Harmful if disclosed | Customer identifiers, job payloads |
| `secret` | Credential or key material | API tokens, signing keys |
```

Use nothing outside it, on the same argument that fixes the status vocabularies.
