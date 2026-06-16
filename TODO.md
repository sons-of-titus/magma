# Phase 1 — Text Engine Overhaul: Remaining Work

## Status
Core types done: `Position`, `BufferView`, `Buffer` (content-only), `Editor.buffers/views` updated.
Render files done: `frame.rs`, `highlight_pass.rs`, `decorations.rs`, `status_and_popup.rs`.
Command files partially done: `editing.rs`, `motion.rs`, `helpers.rs`, `buffer.rs`, `file.rs`.

376 compile errors remain across 21 files (no-default-features build).

---

## Files Still Needing Fixes

### Command Builtins

- [ ] `src/kernel/command/builtin/colon.rs`
  - `:w`/`:wq` — replace `get_mut` with Arc lock
  - `:e`, `:tab` — replace manual buffer creation with `editor.create_buffer_from_str()`
  - `:r` — use `view.cursor.offset` instead of `buf.cursor()`
  - `:s` — use `view.replace()` for substitute

- [ ] `src/kernel/command/builtin/navigation.rs`
  - `cursor-up`/`cursor-down` — `current_line(buf)` → `current_line(view)`, same for `current_col`
  - `move_to_line_col(buf, ...)` → `move_to_line_col(view, ...)`
  - All cursor reads via `editor.views.get_mut(&buf_id)`

- [ ] `src/kernel/command/builtin/completion.rs`
  - Cursor reads/writes go through view
  - Buffer content reads: lock Arc

- [ ] `src/kernel/command/builtin/misc.rs`
  - Any `buf.cursor()`, `buf.set_cursor()` → view
  - Buffer creation sites → `editor.create_buffer()`

- [ ] `src/kernel/command/builtin/registers.rs`
  - Cursor-relative insertions → `view.insert()`

- [ ] `src/kernel/command/builtin/search.rs`
  - Highlight layers → `view.set_highlights_layer()`
  - Cursor reads → view

- [ ] `src/kernel/command/builtin/vcs.rs`
  - Buffer creation → `editor.create_buffer_from_str()`
  - Any cursor/view-state access → view

- [ ] `src/kernel/command/builtin/visual.rs`
  - Visual selection uses cursor → view cursor
  - Folds → `view.folds`

### Core Dispatch & Input

- [ ] `src/kernel/command/mod.rs`
  - `execute_command` / any direct buffer cursor access → view

- [ ] `src/kernel/input/mod.rs`
  - `focused_buffer_id` still works (window lookup)
  - Anywhere `buf.cursor()` used → view

- [ ] `src/kernel/input/mouse.rs`
  - Mouse click → cursor placement via `view.set_cursor()`

### Runtime / Infrastructure

- [ ] `src/kernel/runtime.rs`
  - Background tasks that touch buffer cursor/state → view

- [ ] `src/kernel/server/mod.rs`
  - Any direct buffer field access → Arc lock + view

- [ ] `src/kernel/task/mod.rs`
  - Quickfix jumps set cursor → `view.set_cursor()`
  - Buffer creation for output buffers → `editor.create_buffer()`

- [ ] `src/kernel/terminal/mod.rs`
  - Terminal buffer creation → `editor.create_buffer()`
  - Terminal cursor/scroll → view

---

### Scripting API (Janet FFI — requires `--features janet` build)

- [ ] `src/kernel/scripting/buffer_query_api.rs`
  - `c_buffer_list` iterates buffers → lock each Arc
  - `c_buffer_create` → `editor.create_buffer()`

- [ ] `src/kernel/scripting/buffer_api.rs`
  - `buf.cursor`, `buf.folds`, `buf.highlights`, `buf.decoration_layers` → view

- [ ] `src/kernel/scripting/buffer_text_api.rs`
  - Content ops on locked buffer, cursor from view

- [ ] `src/kernel/scripting/decoration_api.rs`
  - `buf.decor_*` methods → `view.decor_*()`

- [ ] `src/kernel/scripting/lsp_api.rs`
  - Cursor reads for LSP position → view

- [ ] `src/kernel/scripting/editor_state_api.rs`
  - Any `buf.cursor()` → view

- [ ] `src/kernel/scripting/editor_view_api.rs`
  - Window/view state reads → view

- [ ] `src/kernel/scripting/window_api.rs`
  - Window buffer switching → ensure view exists

- [ ] `src/kernel/scripting/display_api.rs`
  - Modeline cursor info → view

- [ ] `src/kernel/scripting/modality_api.rs`
  - Mode-dependent cursor → view

- [ ] `src/kernel/scripting/search_api.rs`
  - `view.set_highlights_layer()` instead of `buf.set_highlights_layer()`

- [ ] `src/kernel/scripting/selection_api.rs`
  - Selection anchor + view cursor

- [ ] `src/kernel/scripting/overlay_api.rs`
  - Overlay buffer content → lock Arc

- [ ] `src/kernel/scripting/special_buffer_api.rs`
  - Special buffer creation → `editor.create_buffer()`

---

### Test Infrastructure

- [ ] `src/tests/helpers.rs` — already updated by agent (verify)
- [ ] All `src/tests/*.rs` that directly call `buf.cursor()`, `buf.folds`, etc.
  → use `ed.views.get(&key).unwrap().cursor.offset` and similar

---

## Completion Gates (from MIGRATION.md)

1. `cargo test --features janet` — zero failures
2. No file > 400 lines (Rust)
3. No backward compat shims (none introduced)
4. Observable behaviour preserved

## Key Patterns Reference

```rust
// Read buffer content:
let buf = editor.buffers.get(key).unwrap().lock().unwrap();
let text = buf.slice(0, buf.len());

// Read view state:
let view = editor.views.get(&key).unwrap();
let cursor = view.cursor.offset;

// Mutate content + update cursor:
let view = editor.views.get_mut(&key).unwrap();
view.insert(offset, "text");   // delegates to buffer, updates cursor

// Create buffer + view together:
let key = editor.create_buffer("name");
let key = editor.create_buffer_from_str("name", content);

// Remove buffer + view:
editor.buffers.remove(key);
editor.views.remove(&key);
```
