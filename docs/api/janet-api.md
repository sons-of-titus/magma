# Janet API Reference

All functions are registered C bridge functions (`extern "C-unwind"`) callable from Janet.
Format: `### (namespace/fn-name arg …)` → return-type

---

## buffer/ — Buffer Operations

### `(buffer/current)` → slab-key or nil

Return the slab key of the buffer displayed in the focused window, or nil.

### `(buffer/list)` → `[{:slab n :name s :len n} …]`

Return an array of tables describing every buffer. Each table has `:slab`, `:name`, `:len`.

### `(buffer/create name &opt content)` → slab-key

Create a new buffer with `name`. If `content` is given, pre-populate it. Returns the slab key.

### `(buffer/insert slab pos text)` → nil

Insert `text` at byte offset `pos` in buffer `slab`. No-op if the buffer is read-only (emits `buffer-read-only`). Emits `buffer-changed`.

### `(buffer/delete slab start end)` → nil

Delete bytes `[start, end)` from buffer `slab`. No-op if the buffer is read-only. Emits `buffer-changed`.

### `(buffer/slice slab start end)` → string

Return the substring `[start, end)` of buffer `slab` as a string.

### `(buffer/len slab)` → integer

Return the total byte length of the buffer.

### `(buffer/name slab)` → string

Return the buffer's display name.

### `(buffer/path slab)` → string or nil

Return the file path associated with the buffer, or nil.

### `(buffer/set-path slab path)` → nil

Set the file path for the buffer.

### `(buffer/cursor slab)` → integer or nil

Return the cursor position (byte offset) in the buffer.

### `(buffer/set-cursor slab pos)` → nil

Set the cursor position to byte offset `pos`.

### `(buffer/undo slab)` → bool

Undo the last operation. Returns true on success.

### `(buffer/redo slab)` → bool

Redo the last undone operation. Returns true on success.

### `(buffer/mark-saved slab)` → nil

Mark the buffer as saved (clears the modified flag).

### `(buffer/modified? slab)` → bool

Return true if the buffer has unsaved changes since the last `buffer/mark-saved`.

### `(buffer/line-count slab)` → integer

Return the number of lines in the buffer.

### `(buffer/line-start-offset slab line)` → integer or nil

Return the byte offset of the start of `line` (0-based), or nil if out of range.

### `(buffer/line-number slab)` → integer

Return the 0-based line number of the buffer's current cursor position.

### `(buffer/major-mode slab)` → string

Return the major mode name for the buffer (e.g. `"fundamental"`, `"rust-mode"`).

### `(buffer/diagnostics slab)` → `[string …]`

Return the LSP diagnostics for the buffer as an array of formatted strings.

### `(buffer/find-or-create name)` → slab-key

Return the slab key of the buffer named `name`; create it if it does not exist. Guarantees at most one buffer per name.

### `(buffer/get-by-name name)` → slab-key or nil

Return the slab key of the buffer named `name`, or nil. Does not create a buffer.

### `(buffer/set-read-only slab bool)` → nil

Set or clear the read-only flag. When true, `buffer/insert` and `buffer/delete` are no-ops and emit `buffer-read-only`.

### `(buffer/read-only? slab)` → bool

Return true if the buffer is currently read-only.

### `(buffer/set-ephemeral slab bool)` → nil

Set or clear the ephemeral flag. Ephemeral buffers skip "save before closing?" prompts and session restore.

### `(buffer/ephemeral? slab)` → bool

Return true if the buffer is currently ephemeral.

### `(buffer/set-highlights slab & [[start end face] …])` → nil

Replace the base highlight layer with the given byte ranges. Passing no ranges clears the base layer.

### `(buffer/clear-highlights slab)` → nil

Clear all highlights on the base layer.

### `(buffer/set-highlights-layer slab layer-name & [[start end face] …])` → nil

Set highlight ranges for the named layer. Layers are merged in order: base → syntax → semantic → search → selection → others.

### `(buffer/clear-highlights-layer slab layer-name)` → nil

Clear highlight ranges for the named layer. Other layers are unaffected.

### `(buffer/fold slab start end)` → nil

Fold the byte range `[start, end)`. Folds are stored sorted by start byte.

### `(buffer/unfold slab start end)` → nil

Remove the fold matching the exact byte range `[start, end)`.

### `(buffer/unfold-all slab)` → nil

Remove all folds from the buffer.

### `(buffer/folds slab)` → `[[start end] …]`

Return all active folds sorted by start byte.

### `(buffer/set-header-line slab text)` → nil

Set the pre-rendered header line displayed above the buffer content. Pass nil to clear.

### `(buffer/header-line slab)` → string or nil

Return the current header line text, or nil.

### `(buffer/decor-set-inline slab layer line col text face)` → nil

Insert or replace an inline text decoration at (`line`, `col`) in the named decoration `layer`.

### `(buffer/decor-set-eol slab layer line text face)` → nil

Insert or replace an end-of-line decoration for `line` in the named layer.

### `(buffer/decor-set-prefix slab layer line text face)` → nil

Insert or replace a line-prefix decoration for `line` in the named layer.

### `(buffer/decor-clear-layer slab layer)` → nil

Remove all decorations in the named layer.

### `(buffer/decor-clear slab)` → nil

Remove all decoration layers from the buffer.

### `(buffer/decor-get slab layer)` → `[{:type :line n :col n :text "…" :face "…"} …]`

Return all decorations in the named layer as an array of tables.

### `(buffer/decor-count slab layer)` → integer

Return the number of decorations in the named layer.

---

## command/ — Command System

### `(command/define name fn &opt args-spec)` → nil

Define a new command. `name` is a string, `fn` is the handler. Refuses to overwrite an existing Rust-registered command. Optional `args-spec` is a table with `:doc` and `:args`.

### `(command/redefine name fn &opt args-spec)` → nil

Define or overwrite a command. Like `command/define` but silently replaces any existing entry, including Rust-registered commands.

### `(command/run name & args)` → nil

Execute a registered command by name with optional arguments.

### `(command/list)` → `[string …]`

Return an array of all registered command names.

### `(command/exists? name)` → bool

Return true if the command is registered.

---

## editor/ — Editor Global Operations

Functions that operate on the editor as a whole: mode, running state, theme, focused buffer, I/O, input dispatch, and eval.

### `(editor/mode)` → string

Return the current mode name: `"NORMAL"`, `"INSERT"`, `"VISUAL"`, etc.

### `(editor/mode-detail)` → table

Return a table with mode details: `:name`, `:anchor` (visual modes), `:input` / `:direction` (command/search modes).

### `(editor/mode-name)` → string

Return the current mode name string.

### `(editor/mode-accepts-text?)` → bool

Return true if unbound keys are inserted as text in the current mode.

### `(editor/set-mode name &opt opts)` → nil

Set the current editor mode. Accepts `"normal"`, `"insert"`, `"command"`, `"replace"`, `"visual"`, `"visual-line"`, `"visual-block"`, or any string. Optional `opts` table may contain `:accepts-text bool`. Emits `mode-changed`.

### `(editor/set-running bool)` → nil

Set the running flag. Pass false to quit.

### `(editor/set-theme key r g b)` → nil

Set a theme color entry. `key` is a string; `r`, `g`, `b` are 0–255 integers.

### `(editor/theme key)` → `[r g b]` or nil

Return a theme color as a 3-element array, or nil.

### `(editor/focused-buffer)` → slab-key

Return the slab key of the buffer displayed in the focused window.

### `(editor/buffer-list)` → `[slab-key …]`

Return an array of all buffer slab keys.

### `(editor/cursor)` → integer

Return the cursor byte offset in the focused buffer.

### `(editor/cursor-shape)` → string

Return the current cursor shape: `"block"`, `"beam"`, or `"underline"`.

### `(editor/set-cursor-shape shape)` → nil

Set the cursor shape. `shape` must be `"block"`, `"beam"`, or `"underline"`.

### `(editor/on-input fn-name)` → nil

Register `fn-name` as a raw input interceptor called with the key string before keymap dispatch. Pass nil to clear.

### `(editor/on-input-fn)` → string or nil

Return the name of the currently registered input interceptor, or nil.

### `(editor/consume-input)` → nil

Signal that the current key event is consumed. Must be called from within an `editor/on-input` handler.

### `(editor/command-input)` → string or nil

Return the current command-mode input string, or nil if not in command mode.

### `(editor/fs-read path)` → string

Read file content as a string. Signals an error on failure.

### `(editor/fs-write path content)` → nil

Write a string to a file. Signals an error on failure.

### `(editor/fs-exists? path)` → bool

Return true if a file or directory exists at `path`.

### `(editor/shell cmd)` → string

Run `sh -c cmd` synchronously and return combined stdout+stderr. Signals on exec error; ignores exit code.

### `(editor/run-command name &opt args)` → nil

Execute a registered Rust command by name with optional positional string arguments.

### `(editor/load-file path)` → nil

Evaluate a Janet file from disk. Signals an error if the file cannot be read or evaluated.

### `(editor/set-completions items &opt idx)` → nil

Show the completion popup with the given string array `items` and optional selected index.

### `(editor/clear-completions)` → nil

Dismiss the completion popup.

### `(editor/save-layout name)` → nil

Snapshot the current window tree under `name`.

### `(editor/restore-layout name)` → nil

Restore a previously saved window tree. No-op if `name` was not saved.

### `(editor/layout-list)` → `[name …]`

Return an array of all saved layout names.

### `(editor/pending-operator)` → string or nil

Return the pending Vim operator (`"d"`, `"c"`, `"y"`, etc.), or nil.

### `(editor/pending-count)` → integer

Return the accumulated pending count.

### `(editor/pending-prefix)` → string or nil

Return the current pending key prefix (e.g. `"g"`, `"ctrl-w"`), or nil.

### `(editor/eval expr)` → string

Evaluate the Janet expression `expr` and return the printed result. Errors are returned as strings.

### `(editor/log-message text)` → nil

Append `text` to `*Messages*`. Emits `buffer-message-appended`.

### `(editor/warn text)` → nil

Append `text` to `*Warnings*`. Emits `warning-emitted`.

### `(editor/show-help text)` → nil

Replace `*Help*` buffer content with `text` and focus it. Emits `help-shown`.

---

## clipboard/ — System Clipboard

### `(clipboard/get)` → string or nil

Return the system clipboard text, or nil.

### `(clipboard/set text)` → nil

Write `text` to the system clipboard.

---

## face/ — Named Faces and Styles

### `(face/define name style-table)` → nil

Define or redefine a named face. `style-table` accepts `:fg [r g b]`, `:bg [r g b]`, `:bold`, `:italic`, `:underline`, `:strikethrough`, `:dim`. Emits `face-changed`.

### `(face/get name)` → `{:fg […] :bg […] :bold bool …}` or nil

Return the style table for a named face, or nil.

### `(face/make-style style-table)` → integer

Create a one-off anonymous style and return an integer handle usable as a face name.

### `(face/scope-face scope-prefix face-name)` → nil

Register a TextMate-style scope prefix → face name mapping.

### `(face/resolve-scope scope)` → face-name or nil

Resolve a scope string to the best-matching face name, or nil.

---

## font/ — Font Configuration

### `(font/set family size)` → nil

Set the primary font family and size (in points). Emits `font-changed`.

### `(font/size)` → number

Return the current font size in points.

### `(font/set-size n)` → nil

Set the font size to `n` points (minimum 4). Emits `font-changed`.

### `(font/load path alias)` → nil

Load a `.ttf`/`.otf` font file and register it under `alias`. Emits `font-changed`.

### `(font/set-fallback [alias …])` → nil

Set the ordered fallback font list. Emits `font-changed`.

### `(font/set-context context {:family f :size s})` → nil

Store a per-context font override keyed by `context`. Emits `font-changed`.

### `(font/set-ligatures bool)` → nil

Enable or disable font ligatures. Emits `font-changed`.

### `(font/set-glyph-width char-or-codepoint width)` → nil

Override the display-column width of a single character. `width` is 1 or 2.

### `(font/set-glyph-width-range start end width)` → nil

Override the display-column width for every codepoint in the inclusive range `[start, end]`.

### `(font/set-nerd-font bool)` → nil

When true, pre-populate the glyph-width table with width 2 for standard Nerd Font ranges.

### `(font/invalidate)` → nil

Mark the GPU glyph atlas as dirty so it is rebuilt on the next frame.

---

## mark-ring/ — Position Mark Ring

### `(mark-ring/push path offset)` → nil

Push a `(path, byte-offset)` position onto the mark ring (max 100 entries).

### `(mark-ring/pop)` → `{:path p :offset n}` or nil

Remove and return the most recent mark ring entry, or nil.

### `(mark-ring/peek)` → `{:path p :offset n}` or nil

Return the most recent mark ring entry without removing it, or nil.

### `(mark-ring/len)` → integer

Return the number of entries in the mark ring.

---

## minibuffer/ — Minibuffer

### `(minibuffer/open prompt &opt kind)` → nil

Open the minibuffer with `prompt`. `kind` is stored in `plugin-state["minibuffer.kind"]` for handlers. Emits `minibuffer-opened`.

### `(minibuffer/input)` → string or nil

Return the current minibuffer input string, or nil.

### `(minibuffer/set-input text)` → nil

Replace the minibuffer input. Emits `minibuffer-input-changed`.

### `(minibuffer/close)` → nil

Close the minibuffer. Emits `minibuffer-closed`.

### `(minibuffer/prompt)` → string or nil

Return the current minibuffer prompt, or nil.

### `(minibuffer/set-command-input text)` → nil

Overwrite the command-mode minibuffer input string.

---

## module/ — Plugin Search Path

### `(module/path)` → `[path …]`

Return the list of plugin search directories.

### `(module/path-add path)` → nil

Append `path` to the plugin search list. Idempotent.

---

## option/ — Editor Options

### `(option/list)` → `{:key "value" …}`

Return all global editor options as a table.

### `(option/get name)` → string or nil

Get a single global editor option, or nil.

### `(option/set name value)` → nil

Set a single global editor option.

### `(option/get-local name)` → string or nil

Get a buffer-local option for the focused buffer, falling back to the global option.

### `(option/set-local name value)` → nil

Set a buffer-local option on the focused buffer.

---

## overlay/ — Floating Overlays

### `(overlay/create x y w h buf-id)` → overlay-id

Create a floating overlay at (x, y) with dimensions (w × h). `buf-id` is optional.

### `(overlay/destroy id)` → nil

Remove the overlay with the given ID.

### `(overlay/move id x y)` → nil

Move the overlay to position (x, y).

### `(overlay/list)` → `[{:id n :x n :y n :width n :height n :z-order n} …]`

Return all active overlays.

---

## plugin-state/ — Plugin Key-Value Store

### `(plugin-state/get key)` → string or nil

Return a plugin state value by key, or nil.

### `(plugin-state/set key val)` → nil

Store a string value in plugin state.

### `(plugin-state/del key)` → nil

Delete a plugin state entry.

---

## register/ — Named Registers

### `(register/get name)` → string or nil

Get the value of a named register (`"a"`–`"z"`), or nil.

### `(register/set name value)` → nil

Set the value of a named register.

### `(register/yanked-text)` → string or nil

Return the last yanked or deleted text, or nil.

---

## search/ — Search State

### `(search/pattern)` → string or nil

Return the current search pattern, or nil.

### `(search/set-pattern pattern)` → nil

Set the current search pattern.

### `(search/forward?)` → bool

Return true if the last search was in the forward direction.

### `(search/last-find)` → `{:char "x" :forward bool :till bool}` or nil

Return the last `f`/`t` find parameters, or nil.

---

## selection/ — Visual Selection

### `(selection/get)` → `{:anchor n :kind "char"}` or nil

Return the current selection as a table, or nil.

### `(selection/set anchor &opt kind)` → nil

Set the visual selection. `anchor` is a byte offset; `kind` defaults to `"char"`. Emits `selection-changed`.

### `(selection/clear)` → nil

Clear the current selection. Emits `selection-cleared`.

---

## surface/ — Render Surface

### `(surface/set-cell x y char face-name)` → nil

Write one character to the current render surface at (x, y). Only valid inside a `render-frame` handler.

### `(surface/set-text x y text face-name)` → nil

Write a string to the render surface starting at (x, y). Only valid inside a `render-frame` handler.

### `(surface/size)` → `{:width n :height n}`

Return the render surface dimensions. Only valid inside a `render-frame` handler.

---

## ui/ — UI Configuration

### `(ui/set-modeline fn-name)` → nil

Set the Janet command whose return value replaces the built-in status bar text. Pass nil to restore the default.

### `(ui/modeline)` → string or nil

Return the current modeline function name, or nil.

### `(ui/load-theme path)` → nil

Evaluate the Janet theme file at `path`.

### `(ui/set-tab-bar bool)` → nil

When true, reserve the top row for a tab bar and emit `render-tab-bar`.

### `(ui/tab-bar-enabled)` → bool

Return whether the tab bar row is currently enabled.

---

## event/ — Event System

### `(event/on event-name handler-fn)` → subscription-id

Subscribe to an event. `handler-fn` receives the event data table.

### `(event/once event-name handler-fn)` → subscription-id

Subscribe to an event for a single invocation.

### `(event/off subscription-id)` → nil

Remove a subscription.

### `(event/emit event-name data-table)` → nil

Emit an event with the given data table.

### `(event/list-events)` → table

Return a table mapping event names to subscriber counts.

### `(event/list-subscribers event-name)` → `[subscription-id …]`

Return an array of subscription IDs for the named event.

---

## fs/ — Filesystem Primitives

### `(fs/cwd)` → string

Return the current working directory as an absolute path.

### `(fs/chdir path)` → nil

Change the current working directory. Signals an error if inaccessible.

---

## gutter/ — Gutter Provider Architecture (Phase 7)

Built-in providers registered by `builtins/gutter.janet`:

| Provider | Width | Data source |
|----------|-------|-------------|
| `:breakpoints` | 2 | `ed.debug.breakpoints` (live) |
| `:git-signs` | 1 | `sign_cache` (populated by `gutter/provider-update`) |
| `:diagnostics` | 1 | `buf.diagnostics` strings (pre-computed per frame) |
| `:line-numbers` | dynamic | computed from line count |
| `:folding` | 1 | `view.folds` (per-frame) |

### `(gutter/add-provider name &opt provider-fn)` → nil

Register a named gutter provider. If a provider with `name` already exists it is replaced. `provider-fn` is stored for documentation purposes; data is pushed via `gutter/provider-update`. Built-in providers (`:line-numbers`, `:diagnostics`, etc.) are implemented in Rust and automatically dispatch to their respective data sources.

### `(gutter/remove-provider name)` → nil

Remove the provider named `name`. No-op if it does not exist.

### `(gutter/provider-update name buf-id cells)` → nil

Push pre-computed cells into the named provider's sign cache for `buf-id`. `cells` is an array of `{:line N :text "T" :face "F"}` tables. Replaces all existing cells for `(name, buf-id)`. Emits `gutter-provider-updated`.

### `(gutter/provider-list)` → `[{:name "…" :width n} …]`

Return all registered providers in render order.

### `(gutter/set-fold-icons open closed face)` → nil

Set the icons drawn by the `:folding` provider. `open` and `closed` are single-character strings; `face` is the face name for styling.

### `(gutter/set-line-number-format fn-name)` → nil

Set a Janet command invoked per visible line to produce the line-number string. Pass nil to restore the built-in format.

---

## input/ — Modal Input Policy

### `(input/register-operator key line-cmd)` → nil

Register `key` as a Vim operator. `line-cmd` is run when the key is doubled.

### `(input/clear-operators)` → nil

Remove all registered operators.

### `(input/get-operator key)` → string or nil

Return the line-command for `key`, or nil.

### `(input/register-prefix key)` → nil

Register `key` as a two-key prefix.

### `(input/unregister-prefix key)` → nil

Remove a prefix registration.

### `(input/clear-prefixes)` → nil

Remove all prefix registrations.

### `(input/registered-prefix? key)` → bool

Return true if `key` is a registered prefix.

### `(input/register-motion key cmd)` → nil

Register `key` as a motion. `cmd` is executed when composed with an operator.

### `(input/unregister-motion key)` → nil

Remove a motion registration.

### `(input/clear-motions)` → nil

Remove all motion registrations.

### `(input/get-motion key)` → string or nil

Return the command name for motion `key`, or nil.

### `(input/register-char-capture key callback-cmd)` → nil

Register `key` as a char-capture prefix (used for `f`, `t`, `r`, `m`, `q`, `@`, etc.).

### `(input/unregister-char-capture key)` → nil

Remove a char-capture registration.

---

## keymap/ — Keymap System

### `(keymap/set key-sequence command-name &opt layer)` → nil

Bind a key sequence to a command. `layer` defaults to `"global"`.

### `(keymap/unset key-sequence &opt layer)` → nil

Remove a keybinding.

### `(keymap/list &opt layer)` → table

List all keybindings in a layer, or all layers if omitted.

### `(keymap/describe key-sequence)` → string or nil

Return the command name bound to a key sequence, or nil.

### `(keymap/push-layer layer-name)` → nil

Push a named layer onto the active keymap stack.

### `(keymap/pop-layer layer-name)` → nil

Remove a named layer from the active keymap stack.

### `(keymap/list-layers)` → `[string …]`

Return an array of currently active layer names.

### `(keymap/list-layer layer)` → `[[key cmd] …]`

Return all key→command pairs registered in the named layer.

---

## lsp/ — Language Server Protocol

### `(lsp/start language-id command & args)` → nil

Start an LSP server for the given language. No-op if already running.

### `(lsp/notify language-id method &opt json-params)` → nil

Send a JSON-RPC notification to a running LSP server.

### `(lsp/request language-id method params)` → nil

Send a generic JSON-RPC request. Response arrives via `lsp-response` event.

### `(lsp/hover language-id)` → nil

Request hover information at the cursor position. Response via `lsp-hover` event.

### `(lsp/code-actions language-id)` → nil

Request code actions at the cursor position. Response via `lsp-code-actions` event.

### `(lsp/completion language-id)` → nil

Request completion items at the cursor position. Response via `lsp-completion-items` event.

### `(lsp/rename language-id new-name)` → nil

Request a rename of the symbol under the cursor. Response via `lsp-rename-result` event.

### `(lsp/apply-edit edit-json)` → nil

Apply a WorkspaceEdit JSON object to open buffers.

---

## net/ — Network Primitives

### `(net/http-request method url &opt body)` → request-id

Fire an HTTP request. Response arrives via `http-response` or `http-error` event.

### `(net/http-get url)` → request-id

Convenience GET request.

### `(net/http-post url body)` → request-id

Convenience POST request.

### `(net/tcp-connect host port)` → conn-id

Open an outbound TCP connection. Emits `tcp-connected`, `tcp-data`, `tcp-closed`, `tcp-error`.

### `(net/tcp-send conn-id data)` → nil

Send a line of text over an open TCP connection.

### `(net/tcp-close conn-id)` → nil

Close an outbound TCP connection.

### `(net/tcp-listen port)` → server-id

Bind a TCP listener. Emits `tcp-client-connected`, `tcp-client-data`, `tcp-client-disconnected`.

### `(net/tcp-stop server-id)` → nil

Stop a listening TCP server.

### `(net/tcp-broadcast server-id data)` → nil

Send a line to every connected client.

### `(net/tcp-send-to server-id client-id data)` → nil

Send a line to a specific client.

---

## process/ — Subprocess Management

### `(process/spawn cmd &opt args cwd)` → process-id

Spawn a subprocess via `sh -c cmd`. Optional `cwd` sets the working directory. Output arrives via `process-output` events; exit via `process-exit`.

### `(process/kill id)` → nil

Send SIGTERM to the process. No-op if already exited.

### `(process/stdin id text)` → nil

Write `text` to the process's stdin.

### `(process/list)` → `[{:id n :cmd "…" :running bool} …]`

Return all active subprocesses.

---

## project/ — Project Management

### `(project/root)` → string or nil

Return the project root path, or nil.

### `(project/set-root path)` → nil

Set the project root. Pass nil to clear. Emits `project-opened` or `project-closed`.

### `(project/name)` → string or nil

Return the project name, or nil.

### `(project/set-name name)` → nil

Set the project name.

### `(project/files)` → `[string …]`

Return the cached project file list.

### `(project/index-files)` → nil

Walk the project root asynchronously and cache the file list. Results arrive via `file-indexed` event.

### `(project/option-get key)` → string or nil

Get a per-project option value.

### `(project/option-set key val)` → nil

Set a per-project option value.

### `(project/recent)` → `[string …]`

Return recently opened project paths (most recent first).

### `(project/push-recent path)` → nil

Add a path to the recent list and persist to disk.

### `(project/workspace?)` → `{:root "…" :members […]}` or nil

Return the workspace descriptor, or nil if no workspace is configured.

### `(project/workspace-members)` → `[{:name "…" :root "…"} …]`

Return all workspace member subprojects.

### `(project/set-workspace-members [{:name "…" :root "…"} …])` → nil

Set the workspace member list.

### `(project/set-current-member name)` → nil

Switch the active subproject. Emits `project-member-focused`.

### `(project/buffer-set-project buf project-name)` → nil

Associate a buffer with a project name.

### `(project/buffer-project buf)` → string or nil

Return the project name for a buffer, or nil.

### `(project/register name root)` → nil

Register a project in the multi-project registry.

### `(project/unregister name)` → nil

Remove a project from the registry. Emits `project-closed` if it was the active project.

### `(project/list)` → `[{:name "…" :root "…"} …]`

Return all registered projects.

### `(project/path-exists? path)` → bool

Return true if a file or directory exists at `path`.

### `(project/fs-read path)` → string or nil

Read a file's content as a string, or nil on failure.

### `(project/current)` → `{:name :root :language :tasks :build-targets}` or nil

Return a table describing the active project.  Returns nil when no project is
active.

`:name` — project name string  
`:root` — absolute root path string  
`:language` — detected language: `"rust"`, `"node"`, `"python"`, `"make"`, or `""`  
`:tasks` — array of task IDs registered for this project  
`:build-targets` — array of `{:name :kind :task-id}` tables (`:kind` is `:debug` or `:release`)

### `(project/build)` → task-id

Dispatch the project's `build` target through the Task System.  Emits
`task-started`.  Signals an error if no `build` task is defined — call
`(task/detect-project root)` first.

### `(project/test)` → task-id

Dispatch the project's `test` target through the Task System.  Emits
`task-started`.  Signals an error if no `test` task is defined.

### `(project/run)` → task-id

Dispatch the project's `run` target through the Task System.  Emits
`task-started`.  Signals an error if no `run` task is defined.

---

## quickfix/ — Quickfix List

### `(quickfix/set entries)` → nil

Replace the quickfix list. Each entry is a table with `:filename`, `:line`, `:col`, `:message`. Pass an empty array to clear.

### `(quickfix/get)` → `[{:filename :line :col :message} …]`

Return the current quickfix list.

---

## task/ — Background Tasks

### `(task/spawn janet-fn)` → task-id

Run a zero-arg Janet function as a background task. Result arrives via `task-result` event.

### `(task/cancel id)` → nil

Cancel a background task.

---

## ts/ — Tree-sitter Integration

### `(ts/load-grammar path name)` → true

Load a tree-sitter grammar from a shared library and register it under `name`.

### `(ts/set-language buf-id lang-name)` → nil

Associate a tree-sitter language with a buffer.

### `(ts/has-tree? buf-id)` → bool

Return true if a tree-sitter language has been set on the buffer.

### `(ts/parse buf-id)` → bool

Return true if a tree-sitter language is assigned to the buffer (alias for `ts/has-tree?`).

### `(ts/query buf-id query-string)` → `[[start end scope-name] …]` or nil

Parse the buffer with tree-sitter and run a query. Returns byte-range tuples, or nil on failure.

---

## vc/ — Version Control

### `(vc/register-backend table)` → nil

Register a custom VCS backend via shell-command templates. The table must include `:name`, `:detect-marker`, and command-template keys for diff/status/log operations.

---

## window/ — Window Management

### `(window/current)` → window-id

Return the ID of the focused window.

### `(window/list)` → `[window-id …]`

Return an array of all window IDs.

### `(window/split direction)` → window-id

Split the current window. `direction` is `"h"` (horizontal) or `"v"` (vertical). Returns the new window ID.

### `(window/focus id)` → nil

Focus a specific window. Emits `window-focused`.

### `(window/buffer id)` → slab-key or nil

Return the buffer slab key displayed in window `id`.

### `(window/set-buffer id slab)` → nil

Set the buffer displayed in window `id`.

### `(window/dimensions id)` → `[width height]`

Return the window dimensions in characters.

### `(window/width &opt id)` → integer

Return the width of `id` (or the focused window).

### `(window/height &opt id)` → integer

Return the height of `id` (or the focused window).

### `(window/cursor-row &opt id)` → integer

Return the cursor's visible row (0-based) within the window.

### `(window/cursor-col &opt id)` → integer

Return the cursor's visible column (0-based) within the window.

### `(window/scroll-top &opt id)` → integer

Return the top visible line number in the window.

### `(window/resize id w-frac)` → nil

Set the horizontal layout weight for window `id` to `w-frac` (0.0–1.0).

### `(window/set-scroll-top id top)` → nil

Pin the scroll offset for window `id` to line `top`.

### `(window/unpin-scroll id)` → nil

Release the pinned scroll offset; cursor-following scroll resumes.

---

## magma/ — General Utility Functions

### `(magma/time-now)` → string

Return the current local time as `"YYYY-MM-DD HH:MM:SS"`.

---

## semantic/ — Semantic Engine

### `(semantic/symbols buf-id)` → `[[name kind file line col] …]`

Return live symbols extracted from buffer `buf-id` using its registered language provider.  Returns an empty array if no language is set or no provider is registered.  `kind` is one of `"function"`, `"method"`, `"struct"`, `"enum"`, `"class"`, `"module"`, `"constant"`, `"variable"`, `"interface"`, `"unknown"`.

### `(semantic/definitions name)` → `[[name kind file line col] …]`

Look up all known definitions for the symbol `name` in the persistent symbol index.  Returns an empty array if not found.

### `(semantic/references name)` → `[[name file line col] …]`

Return all known reference sites for the symbol `name` from the index.

### `(semantic/documentation symbol)` → string or nil

Return documentation for `symbol` from the first indexed definition that has documentation, or nil.

### `(semantic/index-buffer buf-id)` → nil

Re-index the buffer's symbols into the symbol index and project graph.  Uses the buffer's tree-sitter language (set via `ts/set-language`) and its registered provider.

### `(semantic/register-provider lang type)` → nil

Register a language provider for `lang`.  `type` is `"lsp"` (default) or `"treesitter"` / `"ts"`.

### `(semantic/diagnostics buf-id)` → `[[line col severity message] …]`

Return typed diagnostics for the buffer's file path.  `severity` is one of `"error"`, `"warning"`, `"info"`, `"hint"`.  Diagnostics are populated from LSP `textDocument/publishDiagnostics` notifications.

---

## debug/ — Debug System (Phase 6)

The debug subsystem speaks the Debug Adapter Protocol (DAP) over a spawned subprocess.  All DAP I/O runs on a background thread; results arrive via events on the main thread.

### `(debug/start adapter)` → session-id

Spawn the debug adapter binary `adapter` (e.g. `"codelldb"`, `"debugpy"`) and perform the DAP `initialize` handshake.  Returns a numeric session ID.  The first session started becomes the *active* session.  Emits `debug-session-started`.

### `(debug/continue &opt thread-id)` → nil

Resume execution in the active session.  Uses the stopped thread from the last `debug-stopped` event if `thread-id` is omitted.

### `(debug/step-in &opt thread-id)` → nil

Step into the next function call.

### `(debug/step-over &opt thread-id)` → nil

Step over the current line (DAP `next` command).

### `(debug/step-out &opt thread-id)` → nil

Step out of the current function frame.

### `(debug/add-breakpoint file line)` → nil

Add a breakpoint at line `line` (1-based) of the source file at absolute path `file`.  The `:breakpoints` GutterProvider reads live from `ed.debug.breakpoints` so the gutter updates automatically.  If an active session exists, sends a DAP `setBreakpoints` request.  Emits `debug-breakpoint-changed` with `:action "add"`.

### `(debug/remove-breakpoint file line)` → nil

Remove the breakpoint at `(file, line)`.  The `:breakpoints` provider updates automatically.  Notifies the active session.  Emits `debug-breakpoint-changed` with `:action "remove"`.

### `(debug/evaluate expr &opt frame-id)` → nil

Evaluate the expression `expr` in the context of frame `frame-id` (defaults to 0).  The result arrives asynchronously via the `debug-evaluate-result` event as `{:session-id :result}`.

### `(debug/session)` → session-id or nil

Return the active session ID, or `nil` if no session is running.

### `(debug/breakpoints file)` → `[line …]`

Return an array of line numbers (integers) for all breakpoints set in `file`.

---

### Debug Events

| Event | Payload keys | When fired |
|-------|-------------|------------|
| `debug-session-started` | `:session-id`, `:adapter` | A new DAP session was started |
| `debug-session-ended` | `:session-id` | The adapter process terminated |
| `debug-stopped` | `:session-id`, `:reason`, `:thread-id` | Debuggee stopped (breakpoint, step, etc.) |
| `debug-output` | `:session-id`, `:category`, `:output` | Console output from the debuggee |
| `debug-evaluate-result` | `:session-id`, `:result` | Evaluate response from the adapter |
| `debug-breakpoint-changed` | `:file`, `:line`, `:action` | Breakpoint added or removed |

---

## workspace/ — Workspace Persistence (Phase 8)

The workspace subsystem saves and restores the full editor state: open buffers,
command history, search pattern, options, registers, and plugin state.

State files live under `~/.magma/workspace/` (or `$MAGMA_DIR/magma/workspace/`).
Named sessions are stored in the `sessions/` subdirectory.

### `(workspace/save)` → `{:status "ok"}` or `{:status "error" :error "…"}`

Snapshot the current editor state and write it to `workspace.json` in the
workspace directory.  Emits `workspace-saved` on success.

### `(workspace/restore)` → `{:status "ok" :count N}` or `{:status "error" :error "…"}`

Read `workspace.json` and reconstruct editor state.  File-backed buffers are
re-read from disk; scratch buffers have their content restored verbatim.
Returns the number of buffers recreated in `:count`.  Emits `workspace-restored`
on success.

### `(workspace/session-save name)` → `{:status "ok"}` or `{:status "error" :error "…"}`

Save the current editor state as named session `name` under `sessions/<name>.json`.
Session names are sanitised (alphanumeric and `-` only); all other characters
become `_`.  Emits `workspace-session-saved`.

### `(workspace/session-load name)` → `{:status "ok" :count N}` or `{:status "error" :error "…"}`

Load the named session `name` and apply it to the editor.  Returns the number
of buffers recreated in `:count`.  Emits `workspace-session-loaded`.

### `(workspace/session-list)` → `["name" …]`

Return an array of all saved session names on disk (without the `.json` suffix).

---

### Workspace Events

| Event | Payload keys | When fired |
|-------|-------------|------------|
| `workspace-saved` | `:path`, `:buffer-count` | Workspace saved to disk |
| `workspace-restored` | `:path`, `:buffer-count` | Workspace restored from disk |
| `workspace-session-saved` | `:name`, `:buffer-count` | Named session saved |
| `workspace-session-loaded` | `:name`, `:buffer-count` | Named session loaded |

### Colon-mode verbs

| Verb | Action |
|------|--------|
| `:ws-save` | Save the workspace |
| `:ws-restore` | Restore the workspace |
| `:ws-session-save <name>` | Save a named session |
| `:ws-session-load <name>` | Load a named session |
| `:ws-sessions` | List saved sessions in the message area |

