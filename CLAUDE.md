# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

See [AGENTS.md](AGENTS.md) for sprint completion gates (tests, docs, file-size limits) that apply to every contribution.

---

## Build & Test

```bash
# Build with all features (default — includes janet + gui)
cargo build

# Build without GUI (TUI only)
cargo build --no-default-features --features janet

# Run tests (must always pass before closing any sprint)
cargo test --features janet

# Run a single test by name substring
cargo test --features janet buffer_major_mode

# Run only the Janet API tests serially (required when debugging Janet VM issues)
cargo test --features janet sprint1_janet_tests -- --test-threads=1

# Enable debug logging at runtime
MAGMA_LOG=1 cargo run --features janet
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

Two renderers: `TuiRenderer` (`src/render/tui.rs`, crossterm) and `GuiApp` (`src/render/gui.rs`, egui).  Both call `render_frame` from `src/render/frame.rs` which writes to a `Surface` (2-D `Vec<Cell>`).  The TUI renderer then diffs the surface against the terminal.

### Test layout (`src/tests.rs`)

| Module | Content |
|--------|---------|
| `mod command_tests` | Pure Rust — cursor, editing, modal state |
| `mod dispatch_tests` | Full key→dispatch→text path |
| `mod sprint1_tests` | Sprint 1 Rust primitives (no Janet VM) |
| `mod sprint1_janet_tests` | Sprint 1 Janet API (requires `JANET_VM_LOCK`) |
| `mod janet_tests` | General Janet integration |

New sprints add a `sprint<N>_tests` + `sprint<N>_janet_tests` pair following the same pattern.
