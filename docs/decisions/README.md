# Decisions

One Architecture Decision Record per file, `NNNN-slug.md`, for choices that should outlive the task
that caused them. An accepted record is immutable in its reasoning: to reverse it, write a new one,
mark the old `superseded` with `superseded_by`, and keep its row here. Use the installed
`adr-authoring` skill to create and validate the next numbered decision record.

| ID | Decision | Status | Decided by | Superseded by |
| --- | --- | --- | --- | --- |

None yet. Decisions taken so far are recorded in the pull requests that carried them and in
`BACKLOG.md`'s reasoning. A choice that outlives the task that caused it gets a record here; the
first candidates are the ones `BACKLOG.md` argues at length, such as whether a pane's model
failure should be visible separately from its liveness.
