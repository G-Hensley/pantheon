# Owner steps

Some finished-condition or validation steps need something the implementing agent cannot do. Found
mid-run, each becomes an ad hoc handover and a fallback note in the Reckon record. Found at
planning time, each is one named action for the owner, with the agent preparing everything up to
it.

## The marker

Write the step where it belongs (the finished condition or the validation section) and prefix it
`owner step:`. Say two things in it:

- **What the agent prepares:** the file at a stated path, or the exact command, written out in
  full with its arguments.
- **What the owner does:** the one action, and where its evidence lands.

```markdown
- owner step: attach before-and-after screenshots of the settings page to the pull request. The
  agent writes them to `docs/evidence/settings/` and names each file in the pull request body; the
  owner drags them into a pull-request comment.
- owner step: mirror the repository to the private organization remote. The agent prepares the
  local branch and prints the command; the owner runs
  `git push --mirror git@github.com:example-org/example-private.git`.
```

A step is an owner step only when an agent cannot perform it here, not when the agent would rather
not, and not when it merely needs approval. A step that needs the owner's approval before the agent
performs it is a guarded change, named as one (see the guarded parts in `SKILL.md` step 3).

## Known limits

Cite these when marking a step. Each is per host where known; another host may differ, so say
which host the plan assumes.

| Limit | Hosts where observed | Source |
| --- | --- | --- |
| `gh` cannot upload an image to an issue or pull-request comment; the web interface can. | Any host using `gh` | G-Hensley/ai-estate-mapper#59 and G-Hensley/ai-estate-mapper#65 (owner attached seven screenshots by hand) |
| A host safety classifier may refuse a push to another owner's or organization's remote even after the owner approves it in the session. | Claude Code, auto mode | G-Hensley/ai-estate-mapper#54 (`git push --mirror` to a private organization refused as data exfiltration; the owner ran the agent's script) |
| A physical action, or one needing a credential the agent is not given (a hardware key, an account login, a payment), is the owner's. | All | By definition |

A refusal is an execution boundary. The fix is planning the step for the owner, never working
around the refusal with another tool, command or API.
