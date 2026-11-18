# Private Herdr terminal experiment

Read the [technical report](../../docs/reference/herdr-runtime-experiment.md)
before interpreting the result. This is an optional Linux x86_64 experiment,
using Core 0.1.21 generation 1 and the existing Pantheon `TerminalPane`.
The direct framed socket adapter never starts a missing runtime. It does not
start MCP, dispatch work, import credentials or touch production configuration.

## Reproduce

Prerequisites: Pantheon's existing Linux build dependencies, pnpm, Rust, Python
3.12+, curl, and an installed Codex executable for the optional agent fixture.
No authentication is needed. Use the isolated proposal checkout, not a running
production checkout. The commands below run from its repository root.

1. Install locked dependencies, then fetch the pinned Core archive once. The
   fetcher creates `/tmp/pantheon-herdr-dist` with mode 0700, validates its
   ownership/path, refuses nonempty directories, checks SHA-256 and extracts
   only the expected regular files. It does not install a global command.

   ```bash
   pnpm install --frozen-lockfile
   bash experiments/herdr/fetch.sh
   ```

2. Run the opt-in live transport test with a new fixture directory. It leaves
   evidence files and stops its private daemon on success. On failure inspect
   the fixture before stopping it; do not delete recorded work. For a rerun,
   set `PANTHEON_HERDR_TEST_ROOT` to another unused absolute path.

   ```bash
   bash experiments/herdr/test-transport.sh
   ```

3. Build the separate packaged debug app, with no Vite network listener. Run
   this after a full `cargo test`, which can replace the debug executable with
   a different Tauri build configuration. The transport wrapper uses `--lib`
   to avoid that replacement.

   ```bash
   bash experiments/herdr/build.sh
   ```

4. Create a new empty-profile runtime and launch its viewer. `runtime.py start`
   refuses an existing root. Set `PANTHEON_HERDR_ROOT` to another unused absolute
   path for a new fixture. `run.sh` defaults to the same path and isolates GUI
   storage. Set `PANTHEON_HERDR_CODEX` if Codex is not on PATH.

   ```bash
   python3 experiments/herdr/runtime.py start
   bash experiments/herdr/run.sh
   ```

   Type a shell variable assignment and some output, close the window, then
   run the viewer again. The PID/creation identity must remain the same and
   the variable/output must remain. Select **Codex (empty profile)** to see
   its unauthenticated welcome screen; closing and reopening then selecting
   Codex reattaches that process. Do not sign in or copy an existing profile
   as part of this fixture. **Reattach** replaces the viewer; **Terminate**
   deliberately ends the selected process and does not create a replacement.

5. Stop only this fixture when finished. Core `server.stop` ends its programs;
   saved files, bindings and screenshots remain for inspection. A finished
   binding intentionally refuses reattachment, so use a new root for a new test.

   ```bash
   python3 experiments/herdr/runtime.py stop
   ```

The root is a clean HOME/XDG/Codex environment; it is **not a filesystem sandbox**
against malicious same-user programs. The runtime remains privileged. Never
point this experiment at a normal Herdr socket, existing agent profile or live
worktree. `keep=false` prevents a restarted shell masquerading as the original.
Core restart recovery, remote SSH and non-Linux operation are outside the code.

## Desktop verification

`gui_probe.py` requires an X11/XWayland display and Python Pillow. It targets
only the experiment's titled window, closes it through the window manager and
captures that window. The opt-in `PANTHEON_HERDR_PROBE` flag injects one of three
fixed scripts from `dom_probe.js`; arbitrary script paths are not accepted.
The script drives xterm's DOM paste and Enter handlers and the Codex button.
This exercises the renderer and Tauri IPC, but does not prove physical keyboard
or OS clipboard integration. Earlier XTest/XSendEvent attempts did not reach
the terminal and were removed from the retained probe.

After building, create a fresh root and run:

```bash
export PANTHEON_HERDR_ROOT=/tmp/pantheon-herdr-gui-demo
python3 experiments/herdr/runtime.py start
python3 experiments/herdr/gui_probe.py
python3 experiments/herdr/runtime.py stop
```

The probe checks same PID and creation time for shell and Codex across full app
exit/relaunch, a preserved shell variable, Unicode output and output produced
while the window is absent. It writes `gui-result.json`, screenshots and
`gui.log` under the fixture root. The screenshots require visual inspection:
terminal-read assertions alone cannot prove rendering fidelity.

## Evidence record

Final direct-adapter evidence, executed 2026-11-18:

| Validation | Observed outcome |
| --- | --- |
| Live Rust transport fixture | Passed in 1.02 seconds: same process and variable after reattach, 35x100 resize, 1 MiB ordered input, Unicode/ANSI bytes, 5 MiB output, connection loss without process exit, actual killed-process event, and refusal to replace a finished binding. |
| Desktop DOM probe | Passed with shell PID 2381873 / creation 1795033451941 and Codex PID 2382249 / creation 1795033458435 unchanged across app restarts. Exact output-line checks prove output arrived while the window was closed. |
| Regression checks | 471 Rust unit tests and 2 integration tests passed. The live test was explicitly run separately; the pre-existing real authenticated-Codex integration remained ignored. 221 frontend tests and the skill runner passed. Normal frontend and separate packaged experiment builds passed. Dependency audit reported no known vulnerabilities. |
| Static checks | Python compilation, shell syntax and documentation structure checks passed. Final Rust formatting and feature-enabled `cargo clippy --all-targets` passed. |
| Limits | Native OS keyboard/clipboard unverified, agent authenticated work untested, other CLIs/platforms/SSH and MCP recovery untested. Core replacement is refused rather than recovered. |

The inspected [shell screenshot](evidence/shell-reattached.png) shows the same
shell PID, green Unicode, output produced while closed and the preserved
variable. The [Codex screenshot](evidence/codex-reattached.png) shows the same
agent PID at its unauthenticated welcome screen.
[Machine-readable result](evidence/gui-result.json) records the probe identity.
These are synthetic fixture artifacts, not user project data.

Earlier measurements established feasibility using an `hc attach` bridge. It
was removed because it could auto-start a missing core. The first transport
harness timeout came from repeatedly scanning the accumulated 5 MiB output;
scanning only the new suffix fixed it. A desktop probe initially launched the
wrong debug configuration after `cargo test` replaced the executable; rebuilding
the separate Tauri profile fixed it. The initial download via Python urllib
received HTTP 403; the retained checksum-verifying helper uses curl successfully.
No initial failed probe is counted as a pass.

## Files and ownership

| File | Purpose |
| --- | --- |
| `fetch.sh`, `fetch.py`, `runtime.py` | Pinned private binary and empty-profile runtime lifecycle. |
| `build.sh`, `run.sh` | Separate optional app build and viewer launch. |
| `test-transport.sh`, `gui_probe.py`, `dom_probe.js` | Explicit synthetic validation, never enabled by the default app. |
| `evidence/` | Inspected synthetic desktop screenshots and result record. |
| `daedalus-synthetic.json` | Proposed read-only view data only. Not a Daedalus API, scheduler, or database adapter. |

The synthetic Daedalus view deliberately has no runtime mapping, nullable
unreported usage, and a separate derived human-attention field. It is a review
artifact for an owner-controlled future export, not a competing job authority.
