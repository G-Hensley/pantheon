# Pantheon

A desktop cockpit for running several AI coding agents side by side, coordinated
rather than siloed. Each agent gets a live terminal pane; panes can share a
context store, and one can be promoted to conductor to fan work out and collect
results.

Runs on Windows and Linux: the terminal layer is `portable-pty`, which is ConPTY on
Windows and a Unix PTY elsewhere. macOS is unexercised rather than ruled out.
Status is working prototype, so read the Known gaps section of `README.md`
before relying on any part of it.

This file is orientation for this project only. The shared working-style rules, the
guarded-change policy, and the greenfield stack defaults live in
`~/Projects/global-llm-configs/AGENTS.md`, which every agent host loads. They are not
restated here on purpose; if they seem to be missing, the host's symlink to that file is
broken, so fix the symlink rather than copying the rules back in.

## Where things are

- `src/` : the React and TypeScript front end, built with Vite.
- `src-tauri/src/` : the Rust side. `mcp.rs` is the in-process MCP server the
  agents talk to, `worktree.rs` is the git worktree isolation, `lib.rs` and
  `main.rs` are the app.
- `ui-gallery/` : component previews.
- `README.md` : what Pantheon is and what it cannot yet do.
- `CONTRIBUTING.md` : setup, building, testing, branching, review, and pull
  requests. It is the authority on all of those; do not duplicate it here.
- `BACKLOG.md` : the reasoning behind the open work. Entries record what was
  measured, which hypotheses were refuted, and what was tried and did not work.
  That is the part worth keeping, and it is why it stays prose.

Build with `pnpm`. `dev.cmd` and `build.cmd` on Windows, or `dev.sh` and
`build.sh` on Linux, wrap the usual commands.

## Tasks

Read `docs/github-issues.md` before picking up work. It records a staged
handoff, so first verify its activation checkpoint:

- Before verified activation, `.tasks/` remains authoritative. Use
  `lexicon task` for an authorized update, except while the conductor has
  explicitly paused legacy writers for the handoff.
- After verified activation, GitHub Issues owns deliverable identity,
  acceptance and resolution, and the repository's selected GitHub Project
  owns stage and priority. Never run `lexicon task` or edit `.tasks/`; the
  files become a read-only historical ledger.
- A staged issue or Ready field is not permission to start. Read its full
  body, dependencies, assignee, comments and latest conductor/session claim.

`docs/github-tracking-migration.md` maps every reviewed source identity to
its owning issue or historical disposition. It is provenance, not another
tracker: the active issue wins if the map and issue later disagree.
Reasoning stays in `BACKLOG.md`; an issue links to it rather than restating
it.

This manual tracker handoff installs no MCP tool, dispatch behavior, or
runtime change. Pantheon's own dispatch/session journal (`brain.jsonl`) is
unrelated and untouched by it.
