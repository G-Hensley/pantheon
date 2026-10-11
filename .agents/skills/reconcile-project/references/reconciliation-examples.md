# Reconciliation example

A desktop reader's accepted change is keyboard zoom. At integration revision
`a1b2c3d`, `ui/shortcuts.ts:18` handles the shortcut, and the observed focused UI
test passes. `notes/accessibility.md:12` still says keyboard zoom is missing.
The release manifest contains an older revision. No live application was checked.

A bounded report could say:

- **Documentation repair, built and undocumented:** update the accessibility
  note. The implementation at `a1b2c3d` and observed shortcut test contradict its
  missing-feature claim. This is no reason to implement the shortcut twice.
- **Release verification:** compare the manifest's revision with the integration
  change and follow the project's release criteria. The focused test does not
  establish release inclusion or live behavior.

The implementation claim is supported for the tested shortcut at that revision.
The note still defines relevant accessibility intent; a shortcut test would not
establish that every other accessibility requirement was satisfied. The manifest
establishes only its recorded release identity. Live behavior remains unknown.

This example illustrates the report shape, not an evaluation fixture or an
exhaustive list of states to expect in a particular project.
