# Enforcement layers

Four places a repository can run a check. They are separate interfaces with different enforcement
properties, not four ways of writing the same decision.

| Layer | Runs | Binds | Bypassed by | Use it for |
| --- | --- | --- | --- | --- |
| Agent-host hooks | Inside an agent host, on session or tool events | Contributors running that host with the project's configuration loaded | Another host, another editor, or editing the configuration | Guardrails and fast feedback for agents |
| Local commit hooks | In the contributor's Git client | Contributors who installed them | `--no-verify`, or never installing them | Fast feedback before a push |
| Server-side checks | In CI, on every pull request | Every change that merges through the platform | Nothing, when they are required by a ruleset; anything, when they are not | Every control the plan calls required |
| Rulesets | On the hosting platform | Every push and merge to the protected branch | Whoever holds bypass permission; under a personal account, the owner | Making server-side checks required |

## Rules that follow

- **A required control lives on the server.** A check can run at every layer for speed; its
  enforcement is the required server-side check. A plan whose only secret check is a pre-commit
  hook has an advisory secret check.
- **A server-side check is enforcement only once a ruleset requires it.** A workflow that runs and
  is not required reports; it does not block. `rulesets` names the required checks and
  `server-checks` names the jobs, and the names must match.
- **Agent-host hooks are per contributor and per host.** Plan them for the contributor host set.
  A host that tooling cannot generate configuration for still belongs in the plan, recorded as
  receiving no hooks, with the server layer covering what they would have checked.
- **Under a personal account, rulesets are self-governance.** The owner can edit or bypass them.
  Say so rather than describing them as independent control.
