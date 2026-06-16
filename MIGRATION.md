# Magma — Migration Plan

## From Post-Roadmap State to Target Architecture

This document defines the migration from the state of the editor **after all 25
ROADMAP sprints are complete** to the architecture described in the
*Architecture Specification Document (v1.0)*.

The roadmap sprints address internal hygiene — splitting files, type-safety,
namespace cleanup, test infrastructure.  The architecture specification
introduces entirely new subsystems and a fundamentally different structural
philosophy.

## No Backward Compatibility — Ever

This migration explicitly rejects all backward compatibility.

Every PR in every phase must:
- **Delete old names, paths, and types entirely** — not keep them alongside
  replacements, not deprecate them for one cycle, not alias them.
- **Update every call site in the same commit** — Rust, Janet, and
  documentation simultaneously.
- **Remove shims immediately** — a compat shim merged in Phase 1 must be
  deleted in Phase 1, not carried forward until Phase 2.
- **Refuse feature flags and feature-gated old paths** — if a subsystem is
  restructured, the old structure is deleted.  There is no `--legacy-layout`
  flag, no `#[cfg(feature = "legacy_buffer")]`, no `type OldName = NewName`.

The rule from AGENTS.md applies verbatim: *"No feature flags or
backwards-compat shims when you can just change the code."*  A PR that
introduces a shim, alias, or deprecation warning "to ease migration" is not
mergeable — regardless of how convenient it would be.

The `magma/api-compat?` function (Sprint 24) is the **only** form of
compatibility the system recognises: it lets *plugins* check the API version
they're running on.  It is not a mechanism for the core to carry dead code.

---

## Baseline: Post-Roadmap State

After Sprints 14-25 the editor has:

| Area | State |
|------|-------|
| `command/builtin.rs` | Split into 14 focused files under `command/builtin/` |
| `builtins/init.janet` | Split into ~38 files, all < 200 lines |
| `state/mod.rs` | Editor struct with 7 sub-structs, 256 lines |
| Oversized Rust API files | All < 400 lines; `editor_api.rs`, `buffer_api.rs`, `mod.rs`, `frame.rs`, `gui.rs`, `project_api.rs` split |
| `docs/api/janet-api.md` | 228+ functions documented, deduplicated, consistent format |
| `editor/` namespace | ≤ 20 functions; `minibuffer/`, `selection/`, `register/`, `option/`, `plugin-state/`, `clipboard/`, `search/`, `mark-ring/` extracted |
| Legacy debt | `LayoutNode` removed or used, `TerminalEvent` → `InputEvent`, no `#[allow(dead_code)]` at module level, no backwards-compat shims |
| Event system | `keys.rs` constants, typed payload structs, `docs/events.md` |
| Command registry | Unified `execute_command` fallthrough, Janet commands visible in Rust registry |
| Script runtime | `ScriptRuntime` trait in `src/scripting/`, `JanetRuntime` + `NullRuntime` impls |
| Janet namespace hygiene | `magma/` namespace, `magma-api-version`, `docs/api/versioning.md` |
| Test infrastructure | `test_pairs!` macro, `helpers.rs`, `janet_test!` macro, `mod.rs` ≤ 40 lines |
| LSP | Existing `src/lsp/` — ad-hoc, not abstracted as Language Providers |

**What does NOT exist after the roadmap:**
- Formal kernel subsystem (`kernel/`)
- Semantic Engine (parsers, symbols, project graph)
- Task System (everything-executable-as-task)
- Debugger subsystem
- Project Model as first-class programmable object
- Gutter Provider abstraction
- View Tree (model → view tree → renderer)
- Workspace persistence model
- AI / Agent architecture
- Concurrency model (async workers)
- Security / permissions model
- Plugin capability system (beyond Janet scripts)
- Language Provider abstraction (`languages/`)

---

## Target Architecture Summary

From the Architecture Specification Document:

```
                    ┌──────────┐
                    │  Janet   │  — behavior layer
                    │ Runtime  │
                    └────┬─────┘
                         │
                    ┌────▼─────┐
                    │Extension │
                    │   API    │
                    └────┬─────┘
                         │
                    ┌────▼─────┐
                    │  Event   │
                    │  System  │
                    └────┬─────┘
    ┌────────────────────┼────────────────────┐
    │              Rust Kernel                │
    │  ┌──────────┐ ┌───────────┐             │
    │  │  Text    │ │ Semantic  │             │
    │  │  Engine  │ │  Engine   │             │
    │  ├──────────┤ ├───────────┤             │
    │  │ Project  │ │  Runtime  │             │
    │  │  Model   │ │   Model   │             │
    │  ├──────────┤ ├───────────┤             │
    │  │  Debug   │ │   Task    │             │
    │  │  System  │ │  System   │             │
    │  ├──────────┤ ├───────────┤             │
    │  │ Storage  │ │ Renderer  │             │
    │  └──────────┘ └───────────┘             │
    └─────────────────────────────────────────┘
                         │
                    ┌────▼─────┐
                    │    UI    │
                    └──────────┘
```

Key differences from current architecture:

| Current (post-roadmap) | Target |
|------------------------|--------|
| Monolithic `src/buffer/`, `src/command/`, `src/state/` | `kernel/` directory with 6 sub-systems |
| Ad-hoc LSP in `src/lsp/` | `Semantic Engine` + `Language Provider` abstraction |
| No formal task abstraction | `Task System` — everything is a task |
| No debugger subsystem | `Debug System` — breakpoints, stepping, watches |
| Project as state sub-struct | Project as first-class programmable object |
| Gutter via direct API calls | Gutter Provider abstraction (`render()`, `width()`, `update()`) |
| Tightly coupled renderer | Model → View Tree → Renderer pipeline |
| Sync execution model | Async main + worker threads |
| No plugin capability model | Capability-based Plugin System |
| No AI architecture | Agent with Context Provider, Planner, Executor |
| No security model | Extension permission capabilities |
| Position as raw `(line, col)` | Position as `{line, column, offset}` typed object |

---

## Migration Phases

### Phase 0 — Kernel Extraction (restructure, no new behaviour)

**Goal:** Establish the `kernel/` directory as the canonical Rust core without
changing observable behaviour.

| Step | What | Risk |
|------|------|------|
| 0.1 | Create `kernel/` directory with `kernel/mod.rs` re-exporting existing subsystems | Low — pure file moves |
| 0.2 | Move `src/event/` → `kernel/event/` | Low |
| 0.3 | Move `src/state/` → `kernel/state/` (Editor struct + sub-structs stay here) | Low |
| 0.4 | Move `src/buffer/` → `kernel/text_engine/` — rename to TextEngine | Medium — all imports change |
| 0.5 | Move `src/lsp/` → `kernel/semantic/` as foundation of Semantic Engine | Medium — interface may shift |
| 0.6 | Move `src/process/` + `src/task/` → `kernel/task/` (unify as Task System) | Medium — merge two subsystems |
| 0.7 | Move `src/render/` → `kernel/render/` | Low |
| 0.8 | Move `src/fs/` → `kernel/storage/`, add workspace persistence | Medium — new code |
| 0.9 | Move `src/keymap/` → `kernel/keymap/` | Low |
| 0.10 | Move `src/janet_bridge/` → `kernel/scripting/` (the bridge stays, ScriptRuntime trait lives here) | Low |
| 0.11 | Move `src/vc/`, `src/dired.rs` → `kernel/vc/` | Low |

**Completion:** `src/` contains only `main.rs`, `lib.rs`, top-level plumbing.
All logic lives under `kernel/<subsystem>/`.

**Size cost:** ~60 file moves, ~300 import-line changes in `Cargo.toml` and
`mod.rs` files.  One sprint.

---

### Phase 1 — Text Engine Overhaul ✅ COMPLETE

**Goal:** Transform `kernel/text_engine/` from a buffer-with-LSP into the
Architecture Spec's Text Engine: buffer, rope storage, decorations, views.

| Step | What | Status |
|------|------|--------|
| 1.1 | Introduce **Position** struct (`line`, `column`, `offset`) as the canonical position type throughout the engine | ✅ |
| 1.2 | Replace all raw `(line, usize)` / `usize` cursor positions with `Position` | ✅ |
| 1.3 | Extract `BufferView` from `Buffer` — a view has cursor state, decorations, folds; the buffer owns only content + undo history | ✅ |
| 1.4 | Give `BufferView` its own decoration layer stack (independent of the buffer's layers) | ✅ |
| 1.5 | Make `Buffer` reference-counted (`Arc<Mutex<Buffer>>`) so multiple views can share the same backing store | ✅ |
| 1.6 | Verify: all cursor operations, undo, editing work identically through the view | ✅ |

**Design constraint:** A buffer has N views.  Each view has its own cursor,
selection, folds, scroll position, and decoration overlay.  The buffer has
none of these — it is pure content + history.

**Completion notes (2026-06-16):**
- `editor.buffers: Slab<Arc<Mutex<Buffer>>>` — content is lock-guarded
- `editor.views: HashMap<usize, BufferView>` — per-window view state
- `Editor::create_buffer()` / `create_buffer_from_str()` create both atomically
- All 43 production files and all test files updated to use the new API
- Double-lock deadlocks (same `Arc` locked twice in one expression) fixed in 9 sites:
  `helpers.rs`, `colon.rs` (×2), `misc.rs`, `editing.rs`, `completion.rs` (×2),
  `unified_input_janet_tests.rs` (×3), `text_engine/tests.rs`
- `render_frame` deadlock fixed: `compute_scroll_state_raw` introduced so the
  frame renderer can use already-locked buffer data without re-acquiring the lock
- `cargo test --features janet -- --test-threads=1`: zero failures

---

### Phase 2 — View Tree + Renderer Separation ✅ COMPLETE

**Goal:** Decouple rendering from the model via a View Tree, enabling
programmatic view composition (EditorView, SidebarView, TerminalView, etc.).

| Step | What | Status |
|------|------|--------|
| 2.1 | Define `View` trait in `kernel/render/view.rs`: `fn render(&self, surface: &mut Surface, ctx: &RenderCtx)` | ✅ |
| 2.2 | Implement `EditorView` — wraps a `BufferView`, renders text + cursor + decorations | ✅ |
| 2.3 | Implement `SidebarView` — wraps `ProjectManager`, renders file tree | ✅ |
| 2.4 | Implement `TerminalView` — wraps a PTY, renders terminal output | ✅ |
| 2.5 | Build `ViewTree` — ordered list of views with layout constraints (split, tab, float) | ✅ |
| 2.6 | Rewrite `kernel/render/frame.rs` to traverse `ViewTree` and dispatch to each view's `render()` | ✅ |
| 2.7 | Move window split logic from `src/window/` into `ViewTree` layout | ✅ |
| 2.8 | Verify: all existing rendering (modeline, tab-bar, gutter) works through view renderers | ✅ |

**Completion notes (2026-06-16):**
- `kernel/window/mod.rs` deleted entirely — `WindowTree` and `Window` replaced by `ViewTree` and `Pane`
- `editor.windows: WindowTree` → `editor.view_tree: ViewTree` — all 40+ call sites updated atomically
- `View` trait + `Rect` + `RenderCtx` in `kernel/render/view.rs`
- `EditorView` (`kernel/render/editor_view.rs`) — renders buffer text, gutter, cursor, decorations, folds, multi-cursor; only renders cursor and completion popup for the focused pane
- `TerminalView` (`kernel/render/terminal_view.rs`) — renders PTY output into pane area
- `SidebarView` (`kernel/render/sidebar_view.rs`) — renders project file tree from `ProjectManager`
- `ViewTree` (`kernel/render/view_tree.rs`) — replaces `WindowTree`; adds `LayoutConstraint` enum (Split/Tab/Float) and `SplitDirection` per pane; redistribute logic moved from `window/mod.rs`
- `render_frame` rewritten to iterate all panes, dispatch to views; global elements (overlays, status bar, completion popup) rendered after
- `saved_layouts` type updated: `Vec<Window>` → `Vec<Pane>` 
- `render_terminal_frame` from `status_and_popup.rs` deleted — logic lives in `TerminalView`
- 20 new tests in `tests/view_tree_tests.rs` + `tests/view_tree_janet_tests.rs`
- `cargo test --features janet -- --test-threads=1`: 870 passed, 0 failed

---

### Phase 3 — Semantic Engine

**Goal:** Elevate `src/lsp/` into a general-purpose Semantic Engine that parses
code, extracts symbols, builds a project graph, and powers intelligence
features — with or without an LSP server.

| Step | What |
|------|------|
| 3.1 | Define `LanguageProvider` trait in `kernel/semantic/provider.rs`: `parse()`, `symbols()`, `diagnostics()`, `completion()`, `format()`, `debug_adapter()` |
| 3.2 | Implement `LspLanguageProvider` — wraps the existing LSP client in the trait |
| 3.3 | Implement `TreesitterLanguageProvider` — wraps tree-sitter for offline parsing |
| 3.4 | Build `SymbolIndex` — stores symbol definitions, references, documentation |
| 3.5 | Build `ProjectGraph` — tracks relationships between files, symbols, modules |
| 3.6 | Wire `diagnostics()` into the gutter and decoration systems |
| 3.7 | Wire `completion()` into the existing completion popup |
| 3.8 | Add Janet API: `(semantic/symbols buffer-id)`, `(semantic/references name)`, `(semantic/documentation symbol)` |

**Design constraint:** The Semantic Engine must degrade gracefully — no LSP
server = tree-sitter only; no tree-sitter = empty results.  Never block the
UI thread.

---

### Phase 4 — Task System

**Goal:** Every executable operation is a `Task` — build, test, deploy, format,
generate.  Tasks are composable, observable, and scriptable from Janet.

| Step | What |
|------|------|
| 4.1 | Define `Task` struct: `name`, `command`, `environment`, `dependencies: Vec<TaskId>`, `output: TaskOutput` |
| 4.2 | Define `TaskOutput`: `stdout`, `stderr`, `exit_code`, `duration` |
| 4.3 | Build `TaskScheduler` in `kernel/task/scheduler.rs` — dependency resolution, concurrent execution, cancellation |
| 4.4 | Merge `src/process/` into `kernel/task/` — a process is a task that runs an external command |
| 4.5 | Add Janet API: `(task/run name &opt args)`, `(task/cancel id)`, `(task/on-complete id callback)` |
| 4.6 | Wire project build/test/run through tasks (see Phase 5) |

**Design constraint:** Tasks run on worker threads.  The Janet callback for
`task/on-complete` fires on the main thread via the event bus.

---

### Phase 5 — Project Model

**Goal:** Project becomes a first-class programmable object with build, test,
run, dependency graph, and module structure.

| Step | What |
|------|------|
| 5.1 | Promote `ProjectManager` from `kernel/state/project.rs` to `kernel/project/mod.rs` |
| 5.2 | Define `Project` struct: `modules: Vec<Module>`, `dependencies: Vec<Dependency>`, `tasks: Vec<TaskId>`, `build_targets: Vec<BuildTarget>`, `runtime_config: Config` |
| 5.3 | Define `BuildTarget`: `name`, `kind` (debug/release), `task_id`, `artifacts` |
| 5.4 | Implement `project.build()`, `project.test()`, `project.run()` — each dispatches to the Task System |
| 5.5 | Add project auto-detection (Cargo.toml → Rust, package.json → Node, etc.) |
| 5.6 | Add Janet API: `(project/current)`, `(project/build)`, `(project/test)`, `(project/run)` |
| 5.7 | Wire Project Model into the Semantic Engine's `ProjectGraph` |

---

### Phase 6 — Debug System

**Goal:** Debugger as a core subsystem, not a plugin.  Supports breakpoints,
stepping, watches, evaluation, timeline debugging.

| Step | What |
|------|------|
| 6.1 | Create `kernel/debug/` with `DebugManager` orchestrating sessions |
| 6.2 | Define `DebugSession` — owns a process, thread list, frame stack, variable cache |
| 6.3 | Define DAP types: `Breakpoint`, `StackFrame`, `Variable`, `Thread`, `MemoryRegion` |
| 6.4 | Implement DAP client (reuse or wrap `lsp_client.rs` patterns) |
| 6.5 | Implement `DebugAdapterProvider` as a Language Provider method — each language provides its adapter |
| 6.6 | Wire breakpoints into the Gutter Provider system (breakpoint gutter column) |
| 6.7 | Add Janet API: `(debug/start adapter)`, `(debug/continue)`, `(debug/step-in)`, `(debug/step-over)`, `(debug/step-out)`, `(debug/add-breakpoint file line)`, `(debug/evaluate expr)` |

---

### Phase 7 — Gutter Provider Architecture

**Goal:** Replace the fixed gutter with a provider system where any subsystem
can contribute a gutter column.

| Step | What |
|------|------|
| 7.1 | Define `GutterProvider` trait: `fn render(&self, line: usize, ctx: &GutterCtx) -> Cell`, `fn width(&self) -> usize`, `fn update(&mut self, buffer: &Buffer)` |
| 7.2 | Implement providers: `LineNumbers`, `GitSigns` (from VC), `Diagnostics` (from Semantic Engine), `Breakpoints` (from Debug System), `Coverage` (future) |
| 7.3 | Replace `GutterState` with a provider registry — ordered list of active providers per buffer view |
| 7.4 | Add Janet API: `(gutter/add-provider name provider-fn)`, `(gutter/remove-provider name)` |

---

### Phase 8 — Workspace Persistence

**Goal:** The editor saves and restores its full state: open buffers, layout,
history, projects, settings, semantic cache.

| Step | What |
|------|------|
| 8.1 | Create `kernel/storage/workspace.rs` |
| 8.2 | Define `WorkspaceState` — serializable snapshot of Editor state |
| 8.3 | Implement `workspace.save()` — writes to `~/.magma/workspace/` |
| 8.4 | Implement `workspace.restore()` — reads and reconstructs Editor state |
| 8.5 | Persist: buffers/ (unsaved content), history/ (command + search history), index/ (semantic cache), settings/ (options), sessions/ (named session snapshots) |
| 8.6 | Add Janet API: `(workspace/save)`, `(workspace/restore)`, `(workspace/session-save name)`, `(workspace/session-load name)` |

---

### Phase 9 — Concurrency Model

**Goal:** Async execution with a main thread (UI + Janet) and worker threads
(indexing, parsing, builds, diagnostics).

| Step | What |
|------|------|
| 9.1 | Introduce `kernel/scheduler/` with a thread pool |
| 9.2 | Define `Work` trait: unit of async work with progress + cancellation |
| 9.3 | Route all blocking operations (LSP, parsing, builds, git) through the scheduler |
| 9.4 | Ensure all Janet callbacks are dispatched on the main thread via the event bus |
| 9.5 | Add progress reporting: `(scheduler/progress task-id)` → `{:done N :total M}` |

**Design constraint:** The Janet runtime is pinned to the main thread.
Worker threads communicate results via the event bus only.

---

### Phase 10 — Plugin Capability System

**Goal:** Extensions declare capabilities (filesystem, network, process,
editor access).  The runtime enforces them.

| Step | What |
|------|------|
| 10.1 | Define `Capability` enum: `Filesystem`, `Network`, `ProcessExecution`, `EditorAccess` |
| 10.2 | Define `ExtensionManifest`: `name`, `version`, `capabilities: Vec<Capability>`, `entry_point` |
| 10.3 | Build `ExtensionRegistry` in `kernel/runtime/` |
| 10.4 | Add manifest validation at load time — reject extensions requesting unapproved capabilities |
| 10.5 | Wrap all Janet bridge C functions in capability checks — `fs-write` requires `Filesystem`, `shell` requires `ProcessExecution` |
| 10.6 | Add Janet API: `(extension/manifest)`, `(extension/list)`, `(extension/capabilities name)` |

---

### Phase 11 — AI / Agent Architecture

**Goal:** AI is a system participant with access to editor APIs, not a chat
window.

| Step | What |
|------|------|
| 11.1 | Create `kernel/agent/` |
| 11.2 | Define `ContextProvider` trait — gathers editor state, buffer content, symbols, diagnostics, project structure |
| 11.3 | Define `Planner` trait — takes a goal + context, produces a plan (sequence of editor API calls) |
| 11.4 | Define `Executor` trait — executes a plan against the editor, reports results |
| 11.5 | Implement `LlmContextProvider` (wraps an LLM API), `BuiltinPlanner` (simple tool-use), `SafeExecutor` (sandboxed) |
| 11.6 | Wire agent events into the event bus: `agent/thought`, `agent/action`, `agent/error` |
| 11.7 | Add Janet API: `(agent/ask prompt)`, `(agent/run-task description)`, `(agent/on-event kind callback)` |

**Design constraint:** Agents never block the UI.  Agent actions are
dispatched as commands through the command registry, which means they can be
intercepted, logged, or rejected by the security model.

---

### Phase 12 — Language Provider Ecosystem

**Goal:** Languages are external Janet+modules loaded from `languages/`,
not hardcoded in Rust.

| Step | What |
|------|------|
| 12.1 | Define `languages/<lang>/` directory convention |
| 12.2 | Each language directory provides: `provider.janet` (Language Provider impl), `syntax.scm` (tree-sitter queries), `config.janet` (format, completion settings) |
| 12.3 | Build `LanguageRegistry` in `kernel/semantic/` — discovers languages, loads providers |
| 12.4 | Port existing Rust LSP logic into Janet language providers where possible |
| 12.5 | Add Janet API: `(language/register name provider)`, `(language/list)`, `(language/for-buffer buffer-id)` |

---

## Dependency Graph

Phases must execute in order, but some run in parallel:

```
Phase 0 (Kernel Extraction)
  │
  ├──► Phase 1 (Text Engine) ──► Phase 2 (View Tree)
  │
  ├──► Phase 3 (Semantic Engine) ──► Phase 5 (Project) ──► Phase 6 (Debug)
  │                                       │
  │                                       └──► Phase 4 (Task System)
  │
  ├──► Phase 7 (Gutter Providers) — depends on Phase 1 + 3 + 6
  ├──► Phase 8 (Workspace) — depends on Phase 0 + 1 + 5
  ├──► Phase 9 (Concurrency) — can start after Phase 0, must finish before Phase 10
  ├──► Phase 10 (Plugin System) — depends on Phase 9
  ├──► Phase 11 (AI/Agent) — can start after Phase 4, depends on Phase 10
  └──► Phase 12 (Language Ecosystem) — depends on Phase 3 + 10
```

**Recommended execution order:**
1. Phase 0 (one sprint)
2. Phase 1 + Phase 3 + Phase 9 (parallel, two sprints)
3. Phase 2 + Phase 4 + Phase 5 (parallel, two sprints)
4. Phase 6 + Phase 7 (parallel, one sprint)
5. Phase 8 + Phase 10 (parallel, one sprint)
6. Phase 11 + Phase 12 (parallel, two sprints)

**Total: ~9 sprints** (assuming 2-week sprints, ~18 weeks).

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Phase 0 breaks existing tests | High | High | Run full test suite after every file move; fix imports iteratively |
| Phase 1.5 (Arc<Buffer>) breaks mutation assumptions | Medium | High | Audit every `&mut Buffer` call site; add compile-time borrow check tests |
| Phase 9 (concurrency) introduces data races | Medium | Critical | All shared state behind `Editor`'s exclusive `&mut`; worker threads communicate via events only |
| Phase 11 (agents) raises security concerns | Medium | Medium | Capability system in Phase 10 gates agent actions |
| Phase 12 duplicates existing Janet extension mechanism | Low | Medium | Janet extensions become Language Providers naturally — no duplication |
| Phase 8 (workspace persistence) serialises Editor at rest | Low | Low | Serde derives already exist on most state types |
| Scope creep from the Architecture Spec | High | Medium | Each phase has fixed completion criteria; defer unspecified features |
| Team morale from prolonged refactoring | Medium | Medium | Each phase ships end-user visible value (Phase 1: smoother navigation, Phase 3: better completions, Phase 4: build integration) |

---

## Completion Gates (applies to every phase)

1. **Tests:** `cargo test --features janet` — zero failures.
2. **File size:** No file > 400 lines (Rust) or > 200 lines (Janet).
3. **Documentation:** Every new Janet C function in `docs/api/janet-api.md`;
   architecture decisions in `docs/architecture/`.
4. **No backward compatibility:** Old names, paths, types, and structures are
   **deleted** in the same PR that introduces their replacements — no aliases,
   no shims, no deprecation warnings, no `type` aliases, no feature-gated old
   paths.
5. **Observable behaviour preserved** (Phases 0-2) or explicitly specified
   (Phases 3+): end-user Janet scripts continue to work unless a breaking
   change is documented in the sprint notes.  Breaking changes are expected
   and embraced — they are not a reason to introduce compatibility shims.
