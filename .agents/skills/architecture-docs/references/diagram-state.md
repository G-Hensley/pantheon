# State labels in diagram IR

The diagram is generated from a committed intermediate representation, so a state claim in it can be
checked. There is no `state` field, and `additionalProperties: false` means an invented one fails
validation, so state goes in fields that already exist.

## Where each claim goes

| The claim | Where it goes |
| --- | --- |
| A node's state | `tag`, as the literal `observed`, `proposed` or `unbuilt` |
| The legend explaining the tags | a `cards` entry |
| A trust boundary | `boundaries` with `kind: "security-group"`, in an architecture diagram only |
| Evidence behind `observed` | `meta.repository`, which requires a GitHub URL and a 40-character commit SHA |
| Which source backs a component | `sources`, up to three `{path, line, end_line, label}` entries, on architecture components only |

`tag` is available on architecture `components`, dataflow `nodes`, lifecycle `states` and workflow
`nodes`. So the rule is: **every node-like element carries its state in `tag`.**

## What cannot carry state

The gaps are all on the things between the nodes, and they are real.

- **Edges cannot.** An architecture `connection`, a dataflow `flow`, a lifecycle `transition` and a
  workflow `edge` all have `variant` and no `tag`. `variant` holds `default`, `emphasis`, `security`
  or `dashed`. `dashed` marks an edge as not-default and cannot distinguish `proposed` from
  `unbuilt`, and `security` is already spoken for. None of them has `sources` either, so an edge
  cannot cite its own evidence.
- **Sequence participants cannot.** They have `id`, `type`, `label`, `sublabel` and `brand`. No
  `tag`, no `sources`. A sequence diagram carries its state entirely in prose.
- **Only the architecture schema has `meta.repository`,** and it holds one repository at one
  revision.
- **Only the architecture schema has `boundaries`,** which is why a threat model needs two diagrams
  rather than one.

For anything the IR cannot label, the state lives in the prose beside the diagram, at the smallest
structure that contains it:

```markdown
Every connection in this diagram is `proposed`; nothing in the request path has been built yet.
The nodes carry their own states.
```

## Two vocabularies, not one

Name the **file** by the altitude it shows, in C4 terms, because that is what a reader looks for:
`system-context`, `container`, `component`, `dynamic`, `deployment`.

Set the IR's **`diagram_type`** to the shape the renderer validates: `architecture`, `dataflow`,
`sequence`, `lifecycle`, `workflow`. C4 names the altitude; `diagram_type` names the geometry. A
`container.json` is usually `diagram_type: architecture`; an `authentication-flow.json` is usually
`sequence`.

## The three mechanical rules

1. Edit `src/`, never `generated/`. A change that edits only a generated file is a defect.
2. Regenerate in the same change that edits the IR, so the two never disagree in a merged commit.
3. `docs/README.md` records the exact regeneration command for that repository. Verify it against
   the renderer before writing it down, and do not copy a command between repositories on faith.

## Set `meta.repository` deliberately

It pins the diagram to the revision at which its claims were true, which is the difference between a
claim and a dated claim, and a dated claim is the only kind that ages honestly. It holds a single
repository, so a diagram spanning several needs a revision table in the prose beside it.
