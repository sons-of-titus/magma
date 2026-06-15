# Magma — Future Sprints

> **Architecture contract (non-negotiable across all sprints)**
>
> The Rust core owns: state, primitives, event bus, command registry, keymap
> manager, rendering surface, Janet FFI bridge.  It exposes everything through
> stable APIs — commands, events, and Janet C functions.
>
> Behavior, configuration, major modes, language support, UI polish, and every
> user-facing feature live in Janet scripts.  If a feature requires hardcoding
> knowledge of a specific language, file type, or workflow into Rust, it belongs
> in an extension instead.

---

## Sprint Completion Contract (mandatory for every sprint)

A sprint is **not complete** until all three gates pass.  Skipping any gate
means the sprint stays open regardless of how much code was written.

### 1. Tests — required for every new primitive

Every Rust C function, every Rust command, and every Janet macro added in a
sprint must have at least one test that exercises its observable behavior.
Tests live under `src/tests/` as a pair of files named after the **feature
area** (not the sprint number):

- **Pure Rust behavior** (no Janet VM): `src/tests/<feature>_tests.rs` using
  `make_editor` + `run` helpers.  No `JANET_VM_LOCK` needed.
- **Janet API behavior** (C functions, macros, event handlers):
  `src/tests/<feature>_janet_tests.rs` under `#[cfg(feature = "janet")]`.
  Every test must acquire `janet_bridge::JANET_VM_LOCK` with
  `lock().unwrap_or_else(|e| e.into_inner())` to serialize Janet VM access.

Declare both files in `src/tests/mod.rs`.  File names must reflect what is
being tested, not which sprint it was written in — sprint numbers rot as the
codebase grows.

Rules:
- `cargo test --features janet` must pass with zero failures before the sprint
  closes.
- Tests must assert observable Rust-side state (field values, event counts,
  command registry membership), not just that Janet eval returned "ok".
- Event-dispatch tests that require `call_janet_fn_with_arg → janet_continue`
  chains must register the target command as a **Rust command**
  (`ed.commands.register_fn`) rather than a Janet-defined command, because
  Janet does not support nested `janet_continue` calls initiated outside
  `janet_dostring`.

### 2. Documentation — update `docs/api/janet-api.md`

Every Janet C function added to the bridge must be documented in
`docs/api/janet-api.md` before the sprint closes.  The entry format is:

```
### `(module/fn-name arg1 arg2)` → return-type

One-line description.  Arguments and return type on separate lines if needed.
```

No implementation details in the docs.  Document the contract, not the code.

### 3. File size — one abstraction per file

No source file may grow beyond ~400 lines without being split.  The rule is
one coherent abstraction per file:

- A new group of related C functions → new `src/janet_bridge/<topic>_api.rs`
  registered in `src/janet_bridge/mod.rs`.
- A new subsystem (project, process, LSP) → new `src/<subsystem>/mod.rs`.
- Janet scripts longer than ~200 lines → split into
  `builtins/<topic>.janet` and `require` it from `init.janet`.

Reviewers must reject PRs that add significant logic to an already-large file
when a natural split point exists.

### 4. No backwards compatibility — ever

Old names, shims, aliases, feature flags, and "deprecated but kept for one
sprint" wrappers are forbidden.  When something is renamed or removed, every
call site is updated in the same PR.  If a Janet C function is renamed, the
old registration is deleted immediately — not kept alongside the new one.  If a
field is renamed on `Editor`, every reference in Rust and Janet is updated in
the same commit.

The rule from AGENTS.md applies verbatim: **"No feature flags or
backwards-compat shims when you can just change the code."**  This is not a
suggestion.  A PR that introduces a shim to ease migration is not mergeable,
regardless of how convenient the shim would be.

---

## ✅ Sprint 14 — Split `command/builtin.rs` — COMPLETE

> **Was:** `src/command/builtin.rs` at 3802 lines — the largest file in the project.

Split into 14 focused files under `src/command/builtin/`:

| File | Lines | Content |
|------|-------|---------|
| `builtin.rs` (coordinator) | 37 | Thin coordinator, `mod` declarations + `register_builtin_commands` |
| `helpers.rs` | 58 | Shared utilities: `get_current_buffer_id`, `emit_cursor_moved`, cursor/line helpers |
| `buffer.rs` | 222 | `save-buffer`, `open-file`, `find-files`, `close-buffer`, `alternate-buffer` |
| `navigation.rs` | 291 | Basic cursor movement, goto-*, scroll-to-*, change list navigation |
| `motion.rs` | 354 | Word motions, find-f/t, paragraph/sentence, marks, buffer cycling |
| `editing.rs` | 366 | insert/delete/undo/redo, case ops, number increment, eval-region/buffer |
| `visual.rs` | 346 | Visual modes, block-visual insert/append, multi-cursor, selection ops |
| `search.rs` | 300 | Search entry/exit, execute, repeat/reverse, keyword-context |
| `registers.rs` | 282 | Yank/put variants, macros (record/play), marks (set/jump) |
| `window.rs` | 186 | Window splits, tabnext/tabprev, terminal commands, buffer-next/prev |
| `file.rs` | 341 | Dired browser + all dired helpers, open-file-at-cursor |
| `vcs.rs` | 360 | VC commands + helpers, quickfix, shell-command, make |
| `completion.rs` | 228 | Completion popup + snippet expand/tabstop navigation |
| `misc.rs` | 338 | Mode entry/exit (insert/replace/command), major-mode, quit variants |
| `colon.rs` | 167 | Fallback `:command` Rust dispatcher (overridden by Janet in practice) |

**All gates passed:**
- `src/command/builtin.rs` = 37 lines ✓
- Every split file ≤ 400 lines ✓
- Zero new behaviour — identical command set ✓
- `cargo test --features janet` → 820 passed, 0 failed ✓

---

## ✅ Sprint 15 — Split `builtins/init.janet` and oversized Janet files — COMPLETE

> **Was:** `init.janet` at 762 lines; `ui.janet` at 325 lines; `plugins/tags.janet` at 223 lines.

**`init.janet` split** — extracted 7 files, load order registered in `src/janet_bridge/mod.rs`:

| File | Lines | Content |
|------|-------|---------|
| `builtins/init.janet` | 106 | `trim-start`, Catppuccin face definitions, `(print "Magma ready")` |
| `builtins/scroll_commands.janet` | 65 | `scroll-line-down/up`, `scroll-half-page-down/up`, `scroll-page-down/up` |
| `builtins/indent_commands.janet` | 74 | `indent-line`, `deindent-line`, `autoindent-line`, `delete-to-bol/eol` |
| `builtins/colon_mode.janet` | 190 | `*colon-plugins*`, all colon dispatch logic, `command-execute`, `command-backspace` |
| `builtins/completion_mode.janet` | 68 | `command-complete`, `command-complete-prev`, colon verb cycling |
| `builtins/multi_cursor.janet` | 69 | Multi-cursor helpers, `multi-cursor-next-occurrence`, completion command wrappers |
| `builtins/major_modes.janet` | 174 | `major-mode/define` macro, `major-mode/activate`, auto-detect hook, extension map |
| `builtins/plugin_loader.janet` | 24 | `*loaded-modules*`, `require` |

**`ui.janet` split** — extracted 3 files:

| File | Lines | Content |
|------|-------|---------|
| `builtins/ui.janet` | 101 | Palette, face definitions, event wiring, `:colorscheme`, editor-ready defaults |
| `builtins/modeline.janet` | 103 | Mode pill helpers, git branch detection, surface helpers, `draw-modeline` |
| `builtins/tab_bar.janet` | 41 | `draw-tab-bar`, buffer label helpers |
| `builtins/scroll_policy.janet` | 42 | `*scroll-policy*`, `apply-scroll-policy`, `:scrollpolicy` colon verb |

**`plugins/tags.janet` split:**

| File | Lines | Content |
|------|-------|---------|
| `builtins/plugins/tags_gen.janet` | 98 | Tag file discovery, ctags parsing, cache, `tags/generate` |
| `builtins/plugins/tags_nav.janet` | 113 | Tag stack, `tags/jump`, `tags/pop`, word-at-cursor, commands, keymaps |

**All gates passed:**
- `builtins/init.janet` = 106 lines ✓
- All extracted files ≤ 200 lines ✓
- `ui.janet` = 101 lines ✓
- All colon verbs, major modes, and completion unchanged ✓
- `cargo test --features janet` → 820 passed, 0 failed ✓

---

## ✅ Sprint 16 — Extract sub-structs from the `Editor` god-object — COMPLETE

> **Was:** `src/state/mod.rs` at 518 lines with ~90 direct fields on `Editor`.

`Editor` now holds 7 typed sub-structs by value (no `Box`, identical layout).
All call sites renamed in the same change (e.g. `ed.completion_visible` → `ed.completion.visible`).

| Sub-struct | `Editor` field | New file | Lines |
|------------|---------------|----------|-------|
| `SnippetState` | `editor.snippet` | `src/state/snippet.rs` | 7 |
| `MultiCursorState` + `ExtraCursor` | `editor.multi_cursor` | `src/state/cursor.rs` | 13 |
| `BlockVisualState` | `editor.block_visual` | `src/state/visual.rs` | 9 |
| `CompletionState` | `editor.completion` | `src/state/completion.rs` | 8 |
| `ProjectManager` (+ `ProjectState`, `ProjectMember`, `Workspace`) | `editor.project_manager` | `src/state/project.rs` | 36 |
| `IoState` | `editor.io` | `src/state/io.rs` | 28 |
| `GutterState` (+ `GutterSign`, `GutterColumn`, `FoldIcons`) | `editor.gutter` | `src/state/gutter.rs` | 57 |
| `FontConfig` + `ContextFont` | (field unchanged) | `src/state/font.rs` | 34 |
| `Overlay` + `LayoutSnapshot` | (fields unchanged) | `src/state/overlay.rs` | 30 |

**`src/state/mod.rs`** shrank from 518 → **256 lines**: module declarations,
re-exports, `Editor` struct definition, `Editor::new`, and its methods.

**All gates passed:**
- `src/state/mod.rs` = 256 lines ✓
- Each extracted file ≤ 57 lines ✓
- No observable behaviour change ✓
- `cargo test --features janet` → 820 passed, 0 failed ✓

---

## ✅ Sprint 17 — Split oversized Rust API files — COMPLETE

> **Was:** Seven Rust files exceeding the 400-line limit.

Split into 13 focused files:

| Original file | Was | Now | New files |
|---|---|---|---|
| `src/janet_bridge/editor_api.rs` | 727 | deleted | `editor_state_api.rs` (296), `editor_io_api.rs` (161), `editor_view_api.rs` (237) |
| `src/janet_bridge/buffer_api.rs` | 618 | 210 | `buffer_text_api.rs` (168), `buffer_query_api.rs` (230) |
| `src/janet_bridge/project_api.rs` | 469 | 218 | `project_registry_api.rs` (214) |
| `src/janet_bridge/mod.rs` | 575 | 323 | `loader.rs` (153) |
| `src/buffer/mod.rs` | 598 | 344 | `decoration.rs` (49), `fold.rs` (22), `tests.rs` (tests) |
| `src/render/gui.rs` | 489 | 282 | `gui_fonts.rs` (46), `gui_render.rs` (132) |
| `src/render/frame.rs` | 527 | 382 | `highlight_pass.rs` (106), additions to `status_and_popup.rs` |

**All gates passed:**
- All 7 original files brought below 400 lines ✓
- No change in observable behaviour ✓
- No backwards-compatibility shims ✓
- `cargo test --features janet` → 820 passed, 0 failed ✓

---

## ✅ Sprint 18 — Complete Janet API documentation — COMPLETE

> **Was:** `docs/api/janet-api.md` documented ~147 functions in two incompatible
> formats, with ~81 functions missing entirely and a duplicate `buffer/modified?`
> entry.

`docs/api/janet-api.md` rewritten from scratch:

- **254 functions** documented (covers all registered `extern "C-unwind"` functions)
- Single consistent `### (namespace/fn-name arg …)` → return-type heading format
- Zero duplicate entries
- No "Planned" or "TODO" sections
- 13 previously undocumented functions added: `editor/shell`, `editor/run-command`, `editor/load-file`, `editor/set-theme`, `editor/theme`, `editor/option-get-local`, `editor/option-set-local`, `editor/command-input`, `editor/set-completions`, `editor/clear-completions`, `buffer/major-mode`, `buffer/diagnostics`, `vc/register-backend`
- Legacy `editor/gutter-set`, `editor/gutter-clear`, `editor/gutter-clear-line` marked deprecated with pointer to `gutter/sign-set`

**All gates passed:**
- Every `extern "C-unwind"` function has an entry in `janet-api.md` ✓
- Zero duplicate entries ✓
- Single consistent heading format throughout ✓
- No "planned" or "TODO" entries ✓

---

## ✅ Sprint 19 — Refactor the `editor/` catch-all namespace — COMPLETE

> **Was:** The `editor/` namespace contained **63 documented functions** spanning 20+
> conceptual areas (mode, minibuffer, selection, registers, options, plugin
> state, clipboard, search, cursor, faces, fonts, layout, surface, overlays,
> gutter, modeline, tab-bar, mark-ring, module paths, input, and commands).
> The namespace was a catch-all that made API discovery impossible.
>
> The old `editor/gutter-set` and the new `gutter/sign-set` coexisted as two
> different APIs for the same operation, and both were registered.

### Principle

Functions stay in `editor/` only if they truly operate on the editor as a
whole (running, focused buffer, mode transitions, eval).  Everything else moves
to a dedicated namespace that matches the Janet community's expectation of
module-level cohesion.

### Namespace migration plan

| New namespace | Functions moved from `editor/` | New file |
|---------------|-------------------------------|----------|
| `minibuffer/` | `minibuffer-open`, `minibuffer-close`, `minibuffer-input`, `minibuffer-set-input`, `minibuffer-prompt` | `src/janet_bridge/minibuffer_api.rs` |
| `selection/` | `selection`, `selection-set`, `selection-clear` | `src/janet_bridge/selection_api.rs` |
| `register/` | `register-get`, `register-set`, `yanked-text` | `src/janet_bridge/register_api.rs` |
| `option/` | `options`, `option-get`, `option-set`, `option-get-local`, `option-set-local` | `src/janet_bridge/option_api.rs` |
| `plugin-state/` | `plugin-state-get`, `plugin-state-set`, `plugin-state-del` | `src/janet_bridge/plugin_state_api.rs` |
| `clipboard/` | `clipboard-get`, `clipboard-set` | extend `src/janet_bridge/editor_state_api.rs` |
| `search/` | `search-pattern`, `search-pattern-set`, `search-forward?` | `src/janet_bridge/search_api.rs` |
| `face/` | `define-face`, `face`, `make-style`, `scope-face`, `resolve-scope` | already in `face_api.rs` — rename prefix only |
| `font/` | all `set-font`, `font-size`, `load-font`, `set-font-*`, `set-glyph-width`, `set-nerd-font`, `font-invalidate` | already in `font_api.rs` — rename prefix only |
| `surface/` | `surface-set-cell`, `surface-set-text`, `surface-size` | already in `ui_api.rs` — rename prefix only |
| `overlay/` | `overlay-create`, `overlay-destroy`, `overlay-move`, `overlay-list` | already in `overlay_api.rs` — rename prefix only |
| `mark-ring/` | `mark-ring-push`, `mark-ring-pop`, `mark-ring-peek`, `mark-ring-len` | extend `src/janet_bridge/ecosystem_api.rs` |
| `module/` | `module-path`, `module-path-add` | extend `src/janet_bridge/ecosystem_api.rs` |

### Legacy gutter API

Remove the `editor/gutter-set`, `editor/gutter-clear`, `editor/gutter-clear-line`
C functions.  All gutter callers in Janet files already use `gutter/sign-set` —
confirm with a grep before removing.

**All gates passed:**
- `editor/` namespace = 20 functions (truly editor-global operations only) ✓
- 13 new namespaces created: `minibuffer/`, `selection/`, `register/`, `option/`, `plugin-state/`, `clipboard/`, `search/`, `face/`, `font/`, `surface/`, `overlay/`, `mark-ring/`, `module/`, `ui/` ✓
- All Janet builtins updated to use new namespaces ✓
- Old `editor/foo` C registrations deleted — no aliases, no shims ✓
- Legacy `editor/gutter-set`, `editor/gutter-clear`, `editor/gutter-clear-line` removed ✓
- `gutter/set-fold-icons` replaces `editor/gutter-set-fold-icons` ✓
- `cargo test --features janet` → 846 passed, 0 failed ✓
- `janet-api.md` fully updated — 13 new namespace sections, `editor/` section trimmed ✓
- 26 new tests added (7 pure-Rust + 19 Janet) ✓

---

## ✅ Sprint 20 — Remove legacy debt and dead code — COMPLETE

> **Was:**
> Several areas of the codebase carried dead weight that misled contributors
> and suppressed real compiler warnings:
>
> - `window/mod.rs`: the `LayoutNode` tree (`#[allow(dead_code)]`) was never
>   used for layout computation — `redistribute()` iterates a flat `Vec<Window>`
>   and ignores the tree.  Every `split()` call builds a `Split` node that is
>   never read back.
> - `render/mod.rs`: `TerminalEvent` is type-aliased to `InputEvent` with a
>   comment saying "will be removed" — but all backends still emit `TerminalEvent`.
>   The migration started in Sprint 11d has not been completed.
> - `janet_bridge/mod.rs` has `#[allow(dead_code, unused_imports)]` suppressing
>   warnings across the entire bridge module.
> - Sprint 19's backwards-compatibility shims (`editor/foo` aliases) are due
>   for removal.

### LayoutNode tree

Chose **Option A**: removed `LayoutNode` enum, `root` field, and `split()`.
`WindowTree` is now a flat `Vec<Window>` with `add_window()` for proportional layout.

### `TerminalEvent` → `InputEvent` completion

Removed `pub enum TerminalEvent` from `render/mod.rs`. `RenderTrait::poll_event`
now returns `Option<InputEvent>` directly. Updated `tui.rs`, `main.rs`.

### `#[allow(dead_code, unused_imports)]` removal

Removed the crate-level attribute from `janet_bridge/mod.rs`.
- `Once` import: deleted (genuinely unused)
- `JANET_VM_LOCK`: annotated at item level with a comment explaining it's used by tests

**All gates passed:**
- `LayoutNode` enum and `root: LayoutNode` field removed from `window/mod.rs` ✓
- `split()` replaced by `add_window()` with `weight = 1.0 / new_count` ✓
- `TerminalEvent` type alias gone; `RenderTrait::poll_event` returns `Option<InputEvent>` ✓
- `pub use crate::input::event::InputEvent` in `render/mod.rs` ✓
- `#[allow(dead_code, unused_imports)]` removed from `janet_bridge/mod.rs` ✓
- `JANET_VM_LOCK` annotated at item level: `#[allow(dead_code)]` with explanation ✓
- No backwards-compatibility aliases anywhere ✓
- `cargo test --features janet` → 846 passed, 0 failed ✓

---

## Sprint 21 — Typed event system

> **Current state:**
> Every event payload is `HashMap<String, String>` — fully type-erased.  A typo
> in a key name (e.g., `"buffer-is"` instead of `"buffer-id"`) silently produces
> `None` on the receiver side with no compiler or runtime warning.  There is no
> canonical source of truth for what keys a given event carries; developers must
> grep the codebase to discover payloads.

### Phase A — Event contract table (no code change)

Add `docs/events.md`: a single table listing every named event, every key it
carries, the type of each value (all strings today, but annotated with semantic
type), and which Rust call site emits it.  This is the ground truth for both
Rust and Janet subscribers.

### Phase B — Typed payload structs

Replace `HashMap<String, String>` emit sites with typed structs that serialize
to the hash map at the boundary, so the compiler catches field-name typos:

```rust
pub struct BufferFocusedEvent { pub buffer_id: usize, pub path: Option<String> }

impl From<BufferFocusedEvent> for EventData {
    fn from(e: BufferFocusedEvent) -> Self {
        let mut m = HashMap::new();
        m.insert("buffer-id".into(), e.buffer_id.to_string());
        if let Some(p) = e.path { m.insert("path".into(), p); }
        m
    }
}
```

`events.emit("buffer-focused", BufferFocusedEvent { buffer_id, path }.into())`

The `EventData` type alias and the Janet-side `event/on` API remain unchanged —
Janet still receives a string-keyed table.  Only the Rust emit sites gain type
safety.

### Phase C — Key constant module

Introduce `src/event/keys.rs` with `pub const` string literals for every event
name and every payload key:

```rust
pub mod events {
    pub const BUFFER_FOCUSED: &str = "buffer-focused";
    pub mod buffer_focused {
        pub const BUFFER_ID: &str = "buffer-id";
        pub const PATH: &str = "path";
    }
}
```

All Rust emit sites and Janet bridge C functions use the constants.  A typo
becomes a compile error instead of a silent `None`.

**Completion criteria:**
- `docs/events.md` lists all 40+ events with full payload schemas
- All Rust emit sites use typed payload structs
- All key strings centralised in `src/event/keys.rs` constants
- Janet API unchanged — existing `event/on` handlers work without modification
- No backwards-compatibility: the old bare `HashMap::new()` emit pattern is removed everywhere, not kept alongside the new structs
- `cargo test --features janet` → 0 failures

---

## Sprint 22 — Unified command registry

> **Current state:**
> Two command tables coexist without a unified lookup path:
> 1. `CommandRegistry` (Rust) — populated by `cmds.register_fn(...)`.
> 2. `*janet-commands*` (Janet) — populated by `(command/define name fn)`.
>
> `execute_command` only searches the Rust registry.  `editor/run-command`
> calls `execute_command` and falls back to `*janet-commands*` — but only via
> the Janet bridge, not via Rust itself.  If a Rust command and a Janet command
> share a name, behaviour is undefined.  Janet-defined commands cannot be
> introspected from Rust (no `get_entry` equivalent).

### Unified lookup

Extend `execute_command` to fall through to the Janet command table when no
Rust command matches:

```rust
pub fn execute_command(ed: &mut Editor, name: &str, args: &ArgMap)
    -> Result<(), String>
{
    // 1. Check Rust registry first.
    if let Some(entry) = ed.commands.get_entry(name) {
        let handler = entry.handler.clone();
        return handler(ed, args);
    }
    // 2. Fall through to Janet command table.
    #[cfg(feature = "janet")]
    return janet_bridge::call_janet_command(ed, name, args);
    #[cfg(not(feature = "janet"))]
    Err(format!("Unknown command: {name}"))
}
```

`janet_bridge::call_janet_command` looks up `*janet-commands*[name]` and calls
it via `janet_dostring`.  Arguments from `args` are serialised into a Janet
table passed as the first argument.

### Janet `command/define` registers into Rust

When `(command/define name fn)` is called, in addition to writing to
`*janet-commands*`, also register a thin Rust wrapper in `CommandRegistry` so
that `ed.commands.get_entry(name)` returns something:

```rust
ed.commands.register_fn(name, "", vec![], move |ed, args| {
    janet_bridge::call_janet_command(ed, name, args)
});
```

This lets `M-x` completion, `keymap/set` resolution, and any Rust code that
introspects the command registry see Janet-defined commands.

### Name conflict policy

If `command/define` is called with a name already in the Rust registry, emit
`editor/warn` and refuse to overwrite.  If intentional replacement is needed,
a new `(command/redefine name fn)` form bypasses the guard.

**Completion criteria:**
- `execute_command` falls through to Janet table when Rust registry has no match
- `ed.commands.get_entry("scroll-line-down")` returns an entry (Janet-defined)
- Name conflict at `command/define` time emits a warning
- All existing Rust and Janet commands still work
- No backwards-compatibility dual-path kept alongside the new unified path; old workarounds in call sites removed
- `cargo test --features janet` → 0 failures

---

## Sprint 23 — Janet runtime trait abstraction

> **Current state:**
> The entire codebase compiles against a concrete `janet_bridge` module.  The
> `#[cfg(feature = "janet")]` gates produce stub functions when the feature is
> disabled, but:
> - The stubs have different signatures in some cases
> - Callers cannot depend on a stable API without the feature flag
> - Swapping Janet for another scripting runtime (or MagmaLisp, see Sprint 14's
>   successor) requires touching every call site
>
> There is no trait capturing what the Rust core needs from its scripting host.

### `ScriptRuntime` trait

```rust
pub trait ScriptRuntime: Send + Sync {
    fn init(&mut self, editor: *mut Editor);
    fn eval(&mut self, expr: &str) -> String;
    fn eval_result(&mut self, expr: &str) -> Result<String, String>;
    fn load_file(&mut self, path: &str) -> Result<(), String>;
    fn call_command(&mut self, ed: &mut Editor, name: &str, args: &ArgMap)
        -> Result<(), String>;
}
```

`Editor` holds `runtime: Option<Box<dyn ScriptRuntime>>` instead of calling
`janet_bridge::*` functions directly.

### `JanetRuntime` impl

`src/janet_bridge/runtime.rs` provides `pub struct JanetRuntime;` implementing
`ScriptRuntime`.  The existing `mod.rs` functions become methods.

### Stub runtime

`src/scripting/null_runtime.rs` provides `NullRuntime` that returns errors
for all calls.  Used when `features = []` (no Janet, no MagmaLisp).

### Call-site migration

Replace all `janet_bridge::eval(ed, ...)` calls with `ed.runtime.eval(...)`.
Replace `janet_bridge::init(ed)` with `ed.runtime = Some(Box::new(JanetRuntime::new()))`.

**Completion criteria:**
- `ScriptRuntime` trait defined in `src/scripting/mod.rs`
- `JanetRuntime` and `NullRuntime` both implement it
- No direct `janet_bridge::` calls outside `src/janet_bridge/`
- `cargo build` (without `--features janet`) compiles and produces a functional
  headless editor binary
- No backwards-compatibility: the old direct `janet_bridge::eval(ed, ...)` call pattern is removed entirely, not kept alongside `ed.runtime.eval(...)`
- `cargo test --features janet` → 0 failures

---

## Sprint 24 — Janet utility namespace hygiene and API versioning

> **Current state:**
> The builtins define editor-specific utilities under generic-sounding
> namespaces that risk conflicting with user extensions:
> - `string/trim`, `string/trim-trailing`, `string/ensure-trailing-newline`,
>   `string/indent` — shadow or extend Janet's built-in string module
> - `table/deep-merge` — shadows Janet's table module
> - `time/now`, `debug/log` — occupy names a user plugin might want
>
> There is also no API versioning strategy despite active drift
> (`editor/gutter-set` → `gutter/sign-set`).

### Namespace migration

Rename the conflicting utilities:

| Old name | New name |
|----------|----------|
| `string/trim` (magma variant) | `magma/trim` |
| `string/trim-trailing` | `magma/trim-trailing` |
| `string/ensure-trailing-newline` | `magma/ensure-trailing-newline` |
| `string/indent` | `magma/indent` |
| `table/deep-merge` | `magma/deep-merge` |
| `time/now` | `magma/time-now` |
| `debug/log` | `magma/debug-log` |

All internal usages updated.  Janet's standard `string/` and `table/` modules
are unaffected.

### API versioning

Add a `(def magma-api-version [0 24 0])` constant to `builtins/init.janet`.
Document the versioning policy in `docs/api/versioning.md`:
- Patch: backward-compatible additions
- Minor: additions + deprecations (old names log warnings for one minor version)
- Major: breaking removals

Add `(magma/api-compat? major minor)` Janet function that returns `true` if the
running API is at least that version — lets plugins declare their minimum
requirement.

**Completion criteria:**
- All 7 utility functions renamed under `magma/`; old names under `string/`, `table/`, `time/`, `debug/` deleted
- `magma-api-version` constant defined and documented
- `magma/api-compat?` function implemented and tested
- `docs/api/versioning.md` written
- No backwards-compatibility: old generic-namespace names are removed, not kept as aliases
- `cargo test --features janet` → 0 failures

---

## Sprint 25 — Test infrastructure cleanup

> **Current state:**
> `src/tests/mod.rs` is **129 lines** of boilerplate: every feature pair
> (Rust tests + Janet tests) requires 4 lines in `mod.rs`.  25+ pairs makes it
> a maintenance burden — developers forget to add entries, or add them in the
> wrong order.
>
> Several test helpers (`make_editor`, `run`, the JANET_VM_LOCK pattern) are
> copy-pasted into every Janet test file instead of being shared.

### `test_pairs!` macro

```rust
// In src/tests/mod.rs:
macro_rules! test_pairs {
    ($base:ident) => {
        mod $base;
        #[cfg(feature = "janet")]
        mod paste::paste! { [<$base _janet>] }
    };
    ($($base:ident),+) => { $(test_pairs!($base);)+ };
}

test_pairs!(
    command, event, mode, insertable,
    extension_foundation, special_buffers, process_async,
    // …
);
```

`src/tests/mod.rs` shrinks from 129 lines to ~30 lines.

### Shared test helpers

Create `src/tests/helpers.rs`:

```rust
pub fn make_editor() -> Editor { … }
pub fn make_editor_with_buffer(content: &str) -> (Editor, usize) { … }
pub fn acquire_janet_lock() -> std::sync::MutexGuard<'static, ()> {
    janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
```

Remove the copy-pasted `make_editor` and `acquire_janet_lock` from every
individual test file.

### Consistent Janet test boilerplate

Every Janet test file currently opens with:
```rust
let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
let mut ed = make_editor();
janet_bridge::init(&mut ed);
```

Replace with a single macro:
```rust
janet_test!(ed, {
    // test body — `ed` is the initialised Editor
});
```

**Completion criteria:**
- `src/tests/mod.rs` ≤ 40 lines
- `src/tests/helpers.rs` contains shared `make_editor` and lock helper
- `janet_test!` macro used in at least 5 test files as proof of concept
- No backwards-compatibility: copy-pasted `make_editor` definitions removed from individual test files, not kept alongside the shared helper
- `cargo test --features janet` → 0 failures; identical test count to before

---

## Ongoing — API Completeness and Quality

These are not sprint-gated; they should be addressed continuously.

| Item | Owner | Notes |
|------|-------|-------|
| Fix Janet FFI alignment panics on macOS (`evil-janet`) | Rust | Upstream issue; workaround or pin version |
| Property-based tests for rope | Rust | Fuzz insert/delete sequences against a naive `String` oracle |
| Keep `docs/api/janet-api.md` in sync | Docs | Every new C function must be documented before merge |
| `editor/gutter-set` deprecation path | Janet | After Sprint 19, remove in Sprint 20 |
| Cyclic coupling between `state` and `command` | Architecture | Incremental: as sub-structs are extracted, opportunities for decoupling emerge |
