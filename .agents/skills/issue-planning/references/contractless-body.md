# The five-section body, for a repository with no issue contract

Use this only where the receiving repository states no issue contract: no body sections in its
`AGENTS.md` or contributing guide, no `.github/ISSUE_TEMPLATE/` work form, no checker. Where one
exists, [repository-contract.md](repository-contract.md) wins and this block is not written at all.

```markdown
## Specification

SPEC-0012

## Requirements

- FR-002
- FR-004

## Design

docs/specs/0012-authentication/design.md

## Acceptance Criteria

Observable conditions that must hold before this closes.

## Verification

How they were shown to hold: the command, the test, the output.
```

`Design` is omitted where the specification has none. `Verification` is filled in as the work
finishes rather than at filing time, and an empty heading is better than no heading, because it is
the section most often skipped once the code works.

`Acceptance Criteria` carries the finished condition described in [issue-body.md](issue-body.md).
