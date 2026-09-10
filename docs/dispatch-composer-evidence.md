# Dispatch composer evidence

Source: issue #51 (dispatch loses 1 KiB chunks) and `BACKLOG.md` line 209.
Backs `pane_input::max_prompt_bytes` (`src-tauri/src/pane_input.rs:25`)
granting Linux Codex an 8192-byte prompt limit
(`CODEX_LINUX_MAX_BYTES`, `pane_input.rs:7`) instead of the legacy
1023-byte bound (`LEGACY_MAX_BYTES`, `pane_input.rs:6`) every other
target/platform combination keeps. Line numbers below are current as of
this change; they will drift with unrelated edits like any other in-repo
reference.

Reproduces `src-tauri/tests/cli_composer.rs`'s
`real_codex_composer_preserves_the_complete_production_payload`: the one
real-CLI proof this limit rests on. Nothing else in the test suite spawns
an actual Codex process.

## What this proves, and what it does not

Proves that a real Codex CLI composer, on Linux, holds the complete prompt
`pane_input::write_prompt` puts on the wire, at sizes up to and including
the new 8192-byte cap, for both ASCII and UTF-8 content, with no leading or
trailing loss. The probe exports the composer's buffer through an external
`EDITOR` before asserting anything; nothing is inferred from what merely
renders in the terminal.

Does not prove end-to-end submission: no probe run for this evidence sends
Enter or the `"\r"` Codex expects for it, and no model request is made.
Quiet-terminal detection after a real submission is a separate, unproven
concern the brief for this change excluded deliberately. This file also
does not cover Windows, OpenCode or Claude; the change under it leaves all
three, and every non-Linux platform, at the 1023-byte legacy bound
unconditionally.

## Prerequisites

- Linux only: `src-tauri/tests/cli_composer.rs` is
  `#![cfg(target_os = "linux")]` and will not compile elsewhere.
- A `codex` executable on `PATH`. This evidence was recorded against
  codex-cli 0.154.0.
- An existing, already-trusted Codex checkout directory. The probe never
  grants trust or changes configuration; it fails fast if Codex would need
  either. Point `PANTHEON_CLI_TEST_CWD` (below) at a directory you have
  already run `codex` in interactively at least once.
- `portable-pty`, `libc` and `tempfile` are already repository
  dependencies (`src-tauri/Cargo.toml`); no extra setup beyond a normal
  `cargo build` in `src-tauri/`.

## Invocation

The test is `#[ignore]` (it launches a real, unsandboxed CLI process), so
it never runs as part of `cargo test`'s default set and must be requested
explicitly:

```sh
cd src-tauri
PANTHEON_CLI_TEST_CWD=/path/to/an/already-trusted/codex/checkout \
  cargo test --test cli_composer -- --ignored --nocapture
```

`--nocapture` is required to see the printed measurement table; without it
`cargo test` swallows the test's stdout on success. Add
`PANTHEON_CLI_EVIDENCE_DIR=/path/to/a/directory` to also write each case's
expected and received bytes as `<encoding>-<size>.expected` /
`.received` files there, for a byte-level diff independent of the test's
own comparison.

The probe never sends Enter (only Codex's external-editor key, `Ctrl-G`,
`\x07`) and passes `-s read-only -a never`, so no model request can be
made regardless of what is typed. Its own process group is killed on exit,
bounded by a 60-second watchdog, and its scratch files live in a
`tempfile::tempdir()`.

## Recorded run

```
$ codex --version
codex-cli 0.154.0
```

Platform: Linux 7.0.0-30-generic x86_64.

| encoding | requested | wire | received | leading_loss | complete_equal |
| --- | --- | --- | --- | --- | --- |
| ascii | 1023 | 1035 | 1023 | 0 | true |
| ascii | 1024 | 1036 | 1024 | 0 | true |
| ascii | 2048 | 2060 | 2048 | 0 | true |
| ascii | 2645 | 2657 | 2645 | 0 | true |
| ascii | 4096 | 4108 | 4096 | 0 | true |
| ascii | 8192 | 8204 | 8192 | 0 | true |
| utf8 | 1023 | 1035 | 1023 | 0 | true |
| utf8 | 1024 | 1036 | 1024 | 0 | true |
| utf8 | 2048 | 2060 | 2048 | 0 | true |
| utf8 | 2645 | 2657 | 2645 | 0 | true |
| utf8 | 4096 | 4108 | 4096 | 0 | true |
| utf8 | 8192 | 8204 | 8192 | 0 | true |

`wire` is `requested + 12`: the existing bracketed-paste framing
(`pane_input::frame`, `\x1b[200~...\x1b[201~`) adds exactly 6 bytes on
each side regardless of size or encoding. `received` equals `requested`
in every case (`complete_equal: true`), and `leading_loss` is `0`
throughout: nothing is dropped from the front of the payload, the loss
pattern the 1 KiB legacy bound was originally chosen around.

An independent reviewer (a different session, same machine) reproduced
this table twice: once by compiling and running this same test file
out of tree, and again with a second harness built directly against
`pane_input::write_prompt`, writing into a real Codex PTY and exporting
through an external `EDITOR` the same way. Both reproductions matched
12/12 exact SHA-256 equality with correct leading bytes. That reviewer
also confirmed the cap is not sitting on a cliff: with the size guard
skipped but the production framing kept, the same composer held 8193,
12288, 16384, 32768 and 65536 bytes intact, so 8192 has headroom rather
than being the last size that happens to work.

## Production terminal mode

This probe launches Codex with `--no-alt-screen -s read-only -a never` so
its PTY output can be scanned deterministically for the composer's `Tip:`
readiness marker. Production `spawn_session` (`src-tauri/src/lib.rs`)
passes none of those flags. An independent reviewer re-ran the 8192-byte
ASCII and UTF-8 cases without `--no-alt-screen`, i.e. in production's
actual screen mode, and both were exact-equal to the same table above.
Screen mode does not change the result; `-s read-only -a never` only
narrows what Codex is willing to do with the input, not how it renders or
buffers it.

## Limits

- One machine, one CLI build. No Windows, OpenCode or Claude evidence
  exists here; the change this backs correctly leaves all of them at the
  1023-byte legacy bound.
- Composer fidelity only, as above: submission, Enter, and quiet-terminal
  detection at 8 KiB are unproven and out of scope for this evidence.
- This is a real, unsandboxed CLI launch. It grants no trust and changes
  no configuration, but it does spawn a genuine `codex` process against
  whatever `PANTHEON_CLI_TEST_CWD` points at; do not point it at a
  checkout you cannot afford to have Codex briefly attach to.
