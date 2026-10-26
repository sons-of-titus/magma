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
| `kernel::scheduler::…` | `pub use scheduler::…` in `src/kernel/scheduler/mod.rs` |
| `kernel::snippet::…` | `pub use snippet::{TabStop, parse_tabstops};` in `src/kernel/snippet/mod.rs` |
| `kernel::icons::…` | `pub use icons::{…}` and `pub use icons::vscode;` in `src/kernel/icons/mod.rs` |
| `kernel::storage::{FileSystem, …}` | `pub use fs::{…}` in `src/kernel/storage/mod.rs` |
| `kernel::storage::disk::…` | `pub use fs::{DiskFileSystem, atomic_write};` in `src/kernel/storage/disk.rs` |
| `kernel::session::…` | `pub use session::{SessionState, …};` in `src/kernel/session/mod.rs` |

Benchmarks (`benches/`) and integration tests (`src/tests/`) import through
these paths and must keep compiling unchanged.

## Dependency graph

```
   id ──▶ text ──▶ semantic          vt

   vcs   bgpool   scheduler   snippet   icons   fs   session   logging
   util   event   keymap   extension   net   clipboard

     ┌─────────────────────────────────────────────────────────────┐
     │                         magma (kernel)                      │
     │  state::Editor · command · render · scripting · runtime ·   │
     │  task · project · debug · index · vc · terminal · …         │
     └─────────────────────────────────────────────────────────────┘
```

Only `id → text → semantic` has intra-crate edges; every other crate is a
leaf the kernel depends on.

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
as `kernel::vc::backends::…`; the crate's own status parser
(`parse_porcelain_z`) is public too, so backends and tests share one parser
rather than duplicating it.

The backends live in `crates/vcs/src/backends/`, one file per VCS
(`git.rs`, `hg.rs`, `svn.rs`, `jj.rs`), with `shell_template.rs` for
runtime-registered backends and `parsers.rs` for their shared output
parsers.

### `crates/bgpool`

The scheduling half of the background system: `Pool<E>` (a cloneable handle
over a tokio runtime and an event channel), the three priority lanes
(foreground, indexing, maintenance) with bounded concurrency, and
ticket-based coalescing so a superseded job skips instead of doing stale
work. `spawn`, `spawn_blocking`, `spawn_on`, `block_on`, and `send_event`
are the whole surface.

The pool is generic over the event type and has no Magma dependency, so a
subsystem crate can own its event vocabulary and still share the scheduling
policy.

The kernel's `src/kernel/runtime/` keeps `BackgroundEvent` (the editor's
merged event vocabulary) and the handlers that apply an event to `Editor`.
`BackgroundHandle` is a thin specialization of `bgpool::Pool<BackgroundEvent>`
plus the typed file read/write helpers; it derefs to the pool, so call sites
(`bg.sender`, `bg.spawn_on`, `bg.read_file`) are unchanged.

### `crates/scheduler`

The bookkeeping half of background work: `WorkId`, `WorkStatus`,
`WorkProgress`, `CancellationToken`, `ProgressReporter`, the `Work` trait, and
`WorkScheduler`, which tracks items and maps external task/process ids onto
their work ids. It has no dependencies: not tokio, not the editor.

Execution and delivery are abstracted by two small pieces:

- `WorkEvent` — `Progress { id, done, total }`, `Completed { id }`,
  `Failed { id, error }`.
- `WorkHost` — `spawn(job)` plus `sink() -> WorkSink`, where `WorkSink` is a
  cloneable `Fn(WorkEvent)` handle.

The kernel implements `WorkHost for BackgroundHandle` in
`src/kernel/scheduler/mod.rs`: `spawn` forwards to the bgpool's blocking
pool, and `sink` maps each `WorkEvent` onto the matching `BackgroundEvent`
variant the foreground already handles. Call sites still read
`ed.scheduler.submit_fn(name, &bg, …)` — `&BackgroundHandle` coerces to
`&dyn WorkHost`.

`WorkScheduler` tracks status itself; the kernel's handlers still apply the
incoming events (`update_progress`, `mark_completed`, `mark_failed`) so the
event bus subscribers see them in the same order as before.

### `crates/snippet`

The snippet engine: `TabStop`, `parse_tabstops` (brace placeholders,
`$N` stops, `$0` final cursor), `expand(trigger)`, and the built-in snippet
table. It is pure — no editor state, no buffer access.

Kept in the kernel (`src/kernel/snippet/mod.rs`): the two editor-facing
stubs the command layer already calls, `expand_snippet(&Editor, trigger)`
and `register_snippet(&mut Editor, …)`. The first delegates to
`snippet::expand`; the second is still a no-op because snippets are not yet
stored on `Editor`.

Known limitation carried over unchanged: a bare multi-digit stop (`$12`)
does not terminate in `parse_tabstops`. Built-in expansions use only the
`${…}` form, so it is unreachable from the current UI.

### `crates/icons`

The icon-pack model and loader: `IconKind` (glyph or SVG), `IconPack` with
its lookup precedence (exact file name, then longest extension suffix, then
the pack default), folder icons, `pack_search_dirs`, and `vscode::load_pack`
for VS Code `icon-theme.json` manifests (`iconDefinitions`, `iconPath`,
`fontCharacter`, `fontColor`).

Kept in the kernel (`src/kernel/icons/mod.rs`): `sync_option(&mut Editor)`,
the per-frame follower for the `icon-pack` option. It reads
`Editor.options`, swaps `Editor.icons`, restores `Editor.default_icons` for
`"default"`, and clears the option plus reports an error when a pack fails to
load. Everything else is re-exported, including the `vscode` module, so
`kernel::icons::vscode::load_pack` still resolves.

### `crates/fs`

The file system seam: `FileSystem` (read/write/exists/dir), `AsyncFileSystem`
(the same four over tokio), `FsError` with its `std::io::Error` mapping,
`DiskFileSystem`, and `atomic_write` — the temp-file + fsync + rename path
that makes a crash unable to leave a torn file.

`Editor.fs` is a `Box<dyn FileSystem>`, so the trait has to live below the
kernel for any test double to implement it without depending on `Editor`.

Kept in the kernel (`src/kernel/storage/`): `crash_recovery`,
`reload`, `watcher`, and `workspace`, all of which read or write `Editor`
state.  `disk.rs` is a three-line re-export shim so
`kernel::storage::disk::DiskFileSystem` and `…::atomic_write` keep
resolving.

### `crates/session`

The session file format: `SessionState` with `SessionBuffer` (path, cursor),
`SessionBreakpoint` (file, line, `enabled` defaulting to true), and watches,
plus `session_file`, `write`, and `read` for
`<root>/.magma/session.json`. `read` yields `None` for a missing or
unparsable file — a damaged session is not actionable.

Kept in the kernel (`src/kernel/session/mod.rs`): `capture(&Editor)`, which
walks buffers/views/breakpoints to build the state, `restore(&mut Editor)`,
which reopens files and re-applies the state, `save`/`save_if_project`, and
the `open-file` call those use.

## Remaining blockers

### The background-event hub

`runtime::BackgroundEvent` is one enum containing every subsystem's async
results. The scheduling half is now the `bgpool` crate (`Pool<E>` is generic
over the event type), but the vocabulary itself still ties subsystems to the
kernel:

- `task` ↔ `runtime` (task scheduler sends `TaskOutputLine`, `TaskRunCompleted`)
- `debug` ↔ `runtime` (`DapStarted`, `DapStopped`, …)
- `semantic::lsp` ↔ `runtime` (`LspStarted`, `LspDiagnostics`, …)
- `index`, `project`, `vc` also send variants through it

Extracting any of those subsystems requires splitting the vocabulary into
per-subsystem event enums and giving each subsystem its own `bgpool::Pool`
over its own type, with the foreground either merging channels (a forwarding
task changes cross-subsystem delivery order) or keeping one channel and
wrapping the sub-events (`BackgroundEvent::Task(TaskEvent)`), which still
leaves the sender naming the kernel type. Both are real design changes, not
file moves. Until one is chosen those subsystems stay in `kernel` and the
enum remains the primary blocker.

### `Editor` coupling

`command` (89 refs), `render` (157), `scripting` (53), and `runtime` (24)
operate directly on the concrete `Editor`. Extracting them needs a narrow
context/capability interface (viewport, text snapshots, theme, cursor for
render; command surface and capabilities for scripting), not a crate move.

## Verification

The extraction must keep these green:

```bash
cargo check --workspace --all-targets
cargo test --workspace          # 1676 passed (1610 pre-existing + 66 new crate tests)
cargo test -p text              # buffer/text-engine unit tests
cargo test -p semantic          # provider/engine unit tests
cargo test -p vt                # terminal screen/ANSI parser tests
cargo test -p vcs               # backends, parsers, live-git round trips
cargo test -p bgpool            # lane submission and event-channel tests
cargo test -p scheduler         # work bookkeeping, progress, cancellation, host wiring
cargo test -p snippet           # tab stops, placeholders, built-in expansions
cargo test -p icons             # lookup precedence, folder icons, VS Code manifests
cargo test -p fs                # sync/async round trips, atomic write, error mapping
cargo test -p session           # session file round trip, defaults, corrupt-file handling
```

`cargo fmt --check` fails on `benches/` at `a8ffde6`; that predates this work
and is not caused by the extraction.
