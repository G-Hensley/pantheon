# Pantheon tracking migration map

Status: staged. Activation is governed by [the issue workflow](github-issues.md)
and the cutover issue identified by marker
`G-Hensley/pantheon:cutover-activation-pending`. Its staged publication is
https://github.com/G-Hensley/pantheon/issues/70.

This map preserves source identities and reviewed dispositions. It is not a
second live task tracker. GitHub Issues owns deliverable identity, acceptance
and resolution as soon as the activation checkpoint's step 5 records verified
activation; the repository's selected GitHub Project owns stage and priority
for the whole queue only once step 7 also completes and general claims
resume. Historical `.tasks/` files remain byte-identical and read-only from
step 5 onward.

Inspected main: `5cd35880b71658abcf69d6077e41196cb164e846`. Inventory: 38
records and this repository's own documents plus 19 task files as bounded
sources. Approved packet: 23 issues. The reviewed publication journal binds
all 23 issue URLs and Project item identities. Re-read the current issues,
Project items, fields, and dependencies at activation.

A small number of reviewed source records are genuinely `duplicate`: retained
work already owned by an existing parent record in a private cross-project
ledger. That ledger is not reproduced here, so they appear below with a
generic private-owner explanation and no link, not as `declined` (this
repository does not reject that work; it simply is not the owner).

## Owning issues

| Stable key | Title | Proposed Project fields | Issue |
| --- | --- | --- | --- |
| `3xcwdb` | Raise the dispatch brief limit with verified host-input coverage | Ready / P1 | https://github.com/G-Hensley/pantheon/issues/51 |
| `review-identity-disclosure` | Decide whether reviews should mask the implementer identity | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/52 |
| `e67nyf` | Define the product boundary for free-tier spending limits | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/53 |
| `kyzzsx` | Evaluate fallback MCP identity assurance against the documented threat model | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/54 |
| `q0h7c4` | Notify the conductor when panes are idle | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/55 |
| `rg3wmk` | Define model capability profiles and their evidence requirements | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/56 |
| `t9x41e` | Explain interactive versus headless timeout behavior for local models | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/57 |
| `y4fz0h` | Extend headless dispatch to the other supported CLIs | Backlog / P2 | https://github.com/G-Hensley/pantheon/issues/58 |
| `zmk94k` | Measure whether disabling tool definitions reduces context use | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/59 |
| `model-health-state` | Report model health independently of the failing pane | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/60 |
| `context-window-visibility` | Add context reset with an open-task interlock | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/61 |
| `model-switch-recovery` | Add controlled recovery by switching a pane to another model | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/62 |
| `context-window-occupancy` | Report estimated context-window occupancy per pane | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/63 |
| `shared-idea:local-model-trust-gradient` | Decide whether per-backend trust profiles should restrict weak-model panes | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/64 |
| `shared-idea:squad-comparison` | Triage task-lifecycle presentation and recovery improvements | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/65 |
| `shared-idea:ask-conductor-escalation-predicate` | Define escalation triggers for ask_conductor | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/66 |
| `shared-idea:coordinated-rollback-decision` | Evaluate coordinated code and conversation rollback | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/67 |
| `workspace-reap` | Recover unreferenced worktrees and show branch, diff and preserved paths | Backlog / P2 | https://github.com/G-Hensley/pantheon/issues/68 |
| `known-gaps-bundle` | Triage five remaining README limitations | Backlog / P3 | https://github.com/G-Hensley/pantheon/issues/69 |
| `cutover-activation-pending` | Prepare and activate GitHub issue tracking | Ready / P1 | https://github.com/G-Hensley/pantheon/issues/70 |
| `s9xc2s` | Verify the dispatch-allowance reset in an isolated app fixture | Review / P2 | https://github.com/G-Hensley/pantheon/issues/71 |
| `rd85p4` | Verify project-scoped pane restoration across an app restart | Review / P2 | https://github.com/G-Hensley/pantheon/issues/72 |
| `jz8nsh` | Verify session approval and first-task acknowledgement end to end | Review / P1 | https://github.com/G-Hensley/pantheon/issues/73 |

The proposed fields above are staged metadata, not permission to execute. The
reviewed publication journal binds each key to its issue URL and Project item
ID and records verified Status and Priority fields. Re-read those fields and
dependency relationships before activation.

## Source record dispositions

All 38 reviewed record keys are accounted for below. Multiple records may map
to one issue. Completed, duplicate, declined, and historical records are not
recreated as implementation work.

| Source key | Disposition | Owning issue, evidence, or historical reason |
| --- | --- | --- |
| `pantheon:6v3ebz` | completed | `.tasks/6v3ebz.json`, status done; PR [#17](https://github.com/G-Hensley/pantheon/pull/17) |
| `pantheon:7c7f3j` | completed | `.tasks/7c7f3j.json`, status done; PR [#25](https://github.com/G-Hensley/pantheon/pull/25) |
| `pantheon:d4nhzh` | completed | `.tasks/d4nhzh.json`, status done |
| `pantheon:d575h4` | completed | `.tasks/d575h4.json`, status done; PR [#16](https://github.com/G-Hensley/pantheon/pull/16) |
| `pantheon:dq3s0j` | completed | `.tasks/dq3s0j.json`, status done |
| `pantheon:e18r66` | completed | `.tasks/e18r66.json`, status done |
| `pantheon:gre41p` | completed | `.tasks/gre41p.json`, status done; PRs [#29](https://github.com/G-Hensley/pantheon/pull/29)/[#30](https://github.com/G-Hensley/pantheon/pull/30) |
| `pantheon:qf7d1f` | completed | PR [#47](https://github.com/G-Hensley/pantheon/pull/47); the task's own `done_when` does not require live-pane observation |
| `pantheon:pr-42-dependabot-rust` | completed | PR [#42](https://github.com/G-Hensley/pantheon/pull/42), merged; not a tracked deliverable |
| `pantheon:pr-33-dependabot-frontend` | completed | PR [#33](https://github.com/G-Hensley/pantheon/pull/33), merged; not a tracked deliverable |
| `pantheon:work-item-contract-adopted` | completed | PR [#50](https://github.com/G-Hensley/pantheon/pull/50); superseded for further cutover work by `cutover-activation-pending` below |
| `pantheon:stale-local-worktrees` | completed | Housekeeping observation; the actionable piece is issue [`workspace-reap`](https://github.com/G-Hensley/pantheon/issues/68) |
| `pantheon:s9xc2s` | new-issue | Issue [`s9xc2s`](https://github.com/G-Hensley/pantheon/issues/71) |
| `pantheon:rd85p4` | new-issue | Issue [`rd85p4`](https://github.com/G-Hensley/pantheon/issues/72) |
| `pantheon:jz8nsh` | new-issue | Issue [`jz8nsh`](https://github.com/G-Hensley/pantheon/issues/73) |
| `pantheon:cutover-activation-pending` | new-issue | Issue [`cutover-activation-pending`](https://github.com/G-Hensley/pantheon/issues/70) |
| `pantheon:3xcwdb` | new-issue | Issue [`3xcwdb`](https://github.com/G-Hensley/pantheon/issues/51) |
| `pantheon:review-identity-disclosure` | decision-needed | Issue [`review-identity-disclosure`](https://github.com/G-Hensley/pantheon/issues/52) |
| `pantheon:e67nyf` | decision-needed | Issue [`e67nyf`](https://github.com/G-Hensley/pantheon/issues/53) |
| `pantheon:kyzzsx` | decision-needed | Issue [`kyzzsx`](https://github.com/G-Hensley/pantheon/issues/54) |
| `pantheon:q0h7c4` | new-issue | Issue [`q0h7c4`](https://github.com/G-Hensley/pantheon/issues/55) |
| `pantheon:rg3wmk` | new-issue | Issue [`rg3wmk`](https://github.com/G-Hensley/pantheon/issues/56) |
| `pantheon:t9x41e` | new-issue | Issue [`t9x41e`](https://github.com/G-Hensley/pantheon/issues/57) |
| `pantheon:y4fz0h` | new-issue | Issue [`y4fz0h`](https://github.com/G-Hensley/pantheon/issues/58) |
| `pantheon:zmk94k` | new-issue | Issue [`zmk94k`](https://github.com/G-Hensley/pantheon/issues/59) |
| `pantheon:model-health-state` | new-issue | Issues [`model-health-state`](https://github.com/G-Hensley/pantheon/issues/60) and [`model-switch-recovery`](https://github.com/G-Hensley/pantheon/issues/62) |
| `pantheon:context-window-visibility` | new-issue | Issues [`context-window-visibility`](https://github.com/G-Hensley/pantheon/issues/61) and [`context-window-occupancy`](https://github.com/G-Hensley/pantheon/issues/63) |
| `pantheon:workspace-reap` | new-issue | Issue [`workspace-reap`](https://github.com/G-Hensley/pantheon/issues/68) |
| `pantheon:known-gaps-bundle` | new-issue | Issue [`known-gaps-bundle`](https://github.com/G-Hensley/pantheon/issues/69) |
| `pantheon:dirty-checkout-agent-sync-artifacts` | duplicate | Existing parent record in a private cross-project ledger, not reproduced here |
| `pantheon:shared-idea:orchestrator-shapes-not-adopted` | declined | Read for the shapes handled, not adopted; no action implied |
| `pantheon:shared-idea:local-model-trust-gradient` | decision-needed | Issue [`local-model-trust-gradient`](https://github.com/G-Hensley/pantheon/issues/64) |
| `pantheon:shared-idea:cross-system-stage-contract` | duplicate | Existing parent record in a private cross-project ledger, not reproduced here |
| `pantheon:shared-idea:squad-comparison` | decision-needed | Issue [`squad-comparison`](https://github.com/G-Hensley/pantheon/issues/65) |
| `pantheon:shared-idea:ask-conductor-escalation-predicate` | decision-needed | Issue [`ask-conductor-escalation-predicate`](https://github.com/G-Hensley/pantheon/issues/66) |
| `pantheon:shared-idea:coordinated-rollback-decision` | decision-needed | Issue [`coordinated-rollback-decision`](https://github.com/G-Hensley/pantheon/issues/67) |
| `pantheon:shared-idea:cross-project-record-not-owned-here` | duplicate | Existing parent record in a private cross-project ledger, not reproduced here |
| `pantheon:blind-review-packet` | duplicate | Already owned by issue [`review-identity-disclosure`](https://github.com/G-Hensley/pantheon/issues/52); a supplemental issue was opened and closed not planned as a duplicate, no separate decision retained |

## Source byte ledger

These are inspection identities, not portable commands or required working
directories. The hash is the immutable identity. Recheck every current path
immediately before the migration write and again while claims remain paused
after the human merge. This ledger covers files inside this repository only;
the private cross-project sources this review also read (a root-supplied
excerpt of shared ideas, and a prior internal work map) are recorded, with
their own hashes, only in the private full inventory, not reproduced here.

The combined `sha256sum .tasks/*.json` manifest, sorted, hashes to
`3f82c359deec8574018ca3cba681775e885788a26017b6504400d3c70847acf5`.

| Inspected source | Revision | SHA-256 |
| --- | --- | --- |
| `AGENTS.md` | origin/main `5cd35880b71658abcf69d6077e41196cb164e846` | `f9ab9d7334ace502f3d6dcc80213593b621028e8c672afac60906cd070906e03` |
| `README.md` | origin/main `5cd35880b71658abcf69d6077e41196cb164e846` | `c3dcaff7bcbcc6de40952bb698a462902f1bc4f305480d5e7ac7a8672ff2ce79` |
| `CONTRIBUTING.md` | origin/main `5cd35880b71658abcf69d6077e41196cb164e846` | `82fb545a9a3f7ced8fd8aa4d45e6e2789b6af59e0c4a7c136f3ab72988d4c62a` |
| `BACKLOG.md` | origin/main `5cd35880b71658abcf69d6077e41196cb164e846` | `d84c96ad88abf16f034a9e5ae5b202ba2efe5b2a0c277d4f03fa2a8cfae81190` |
| `SECURITY.md` | origin/main `5cd35880b71658abcf69d6077e41196cb164e846` | `ca3698aad2a0781f173c7ec9fcca2ee3d56ef10c1e12fc0689871803355b2d7d` |
| `docs/design/context-window/design.md` | origin/main `5cd35880b71658abcf69d6077e41196cb164e846` | `4258b051dcc9fe5f6b7f03aa66a7320d7b590db4e8237294da70da6407b16b06` |
| `.github/ISSUE_TEMPLATE/work-item.yml` | origin/main `5cd35880b71658abcf69d6077e41196cb164e846` | `d11f8b95a2a4d365dba6545950c112e1a61e76a017790bcc1b37404e2614fcd0` |
| `.tasks/6v3ebz.json` | origin/main | `4e309e060b2251056d412362246a692bb92613fae5cd781683ca0347edb548c3` |
| `.tasks/7c7f3j.json` | origin/main | `8d2e5dda96628d1555c718dee0b0dcf8753a850b18ce6d6860cbeb85fd989054` |
| `.tasks/d4nhzh.json` | origin/main | `51fe9b1dc3a0c4d3d619b9e0e692c6ea74446cb73aedf5c73e2b279bdb651f71` |
| `.tasks/d575h4.json` | origin/main | `84e72001ef38763e6dbd50583d9affc2b348407ba27b3f63dba1611ac535bd4d` |
| `.tasks/dq3s0j.json` | origin/main | `62d925212210848425a746a12c771e03c4bb26e9428218d0e7011637b27ef76b` |
| `.tasks/e18r66.json` | origin/main | `7137047fb6bd7ee2ba48c8fd857a77e4e3e46d14c6a83b095a00092c3d093c20` |
| `.tasks/gre41p.json` | origin/main | `b66b41e623073c430311e89070413691a190e65a7f5287e788ebb123c9597370` |
| `.tasks/s9xc2s.json` | origin/main | `29b40bddc32ce8da20834dbf6695f50184e459794de1a504f5b0f988ff5ecb2b` |
| `.tasks/qf7d1f.json` | origin/main | `82e91975d077cf26e9a8016f470b379dc8b2b87579f77e26c5651b98e9d3d72a` |
| `.tasks/rd85p4.json` | origin/main | `0924d9e3e34ec9fe135ca36cd772a8f9dde39d9db47f0c43ea3f4c0d27cb3332` |
| `.tasks/jz8nsh.json` | origin/main | `8a8e120c3667e1a77a0e8fb04963a0b3584a061794344a53eaea2415555313fe` |
| `.tasks/3xcwdb.json` | origin/main | `ba20f4ed8205ca6e3ceff9a9922b8d7a54a466b4cc46b661b851e46c0caa3fe8` |
| `.tasks/e67nyf.json` | origin/main | `0ddd0205cc312f420c7eb2cffd524f2f8e2e14e08da6b3c6ab43acdf4b866c8d` |
| `.tasks/kyzzsx.json` | origin/main | `683de1ffb9dadd33e7afa825c697bf27f279cae1f2b58ccfb208d84bc021c77d` |
| `.tasks/q0h7c4.json` | origin/main | `24c0842b088a26d1d41e1524bb2d13e99d83b0b09fed3406cf4a484195ec23fa` |
| `.tasks/rg3wmk.json` | origin/main | `a38b1c5db4533d9c09784fee4e4f86b79d02140f7883f44cf55f09d2b48a45d2` |
| `.tasks/t9x41e.json` | origin/main | `18177e6c00f7726c06016f8b82485e65d0034dbeefddac2e0ae542528e7bfc95` |
| `.tasks/y4fz0h.json` | origin/main | `1312214575c791b42b4995da0cfc7ec8e073c7b4e8d9672ad03edce0416402c8` |
| `.tasks/zmk94k.json` | origin/main | `4961ffd73bce3dd4500f34d8fcc81a08389899245a195ba917fdacde6869fe2b` |

## Writer inventory and retained variants

- `AGENTS.md` is the active documented writer surface. It directs humans and
  agents to the `lexicon task` CLI, which writes `.tasks/` with
  compare-and-swap.
- Pantheon's own in-process MCP server journals live pane dispatch and task
  assignment to `brain.jsonl`. That journal is unrelated to this repository's
  backlog and is not part of this migration.
- No automated `.tasks/` writer beyond `lexicon task` was found in the
  inspected `.agents/`, `.claude/agents/`, `.codex/agents/`, or
  `.opencode/agents/` project sources. This is a bounded inspection result,
  not proof that an unregistered checkout or live session cannot write.
- The repository already carries `.github/ISSUE_TEMPLATE/work-item.yml`,
  added by pull request [#50](https://github.com/G-Hensley/pantheon/pull/50).
  It is not replaced.
- Twenty non-main worktrees exist for this repository at inventory time, all
  clean or independently confirmed superseded by an already-merged pull
  request; zero branches match this application's own isolation naming
  (`pantheon/<id>-<uid>`), confirming none were created by the app itself.
  This proposal does not modify, select, delete, or clean any of them; see
  issue [`workspace-reap`](https://github.com/G-Hensley/pantheon/issues/68)
  for the follow-up this observation motivates.

## Coverage limits

- This migration concerns manual project tracking only. It does not build or
  enable any GitHub-issue task intake, executor, status publisher, hook,
  webhook, reconciler, or unattended claim path.
- Three issues (`s9xc2s`, `rd85p4`, `jz8nsh`) carry a merged pull request
  whose own body discloses that a live/runtime observation is still
  unobserved. Their desired status after activation is Review precisely
  because that observation is still missing, not despite it: Review is the
  state that waits on it, not a claim the wait is already over. Before
  activation their staged field is Backlog, not an operative Review claim.
  Once activation is recorded and, separately, the missing observation is
  recorded against a disposable, isolated test fixture rather than a live
  workspace, reassess each issue's acceptance and closure on its own merits.
- GitHub Discussions, wiki pages, and any non-Issues/PR GitHub surface were
  not checked as part of this migration.
- The reviewed publication journal binds all 23 issue URLs and Project item
  IDs and records verified Status and Priority fields. Dependency
  relationships remain subject to the activation recheck.
- This map does not establish tracker activation, drain any writer, merge
  the proposal, or run a provider.
