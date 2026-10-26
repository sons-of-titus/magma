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
| `kernel::index::{extract, store, …}` | `pub use index::{IndexRecord, IndexStats, extract, record_from_text, store};` in `src/kernel/index/mod.rs` |
| `kernel::run_config::{RunConfig, defaults_for}` | `pub use run_config::{RunConfig, defaults_for};` in `src/kernel/run_config/mod.rs` |
| `kernel::project::{detect, gitignore, toolchain}` | `pub use project::{detect, gitignore, toolchain};` in `src/kernel/project/mod.rs` |
| `kernel::storage::crash_recovery::…` | `pub use crash_recovery::{…}` in `src/kernel/storage/crash_recovery.rs` |
| `kernel::task::{diag_parse::…, QuickfixEntry}` | `pub use tool_diag::{ToolDiag, parse_tool_output, QuickfixEntry};` split across `src/kernel/task/diag_parse.rs` and `src/kernel/task/mod.rs` |
| `kernel::debug::{types, session, …}` | `pub use debug_model::{session, types};` in `src/kernel/debug/mod.rs` |
| `kernel::state::{font, FontConfig, ContextFont}` | `pub use font;` + `pub use font::{ContextFont, FontConfig};` in `src/kernel/state/mod.rs` |
| `kernel::render::view_tree::…` | `pub use view_tree;` in `src/kernel/render/mod.rs` |
| `kernel::render::surface::…` | `pub use surface;` in `src/kernel/render/mod.rs` |
| `kernel::render::gpu_atlas::…`, `gpu_paint_callback::…` | `pub use gpu::gpu_atlas;` / `pub use gpu::gpu_paint_callback;` in `src/kernel/render/mod.rs` |
| `kernel::terminal::session::TerminalSession` | `pub use pty::TerminalSession;` in `src/kernel/terminal/session.rs` |

Benchmarks (`benches/`) and integration tests (`src/tests/`) import through
these paths and must keep compiling unchanged.

## Dependency graph

```
   id ──▶ text ──▶ semantic          vt

   vcs   bgpool   scheduler   snippet   icons   fs   session   index
   project ──▶ run_config   crash_recovery ──▶ fs   tool_diag ──▶ semantic
   debug_model   font   view_tree ──▶ id   surface ◀── gpu   logging
   pty ──▶ util, vt
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
| `lsp.rs` | spawns work and emits `runtime::BackgroundEvent::Lsp(semantic::Event)` (`Started`, `Diagnostics`, `Response`, …) through `runtime::BackgroundHandle` |
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

### The background-event hub

`runtime::BackgroundEvent` remains the editor's single merged vocabulary and
the foreground drains one channel from it. What changed is ownership: a
subsystem defines its own `Event`, and the merged enum wraps it instead of
restating its payloads.

The task subsystem is the first boundary:

- `kernel::task::Event` (`src/kernel/task/event.rs`) owns `OutputLine` and
  `RunCompleted`.
- `TaskScheduler` sends `BackgroundEvent::Task(task::Event)` on the shared
  `BackgroundHandle` channel.
- `runtime::handlers::task` matches the inner enum and applies it to `Editor`.
- `src/tests/task_event_tests.rs` pins payload round trips, asserts a mixed
  task/fs/custom sequence arrives complete and in send order, and checks
  success and failure completion dispatch.

The debug subsystem follows the same recipe:

- `kernel::debug::Event` (`src/kernel/debug/event.rs`) owns `Started`,
  `Stopped`, `Output`, `Terminated`, `Response`, and `CommandResult`.
- `dap_client`'s reader thread and `debug::spawn_session` send
  `BackgroundEvent::Dap(debug::Event)` on the same channel.
- `runtime::handlers::dap` matches the inner enum.
- `src/tests/dap_event_tests.rs` pins the wrapper payloads, asserts a mixed
  DAP/fs/custom sequence arrives complete and in send order, and checks output
  and termination dispatch.

The LSP subsystem follows the same recipe:

- `kernel::semantic::Event` (`src/kernel/semantic/event.rs`) owns `Diagnostics`,
  `Completion`, `Response`, `Hover`, `Definition`, `CodeActions`,
  `CompletionItems`, `RenameResult`, `Progress`, `ServerExited`, `Started`, and
  `ApplyEdit`.
- The client's reader thread and the startup thread in `scripting/lsp_api.rs`
  send `BackgroundEvent::Lsp(semantic::Event)` on the same channel.
- `runtime::handlers::lsp` matches the inner enum.
- `src/tests/lsp_event_tests.rs` pins the wrapper payloads, asserts a mixed
  LSP/fs/custom sequence arrives complete and in send order, and checks
  diagnostics and completion dispatch.

The wrapper changes neither transport nor order: both constructors send on the
same channel they used before, and `process_background_event` still marks the
frame dirty and dispatches through one exhaustive match. `TaskReady` remains a
`BackgroundEvent` variant — it is the scripting/process subsystem's readiness
signal, not a task-result event.

Still merged in the central enum:

- `index`, `project`, `vc`

Each follows the task recipe: define `<subsystem>::Event`, add one
`BackgroundEvent::<Subsystem>(…)` wrapper, route the existing constructor
sites, and keep the single channel. The sender still names
`runtime::BackgroundEvent`, which is deliberate: `bgpool` stays generic and
the kernel owns transport composition, while a subsystem crate needs only its
own event type.

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

### `crates/index`

The project symbol index format: `IndexedSymbol`, the text-level `extract`
(keyword patterns per language family, comment lines skipped, qualifiers like
`pub(crate)` stripped), `IndexRecord` with `record_from_text`, `IndexStats`,
and the `store` module — `FileRecord`, `SCHEMA_VERSION`, `content_hash`
(FNV-1a 64), `load`/`save` (JSON lines, temp+rename, foreign schema versions
discarded, corrupt lines skipped), and
`index_path_for_root(root, state_dir)`.

`index_path_for_root` takes the state directory as a parameter instead of
reading it, so the crate never touches global configuration; the kernel
passes `persist::state_dir()` at its two call sites.

Kept in the kernel (`src/kernel/index/mod.rs`): `IndexStore` (the `Editor`
field that merges records, tracks pending work and dirty state, and flushes)
and `reconcile_project_index`, which walks the project, finds stale stamps,
and spawns chunked extraction on the indexing lane. Both are re-exported
paths, so `kernel::index::extract::…` and `kernel::index::store::…` still
resolve.

### `crates/run_config`

The run-configuration shape: `RunConfig` (name, command, args, cwd, env,
`build_before_run`, `debuggable`), `to_launch_args` for the DAP `launch`
request, `resolve_cwd` (absent/blank = project root, relative joins the root,
absolute as-is), and `defaults_for(language)` — the build/test/run seeds for
rust, go, node, make, python, cmake, maven, and gradle.

Kept in the kernel (`src/kernel/run_config/mod.rs`): `RunConfigManager`
(`configs`/`active`/`pending`, `named`, `names`, `upsert`, `remove`,
`load`/`save`/`load_or_defaults`, and the `launch`/`launch_direct`/`stop`
paths that need the task scheduler and background handle), plus the
editor-facing `upsert_config`, `continue_pending`, and `announce_launch`.
`RunConfig` and `defaults_for` are re-exported.

### `crates/project`

Three editor-free helpers used by the project layer:

- `detect` — the marker-file table (`Cargo.toml`, `go.mod`, …) with first
  match winning, DAP adapter candidates per language, and `find_adapter`,
  which resolves the first installed candidate through `toolchain`.
- `gitignore` — the common `.gitignore` subset: comments, blanks, `*`/`?`
  within a segment, `**` across segments, directory-only rules, anchoring,
  and negation with last-rule-wins, plus the root-first `IgnoreStack`.
- `toolchain` — `find`/`find_in`/`find_extended`: `$PATH`, then well-known
  install roots (keg-only Homebrew LLVM, MacPorts, CodeLLDB), then `xcrun
  --find` on macOS.

`detect`'s "every detected language has defaults" guard depends on
`run_config::defaults_for`, which is why this crate sits on top of
`run_config`.

Kept in the kernel (`src/kernel/project/mod.rs`): `Project`,
`ProjectManager`, workspace membership, and `refresh_files` — the walk that
drives `gitignore::IgnoreStack` and needs editor state and Janet handlers.

### `crates/crash_recovery`

The snapshot format: `CrashSnapshot`, `RecoverableSession`, per-session
directories under `<state_dir>/crash`, FNV-1a identity stems, the JSON
manifest (including its `recovered` flag), `write_snapshot` /
`drop_snapshot` / `clear_session`, `mark_recovered`, and
`collect_from`/`collect_recoverable` — the scan that prunes all but the
newest four sessions and skips ones already handed back. Snapshot writes go
through `fs::atomic_write`.

Every function takes the state dir or crash root as a parameter, so nothing
in the crate reads global configuration.

Kept in the kernel (`src/kernel/storage/crash_recovery.rs`): `recover_from`,
which opens survivors as read-only `*recovered …*` buffers, and
`recover_into`, the startup path that reports how many came back, plus a
`new_session_dir()` wrapper that supplies `persist::state_dir()`. The rest
is re-exported through the same module path.

### `crates/tool_diag`

The build/test output parser: `parse_tool_output(command, text, cwd)` turns
combined compiler and test output into `ToolDiag` findings, which pair a
resolved file path with a `semantic::Diagnostic`.  It recognises rustc/cargo
message headers plus their ` --> path:line:col` span, `panicked at`
locations from `cargo test`, and generic `path:line[:col]: message` lines
from make, pytest, and `rg --vimgrep`, while rejecting prose that merely
contains colons.  `ToolDiag::quickfix` converts a finding to a one-based
`QuickfixEntry` (also defined here, with its empty-error `Default`).

The crate is pure — output text in, findings out.  It reaches diagnostics
through `semantic`, which re-exports the `text` diagnostic model.

Kept in the kernel: `apply_tool_diagnostics` in `src/kernel/task/mod.rs`,
which writes findings onto the buffer's tool diagnostics and the editor's
quickfix list, plus `QUICKFIX_MAX_RESULTS`.  `src/kernel/task/diag_parse.rs`
is a re-export shim, and `QuickfixEntry` is re-exported from `task`, so
`kernel::task::QuickfixEntry` and `kernel::task::diag_parse::…` still
resolve.

### `crates/debug_model`

The DAP state model: `Breakpoint` (with its `enabled` flag, which DAP itself
lacks), `StackFrame`, `Variable`, `Thread`, `MemoryRegion`, the id aliases,
and `DebugSession` — threads, frames, per-`variablesReference` scopes, watch
results and their dispatch queue, the stopped location, a capped console
ring, verified breakpoints, and the adapter sequence counter.

The crate is pure state: no adapter process, no channels, no editor.
`DebugSession::new`/`next_seq`/`push_console`/`note_stopped`/`set_frames`/
`set_threads`/`set_variables`/`set_verified_breakpoints`/`frame` moved
verbatim.

Kept in the kernel: `DebugManager` (session registry, per-file breakpoints,
watches, live `DapHandle` connections) in `src/kernel/debug/mod.rs` and the
adapter I/O in `dap_client.rs`. The `types` and `session` modules are
re-exported, so `kernel::debug::types::…`, `kernel::debug::session::…`, and
the flat `kernel::debug::{Breakpoint, DebugSession, …}` paths all still
resolve.

### `crates/font`

`FontConfig` — family, size, weight, fallback list, glyph widths, ligatures,
per-context overrides, and the loaded face bytes — plus `ContextFont`, the
two registration aliases (`magma-mono`, `system-mono`), and the loading
policy: `bootstrap_bundled` (the two embedded JetBrains Mono faces),
`load_system_mono` (first readable candidate), `bootstrap_default_font`
(system mono, else bundled, and a no-op once anything is loaded),
`primary_font`, and `default_font_candidates` (Nerd Font copies under
`~/Library/Fonts`, then Monaco, DejaVu, and Consolas).

The crate is pure configuration — it never rasterizes. The GUI font registry
(`render/gui_fonts.rs`) and the GPU atlas stay in the kernel and read the
config.

`src/kernel/state/mod.rs` re-exports the module and the two public types, so
`kernel::state::font::default_font_candidates()` and
`kernel::state::FontConfig` keep resolving.

### `crates/view_tree`

The pane layout model: `ViewTree`, `Pane`, `SplitDirection`, and
`LayoutConstraint` (split, tab, float).  It owns the pane list, the id
allocator, focus, the terminal dimensions, the current split axis, and the
`redistribute` pass that turns weights into geometry — tab panes stack at
full size, split panes tile along one axis with the last pane absorbing
rounding, and the status line keeps one row.

The crate is pure layout state: no rendering, no input routing, and no
buffer ownership — a pane only stores a `buffer_id`.

`src/kernel/render/mod.rs` re-exports the module, so
`kernel::render::view_tree::{ViewTree, Pane, …}` still resolves for the
renderer, the mouse layer, and the existing layout tests.

### `crates/surface`

The character grid the editor pane draws into: `Surface` (width, height,
cells), `Cell` (character plus style), and `Style` (fg/bg, bold, italic,
underline, strikethrough, dim).  Writes outside the grid are dropped,
`set_text` advances one cell per character and keeps the existing style when
none is given, `clear` resets cells, `fill_style` paints a background while
keeping characters, and `resize` starts blank (a same-size resize is a
no-op).

The crate is a container only — nothing here rasterizes or paints.  The
renderer and the GUI paint callback read the grid.

`src/kernel/render/mod.rs` re-exports the module, so
`kernel::render::surface::{Surface, Cell, Style}` still resolves for every
view and test.

### `crates/gpu`

The GPU text path, split from the kernel's render layer:

- `gpu_atlas` — the CPU-side atlas: `GpuGlyphAtlas` with its UV grid for the
  baked character set (printable ASCII, box-drawing, arrows, Latin-1), the
  placeholder solid-white pixel buffer, the dirty flag, `build_glyph_instances`
  and `build_rect_instances` from a `Surface`, and the vertex expanders.  No
  wgpu types, so it is testable without a device.
- `gpu_rasterizer` (`gui` feature) — `ab_glyph` rasterization of the atlas,
  centred per cell with a baseline from the font's ascent metrics and a
  solid-white fallback when the face fails to parse.
- `gpu_paint_callback` (`gui` feature) — the `egui_wgpu` paint callback that
  creates the atlas texture and draws glyph and rect vertices.

The crate depends on `surface` for the grid and enables its `gui` feature
from the kernel's `gui` feature, so `--no-default-features` builds only the
atlas.  `src/kernel/render/mod.rs` re-exports `gpu_atlas` and
`gpu_paint_callback`, so `kernel::render::gpu_atlas::…` and
`kernel::render::gpu_paint_callback::…` still resolve.  `gpu_rasterizer` was
`pub(crate)` and is now reached through `gpu::gpu_rasterizer` (its one test
call site moved with it).

### `crates/pty`

One managed PTY session: the child's pid, the writer half of the master, the
master itself (behind a `Mutex`, since `dyn MasterPty` is `Send` but not
`Sync` and `Editor` must stay `Sync`), the `ChildKiller` handle used after the
child moves to the waiter thread, and the `TerminalScreen` the reader thread
fills.  Sessions never live in the buffer slab.

The crate owns the lifecycle seam: `TerminalSession::new`, `send_input`,
`resize`, and `kill` (a whole-group kill through `util::proctree`, then the
kill handle).  It also owns tab naming: `rename` marks the name user-owned and
returns whether it changed, and `renumber_all` regenerates `"{label} {index}"`
with gapless per-label numbering while skipping renamed tabs.

`TerminalManager` — the session vector, the focused id, key routing, spawning,
and event pumping — stays in the kernel at `src/kernel/terminal/mod.rs`, which
now builds sessions through the constructor and delegates `rename`/`renumber`
to the crate.  `src/kernel/terminal/session.rs` is a re-export shim, so
`kernel::terminal::session::TerminalSession` and
`kernel::terminal::TerminalSession` still resolve.

Crate tests spawn real `sh -c 'read _'` / `sleep 30` children under a PTY:
renumbering (gapless per label, renamed tabs untouched), the rename
changed/unchanged return, input and resize reaching the master, kill
terminating the child, and kill through the handle when the pid is absent.

## Remaining blockers

### Background-event adoption

The task, debug, and LSP vocabularies are now namespaced (see above), which
proves the single-channel wrapper without changing delivery order. The same
split is still pending for `index`, `project`, and `vc`; until then those
subsystems keep their variants in the central enum and stay in the kernel.

### `Editor` coupling

`command` (89 refs), `render` (157), `scripting` (53), and `runtime` (24)
operate directly on the concrete `Editor`. Extracting them needs a narrow
context/capability interface (viewport, text snapshots, theme, cursor for
render; command surface and capabilities for scripting), not a crate move.

## Verification

The extraction must keep these green:

```bash
cargo check --workspace --all-targets
cargo test --workspace          # 1752 passed (1742 baseline + 10 event-boundary regression tests)
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
cargo test -p index             # extraction per language family, index format round trip/versioning
cargo test -p run_config        # defaults per language, cwd resolution, DAP launch args
cargo test -p project           # detection priority, .gitignore rules, toolchain lookup
cargo test -p crash_recovery    # snapshot round trips, pruning, recovery marking
cargo test -p tool_diag         # rustc/panic/generic parsing, quickfix conversion
cargo test -p debug_model       # session lifecycle, console cap, sequence numbers
cargo test -p font              # defaults, bundled bootstrap, candidate ordering
cargo test -p view_tree         # geometry, splits, tabs, focus, reorder
cargo test -p surface           # bounded writes, text, clear, fill, resize
cargo test -p gpu               # atlas layout, instance builders, vertex expansion
cargo test -p gpu --features gui  # + glyph rasterization and the paint callback
cargo test -p pty               # real PTY children: input, resize, kill, tab naming
```

`cargo fmt --check` fails on `benches/` at `a8ffde6`; that predates this work
and is not caused by the extraction.
