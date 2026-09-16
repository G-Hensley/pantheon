# Session restore

Owned by #72. That issue is authoritative for scope, acceptance and status; this document is
the reasoning it links to rather than restates.

What survives an app restart and what cannot: the roster restores topology and configuration, and the PTY, the child process and the agent's own conversation die with the previous app.

Moved here from `BACKLOG.md` on 2026-09-16 under `orion:SPEC-0001` FR-009, which reduces that file
to one line per item. The text below is unchanged from the section titled "Restoring panes does not restore the agents that occupied them", so an issue
citing that section by name is citing this. It is a live design argument, not an archived one: the
work it describes has not been done.

---

## Restoring panes does not restore the agents that occupied them

Session restore is partly built, but the honest boundary matters more than the
word "restore". `src/lib/panes.ts:1-13` states what survives: what each pane was,
not what it was doing. The roster stores the pane id, current session type,
brain, isolation flag, and saved worktree at `src/lib/panes.ts:19-31`.
`src/App.tsx:44-57` turns those records into fresh running panes on launch, and
`src/App.tsx:119-144` restores brain assignments and saves roster changes.

What comes back is topology and configuration. The PTY and child process died
with the previous app. Conversation history lives inside the agent, internal
reasoning is not exposed through MCP, terminal scrollback belonged to the old
frontend and PTY, and in-flight requests cannot cross a server restart. A
restored pane is therefore a fresh agent wearing an old pane id. UI and
documentation should say "restore pane layout" or "reopen panes", never imply
that Pantheon resumed an agent's context.

Worktree identity is the exception because it is durable state outside the
process. The frontend records the worktree reported by the backend so the next
launch can return to it (`src/App.tsx:146-158`), and
`src-tauri/src/worktree.rs:20-32` persists the fields needed to re-adopt it.
The safety rule at `src-tauri/src/worktree.rs:168-180` is the important part:
an existing directory is never written off merely because it cannot be
reattached. It may contain uncommitted agent work. Restore must refuse or surface
that condition, never silently create a replacement that strands the old work,
and never delete a dirty worktree as cleanup.

**The proposed project `.pantheon/layout.json` is not the path forward.** It was a
design for a system that did not yet restore panes. The implementation now owns
roster and layout state end to end in the frontend:
`src/lib/panes.ts:105-120` reads and writes `pantheon.panes`, while
`src/App.tsx:62-95` reads layout settings and the selected project from
`localStorage`. Adding a second layout file now would create two authorities for
the same pane order, brain assignment, isolation flag, and layout settings.
Conflict-resolution rules between them would be complexity caused by the new
store, not by the product.

The tradeoff that motivated project-scoped storage is still real.
`localStorage` is machine-local and `pantheon.panes` is one global key
(`src/lib/panes.ts:17`). A roster created for one repository can therefore be
read after the selected project changes, even though its brain assignments and
worktree references belong to the earlier repository. Project movement and
another machine are separate cases: a project-owned file travels with the
checkout but writes product UI state into the repository; a project-keyed
frontend store stays local but can distinguish repositories without adding
tracked or untracked files. The current implementation chose frontend
ownership, so the next step is to make that choice project-aware rather than
reviving `layout.json`.

Conductor identity restore is now implemented. `src/lib/panes.ts` persists the
conductor pane id beside the roster, under its own storage key, and only while
that id names a pane still in the roster; a promote, a demote, or the
conductor's own pane closing all pass through this on every change. On launch,
`src/App.tsx` restores the role only to that exact pane once the roster has
been built: `restoreConductor` calls `setConductor` for the saved id when it
is part of the roster, or clears the saved id and adds a notice to the
existing restore-problems banner when it is not, without ever promoting a
different pane instead. A pane that is part of the roster but fails to spawn
is caught separately, once that failure is known: `noteSpawnFailure` retracts
the role the same way, clear rather than reassigned. The Tauri command at
`src-tauri/src/lib.rs` still only forwards the value, and `src-tauri/src/mcp.rs`
still keeps it in memory; nothing on the backend persists, so this restore is
frontend-owned end to end, matching the roster it rides beside. One gap this
does not close: `Shared::set_conductor` in mcp.rs only briefs a pane already
connected to MCP, and restore runs before any pane has spawned, so the
restored pane gets no composer briefing and discovers the role only when it
calls `list_sessions` on connect. That fix belongs in the backend, at connect
time, not here.

**The shape still to aim for.** Key roster and conductor state by a stable
project identity instead of one global `localStorage` bucket. Continue
treating each pane as a new process, preserve saved worktree references, and
make partial restore failures visible without discarding the panes that
remain valid. Layout settings may stay machine-local; the question is which
settings are genuinely project-specific, not whether every setting can be put
into one file.

Open questions worth settling before building:

- **Project identity.** A normalized absolute path is simple but changes when a
  repository moves. Repository metadata can survive a move but needs a stable,
  non-secret identifier and a fallback for non-Git projects.
- **Storage scope.** Project-keyed `localStorage` matches the implementation and
  avoids repository files. A project-owned store is portable across machines
  but changes the repository boundary and needs an ignore and migration policy.
- **Restore timing.** Automatic reopening is fast and is today's behavior. An
  explicit prompt gives the user a way to start fresh when a saved roster is
  large or stale.
- **What layout belongs to the project.** Pane membership and worktrees clearly
  do. Window height and column preference may be user and machine preferences
  rather than repository state.
- **Dirty worktree recovery.** Reuse is already safer than replacement. If
  reattachment fails, the UI still needs to lead the user to the preserved path
  and explain why the pane was not reopened.

**Project scoping shipped, task `rd85p4`.** The roster and the conductor id are
now keyed by the project the window opened with, so a roster built for one
repository is no longer read back after the selected project changes. Four of
the five open questions above are settled by that, and the fifth is answered
more narrowly than it was asked.

*Project identity* is the normalized absolute path already stored under
`pantheon.project`, encoded with `encodeURIComponent` rather than hashed so a
misbehaving restore can still be diagnosed by reading the key. A repository that
moves therefore starts a fresh roster. That is the known cost of the simple
option, and it was taken deliberately: the alternative pulls git metadata reads
into the frontend to key a machine-local convenience store. *Storage scope* stays
project-keyed `localStorage`, which is the frontend ownership this section
already argued for, with "no project" given its own bucket rather than sharing
the unscoped key. *Restore timing* is unchanged, still automatic. *What layout
belongs to the project* is answered by leaving `layout`, `layout.columns` and
`layout.paneHeight` unscoped: they are machine preferences, and only the roster
and conductor id name panes, brains and worktree paths that mean anything solely
inside one repository. *Dirty worktree recovery* is untouched and still open.

**Two things the scoping had to get right, both of which took a design change.**

*A window that changes project mid-session stops persisting.* The roster is read
once at mount but written on every pane change, so the scope a write uses cannot
simply follow the selection. Keying writes by the currently selected project
saves panes launched under the second project into the first project's bucket,
which is the contamination the scoping exists to prevent. Keying them by the
startup project instead records those same panes as belonging to a repository
they were never opened in. Neither produces a roster worth restoring, so once
the selected project changes the window stops writing altogether, says so in the
existing restore-problems banner, and leaves both saved rosters and every live
pane exactly as they are. A restart reloads against the new project and
persistence resumes there. The freeze is one-way for the life of the window:
returning to the startup project does not make the roster trustworthy again,
because the panes launched meanwhile are still in the list. Reopening panes on a
live project switch is a larger question about what happens to running agents,
and it is not answered here.

*With one exception, for the case the freeze would otherwise break.* A window
that opened with no project at all is every first launch, and there the ordinary
path is to pick a project and then launch panes. Freezing on that pick would mean
a first session's panes were never saved, which is a worse product than the one
this entry set out to fix. So an empty window may bind to the first project it is
given and go on persisting there. Every condition has to hold: the window opened
with no project, it has no panes and no conductor on screen, it has not already
frozen, and the destination is provably unused. "Provably" is the load-bearing
word. The check is three-valued, and storage that refused a read reports neither
true nor false, so an unknown destination is treated exactly like an occupied
one. It is also stricter than "no saved roster": a destination still holding a
pre-scoping value it could later have adopted counts as occupied, because binding
and persisting would strand that value permanently, nothing reading the unscoped
key again once a scoped copy exists. A value already claimed by another project
does not count against it, since this project could never have adopted it. This
is the whole exception. A window with panes on screen still freezes, so does one
that has already switched once, and so does A to B to A.

*Adopting the pre-scoping roster is ownership-bound, not first-come.* The single
global `pantheon.panes` has to become some project's roster exactly once. The
claim is written before the copy, so a write that fails partway leaves a record
of which scope was mid-adoption: that scope retries on its next read, and every
other scope reads past the leftover value instead of inheriting it. A claim that
cannot be recorded refuses the adoption outright rather than taking the value
unguarded, because an unguarded adoption is precisely the cross-project leak
being fixed; nothing is deleted on that path, so the value is still there to
adopt once storage accepts writes again.

The claim outlives the value it guarded, which was not the first design. Clearing
it on success looked tidy and was wrong: an unscoped key can reappear, most
plausibly from an older build writing the key it still knows about, and a claim
that had been tidied away would let whichever project read next inherit another
repository's panes. A regression test covers exactly that sequence. One short
permanent string is the price of the adoption being once-only rather than
once-at-a-time.

*One claim covers the roster and the conductor together*, which was also not the
first design and matters more than it looks. A claim per key seems obviously
right until the two halves separate: adopting the roster into A and then failing
to copy the conductor leaves the conductor unclaimed, and the next project to
read it takes it. The result is not a dangling id that gets cleared harmlessly.
Pane ids are `sess-N` counters that restart from the same numbers in every
project, so a conductor id adopted into a project whose roster came from
somewhere else lands on that project's own same-numbered pane and silently
promotes the wrong agent. The roster and the conductor are halves of one window
state and are adopted as one, or not at all.
