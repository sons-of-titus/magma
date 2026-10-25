# Getting Started with Magma

## Installation

### Prerequisites

- **Rust** 1.85+ (install via [rustup](https://rustup.rs)); Janet is embedded — no separate install needed.
- `rg` (ripgrep) on your PATH for project-wide search.

### Build from Source

```bash
git clone <your-fork>
cd magma
cargo build --release            # default features: janet + gui
./target/release/magma
```

Headless modes exist for scripting and remote use: `magma --server 127.0.0.1:7890`
and `magma --connect 127.0.0.1:7890`.

## First Launch

Magma opens a **welcome window**: *Open Folder*, your recent projects, and
the core keys.  Opening a project (button, recent entry, or `open-project`
from the palette) **opens the main editor window** — a separate window with
the project tree, tabs, and tool windows — while the welcome window hides.
Quitting the editor returns to the welcome window; closing the welcome
window exits the app.

## Editing

Magma is **not modal**: what you type goes into the buffer, always.  There
are no normal/insert modes and no vim semantics in the shipped editor —
those are left to community extensions, which the mode/keymap substrate
supports.

- Type to insert; `backspace`/`delete` edit; `esc` clears the selection.
- `shift` + movement selects; `ctrl-c`/`ctrl-x` copy/cut (the line when
  nothing is selected); `ctrl-v` pastes; `ctrl-a` selects all.
- `ctrl-z` / `ctrl-shift-z` undo and redo; `ctrl-s` saves.

## The keys that matter

| Key | Action |
|-----|--------|
| `ctrl-shift-n` | Go to file |
| `ctrl-shift-o` | Go to symbol |
| `ctrl-space` | Completion at any prefix; also pops up automatically after ~2 typed characters |
| `↑`/`↓`, `tab`/`return`, `esc` | While the completion popup is open: cycle, accept, dismiss |
| `ctrl-]` | Go to definition (tree-sitter, LSP fallback) |
| `ctrl-shift-f` | Search open buffers (`n`/`N` walk the matches) |
| `ctrl-alt-s` | Settings |
| `ctrl-\` | Toggle the left dock |
| `ctrl-q` | Quit — refuses while buffers have unsaved changes |

## Projects and the dev loop

1. **Open** a folder (welcome screen or the `open-project` command).
2. **Edit** — the sidebar opens files on a single click; right-click rows
   for new file / rename / delete / copy path.
3. **Build**: run `build`, `test`, or `run` from the palette — output lands
   in the **Output** tool window (left tool strip); parsed errors appear
   under PROBLEMS.
4. **Jump**: click a problem row (or run `quickfix-next` / `quickfix-prev`)
   to open the file at that line.  `ctrl-shift-f` searches the whole project
   via ripgrep.
5. **Version control**: click the branch in the status bar or run `vc` for
   the status buffer.

## Windows, tabs, terminal, debugger

- Tabs along the top; drag to reorder; closing the last tab lands on a fresh
  scratch buffer.
- Splits: run `window-split` / `window-vsplit` from the palette;
  `window-wider` / `window-narrower` resize the focused pane.
- `terminal-start` opens a terminal session in the docked **Terminal**
  panel — never a buffer.  Each session gets a tab (+ for a new one, ✕ to
  close); click a session to type into it, Esc returns to the editor.
  Sessions keep running while hidden and stay in the dock after they exit
  until closed.
- `:dbg-start`-style debugging is available through the debug API; the
  **Debug** tool window shows threads, the call stack (click to jump), and
  variables at the stopped point.

## Commands — where `:` went

There is no command line in the shipped editor.  Use Go to File
(`ctrl-shift-n`) and Go to Symbol (`ctrl-shift-o`) to jump around, and the
Settings dialog (`ctrl-alt-s`) for options.  Everything else the registry
exposes — `build`, `test`, `run`, `grep`, `messages`, `help`, `colorscheme`,
`tags`, `open-project` — is reachable by name from the menu bar or `:`.

## Configuration

Configuration is Janet code in `~/.config/magma/init.janet` — there is no
separate config format:

```janet
# ~/.config/magma/init.janet
(option/set "scrolloff" "5")          # any option (docs/guide/options.md)
(option/set "auto-save" "true")       # opt-in; default off

(keymap/set "ctrl-e" "buffer-next" :editor)

(event/on "buffer-after-save"
  (fn [data] (editor/echo "saved!")))
```

Every user-facing option is catalogued with types and defaults in
[Options](options.md).

## Extending: modal editing and beyond

The core ships the substrate a modal-editing extension needs —
`editor/set-mode`, keymap layers, the selection and register APIs — but no
modal behavior.  A vim layer is a plugin: define commands, bind keys in your
own layer, and manage modes with `editor/set-mode`.

## Getting Help

- Open Settings (`ctrl-alt-s`) to browse every typed option, or the menu bar for commands.
- `help` opens the help buffer.
- The full Janet API reference lives in [docs/api/janet-api.md](../api/janet-api.md).

## Next Steps

- [Configuration Guide](configuration.md) — options, faces, themes
- [Options reference](options.md) — every option, typed
- [GUI architecture](../architecture/gui-architecture.md) — how the chrome is built
- [Architecture overview](../architecture/overview.md) — the Rust kernel and Janet userland
