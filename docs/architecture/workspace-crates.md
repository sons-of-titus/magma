# Workspace Crates

Magma's kernel is being split into workspace crates along dependency lines.
This page records the boundaries that exist today, the re-export contract that
keeps old paths working, and the coupling that still blocks further extraction.

## Contract

Every extracted crate is re-exported from the kernel so existing paths resolve
without call-site changes:

| Old path | Resolves through |
|---|---|
| `kernel::util::…` | `pub use util;` in `src/kernel/mod.rs` |
| `kernel::event::…` | `pub use event;` |
| `kernel::keymap::…` | `pub use keymap;` |
| `kernel::extension::…` | `pub use extension;` |
| `kernel::net::…` | `pub use net;` |
| `kernel::clipboard::…` | `pub use clipboard;` |
| `kernel::state::id::…` | `pub use id;` in `src/kernel/state` |
| `kernel::text_engine::…` | `pub use text as text_engine;` |
| `kernel::semantic::<core module>::…` | `pub use semantic::<module>;` in `src/kernel/semantic/mod.rs` |
| `kernel::terminal::ansi::…`, `kernel::terminal::screen::…` | `pub use vt::{ansi, screen};` in `src/kernel/terminal/mod.rs` |
| `kernel::vc::…` | `pub use vcs::…` in `src/kernel/vc/mod.rs` |

Benchmarks (`benches/`) and integration tests (`src/tests/`) import through
these paths and must keep compiling unchanged.

## Dependency graph

```
                       logging
                          |
   util   event   keymap  extension   net   clipboard   id   vcs
                                                    |
                                                  text        vt
                                                    |
                                                 semantic
                                                    |
     ┌──────────────────────────────────────────────┴──────────────┐
     │                         magma (kernel)                      │
     │  state::Editor · command · render · scripting · runtime ·   │
     │  task · project · debug · index · vc · terminal · …         │
     └─────────────────────────────────────────────────────────────┘
```

Arrows point at dependencies. There are no cycles between workspace crates.
The kernel may depend on any crate; crates never depend on the kernel.

## What moved, and why

### `crates/id`

The typed `u64` identifiers (`BufferId`, `WindowId`, `CommandId`,
`SubscriptionId`, `HookId`, `MarkId`) are named by buffers, views, commands,
rendering, and the Janet bridge. Keeping them in `state` forced every
would-be crate to depend on the `Editor` module. They now sit at the bottom
of the graph with no dependencies.

### `crates/text`

The buffer layer: `Rope`, `Document`, `LineIndex`, transactions, anchors,
undo, marks, folds, `Buffer`, `BufferView`, decorations, and visual layout.
It has no `Editor` references and no background-runtime coupling.

The buffer diagnostic model (`Diagnostic`, `DiagnosticSeverity`) moved with
it: a `Buffer` owns its `lsp_diagnostics`/`tool_diagnostics` lists, so the
data model lives in `text::diagnostic` and `semantic::provider` re-exports it.

### `crates/semantic`

The provider/engine core: `LanguageProvider` and its data types,
`SemanticEngine`, `SymbolIndex`, `ProjectGraph`, `LanguageRegistry`, the
tree-sitter provider/registry, `LspLanguageProvider`, and the LSP wire
protocol (`lsp_proto`).

Kept in the kernel (`src/kernel/semantic/`), because they are editor-coupled:

| Module | Blocking dependency |
|---|---|
| `lsp.rs` | spawns work and emits `runtime::BackgroundEvent` (`LspStarted`, `LspDiagnostics`, `LspResponse`, …) through `runtime::BackgroundHandle` |
| `edit_apply.rs` | mutates `Editor.buffers` |
| `ts_schedule.rs` | mutates `Editor`, submits to `runtime::BackgroundHandle` |

### `crates/vt`

The terminal emulation core: `TerminalScreen` (cells, colors, styles,
scrollback, cursor), the `feed` escape-sequence parser, and their unit tests.
It has no dependency on PTYs, `Editor`, or the background runtime.

The kernel's `kernel::terminal` keeps the PTY sessions
(`TerminalSession`/`TerminalManager`), process-group cleanup, and the
`BackgroundEvent` plumbing; it re-exports `ansi` and `screen` so
`kernel::terminal::ansi::feed` and `kernel::terminal::screen::TerminalScreen`
still resolve.

### `crates/vcs`

The version-control abstraction: the `VcBackend` trait, its data types
(`VcStatus`, `FileStatus`, `BranchInfo`, `StashEntry`), the git/hg/svn/jj
backends, shell-template backends for plugin-registered VCSes, and the
status/diff/branch/stash/hunk parsers.

`remote.rs` holds `VcRemoteOp`, `run_op`, `status_for_root`, and the
`run_cmd`/`run_cmd_stdin` process primitives. Backends are stateless CLI
wrappers, so `run_op` re-detects one by name instead of moving a trait object
across threads.

Kept in the kernel (`src/kernel/vc/`): `VcState` — the session state on
`Editor` (active backend, repo root, last status, diff view, branches,
stashes) — and `ops.rs` with the side-by-side diff layout
(`DiffData`/`HiddenPane`/`DiffView`). Both are editor state.

Parsers the kernel calls (`split_hunks`, `parse_branch_lines`,
`parse_stash_lines`) are `pub` in the crate and reached through the re-export
as `kernel::vc::backends::…`; the status parser (`parse_porcelain_z`) stays
crate-internal.

## Remaining blockers

### The background-event hub

`runtime::BackgroundEvent` is one enum containing every subsystem's async
results. `BackgroundHandle` is parameterised by it:

- `task` ↔ `runtime` (task scheduler sends `TaskOutputLine`, `TaskRunCompleted`)
- `debug` ↔ `runtime` (`DapStarted`, `DapStopped`, …)
- `semantic::lsp` ↔ `runtime` (`LspStarted`, `LspDiagnostics`, …)
- `index`, `project`, `vc` also send variants through it

Extracting any of those subsystems requires splitting the event vocabulary
per subsystem or making the channel generic over its event type. Neither is a
mechanical change; both change signatures at many call sites. Until then the
subsystems stay in `kernel` and the hub remains the primary blocker.

### `Editor` coupling

`command` (89 refs), `render` (157), `scripting` (53), and `runtime` (24)
operate directly on the concrete `Editor`. Extracting them needs a narrow
context/capability interface (viewport, text snapshots, theme, cursor for
render; command surface and capabilities for scripting), not a crate move.

## Verification

The extraction must keep these green:

```bash
cargo check --workspace --all-targets
cargo test --workspace          # 1604 passed before and after the extraction
cargo test -p text              # buffer/text-engine unit tests
cargo test -p semantic          # provider/engine unit tests
cargo test -p vt                # terminal screen/ANSI parser tests
cargo test -p vcs               # backends, parsers, live-git round trips
```

`cargo fmt --check` fails on `benches/` at `a8ffde6`; that predates this work
and is not caused by the extraction.
