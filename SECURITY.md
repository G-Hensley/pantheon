# Security Policy

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting instead of a public issue:
[github.com/G-Hensley/pantheon/security/advisories/new](https://github.com/G-Hensley/pantheon/security/advisories/new).

This is a solo-maintained project without a fixed response SLA, but reports are
read and taken seriously. Include what you found, how to reproduce it, and the
Pantheon version and OS build. Non-sensitive bugs (crashes, UI glitches, incorrect
behavior with no security implication) belong in the regular issue tracker
instead.

## Supported versions

Pantheon is a working prototype (see the README's "Known gaps" section), and only
the latest commit on `main` is supported. There are no maintained release
branches yet.

## Current threat model

Pantheon is built for one person running trusted AI coding agents against a
project they already trust, on their own machine. It is not designed for a
shared or multi-tenant machine, and it is not designed to sandbox an untrusted
agent from the rest of the system. Concretely, today:

- Every session's MCP endpoint is a loopback HTTP port. Pantheon identifies which
  session is calling by which port the request arrived on. Any local process
  that can reach that port can call the same tools; this is isolation between
  Pantheon's own sessions, not authentication against other software on the
  machine.
- Spawned agent processes inherit the full environment of the Pantheon process,
  including any secrets available to it.
- Git worktree isolation assumes the repository and its contents are already
  trusted. It exists to keep parallel agents from clobbering each other's
  edits, not to contain a hostile one.
- Each session's MCP bearer token is written into that session's generated CLI
  config file under the app data directory. On Unix the file is created with
  mode 0600. On Windows it inherits the containing directory's ACL, which on a
  default single-user profile is already private to that account; no explicit
  DACL is applied. That is a decision, taken 2026-09-04, not an oversight: the
  threat model above excludes shared machines, and the Linux machine this
  project is developed on cannot exercise Windows ACL code.

None of this is a secret; it is documented here so you can decide whether
Pantheon fits your setup. Do not run Pantheon on a machine or account you share
with untrusted users, and do not point it at a project or an agent you do not
already trust. Hardening this further (tighter IPC and environment scoping,
path canonicalization, Windows ACLs on the token files) is tracked as future
work, not promised behavior.

## Scope

In scope: the Pantheon application itself (`src/`, `src-tauri/`), including the
shared MCP brain and worktree isolation.

Out of scope: vulnerabilities in third-party dependencies (report those
upstream, though a heads-up here is still welcome) and the AI agent CLIs
Pantheon launches (Claude Code, Codex, opencode), which are separate projects.
