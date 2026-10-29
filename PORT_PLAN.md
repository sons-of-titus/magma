# Builtins → Native Port Plan

Direction: the `builtins/` Janet scripts that ship with the editor move to
native Rust over time, for performance and for correctness (Janet copies of
kernel state drift; string re-parsing of kernel-formatted data breaks).  What
remains in Janet is the plugin layer: configuration data, command glue, and
extension code, talking to the kernel through a stable surface.

## The contract

Janet code — first-party or plugin — uses exactly three native surfaces:

1. **C functions** (`buffer/*`, `editor/*`, `event/*`, …) documented in
   `docs/api/janet-api.md`.
2. **The command registry** — keymaps, colon verbs, and palettes resolve
   names through `ed.commands`, regardless of which language registered the
   command.
3. **Events** (`event/on`, typed payloads).

Because the registry resolves by name, a port keeps every command name
stable: register the native command under the Janet command's name, delete
the Janet copy, and keymaps/plugins never notice.  Template: the
`debug/toggle-breakpoint` port (C function over `toggle_breakpoint_at`,
Janet copy deleted, name and `meta-f8` binding unchanged).

What moves native: anything on the keystroke/frame/edit path, anything
O(buffer) per event, anything that re-derives state the kernel already holds
in typed form, and anything that must not depend on init-time scripts.

What stays in Janet: themes, keybinding tables, icon packs, language tables,
which-key/help data, dialog and special-buffer composition, plugin lifecycle
— cold-path configuration and glue.

## Phases

### Phase 0 — done elsewhere

Language registry, plugin system, run-configs went native via
`plugin_api.rs` / `run_config_api.rs`; `treesitter.janet`, `major_modes.janet`
and `language.janet` were rewritten on the plugin-task branch.

### Phase 1 — hot paths (this port)

| Janet | Native home | Notes |
|-------|-------------|-------|
| `multi_cursor.janet` word scan | `cursor-add` path in `visual.rs`, reusing `word_and_next` (search.rs) | Was a whole-document `buffer/slice` copy per ctrl-d press.  `buffer/word-under-cursor` C function replaces the Janet helper for `symbols.janet`. |
| `lsp_diag_eol.janet` severity selection | `diagnostics-apply-eol` command (`diagnostics_cmds.rs`) | Reads typed `Buffer::lsp_diagnostics` instead of re-parsing `"[E] line 12:4 …"` display strings.  Janet keeps the per-buffer enable toggle and the event subscription. |
| `indent_commands.janet` | `indent_cmds.rs` | Tab/Shift-Tab are per-keystroke.  `delete-to-eol` goes native, removing the Janet round-trip inside native `change-to-eol`.  `autoindent-line` has no callers and is deleted. |
| `scroll_commands.janet` | `scroll_cmds.rs` | Viewport math over `Pane::scroll_offset` directly, mirroring the wheel path in `input/mouse.rs`. |
| `gutter.janet` line-number formats | deleted | The native `:line-numbers` provider already renders hybrid relative numbers from the `relativenumber` option; the Janet copies were inert (nothing reads `line_number_fn`). |

### Phase 2 — fallback highlighter

`syntax_core.janet` maintains a Janet-side keyword-highlight range cache
spliced by edit spans on every `buffer-changed` event.  Findings from the
post-plugin-rewrite code:

- The tree-sitter path (`treesitter.janet`) now writes the same `"syntax"`
  layer on `buffer-changed`; both handlers run on every edit and loader
  order decides the winner for languages that have both a ts query and
  keyword tables.  Porting must resolve this ownership first: ts query
  present → fallback skips, else fallback owns the layer.
- Port shape: native keyword/comment/string/number scanners over rope
  chunks (the word-char set includes `/ ? ! -`), a native range cache with
  edit-span splicing mirroring `ts_schedule.rs`, and a
  `syntax/define-rules lang keywords comment-marker` C function so the
  keyword tables stay Janet config (`syntax_langs.janet` /
  `syntax_lang_ext.janet` do not move).
- The scanner itself is mechanical; the ownership rule and the cache are
  the design work.  Sequence after the highlight pipeline has soak time.

### Phase 3 — design conflicts (decide, don't port blindly)

- `collab.janet` re-implements the peer-broadcast layer the kernel deleted on
  purpose (`server/mod.rs`): untransformed byte offsets corrupt documents
  when two writers race.  Either rebuild on the serialized JSON-op channel or
  drop the feature.
- `ssh_fs.janet` writes are broken (spawned `ssh cat > path` is never fed
  stdin).  Needs a fix regardless of language.

### Never port

`init.janet` (theme), `ui.janet`, `icons.janet`, keybinding/which-key/help
tables, `special_buffers.janet`, `workspace.janet`, `task`/`scheduler`/`net`
glue, `janet_eval.janet` (REPL), `project.janet` root-walk and workspace
detection, `grep.janet` (cold path, spawns ripgrep), language config tables.
Porting configuration into Rust is the coupling the architecture rule exists
to prevent.
