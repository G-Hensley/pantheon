# The issue body, and the templates a repository carries

## The body shape

The receiving repository's contract decides the sections. Where it has one, write that
([repository-contract.md](repository-contract.md)); where it states none, use the five-section body
in [contractless-body.md](contractless-body.md). The finished condition below applies to either, as
`Done when` in a repository's contract or `Acceptance Criteria` in the fallback.

## The finished condition

The one section worth arguing about. It is not a restatement of the title and it is not a list of
steps.

| Weak | Workable |
| --- | --- |
| Implement session expiry | Sessions issued before the change expire on the next request; `test_session_expiry` covers both sides of the boundary |
| Improve the checker | The checker fails on a documentation index describing a path that does not exist, with a negative fixture proving it |
| Update the docs | `docs/README.md` names every entry under `docs/`, and the pre-standard layout section is gone |

The test: could two people disagree about whether it is done? If yes, it is not a finished
condition yet.

## Traceability

The chain the standard asks a repository to make possible:

```text
Requirement -> Specification -> Design -> Decision -> Contract -> Issue -> PR -> Implementation -> Tests
```

Not every change needs every layer. The objective is sufficient traceability without unnecessary
process, and the two links that carry most of the value are the issue naming its `FR-`, and the
specification's routing table naming the issue.

## The required issue-template set

A repository carries four entries under `.github/ISSUE_TEMPLATE/`, and they are a rule rather than
an illustration:

| File | What it is |
| --- | --- |
| `work-item.yml` | A unit of work with an explicit condition for being finished |
| `bug.yml` | A specific observed defect |
| `security.yml` | Security work: hardening, baseline, dependency and permission policy |
| `config.yml` | GitHub's chooser configuration, which is not a template at all |

A repository MAY add its own. Narrower variants of `work-item.yml` are the ones to avoid: three
overlapping work forms make the filer choose between them at the moment they know least, and that
distinction is a label or a project field rather than a form.

`config.yml` earns its place because it is easy to miss. It holds `blank_issues_enabled` and
`contact_links`, so it decides whether a blank issue is possible and where a security
vulnerability is sent. Its absence is a decision made by default.

**`security.yml` is for security work, not vulnerability reports.** Where `config.yml` routes
vulnerabilities to a private advisory, a form named "security" sits next to a contact link for
security, and a filer holding a vulnerability will plausibly pick the form. So `security.yml`
states in its first block that a vulnerability goes to the advisory instead, before it asks for
anything. A template that has to turn away one kind of filer does it in the first thing they read,
or it does not do it at all.

## Bugs do not usually need a specification

```text
Bug -> Issue -> Fix -> Pull Request -> Verification
```

Two cases change that. Where a bug reveals a missing or incorrect requirement, the specification is
updated first and the issue cites it. Where resolving it needs a significant architectural
decision, an ADR comes first, then the specification or design update, then the issue.
