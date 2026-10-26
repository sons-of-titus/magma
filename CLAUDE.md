# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

See [AGENTS.md](AGENTS.md) for sprint completion gates (tests, docs, file-size limits) that apply to every contribution.

---

## Build & Test

```bash
# Build with all features (default — includes janet + gui)
cargo build

# Build without GUI (headless / server mode only)
cargo build --no-default-features --features janet

# Run tests (must always pass before closing any sprint)
cargo test --features janet

# Run a single test by name substring
cargo test --features janet buffer_major_mode

# Run only the Janet API tests serially (required when debugging Janet VM issues)
cargo test --features janet sprint1_janet_tests -- --test-threads=1

# Enable logging at runtime (levels: error, warn, info, debug, trace)
# MAGMA_LOG defaults to info; MAGMA_DEBUG_HTTP opens the HTTP channel.
MAGMA_LOG=debug cargo run --features janet
```

The `janet` feature links the `evil-janet` amalgamation and is required for all Janet bridge code.  `gui` adds `eframe` (egui).  Both are on by default.

---

## Architecture

Magma is split into a **Rust kernel** and a **Janet userland**.  The kernel owns all state and exposes it exclusively through typed APIs.  Janet owns all behavior.

```
Rust kernel:  state · buffers (rope) · event bus · command registry ·
              keymap manager · rendering surface · Janet FFI bridge
Janet userland: keybindings · major modes · colon-mode verbs ·
                config · plugins · UI formatting
```

### Rust side

| Path | Responsibility |
|------|----------------|
| `src/state/mod.rs` | `Editor` — the single god-object holding all runtime state |
| `src/buffer/mod.rs` | `Buffer` struct (rope + undo tree + marks + local options) |
| `src/event/mod.rs` | `EventBus` — sync VecDeque queue, Before/Transform/After hooks |
| `src/command/mod.rs` | `CommandRegistry` + `execute_command` |
| `src/command/builtin.rs` | All Rust-defined commands (~3000 lines, one `register_fn` per command) |
| `src/keymap/mod.rs` | Layer-based keymap — push/pop named layers per mode |
| `src/render/frame.rs` | `render_frame` — writes to the `Surface` (2-D char+style grid) |
| `src/input.rs` | `dispatch_key` — key → keymap lookup → command execute |
| `src/janet_bridge/mod.rs` | `init()`, `eval()`, `load_file()` — Janet VM lifecycle |
| `src/janet_bridge/*_api.rs` | C function groups registered per namespace (`buffer/`, `editor/`, …) |

`Editor` is wrapped in `Arc<RwLock<Editor>>` in `main.rs`.  Commands receive `&mut Editor` directly; the lock is acquired by the caller.

### Workspace crates

| Path | Responsibility |
|------|----------------|
| `crates/logging` | Leveled `error!`/`warn!`/`info!`/`debug!`/`trace!` macros, gated on `MAGMA_LOG`; imported crate-wide in `src/lib.rs` |
| `crates/id` | Typed `u64` identifiers (`BufferId`, `WindowId`, `MarkId`, …) shared by every subsystem |
| `crates/text` | Text engine: rope, `Document`, `Buffer`, `BufferView`, transactions, undo, marks, folds, and the buffer diagnostic model |
| `crates/semantic` | Language-intelligence core: language providers, semantic engine, symbol index, project graph, tree-sitter, LSP wire protocol |
| `crates/vt` | VT/ANSI terminal screen model and escape-sequence parser (no PTY, no editor state) |
| `crates/vcs` | Version-control layer: `VcBackend` trait, git/hg/svn/jj backends, shell-template backends, remote-operation dispatch and command helpers (no editor state) |
| `crates/util` | Shared primitives: `IdPool` ID allocator, child-process-group hygiene (`proctree`) |
| `crates/event` | `EventBus` — sync queue with Before/Transform/After hook chains; `events`/`keys` name constants |
| `crates/keymap` | Layer-based `KeymapManager` — push/pop named layers per mode |
| `crates/extension` | Plugin capability registry (`ExtensionRegistry`, `ExtensionManifest`, `Capability`) |
| `crates/net` | TCP connection/server state primitives (`NetConnection`, `NetServer`) |
| `crates/clipboard` | System clipboard wrapper (`Clipboard`) |

The kernel re-exports these crates as modules in `src/kernel/mod.rs`
(`pub use event;`, `pub use keymap;`, …) so `crate::kernel::<x>::…` paths keep
resolving. Extract a module into a crate only when its dependencies point
downward: `text` reaches the kernel only through `id`, and `src/kernel/state`
re-exports `id` so `state::id::…` still resolves. Subsystems still coupled to
`Editor` or to the `runtime::BackgroundEvent` channel remain in `src/kernel`.

### Janet side

| Path | What it defines |
|------|-----------------|
| `builtins/vim.janet` | Loaded first — all vim keybindings |
| `builtins/init.janet` | Loaded second — commands, colon-mode, major-mode infrastructure, plugin loader |
| `builtins/syntax.janet` | Syntax-highlight helpers |
| `builtins/plugins/lsp.janet` | LSP colon verbs |
| `~/.config/magma/init.janet` | User config — loaded after builtins |
| `~/.config/magma/plugins/*.janet` | User plugins — loaded via `(require "name")` |

### Event bus

All cross-cutting communication goes through the event bus.  Rust emits events by name with a `HashMap<String, String>` payload.  Janet subscribes with `(event/on "name" handler-fn)`.

Key events: `buffer-created` (carries `path`), `buffer-focused`, `buffer-before-save`, `buffer-after-save`, `buffer-closed`, `major-mode-changed`, `editor-ready`.

`drain_and_dispatch` is called after every key event in the main loop.  Events emitted *during* dispatch are appended and processed in the same call.

### Command system

Commands are registered with `cmds.register_fn(name, doc, arg_specs, closure)`.  Janet can define commands with `(command/define name fn)` — this registers both in `*janet-commands*` (Janet table) and in the Rust `CommandRegistry`.

`execute_command` clones the `Arc<handler>` before calling it so the borrow on `editor.commands` ends before the closure runs with `&mut Editor`.

### Janet FFI bridge

The bridge (`src/janet_bridge/`) exposes Rust state to Janet as C functions via `evil-janet`.

**Critical constraint:** Janet does not support calling `janet_continue` from within a fiber C-callback that was itself started by a direct `janet_continue` (not via `janet_dostring`).  Concretely: if an event handler fires a Janet fiber and that fiber calls `editor/run-command "some-janet-defined-command"`, a second fiber is created inside the first — this silently aborts.  The fix is to register the target command as a Rust command (`ed.commands.register_fn`) instead of a Janet-defined one, or restructure so the logic is invoked via `janet_bridge::eval` / `janet_dostring`.

`JANET_VM_LOCK` (`Mutex<()>`) serialises all Janet tests.  Always acquire it as:
```rust
let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
```

`EDITOR_PTR` is a thread-local raw pointer set by `janet_bridge::init` and `janet_bridge::eval`.  It is what `with_editor(|ed| …)` reads inside every C function.  After `eval` returns, the pointer stays set until the next `eval`/`init` call — `drain_and_dispatch` called from Rust immediately after `eval` will see the correct editor.

### Rendering

One renderer: `GuiApp` (`src/render/gui.rs`, egui/eframe).  The editor pane calls `render_frame` from `src/render/frame.rs`, which writes buffer content to a `Surface` (2-D `Vec<Cell>`); the surface is painted by the wgpu GPU glyph pipeline or the CPU fallback.  All other chrome (tabs, sidebar, status bar, dialogs) is native egui and never touches the surface.  Magma is a GUI application; there is no terminal UI.

### Test layout (`src/tests.rs`)

| Module | Content |
|--------|---------|
| `mod command_tests` | Pure Rust — cursor, editing, modal state |
| `mod dispatch_tests` | Full key→dispatch→text path |
| `mod sprint1_tests` | Sprint 1 Rust primitives (no Janet VM) |
| `mod sprint1_janet_tests` | Sprint 1 Janet API (requires `JANET_VM_LOCK`) |
| `mod janet_tests` | General Janet integration |

New sprints add a `sprint<N>_tests` + `sprint<N>_janet_tests` pair following the same pattern.
