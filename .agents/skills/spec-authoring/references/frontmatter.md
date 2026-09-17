# Frontmatter for a specification and its children

The fields, the vocabulary, and which of the optional fields are worth maintaining. This file
covers `spec` and its child types only. Other document types have their own obligations.

## Required on a `spec`

| Field | Value |
| --- | --- |
| `id` | `SPEC-NNNN`, stable across a title or slug change |
| `title` | Human-readable title |
| `type` | `spec` |
| `status` | From the specification vocabulary below |
| `created` | `YYYY-MM-DD` |

## The specification status vocabulary

```text
draft
accepted
implementing
implemented
superseded
abandoned
```

Typical lifecycle: `draft` to `accepted` to `implementing` to `implemented`. The terminal
transitions that also exist: `draft` or `accepted` to `abandoned`, and `implemented` or `accepted`
to `superseded`.

Statuses are a closed vocabulary. Do not invent one, and do not use a value from another document
type's list. A specification is never `current`, `proposed` or `candidate`; those belong to
architecture documents, ADRs and ideas.

One case is worth naming because it reads like a contradiction and is not: a specification at
`implementing` with v0 complete and v1 open is the normal case. The milestone headings carry that
detail. Milestones are not statuses.

A cross-repository specification is the one document whose status depends on state outside its own
repository: it reaches `implemented` only when every routed issue is closed.

## Child documents carry no identity of their own

`design`, `research` and `verification` are children. Each carries a `spec:` holding the parent's
identifier and carries no `id` and no `status`:

```yaml
---
title: Authentication Design
type: design
spec: SPEC-0012
---
```

Its identity is the parent's identifier plus its filename, written `SPEC-0012/design` where a
cross-reference needs one. Its lifecycle is the parent's. This is what keeps `id` unique across a
repository, which it would not be if a child inherited the parent's `id` literally.

## Optional fields worth carrying

- **`owners`.** Who is answerable for the content, not who wrote it. One entry is the normal case
  and the useful one; where more than one name is genuinely right, the first is answerable.
- **`discussion`.** A link to where the argument happened: the pull request, issue or thread. It
  belongs on the types that are proposed and argued before they settle, which includes `spec`. A
  record of a decision that does not point at the argument leaves a reader unable to tell what was
  considered.
- **`related`.** A list of `relation` and `target` pairs, never buckets keyed by target kind. The
  relations a specification declares: `implemented-by` an issue, `informed-by` research,
  `constrained-by` a decision. Targets must be resolvable from outside the document, so a bare `142`
  names nothing: qualify it as `argus#142`, and qualify a document identifier that crosses a
  repository as `lexicon:SPEC-0004`.
- **`depends_on`.** A separate top-level field, not a `related` relation. A list of identifiers.
- **`supersedes` and `superseded_by`.** Set both sides when a specification replaces another, so
  either record leads to the other.

## Fields that look useful and are not

- **`updated`.** Not required, deliberately. Git knows when the file changed and knows it
  correctly, and a field every change has to remember to touch is the field most likely to be
  wrong. It stays available for the narrow case where a document's own sense of currency differs
  from its last commit.
- **`reviewed`.** Required only on a threat model and on documents a repository has enrolled in a
  declared review policy. A date nobody refreshes reads as assurance, which is worse than no field.
  Set it only where a cadence actually exists.

## Absent and empty mean different things

| Value | Means |
| --- | --- |
| Field absent | Not applicable to this document type or state. Nothing is claimed |
| `[]` or `null` | Applicable, considered, and there genuinely is none |

This distinction is what lets a validator ask a question only of the documents that owe an answer.
Because most relations are optional, an absent relation proves nothing: it narrows a question about
absence and never answers it. Do not build a report on the assumption that a missing `related`
entry means the relationship does not exist.
