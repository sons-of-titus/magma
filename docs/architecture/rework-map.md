# Magma — As-Is Architecture Map (Rework Baseline)

Recorded 2026-10-08, from code paths, not documentation. Every claim was
traced to a file:line. This is the ground truth the architectural rework
starts from; where the rework changes something, the target is stated in
`docs/architecture/rework.md`.

Baseline: `cargo build` green, `cargo test` 1325 passed / 0 failed.

---

## 1. State ownership (as-is)

One struct owns the world. `Editor` (`src/kernel/state/mod.rs:93-175`)
holds ~60 fields: document arena, views, keymaps, commands, event bus,
registers/marks/macros, theme, fonts, overlays, mouse state, completion
popup state, project manager, semantic engine, language registry, task
scheduler, work scheduler, debug manager, agent manager, IO state
(terminals/processes/net), the Janet script runtime, and the background
handle.

- Access model: `Arc<RwLock<Editor>>` (`EditorHandle`, state/mod.rs:386).
  The GUI acquires the write lock 6-8 times and the read lock ~10 times
  per frame (`render/gui_editor_window.rs`). Every command executes
  synchronously under whatever lock the caller holds.
- Buffers are additionally `Arc<Mutex<Buffer>>` inside the slab
  (state/mod.rs:94) — a second lock level needed only because background
  save paths touch buffers directly.
- Janet reaches the editor through a thread-local raw `*mut Editor`
  (`scripting/mod.rs:33`, `EditorScope`), fabricated into `&mut Editor`
  per call — sound only under the single-thread discipline that nothing
  enforces.

**Verdict:** the global lock is the architecture. Long-running work avoids
the lock only by luck (everything heavy is synchronous, so nothing
contends); async results land by taking the same lock on the GUI thread.

## 2. Interactive path (keystroke → pixels)

```
egui frame (gui_editor_window.rs)
  → drain BackgroundEvents (runtime.rs::process_background_event)
  → collect keys (gui_input.rs) → input::dispatch_key (input/mod.rs:87)
      → terminal forwarding | completion popup | on-input hook
      → keymaps.resolve_for_buffer → execute_command (command/mod.rs:137)
      → literal insert (handle_text_input)
  → publish_buffer_changes (state/views.rs:81)  ← locks EVERY buffer
  → dispatch_events_scoped → Janet handlers
  → render_signature (state/views.rs:121)       ← locks EVERY buffer
  → repaint if changed, else 400ms blink timer
  → frame build: fresh Surface per pane (gui_editor_pane.rs:72)
  → CPU text draw (gui_render.rs) or GPU path when MAGMA_GPU_RENDER=1
```

O(all-buffers) or O(document) work on the interactive path:

| Site | Cost | When |
|------|------|------|
| `publish_buffer_changes` (state/views.rs:84) | locks every buffer | every key |
| `render_signature` (state/views.rs:127) | locks every buffer | every frame |
| completion `trigger` (completion.rs:36) | copies whole document text | every word char |
| minimap (gui_minimap.rs:76) | scans all lines + all highlights | every frame |
| sidebar tree rebuild (gui_sidebar.rs:281) | rebuilds FileNode tree from flat list | every frame |
| `selection_range` line scan (state/mod.rs:332) | O(offset) char scan | visual mode |

## 3. Document engine (the strong part)

`text_engine/`: ropey-backed `Document` with a single `commit` gateway
(document.rs:101), validation before mutation, one commit = one revision.
`Buffer` (buffer.rs) adds marks/anchors that track transactions, undo
groups with cursor restore, revision-aware async-save (`mark_saved_at`),
and a pending-change feed drained into `buffer-changed` events.

Known gaps (task §23): `Document.history` is unbounded (memory grows
forever, document.rs:23); `changes_since` silently degrades if history
were bounded; multi-cursor insertion (buffer.rs:203) computes cursor
restore by adding total inserted length, not per-cursor; `insert_many`
does not normalize overlapping offsets (documented as caller duty).

## 4. Language infrastructure (as-is)

- **LSP** (`semantic/client.rs`): `initialize()` sends the request and
  returns immediately — the response is never awaited, `initialized`
  fires before the server answers (client.rs:78-91). Request IDs come
  from a **global static** table shared by all servers (client.rs:15);
  `request_id` is stripped on arrival (runtime.rs:215,225,253,416), so
  results are matched by file path only — stale responses are applied
  regardless of intervening edits. No cancellation, no timeouts, no
  crash detection, no restart, no Drop (child processes leak). A second,
  competing registry (`LspManager`, client.rs:295) is dead code.
- **tree-sitter** (`semantic/ts_registry.rs`): grammar dlopen is real,
  but every parse is a full parse from a full text copy
  (ts_registry.rs:37-83). An incremental path exists and is dead
  (ts_registry.rs:128-177, zero callers); its revision is hardcoded 0.
  Grammars only load if someone calls `ts/load-grammar` from Janet with
  a path; nothing ships grammars.
- **Diagnostics** flow as formatted strings (`"[E] line L:C msg"`,
  provider.rs:105) that get re-parsed — lossy by construction.
- **Symbol index** (`semantic/symbol_index.rs`): in-memory
  name→symbols HashMap, populated only when `index_buffer` is called
  per open buffer. Not persistent, not project-wide.

## 5. Project model + index (as-is)

`Project` (project/mod.rs:87) is `root: Option<PathBuf>` +
`files: Vec<String>`, detected by 4 hardcoded marker files. File indexing
is a blocking recursive walk on the background pool, delivering a flat
path list; contents are never read; nothing is persisted. `ProjectGraph`
has dependency adjacency but nothing ever adds an edge outside tests.

## 6. Execution / scheduling (as-is)

- `BackgroundHandle` (runtime.rs:203) = tokio runtime + unbounded mpsc.
  `spawn_blocking` is called ad hoc from ~15 subsystems; there is no
  priority, no coalescing, no backpressure. The `WorkScheduler`
  (scheduler/mod.rs:108) tracks progress/cancellation for jobs that opt
  in, but spawn order is submission order.
- `TaskScheduler` (task/scheduler.rs): shell tasks run `sh -c` buffered
  to completion; `cancel()` flips a status flag and **the process keeps
  running** (scheduler.rs:189).
- UI-thread IO: `task/mod.rs:20,50` run `Command::output()` synchronously
  on the caller (main) thread; VCS refresh shells out synchronously
  (vc/mod.rs:201); Janet `fs` calls block under the editor lock.

## 7. Subsystem truth table

| Subsystem | Real | Facade / broken |
|-----------|------|-----------------|
| Document engine | rope, revisions, transactions, anchors, revision-aware save | unbounded history; multi-cursor restore |
| Event bus | typed payloads, hooks, scoped Janet dispatch | stringly `HashMap<String,String>` on emit path |
| Commands | typed registry + arg resolution, Rust + Janet verbs | change-list heuristic by name prefix |
| LSP | transport, framing, diagnostics parse | fake-awaited init, global ID table, no staleness/cancel/restart |
| tree-sitter | real dlopen, real queries | full re-parse per call, dead incremental path, no shipped grammars |
| Index | background file walk | no symbols from disk, no persistence, no invalidation granularity |
| DAP | real framing, real events | fire-and-forget init, no pending map, no verify breakpoints, no kill on drop |
| VCS | git/hg/svn/jj status/diff/log/blame/stage/commit via CLI | synchronous on UI thread, hand-parsed porcelain |
| Terminal | PTY spawn, mode, input forwarding | `strip_ansi` "emulator": no cursor addressing, no SGR colors, no alt screen; vim/htop render as garbage. New screen/session files exist unmounted |
| Search | project grep to quickfix | blocking whole-project walk on one thread, uncancellable |
| Build/run | task defs from project markers | buffered output, fake cancel, sync make/shell on main thread |
| Debugger UI | sessions/threads/frames/variables panes | fed by the fire-and-forget client above |
| Agent | session bookkeeping | planner is a prefix-match toy; no LLM backend (fine — but it presents as AI) |
| Server | headless JSON-line protocol | takes the global write lock per op, runs Janet inline under it; "collab" broadcast is offset echo without OT/CRDT |
| Janet | keymaps, commands, events, modes, LSP glue, syntax | `colon/define` undefined → debug/workspace/agent builtins silently fail to load; net_api.rs dead module; ~130 unused C fns; silent load errors |

## 8. Threads (as-is)

1. Main/GUI thread — owns `Arc<RwLock<Editor>>`, all commands, Janet,
   rendering.
2. Tokio runtime workers — LSP/DAP/PTY/TCP/HTTP readers, background
   file IO, task shells.
3. Tokio blocking pool — `spawn_blocking` jobs (walks, greps, saves).
4. FileWatcher thread (notify).
5. Auto-save loop task — takes read lock, then write lock per buffer
   (main.rs:167-209).

Janet must never run off the main thread (longjmp discipline); nothing
enforces this beyond convention.
