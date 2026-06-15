# Concurrency Plan — Making Magma Concurrent with Tokio

## Current State

Magma is entirely single-threaded. No threading, no async runtime, no channels.
Everything — input polling, state mutation, event dispatch, rendering — runs sequentially
on the main thread. The `Arc<RwLock<Editor>>` pattern is preemptive scaffolding that is
never actually contended.

## Guiding Principles

1. **Editor state stays single-threaded** — all mutations happen on the main thread.
   No sharding of `Editor`, no concurrent buffer access. This avoids a class of bugs
   that plague multi-threaded editors (Emacs, Neovim both struggle with this).
2. **Blocking work moves off the main thread** — file I/O, LSP protocol, process
   spawning, plugin loading all go through tokio tasks.
3. **Janet stays on the main thread** — the Janet VM (evil-janet) uses global C state,
   `longjmp`-based signals, and raw `static mut` pointers. Pushing it to another thread
   would require a complete rewrite of the bridge. Instead, Janet API functions that do
   blocking I/O become async internally via `spawn_blocking`.
4. **Decoupled render loop** — the renderer never blocks on I/O.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  MAIN THREAD                     │
│                                                   │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐   │
│  │  Poll    │───→│  State   │───→│  Render  │   │
│  │  Events  │    │  Mutate  │    │          │   │
│  └──────────┘    └──────────┘    └──────────┘   │
│       │               ↑                          │
│       │               │                          │
│       ▼               │                          │
│  ┌──────────────────────────────────────────┐    │
│  │          Tokio Runtime                     │    │
│  │  ┌──────────┐  ┌───────────┐  ┌────────┐ │    │
│  │  │  Async   │  │  LSP      │  │ Timers │ │    │
│  │  │  File IO │  │  Protocol │  │ /Jobs  │ │    │
│  │  └──────────┘  └───────────┘  └────────┘ │    │
│  │  ┌──────────────────────────────────────┐ │    │
│  │  │  Event Channel (mpsc → main thread)  │ │    │
│  │  └──────────────────────────────────────┘ │    │
│  └──────────────────────────────────────────┘    │
└─────────────────────────────────────────────────┘
```

## Phase 0 — Runtime Setup (now)

- [ ] **Add tokio** with `full` features to `Cargo.toml` ✓ *(done)*
- [ ] Create `src/runtime.rs` — thin wrapper around `tokio::runtime::Runtime`
- [ ] Store the runtime handle in `Editor` so background tasks can be spawned
      from commands, keybindings, Janet API, etc.
- [ ] Tick the runtime in the main loop (`runtime.block_on` or `runtime.tick`)

## Phase 1 — Cross-Thread Event Channel

- [ ] Add a `tokio::sync::mpsc::UnboundedReceiver` to the main loop
- [ ] Background tasks send events through `UnboundedSender` (e.g. "file-loaded",
      "lsp-completion-ready", "plugin-reloaded")
- [ ] Main loop drains the channel each frame before processing input + rendering
- [ ] The existing `EventBus` stays as the synchronous dispatch layer; the channel
      is the bridge between threads and the event bus

Key files: `src/event/mod.rs`, `src/main.rs`, new `src/runtime.rs`

## Phase 2 — Async File System

- [ ] Make `FileSystem` trait methods `async` (or add a second async trait)
- [ ] `DiskFileSystem` uses `tokio::fs` instead of `std::fs` for reads/writes
- [ ] Buffer save/load operations are spawned on the tokio runtime
- [ ] UI is not blocked during large file reads — a "Loading..." indicator replaces
      the buffer content until the async task completes

Key files: `src/fs/mod.rs`, `src/fs/disk.rs`, `src/buffer/mod.rs`

## Phase 3 — LSP Background Protocol

- [ ] Implement the `LspBridge` trait using `tokio::process::Command` for LSP
      server child processes
- [ ] JSON-RPC communication over async stdin/stdout (`tokio::io::{AsyncBufReadExt, AsyncWriteExt}`)
- [ ] LSP results (completions, diagnostics, hover info) arrive via the event channel
- [ ] LSP is the biggest win — it's all blocking I/O (stdio to a child process)

Key files: `src/lsp/mod.rs`, new `src/lsp/json_rpc.rs`, `src/lsp/client.rs`

## Phase 4 — Background Auto-Save and Periodic Jobs

- [ ] `editor/fs-write` in Janet triggers a background save; the UI is not blocked
- [ ] Auto-save timer: tokio interval that writes modified buffers every N seconds
- [ ] File watcher: `tokio::fs::watch` or `notify` crate for external file changes

## Phase 5 — Non-Blocking Plugin Loading

- [ ] Janet plugin files (`load-file`) are read asynchronously
- [ ] Plugin compilation/eval still happens on the main thread (Janet requirement)
- [ ] A loading spinner or status message shows while plugins are loading

## What Stays Single-Threaded (explicitly)

| Component | Reason |
|-----------|--------|
| `Editor` state + all fields | No data races, simple reasoning, same as Helix/Kakoune |
| Janet VM (`evil-janet`) | Global C state, longjmp signals, raw pointer bridge |
| `input.rs` dispatch | Vim state machine is inherently ordered |
| `EventBus` dispatch | Sequential hook chains (Before→Transform→After) |
| Rendering (TUI + GUI) | Terminal/GPU APIs are not thread-safe |

## Key Integration Points (already present)

The codebase already has scaffolding that anticipates threading:

| File | Pattern | Now Used For |
|------|---------|--------------|
| `Arc<RwLock<Editor>>` | Thread-safe handle | Single-thread, no contention |
| `FileSystem: Send + Sync` | Trait bounds | Unused |
| `LspBridge: Send` | Trait bound | Stub |
| `RenderTrait: Send` | Trait bound | Unused |
| `HandlerFn: Send + Sync` | Event/command closures | Unused across threads |
| `Clipboard` with `Mutex` | System clipboard | Uncontended |

With tokio, these bounds become actually meaningful.

## Test Strategy

- Existing unit tests remain single-threaded (no changes needed)
- Integration tests for async file I/O use `tokio::test`
- LSP tests mock the child process with a tokio pipe
