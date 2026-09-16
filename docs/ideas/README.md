# Ideas

Candidates, not requirements, one per file as `NNNN-slug.md` with `id: IDEA-NNNN` and a status from
candidate, promoted, deferred, rejected. An agent MUST NOT implement an idea because it is here; an
idea is promoted into a specification or an issue first. A backlog is not an idea and gets no file
here: accepted-but-unscheduled work belongs in the tracker.

None yet, and seven candidates are waiting for one.

`BACKLOG.md` is triaged: every section there is delivered or owned by an issue. The seven that are
neither are in the routed plan at
[`../plans/2026-09-09-pantheon-remaining-work.md`](../plans/2026-09-09-pantheon-remaining-work.md),
section 6, each marked open with owner "Nobody" in its audit: an attention rail, a next-approval
hotkey, listening for the terminal bell rather than for prompt text, a desktop notification when the
window is unfocused, per-project permission allowlists (the untracked half of that section's item 5),
an opt-in tool to read the last N lines of a pane, and a focus layout.

They have no file here yet because they are inside a plan rather than standing on their own. The
promotion trigger is a decision to do one: at that point it gets a file here as `NNNN-slug.md` with
`id: IDEA-NNNN` and status `candidate`, or goes straight to an issue if the decision is firm. The
opt-in pane-reading tool needs a decision record first either way, because it reverses a deliberate
architectural choice that the Rust side never reads pane content.
