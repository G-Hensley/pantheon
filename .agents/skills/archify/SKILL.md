---
name: archify
description: Render validated architecture, workflow, sequence, dataflow, and lifecycle diagrams as self-contained interactive HTML from typed JSON IR, using the bundled Archify renderer. Use when asked to diagram or visualize a system, its components and their edges, a delivery or request workflow, an API call sequence, a data pipeline or lineage, or a state machine, and when a diagram must carry evidence rather than decoration.
---

# Archify

Archify turns a small typed JSON specification into a validated, self-contained HTML diagram. The
renderer, schemas, examples and authoring references are bundled in this skill directory, so it works
without any other checkout on the machine.

Every path below is relative to this skill's own directory. Run commands with that directory as the
working directory, or use absolute paths built from it. Requires Node 18 or newer.

## The authoring path

`references/authoring-contract.md` is the authoritative method and is bundled here. Read it before
authoring. In short:

1. Choose `architecture`, `workflow`, `sequence`, `dataflow` or `lifecycle`.
2. Read the matching schema in `schemas/`, plus `schemas/common.schema.json`, and one matching example
   in `examples/`. Use the example for field shape, not for facts.
3. Write the candidate before inspecting renderer internals. One clear main path, short side branches,
   sparse labels, at most 12 primary nodes. Set `meta.quality_profile` to `"showcase"`.
4. New workflows use `schema_version: 2`. Keep `schema_version: 1` only to preserve an existing
   workflow's fixed geometry.
5. Do not plan coordinates in prose, and do not add `via`, `channelX`, `channelY` or `labelAt` before a
   diagnostic calls for one. Apply at most one diagnosed geometry control per repair.

For a known product identity, query `node bin/archify.mjs brands "<name>" --json` rather than guessing a
brand mark. `references/brand-marks.md` covers an unknown brand with a user-provided URL.

## The acceptance gate

Two commands, in this order, and neither is optional:

```bash
node bin/archify.mjs validate <type> <candidate.json> --quality showcase --json
node bin/archify.mjs deliver  <type> <candidate.json> <output.html> --quality showcase --json
```

- A showcase pass reports all 9 checks with 0 composition errors and 0 warnings. A receipt showing only
  4 checks is basic validation and is not acceptance.
- A non-zero exit is never success. A failed `deliver` leaves any previous output in place, so
  inspecting the output after a failure inspects the stale artifact rather than the failed candidate.
- A passing final validation freezes the candidate. Do not edit it afterward.
- On failure, change only the diagnosed `subject`, verify `evidence`, choose from `supportedFixes`, and
  rerun. If two consecutive rounds do not lower the error count, stop and report the diagnostics
  truthfully rather than continuing to guess.

`node bin/archify.mjs doctor` is the readiness check. **It reports one missing item here, "Architecture
compare runtime and proof fixtures", and that is expected.** The `compare` subcommand's `delta/`
fixtures are deliberately not bundled. Everything `validate` and `deliver` need is present, and both
were verified against this bundle. If you need `compare`, use a full upstream checkout.

## This tree's conventions

- **The IR is the source and is committed.** It lives under `docs/diagrams/src/`, the rendered output
  under `docs/diagrams/generated/`, and both are committed. The repository documentation standard's
  `diagrams.md` is the authority.
- **Archify only.** Mermaid is not used in this tree: one model, one renderer.
- **Every node and edge carries evidence and a state label.** A file path, command or issue, and one of
  observed, proposed or unbuilt. A diagram whose edges are unlabeled asserts that a system works the way
  it was drawn, which is usually the thing being checked.
- A map spanning several repositories pins a revision per repository in a table beside the diagram,
  because `meta.repository` holds exactly one.

## Update checking reaches the network

`scripts/check-update.mjs` fetches a release manifest from a third-party origin over the network. It is
bounded, pins a sha256, and never downloads or executes an update on its own. The behavior is recorded
in `orion/docs/security/` as a known outbound edge of the toolchain.

Do not run it when working offline or when the session must make no outbound request: skip it and
continue, which is what upstream already specifies for an unavailable checker. Never act on the manifest
beyond showing a notice, because taking an update is the user's decision.

## Provenance

Vendored from upstream, unmodified except for this `SKILL.md` and the omission of `delta/`, `test/` and
the pre-rendered `examples/*.html` demos.

| | |
| --- | --- |
| Upstream | https://github.com/tt-a1i/archify |
| Commit | `ed7f4d4b48d4424d36edfed8043de3de8dea6b45` |
| Describe | `v2.16.0-18-ged7f4d4` |
| Version | 2.17.0-dev.1 |
| License | MIT, see `LICENSE` and `THIRD_PARTY_NOTICES.md` |
| Vendored | 2026-09-13 |

To refresh: replace the bundled directories from a clean upstream checkout at a named commit, update
this table, rerun `doctor`, and rerun the acceptance gate against an example. Do not hand-patch the
renderer; a local fix that is not upstream becomes a fork nobody refreshes.

## Do not

- Do not hand-edit generated HTML. Regenerate it from the IR.
- Do not edit anything in `renderers/`, `schemas/` or `bin/`. They are vendored third-party code.
- Do not read renderer or validator source before the first candidate exists. Inspect implementation
  only for an unsupported internal diagnostic, or after two focused repairs have failed.
