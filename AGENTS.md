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

This project's work is tracked in `.tasks/`, one JSON file per task, committed
alongside the code. Read it when you pick work up and update it when you put it
down. A status that no longer matches reality is worse than no status, because
it is the one people trust.

    lexicon task list                    # what is open here
    lexicon task show <id>               # one task, including done_when
    lexicon task set <id> status=doing   # claim it
    lexicon task set <id> status=todo done_when="..."   # several at once
    lexicon task add "Title" --status backlog

Always go through the command rather than editing the JSON. The command holds
the compare-and-swap that stops one agent's write reverting another's, and argv
cannot be malformed the way a hand-written JSON document can.

Reasoning stays in `BACKLOG.md`. A task links to it; it does not swallow it.
