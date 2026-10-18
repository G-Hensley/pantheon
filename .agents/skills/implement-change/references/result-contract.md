# Result and reviewer-brief contract

Fill every key. Use `none: <reason>` or an empty collection when a value does
not apply. Do not replace a key with prose elsewhere in the response. Return
only this shape, with no heading, summary, or other prose before or after it.

```yaml
status: implemented | partially-implemented | blocked | incomplete
target:
  repository:
  branch_or_worktree:
  base:
  change_locator:
  committed: true | false
accepted_behavior:
plan:
plan_record:
changes:
focused_validation:
final_validation:
approvals:
needed_to_advance:
findings_outstanding:
residual_risk:
scope_exclusions:
follow_ups:
review_status:
  state: not-requested | pending | completed
  evidence:
reviewer_brief:
  original_request_or_spec:
  change_locator:
  scope:
  exclusions:
  authority:
  acceptance_criteria:
  plan_context:
  validation_evidence:
  known_risks:
  findings_outstanding:
  requested_lenses:
```

The reviewer brief is independently usable. Repeat information inside it even
when the same fact appears in another result field. "See above", "prepared
above", and equivalent pointers fail the contract.

For an uncommitted change, identify the base commit and working-tree diff. For a
committed change, identify the exact commit or range. Never imply that an
uncommitted diff is immutable.

`implemented` requires passing focused and final validation. Equivalent
commands remain separate evidence when the repository-required command was
unavailable and no instruction authorized substitution. Run a required command
exactly as written; adding a test path, marker, case, or package makes it a
different command and only focused evidence.

For `partially-implemented`, `blocked`, or `incomplete`,
`needed_to_advance` names the exact safe next slice, live approval, decision,
evidence, or validation required. For `implemented`, use `none` with a reason.
