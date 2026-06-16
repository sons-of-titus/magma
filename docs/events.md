# Event Contract Reference

Every named event emitted by the Rust core, its payload keys, and the emitter
location.  This is the ground truth for both Rust and Janet subscribers.

## Legend

| Column | Meaning |
|--------|---------|
| Event | Canonical event name string |
| Key | Payload key (all values are strings in the Janet-facing API) |
| Type | Semantic type of the value (always serialized as `String`) |
| Emitter | Rust module + function that calls `events.emit` |

---

## Buffer lifecycle

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `buffer-created` | `buffer-id` | usize | `command/builtin/buffer.rs` — `open-file` |
| `buffer-created` | `name` | String | |
| `buffer-created` | `path` | String | |
| `buffer-closed` | `buffer-id` | usize | `command/builtin/buffer.rs` — `close-buffer` |
| `buffer-focused` | `buffer-id` | usize | `command/builtin/buffer.rs` — `close-buffer`, `alternate-buffer` |
| `buffer-focused` | `buffer-id` | usize | `command/builtin/window.rs` — `buffer-next`, `buffer-prev` |
| `buffer-changed` | `buffer-id` | usize | `janet_bridge/buffer_text_api.rs` — `insert`, `delete` |
| `buffer-changed` | `buffer-id` | usize | `janet_bridge/buffer_query_api.rs` — `create` |
| `buffer-changed` | `buffer-id` | usize | `command/builtin/window.rs` — `tabnext`, `tabprev` |
| `buffer-read-only` | `buffer-id` | usize | `janet_bridge/buffer_text_api.rs` — `insert`, `delete` |

## Buffer persistence

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `buffer-before-save` | `path` | String | `command/builtin/buffer.rs` — `save-buffer` |
| `buffer-before-save` | `buffer-id` | usize | |
| `buffer-after-save` | `path` | String | `command/builtin/buffer.rs` — `save-buffer` |
| `buffer-after-save` | `buffer-id` | usize | |
| `buffer-message-appended` | `buffer-id` | usize | `janet_bridge/messages_api.rs` — `editor/log-message` |
| `buffer-message-appended` | `text` | String | |
| `warning-emitted` | `buffer-id` | usize | `janet_bridge/messages_api.rs` — `editor/warn` |
| `warning-emitted` | `text` | String | |
| `help-shown` | `buffer-id` | usize | `janet_bridge/messages_api.rs` — `editor/show-help` |

## Cursor

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `cursor-moved` | `buffer-id` | usize | `command/builtin/helpers.rs` — `emit_cursor_moved` |
| `cursor-moved` | `cursor` | usize | |

## Mode

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `mode-changed` | `from` | String | `janet_bridge/modality_api.rs` — `editor/set-mode` |
| `mode-changed` | `to` | String | |
| `major-mode-changed` | `mode` | String | `command/builtin/misc.rs` — `set-major-mode` |
| `major-mode-changed` | `buffer-id` | usize | |

## Selection

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `selection-changed` | *(none)* | — | `janet_bridge/selection_api.rs` — `selection/set` |
| `selection-cleared` | *(none)* | — | `janet_bridge/selection_api.rs` — `selection/clear` |

## Minibuffer

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `minibuffer-opened` | `prompt` | String | `janet_bridge/minibuffer_api.rs` — `minibuffer/open` |
| `minibuffer-opened` | `kind` | String | |
| `minibuffer-input-changed` | *(none)* | — | `janet_bridge/minibuffer_api.rs` — `minibuffer/set-input` |
| `minibuffer-closed` | `input` | String | `janet_bridge/minibuffer_api.rs` — `minibuffer/close` |
| `minibuffer-closed` | `kind` | String | |

## Find / Eval

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `finder-results` | `buffer-id` | usize | `command/builtin/buffer.rs` — `find-files` |
| `finder-results` | `count` | usize | |
| `finder-results` | `pattern` | String | |
| `eval-result` | `value` | String | `command/builtin/editing.rs` — `eval-region`, `eval-buffer` |
| `eval-result` | `error` | String | |

## File I/O (background)

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `file-loaded` | `path` | String | `runtime.rs` — `process_background_event` |
| `file-loaded` | `content` | String | |
| `file-saved` | `path` | String | `runtime.rs` — `process_background_event` |
| `file-error` | `path` | String | `runtime.rs` — `process_background_event` |
| `file-error` | `error` | String | |
| `file-changed` | `path` | String | `runtime.rs` — `process_background_event` |
| `file-changed` | `kind` | String | |
| `file-indexed` | `project-name` | String | `runtime.rs` — `process_background_event` |
| `file-indexed` | `count` | usize | |

## Process / Task

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `process-output` | `id` | u64 | `runtime.rs` — `process_background_event` |
| `process-output` | `line` | String | |
| `process-output` | `stream` | String | |
| `process-exit` | `id` | u64 | `runtime.rs` — `process_background_event` |
| `process-exit` | `exit-code` | i32 | |
| `process-exit` | `cmd` | String | |
| `task-result` | `id` | u64 | `janet_bridge/process_api.rs` — `execute_stored_task` |
| `task-result` | `value` | String | (present on success) |
| `task-result` | `error` | String | (present on failure) |

## LSP

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `lsp-diagnostics` | `path` | String | `runtime.rs` — `process_background_event` |
| `lsp-diagnostics` | `count` | usize | |
| `lsp-diagnostics` | `items` | String | |
| `lsp-completion` | `path` | String | `runtime.rs` — `process_background_event` |
| `lsp-completion` | `items` | String | |
| `lsp-response` | `path` | String | `runtime.rs` — `process_background_event` |
| `lsp-response` | `method` | String | |
| `lsp-response` | `result` | String | |
| `lsp-hover` | `path` | String | `runtime.rs` — `process_background_event` |
| `lsp-hover` | `contents` | String | |
| `lsp-definition` | `path` | String | `runtime.rs` — `process_background_event` |
| `lsp-definition` | `uri` | String | |
| `lsp-definition` | `start-line` | usize | |
| `lsp-definition` | `start-col` | usize | |
| `lsp-definition` | `end-line` | usize | |
| `lsp-definition` | `end-col` | usize | |
| `lsp-code-actions` | `path` | String | `runtime.rs` — `process_background_event` |
| `lsp-code-actions` | `actions` | String | |
| `lsp-completion-items` | `path` | String | `runtime.rs` — `process_background_event` |
| `lsp-completion-items` | `items` | String | |
| `lsp-rename-result` | `path` | String | `runtime.rs` — `process_background_event` |
| `lsp-rename-result` | `edit` | String | |
| `lsp-progress` | `token` | String | `runtime.rs` — `process_background_event` |
| `lsp-progress` | `message` | String | |
| `lsp-progress` | `percentage` | String | |

## GUI / Render

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `editor-ready` | *(none)* | — | `main.rs` — startup |
| `before-quit` | *(none)* | — | `command/builtin/misc.rs` — `quit` |
| `render-frame` | *(none)* | — | `main.rs` — event loop |
| `render-tab-bar` | *(none)* | — | `main.rs` — event loop |
| `face-changed` | `face` | String | `janet_bridge/face_api.rs` — `face/define` |
| `font-changed` | *(none)* | — | `janet_bridge/font_api.rs` — `font/set`, `font/set-size`, etc. |

## Window

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `window-focused` | `id` | u64 | `janet_bridge/window_api.rs` — `window/focus` |
| `window-focused` | `buffer` | usize | |

## Project

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `project-opened` | `root` | String | `janet_bridge/project_api.rs` — `project/set-root` |
| `project-opened` | `name` | String | |
| `project-closed` | *(none)* | — | `janet_bridge/project_api.rs` — `project/set-root` (nil) |
| `project-closed` | *(none)* | — | `janet_bridge/project_registry_api.rs` — `project/unregister` |
| `project-member-focused` | `name` | String | `janet_bridge/workspace_api.rs` — `project/set-current-member` |
| `project-member-focused` | `root` | String | |
| `project-member-focused` | `project-name` | String | |

## Gutter / Decoration

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `gutter-sign-changed` | `column` | String | `janet_bridge/gutter_api.rs` — `gutter/sign-set` |
| `gutter-sign-changed` | `buffer` | usize | |
| `gutter-sign-changed` | `line` | usize | |
| `gutter-clicked` | `column` | String | `input/mouse.rs` — `dispatch_gutter_click` |
| `gutter-clicked` | `line` | usize | |
| `gutter-clicked` | `buf` | usize | |
| `decoration-changed` | `buffer` | usize | `janet_bridge/decoration_api.rs` — `buffer/decor-set-*` |
| `decoration-changed` | `layer` | String | |

## Network (Sprint 13)

| Event | Key | Type | Emitter |
|-------|-----|------|---------|
| `http-response` | `id` | u64 | `runtime.rs` — `process_background_event` |
| `http-response` | `status` | u16 | |
| `http-response` | `body` | String | |
| `http-error` | `id` | u64 | `runtime.rs` — `process_background_event` |
| `http-error` | `error` | String | |
| `tcp-connected` | `id` | u64 | `runtime.rs` — `process_background_event` |
| `tcp-data` | `id` | u64 | `runtime.rs` — `process_background_event` |
| `tcp-data` | `data` | String | |
| `tcp-closed` | `id` | u64 | `runtime.rs` — `process_background_event` |
| `tcp-error` | `id` | u64 | `runtime.rs` — `process_background_event` |
| `tcp-error` | `error` | String | |
| `tcp-client-connected` | `server-id` | u64 | `runtime.rs` — `process_background_event` |
| `tcp-client-connected` | `client-id` | u64 | |
| `tcp-client-data` | `server-id` | u64 | `runtime.rs` — `process_background_event` |
| `tcp-client-data` | `client-id` | u64 | |
| `tcp-client-data` | `data` | String | |
| `tcp-client-disconnected` | `server-id` | u64 | `runtime.rs` — `process_background_event` |
| `tcp-client-disconnected` | `client-id` | u64 | |
