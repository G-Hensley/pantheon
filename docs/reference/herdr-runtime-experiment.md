# Herdr runtime experiment

**Assessment date: 2026-11-18. Scope: reversible Linux experiment, not a migration decision.**

Retain Pantheon's default backend and investigate Herdr behind a terminal boundary.
Herdr can own persistent programs and terminal state; it does not make Pantheon's
in-process coordination persistent. The isolated code uses the existing xterm
component with a direct, verified Herdr socket adapter. It does not introduce a service, scheduler,
telemetry store, privileged MCP tool, or tmux dependency.

The owner explicitly authorized proceeding in an isolated worktree despite the
unverified tracker activation in [#70](https://github.com/G-Hensley/pantheon/issues/70),
leaving both trackers unchanged. This report is evidence and a proposal, not a new
executable backlog or an accepted architecture decision.

## A. Current state

Inspected Pantheon at `9e0b65ddd6731b157425e64d158cfa3b8b8390da`, its instructions,
security rules, restore design and open issues. All seven paths named in the
assignment still exist and retain the stated responsibilities.

| Observed implementation | Consequence |
| --- | --- |
| [`lib.rs`](../../src-tauri/src/lib.rs) owns portable-pty children in `SessionManager`; close and application exit kill them. | Layout restoration is not process reattachment. |
| [`TerminalPane.tsx`](../../src/components/TerminalPane.tsx) renders byte channels with xterm, batches output, sends input and resize through [`ipc.ts`](../../src/lib/ipc.ts). | The renderer can remain. Its lifecycle currently assumes mounting spawns a process. |
| [`worktree.rs`](../../src-tauri/src/worktree.rs) and [restore design](../design/session-restore/) preserve reusable worktree identity and refuse unsafe cleanup. | Runtime identity must be added to this identity, never replace it or justify deleting dirty work. |
| [`mcp.rs`](../../src-tauri/src/mcp.rs) runs per-session loopback servers with bearer tokens bound to sessions, plus shared coordination state. | Older port-only documentation is incomplete. A surviving child retains an endpoint/token belonging to the departed desktop. |
| Task recovery already abandons interrupted pane work, preserves `in_review` and marks a missing reviewer; [`headless.rs`](../../src-tauri/src/headless.rs) has Claude and OpenCode usage accounting. | Do not revive tasks merely because a terminal is live. Headless usage is not wholly absent; reuse its semantics where suitable. |

Open [restore #72](https://github.com/G-Hensley/pantheon/issues/72),
[context #63](https://github.com/G-Hensley/pantheon/issues/63),
[quiet-state #100](https://github.com/G-Hensley/pantheon/issues/100),
[restart #108](https://github.com/G-Hensley/pantheon/issues/108) and
[activity #114](https://github.com/G-Hensley/pantheon/issues/114) provide relevant
history. Issue state alone was not treated as proof that recovery code was absent.

### Coordination failure boundary

When the desktop exits, its MCP listener and in-memory authorization disappear.
An agent can retain its OS process, files and transcript while subsequent MCP
calls fail. Whether each CLI retries, reconnects, or requires a session restart
was **not executed** here. Restoring a listener at another port and minting another
token does not update the old process's configuration. Reusing a port or trusting
a claimed terminal label is not secure reauthentication.

A future rebind must reconcile project/worktree identity, Pantheon agent and task
identity, runtime instance and process creation identity, and a recoverable
credential lifecycle. Stale tasks should remain interrupted pending explicit
reconciliation. The prototype has no MCP endpoint, task journal, conductor,
credential import, automatic task replay or worktree cleanup.

## B. Herdr capabilities and protocol

Core source pinned to [`c7517808d64f06609e4ecf0db1757005373cc3a3`](https://github.com/herdrdev/herdr-core/tree/c7517808d64f06609e4ecf0db1757005373cc3a3),
dated November 16. The [release manifest](https://releases.herdr.dev/core/latest.json)
was checked again November 18: **Core 0.1.21, protocol generation 1**. The Linux
x86_64 binary reported `hc 0.1.21`; the downloaded archive matched SHA-256
`78424755c10a40dde38a0bdb55c20ac19706e9ae05ed68fc2acf1fcff0d941c1`.
The API is preview. Generation compatibility is not a stability promise.

| Desired capability | Classification | Evidence and limit |
| --- | --- | --- |
| Persistent shell and arbitrary agent CLI | Native process support; agent operation requires validation | `terminal.run`, holder processes, `keep=false`. Persistence is within a running host, not survival of reboot or agent exit. |
| Detach/reattach and output while UI is absent | Native plus Pantheon adapter | `terminal.attach`, screen/history repaint and live bytes. Pantheon must persist and verify ownership bindings. No cursor-based exactly-once output replay. |
| Input, resize, termination and lifecycle | Native plus adapter | Framed DATA/RESIZE/CLOSE/CREDIT; `terminal.get`, `terminal.kill`, events. Viewer loss must not be misreported as program exit. |
| Remote agents | Native SSH transport; untested here | `hc connect`/machine support. No SSH profiles, credentials or remote listeners were configured. Login-session policy can still kill programs on logout. |
| React/xterm and structured status | Renderer requires adapter; status native only when reported | Herdr does not require Herdr App. OSC 7501 reports program states; absent reports are unknown, not proof of inactivity. |

The [wire and API reference](https://github.com/herdrdev/herdr-core/blob/c7517808d64f06609e4ecf0db1757005373cc3a3/schema/README.md)
and [OpenRPC schema](https://github.com/herdrdev/herdr-core/blob/c7517808d64f06609e4ecf0db1757005373cc3a3/schema/hc.openrpc.json)
describe newline JSON-RPC on Unix sockets/named pipes or stdio. `hello` can switch
to framed transport: channel u32 little-endian, kind u8, length u32 little-endian,
then payload, maximum 16 MiB. Initial output credit is 256 KiB; lag beyond the
bounded 4 MiB path can require repaint. Events have a snapshot/sequence and
heartbeat; after missed events consume a fresh snapshot, and do not treat a
sequence as globally durable across core instances. `keep=true` may launch a new
shell after loss, so it is unsuitable as evidence of the same agent surviving.

An initial [`hc attach`](https://github.com/herdrdev/herdr-core/blob/c7517808d64f06609e4ecf0db1757005373cc3a3/crates/herdr-core/src/attach.rs)
bridge established feasibility but could auto-start a missing core on Linux.
The final adapter opens the verified socket directly, validates the framed
handshake and implements byte input/output, resize, credit and close. It cannot
auto-start a daemon. Display facts use Core defaults; richer focus/display
negotiation remains future work. Open [#233](https://github.com/herdrdev/herdr-core/issues/233)
tracks missing output resume cursors. Open
[#196](https://github.com/herdrdev/herdr-core/issues/196) reports dropped remote
input during reconnect on an older release. It was not reproduced here and is
not proof that 0.1.21 has the same defect. Remote reconnect and scrollback
deduplication remain acceptance gates.

### Platforms, license and authority

Core release targets are Linux and macOS x86_64/aarch64 and Windows x86_64. Only
Linux x86_64 was executed. Pantheon supports Windows and Linux; its macOS path
remains unexercised. This prototype is **Linux-only** because its socket peer
verification uses `SO_PEERCRED`. A Windows implementation needs named-pipe
ownership/ACL and process-identity checks, not a disabled verification branch.

[Core](https://github.com/herdrdev/herdr-core/blob/c7517808d64f06609e4ecf0db1757005373cc3a3/LICENSE)
and [App](https://github.com/herdrdev/herdr-app/blob/321c9a84bc928f1c31be59e73dad966d86a3e252/LICENSE)
are Apache-2.0 at the inspected revisions. App Cargo version was 0.1.16. App's
GPUI/Ghostty interface is not part of this integration. A distribution must retain
license/notices and review bundled third-party obligations; no Herdr binary is
vendored or installed globally by the proposal.

[Herdr's security boundary](https://github.com/herdrdev/herdr-core/blob/c7517808d64f06609e4ecf0db1757005373cc3a3/docs/security.md)
is privileged same-user access, not per-agent capability authorization. Runtime
clients can launch programs and access files. Local directory permissions and
SSH authentication are not Pantheon conductor permissions. Never export this
runtime API unrestricted through agent MCP. Terminal `identity` describes the
terminal environment, not authenticated ownership.

## C. Isolated prototype and measured evidence

Code is under [`experiments/herdr/`](../../experiments/herdr/README.md),
[`herdr_experiment.rs`](../../src-tauri/src/herdr_experiment.rs), and
[`HerdrExperiment.tsx`](../../src/HerdrExperiment.tsx). A non-default Cargo feature,
explicit environment and separate Tauri profile select it. The regular backend
and existing `TerminalPane` are retained. The experiment has no viewer PTY or
viewer subprocess; Herdr owns the actual shell or agent.

The launcher creates a new owner-only scratch root with empty HOME, Codex and XDG
profiles and a minimal environment. The adapter checks root/subdirectories,
trusted path ancestors, socket type/owner, peer UID/PID, pinned version/generation
and core receipt on the control and data connections.
Reattachment verifies terminal address, creation time, PID, argv, working
directory, labels and `keep=false`. Finished or mismatched terminals fail closed.
Only two fixed fixture commands are available; labels supplement trusted local
bindings and are not credentials. A malicious process with the same user is
outside this isolation boundary. Core replacement is refused; this experiment
does not implement daemon-restart reconciliation.

Control, data, status and stop connections fail closed when the verified core is
absent or replaced. Only the explicit fixture `start` command starts a daemon.
The receipt and PID are not a hostile-same-user defense. Output credit is granted
when the existing renderer channel accepts bytes, not when xterm finishes
painting; the burst test proves transport completion, not a bound on all
frontend memory under a permanently stalled renderer.

**Execution record, final direct adapter:**

| Check | Observed result |
| --- | --- |
| Existing regression suite and build | 471 Rust unit tests and 2 integration tests passed; the opt-in runtime test and pre-existing authenticated Codex integration were excluded from the ordinary run. The runtime test passed separately. Frontend: 221 tests plus the skill regression runner passed. Normal frontend and packaged experiment builds passed; dependency audit found no known vulnerabilities. |
| Real Herdr transport fixture | Passed: same shell PID/creation identity after viewer detach and Engine recreation; shell variable preserved; existing output repainted; resize to 35x100; 1 MiB ordered input in 4093-byte chunks; Unicode/ANSI bytes; 5 MiB output completion; unexpected viewer connection loss leaves process live; explicit kill and no silent replacement. |
| First transport failure | The first 5 MiB check timed out because the test repeatedly rescanned its entire growing buffer. Scanning only the new suffix passed in 2.13 seconds. This was a harness defect, not observed Herdr data loss. |
| Desktop fixture | Passed on final direct adapter: xterm DOM paste/Enter, green Unicode rendering, output produced while the desktop was closed, same shell and Codex PIDs/creation times after app restart, and preserved shell variable. Screenshots were inspected. Codex was an unauthenticated 0.169.0 welcome screen. Native OS keyboard/clipboard injection failed in the X11 harness and remains unverified; no authenticated agent turn was run. |
| Not exercised | Authenticated coding work, Claude/OpenCode TUIs, remote SSH, Windows/macOS, logout/reboot, MCP endpoint loss/recovery, or automatic task reconciliation. No inference charges or credentials were authorized for the fixture. |

The baseline desktop build refused mismatched locked JS/Rust Tauri package
versions. This proposal aligns JS API/dialog/opener minor versions to the
existing Rust lock without changing Rust dependencies. That **also affects
normal builds**, despite the runtime feature gate. Evidence was filed as
[#127](https://github.com/G-Hensley/pantheon/issues/127). Normal desktop behavior
was not manually revalidated; this prerequisite is explicit review scope.
Vite's existing large-chunk warning remains.

## D. Architecture choices

| Choice | Benefit | Maintenance, recovery and risk |
| --- | --- | --- |
| A: Optional Herdr runtime, coordination in desktop | Smallest reversible increment; retains portable-pty on supported platforms. Suitable for persistent terminals with an explicit coordination-unavailable state. | Two runtime adapters need contract tests. Shared UI must distinguish viewer disconnection, runtime loss and program exit. No promise of unattended conductor work. Recommended next experiment. |
| B: Optional Herdr plus a headless Pantheon coordinator | Durable MCP endpoints, identities and task reconciliation could support unattended work. Desktop becomes a client. | Substantially larger service/auth/state migration with installation, upgrades, revocation and crash recovery. Design only after recovery requirements are measured. Not implemented. |
| Sole Herdr runtime | Eventually one process owner and potentially common remote transport. | Early preview dependency, platform and upgrade risk, new installation requirement, no fallback. Does not solve MCP persistence. Not justified by this experiment. |

Keep the proposed runtime interface narrow: launch, attach, input, resize,
detach, terminate, snapshot and lifecycle subscription. Backend-specific
capabilities should be explicit. Herdr owns process truth and terminal history;
Pantheon stores an ownership binding and viewer connection state, not a second
process scheduler. Pantheon retains conductor authority, task policy, worktrees
and agent identity. Serialize input and resize per attachment, and never queue
unacknowledged input across an uncertain reconnection for automatic replay.

## E. Adjacent integrations and telemetry

### Daedalus: observe its authority

Inspected local Daedalus `7807009` and remote
[`5874545`](https://github.com/G-Hensley/daedalus/tree/587454569c53a8c8bc176f81b8cbe501b01ecf9a).
User changes in its classification configuration were preserved. Durable jobs,
attempts, routes, review runs/findings and publications are the authoritative
records. Jobs use queued/running/succeeded/failed/abandoned states. Existing
inspect/report/export tooling is a better boundary than a second queue. Issue
intake/claim primitives do not imply that the complete autonomous issue-to-PR
pipeline is delivered; [#103](https://github.com/G-Hensley/daedalus/issues/103)
and [#122](https://github.com/G-Hensley/daedalus/issues/122) remain relevant.

A Daedalus-owned, redacted, versioned read-only export or future authenticated
read API could project jobs, attempts, route, failures, findings, verification
and publication links. Poll with source timestamps and show staleness. Human
attention is a separate field with provenance, not an invented scheduler state.
Patch access needs an artifact reference and permission check; do not expose
worker filesystem paths or raw prompts. No production database access occurred.

[`daedalus-synthetic.json`](../../experiments/herdr/daedalus-synthetic.json) is a
small **proposed view fixture**, not an implemented Daedalus API or a live
adapter. It intentionally leaves unavailable metrics null and has no terminal
mapping. A Daedalus job becomes a Herdr session only if its owning runner
explicitly supplies that association; Pantheon must never infer one from a job ID.

### Live measurements and historical accounting

Inspected Orrery local `328b9b6` and remote
[`0e18330`](https://github.com/G-Hensley/orrery/tree/0e183301de1c675d86d3ab80be08092ab717fe9a).
Remote source already includes a Claude usage reader; Codex and OpenCode readers
are still marked unbuilt. The broader
[SPEC-0002](https://github.com/G-Hensley/orrery/tree/0e183301de1c675d86d3ab80be08092ab717fe9a/docs/specs/0002-llm-usage)
is a design, not proof of every metric being available. Its accepted ADR-0002
favors independent Python readers and privacy-controlled projections, including
exclusion of raw text and identifying paths/IDs from historical snapshots.

| Measurement class | Available evidence | Pantheon interpretation |
| --- | --- | --- |
| Native Herdr | PID, process state, exit reason, size, labels, reported status/turn/time and events | Process facts are native. Active/waiting/blocked/completed task meaning requires a reliable reporter; absent status is unknown. |
| Agent-reported | Status OSC or hooks; model/effort and usage only where the CLI emits them | Identify reporter and CLI version. A process claiming a status does not authenticate a task result. |
| Host transcript/API accounting | Input/output, caches, model, tools, compactions and usage events vary by CLI; often available only after a turn | Reuse Orrery field definitions and validation examples, without embedding its historical collector/store in Pantheon. Subscription billing is not API-equivalent cost. |
| Inference | Context occupancy estimates, prices, inactivity heuristics and budget forecasts | Include method, denominator, freshness and uncertainty. Unknown is null, never zero. Quiet output alone is not a stall. |

Core's inspected schema has **no native token/context accounting API**.
Community [herdr-metrics](https://github.com/jordanhawkes/herdr-metrics/tree/a4c0c551d8839afb028c62f353e669796f17fa70)
0.3.0 reads external CLI artifacts and uses older pane-plugin hooks. Its session
metric excludes cache reads, so it is not interchangeable with total consumption.
[herdr-usage-bar](https://github.com/silverwolfdoc/herdr-usage-bar/tree/c8431a173fc34d2776e66092b381b63978431ef1)
is another candidate whose compatibility/API was not established. Neither was
installed or treated as a supported Core telemetry service.

A small **schema proposal**, shared by version rather than a shared database,
could carry `schema_version`, producer/reader/CLI versions, observation time,
scope and ephemeral correlation ID, measurement source, nullable token/cache
fields, model/provider/effort, context capacity/occupancy, compaction events,
cost basis and confidence. Distinguish delta, cumulative and per-turn values
and include a reset/epoch identity to prevent double counting. Context occupancy
is the current request/context measure, not all historical tokens. Fields emitted
only after completion must say so. Live identifiers require a privacy projection
before historical export; Orrery remains the historical source of truth.

### Advisory conductor and routing evidence

Context-limit warnings need a measured occupancy and known capacity. Budget
warnings need comparable actual usage; repeated compaction or tool failures can
justify a handoff suggestion. A stall warning needs positive blocked/waiting
signals and age, not terminal silence. No automated compaction, restarts, model
switching or reassignment is included. Decisions selected for a handoff could
use existing context or Mnemosyne under its project-scoped privacy rules;
credentials, raw transcripts and cross-project memory should not be copied.

Crucible local `f5259bf` and remote
[`2f94a73`](https://github.com/G-Hensley/crucible/tree/2f94a732ec112f95693660ec382b7c804486480f)
provide reproducible trials, not a proven universal router. A future evidence
contract should name task/corpus version and hashes, model/provider/CLI/effort,
environment and local/cloud conditions, sample sizes, success/regression rates,
cost/token/latency distributions and uncertainty. Host/model confounding in a
pilot prevents isolated model conclusions. Routing
[#83](https://github.com/G-Hensley/crucible/issues/83) remains future work. Pantheon
and Daedalus should consume evidence with applicability limits, never duplicate
the benchmark engine. Lexicon (`3f8b27a`) remains the shared policy/skill home;
Mnemosyne was inspected locally (`9cced86`) and remotely (`17ef4a8`), not modified.

## F. Prioritized, independently deliverable increments

These are candidate increments pending normal tracker activation and owner
selection, not authorized production work.

| Priority and objective | Scope and dependencies | Acceptance and security | Relative complexity |
| --- | --- | --- | --- |
| 1. Bound the terminal contract | Validate native keyboard/clipboard input; define shared backend capabilities and extend this framed-client harness. Depends on pinned Core schema. | Same-process restart, ordered large paste, resize, Unicode, bounded output, true exit and disconnect tests; preserve fail-closed socket verification, no unowned attach, no input replay. Retain default backend. | Medium |
| 2. Prove platform and failure recovery | Windows named-pipe ownership, Linux daemon restart, explicit remote SSH opt-in, upgrade/version refusal. Depends on 1. | Ownership survives verified reconnect; missing history/status shown explicitly; lost processes never silently substituted. Remote credentials/listeners require separate approval. | Large |
| 3. Specify coordination recovery | Measure each CLI's MCP failure/reconnect behavior with synthetic identities; design A's degraded state and B's service boundary. Depends on reliable process identity. | Explicit abandoned/in-review reconciliation, revocation and restart tests; no task replay or credential broadening. Service implementation remains separate. | Large |
| 4. Add read-only Daedalus supervision | Daedalus-owned redacted export/API and Pantheon projection using the synthetic view. Depends on owner-approved contract. | Job/attempt/route/finding/PR/staleness fields match source; unknowns stay unknown; no queue mutation, database credential or implicit Herdr mapping. | Medium |
| 5. Agree live telemetry semantics | Versioned schema/examples aligned with Orrery, one CLI reader and advisory context/budget display. Later consume Crucible evidence. | Token/cache/delta/reset tests, explicit source/freshness/uncertainty and privacy projection; no duplicate historical store or autonomous optimization. | Medium |

**Disposition: investigate.** Source and transport evidence support a bounded
optional runtime experiment. They do not support sole-runtime migration or
unattended multi-agent coordination. The next useful increment is native-input validation and a small shared runtime
contract, before any platform expansion or coordination service design.
