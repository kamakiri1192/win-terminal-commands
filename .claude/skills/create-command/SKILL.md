---
name: create-command
description: Implement one new macOS-style terminal command for Windows in this repo, end-to-end. Use when asked to implement/add/build/port a command (pbcopy, pbpaste, trash, md5, shasum, say, caffeinate, uuidgen, realpath, readlink, sw_vers, …) — research the spec, write the code + docs, test it, get a codex review, and open a PR. Invoke as `/create-command <command>`.
---

# create-command — implement one command, start to PR

`/create-command <command>` runs the full pipeline this repo uses to add a single
macOS/Unix-style command to Windows: **research → implement → test → codex
review → PR**. One command per invocation; do not scope-creep into a second.

The command name comes from the invocation argument, e.g. `/create-command pbcopy`.
If no name is given, ask which command before doing anything else.

## Inputs

- **`<command>`** (required) — the command to implement, e.g. `pbcopy`.
- **Roadmap / research** — `C:\Users\kamak\Downloads\memo.txt` is the
  author's master plan. It classifies every candidate (Phase 1/2/3, "make /
  don't make"), gives Windows implementation candidates, and contains a
  worked spec for `say`. Read the section for `<command>` and pull only what
  is relevant. Treat the memo as a starting hypothesis, not a spec — verify
  against the real macOS/BSD command's behavior.

## Conventions (read these before writing any code)

These are non-negotiable repo rules. Violating them causes rework.

1. **Additive-only PR.** A command PR adds **new files only**:
   `src/bin/<command>.rs` + `docs/commands/<command>.md`. The diff should be
   **0 deletions**.
   - Do **NOT** edit `README.md` (no command-table rows, no links). README
     consolidation is a separate PR after several command PRs land.
   - Do **NOT** edit `Cargo.toml`. Do **NOT** add a `[[bin]]` entry — Cargo
     auto-discovers `src/bin/*.rs` (verified: `cargo build --bin <command>`
     resolves with no entry). Multiple parallel command PRs all touch those
     two files; keeping each PR additive makes them conflict-free.
2. **Modern Windows APIs first, legacy only as fallback.** For Windows-native
   features prefer WinRT / modern Shell (e.g. `Windows.Media.SpeechSynthesis`
   for `say` — gives Win11 natural voices) over classic Win32/SAPI. Legacy is
   acceptable only as a fallback.
3. **Minimal dependencies.** Commands are pure Rust where feasible (e.g.
   `md5` ships its own RFC-1321 impl, no crate). A single-call Win32 FFI
   (`extern "system"` + `#[link]`) is fine for trivial things (`which`,
   `open`, `sw_vers`). Pulling in the `windows` crate is justified only when
   a command needs a real WinRT object model (`say`). `gzip`/`gunzip` share
   an engine in `src/gzip.rs` invoked from two thin `src/bin` shims — reuse
   that `src/<name>.rs` + `pub mod` pattern if a future command pair shares
   logic.

## The workflow

Work on a branch cut from `master` (the PR base):

```powershell
git checkout master
git pull
git checkout -b feat/add-<command>
```

### 1. Research

- Open `C:\Users\kamak\Downloads\memo.txt`; read the row/spec for `<command>`.
  Note the memo's "make / don't make" verdict — some Unix commands are
  deliberately NOT reimplemented because Windows ships them (`curl`, `tar`,
  `ssh`, …). If the memo says don't make it, stop and confirm with the user.
- Decide the **Windows implementation surface**: modern API first, fallback
  second. Note which `windows`-crate features you'll need (mirror how `say`
  lists them under `[target.'cfg(windows)'.dependencies]`).
- Know the real command's behavior you're matching: argument parsing, exit
  codes, stdin/stdout, output format. macOS/BSD `md5`, GNU `md5sum`, etc.
  differ — pick the macOS/BSD-compatible subset (see how `md5` handles both
  GNU and BSD checksum-file formats).

### 2. Implement

- Create `src/bin/<command>.rs`. Follow the established shape (look at
  `src/bin/md5.rs` / `which.rs`): module-level `//!` doc comment, a `parse_args`
  over `env::args_os().skip(1)`, an `Options` struct, a `run(&Options) -> i32`
  returning the exit code, and a `#[cfg(test)] mod tests` block.
- Mirror the platform split: real impl under `#[cfg(windows)]`, a clear
  "only supported on Windows" stub under `#[cfg(not(windows))]`.
- Create `docs/commands/<command>.md` in the repo's Japanese doc style (see
  `docs/commands/md5.md`): title, `使い方` with `powershell` code blocks,
  `オプション`, exit codes, and any platform gotchas.

### 3. Build & test

```powershell
cargo build --bin <command>
cargo test --bin <command>
```

> The **first build compiles the `windows` crate (~12 s) even for a
> std-only command** — `windows` is a package-level
> `[target.'cfg(windows)'.dependencies]` entry, so Cargo builds it for any
> bin in the package. Subsequent builds are cached. Don't be surprised.

### 4. Run it and verify real behavior

`cargo test` passing is not enough — actually drive the binary:

```powershell
cargo run --bin <command> -- <representative args>
```

- Compare output against the real macOS/BSD command, or a Windows reference
  (e.g. cross-check `md5` against `Get-FileHash -Algorithm MD5`).
- Cover the happy path **and** error/exit-code paths (missing file → exit 1,
  usage error → exit 2, etc.).
- PowerShell piping gotcha for hashing commands: `Get-Content file.zip | md5`
  corrupts bytes (text decode). Verify binary hashing by passing the file as
  an argument, or pipe `Get-Content -AsByteStream` (5.1: `-Encoding Byte`).
- For audio/clipboard/GUI commands with no stdout to assert on, verify by
  side effect (clipboard contents, an audible result, an opened window) and
  say explicitly what you observed.

### 5. Codex review

The `codex` MCP server is **not** available mid-session in worktree sessions,
so run the codex CLI directly. **Use the PowerShell tool, not Bash** — `codex`
is a mise shim and the Bash tool fails with
`mise ERROR batch file arguments are invalid`.

Review the branch diff against `master` (codex is ChatGPT-authenticated;
exit 0 = success):

```powershell
codex review --base master
```

For a pre-commit review of working-tree changes, use `--uncommitted` instead.
LLM reviews take 1–3 min — run it in the background (`run_in_background`) and
you'll get a completion notification.

**Gotcha:** `--uncommitted` is mutually exclusive with the prompt argument.
To add a review focus, pipe a prompt via stdin (drop `--uncommitted`, use `-`):

```powershell
'Review the new <command> command for correctness, macOS/BSD parity, and edge cases.' |
  codex review --base master -
```

Address real findings (the repo's release CI only runs on Release publish, so
there are no PR CI checks to gate on — the codex review is the gate). Re-run
the review after fixes if a finding was substantive.

### 6. Open the PR

Only the two new files should be staged:

```powershell
git add src/bin/<command>.rs docs/commands/<command>.md
git status          # confirm 0 deletions, only the 2 new files
git commit -m "feat: add <command> command for Windows"
git push -u origin HEAD
gh pr create --base master --title "feat: add <command> command" --body @-
```

PR body should cover: what the command does, the macOS/BSD behavior it
mirrors, the Windows API used (modern-first), and a pointer to
`docs/commands/<command>.md`.

## Gotchas

- **Additive-only.** Editing `README.md` or `Cargo.toml` is the #1 way to
  break parallel command PRs. Don't.
- **No `[[bin]]` entry.** `src/bin/*.rs` is auto-discovered; an explicit
  entry is unnecessary and would touch `Cargo.toml`.
- **First build is slow (~12 s)** because the `windows` crate compiles for
  every bin. Not a hang.
- **codex must run under PowerShell.** Bash invocation of the mise shim fails.
- **`--uncommitted` ⊥ prompt.** Pipe a focus prompt via stdin instead.
- **`mcp__codex__*` tools are absent in worktree sessions** — that's why this
  uses the CLI. Don't waste turns looking for the MCP tools.
- **Windows-only build.** `say`/`open`/`sw_vers` use `cfg(windows)` FFI; they
  won't compile on non-Windows. Build/test on Windows (this repo's only
  target).

## Troubleshooting

- **`cargo build` compiles `windows` even though my command is pure std** —
  expected (package-level dep). See Gotchas.
- **Bash tool errors on `codex` with a mise message** — switch to the
  PowerShell tool.
- **`codex review --uncommitted "my focus"` errors about prompt** —
  `--uncommitted` can't take a prompt. Use `'focus' | codex review -`.
- **Merge conflict on `README.md`/`Cargo.toml` when opening the PR** — you
  edited a shared file. Revert those edits; the PR must be additive.
