# Host Routing

The portable skill uses three concepts: `lead`, `worker`, and worker `tier`.
Translate them through the current host instead of placing model IDs in
`SKILL.md`.

## Capability Gate

Before delegation, establish whether the running host supports:

1. child sessions or subagents;
2. per-worker model or effort selection;
3. bounded tools, sandboxing, or write isolation when the task needs them;
4. inspectable status and a return channel.

Unknown means unavailable. Degrade to the lead or direct tools and say which
benefit was lost. Do not silently invoke another installed host to simulate a
missing native capability.

Instruction inheritance is not a delegation capability gate. Restate every
task-critical constraint in the packet whether inheritance is documented,
unknown, or unavailable. Preload a skill only when the worker needs that skill
to complete its own bounded assignment.

## Tier Selection

Model names change faster than the policy. Resolve these profiles against the
installed host and configured provider:

| Profile | Required capability |
| --- | --- |
| `efficient` | Repetitive work, low ambiguity, low consequence, deterministic acceptance |
| `capable` | Local coding or analysis judgment inside a settled design |
| `strong` | High uncertainty, high consequence, cross-component reasoning, or difficult validation |

Use the cheapest profile that has demonstrated acceptable results on the same
class of task. Count briefing, retries, review, and verification, not token price
alone. Without evaluation evidence, inherit the parent's model rather than
guessing that a lower tier is adequate.

## Current Host Shapes

Consult `compatibility/hosts.yaml` in the Lexicon source for dated capability
claims. The portable translation is:

### Claude Code

- Custom agents select `model`, optional `effort`, tools, permission mode,
  skills, background behavior, and optional worktree isolation. Parent
  precedence can change effective permissions, so verify the runtime boundary.
- `inherit` is the safe default. Provider aliases and exact model IDs are local
  configuration, not portable policy.
- Claude supports nested subagents, but this skill keeps nesting off unless the
  lead explicitly designs and budgets a nested graph.

### Codex

- Custom-agent TOML supports `model` and `model_reasoning_effort`. Documentation
  also shows per-agent sandbox, MCP, and skill settings, but local Codex 0.151.0
  probes found `sandbox_mode` and `mcp_servers` dropped while applying the role.
  Treat those per-agent boundaries as unavailable until the installed version
  passes a live probe; use the parent sandbox in the meantime.
- Built-in roles are useful defaults, but their names and behavior do not define
  the portable profiles above.
- Nesting is undocumented. Keep the lead as the only dispatcher.

### OpenCode

- An agent may select a provider-specific `model`; omission inherits the primary
  agent model.
- Native task delegation and explicit mention are available, but automatic
  custom-agent selection failed local probes with two parent models. Invoke the
  intended worker explicitly and inspect the task trace when selection matters.
- There is no provider-neutral cheap-model alias. Map tiers only after listing
  models available from the configured provider.
- Delegation depth is configurable, but breadth has no documented host limit.
  Keep nesting off and bind fan-out to the task's distinct surfaces.

### Agy

Standalone agent schema and the delegation return contract are unverified in the
current Lexicon environment. Treat native delegation as unavailable until the
compatibility ledger is refreshed on a machine with Agy installed.

## Permissions And Instructions

Prompts and tool lists are not portable security boundaries. Use the host's real
sandbox and permission controls, and verify their effective behavior. Even where
the host documents project-instruction inheritance, repeat every constraint that
is essential to this assignment. Never translate one host's permission policy
into another host's syntax by analogy.
