# Janet Runtime (User Layer)

## Overview

Janet is the user-facing programming environment of Magma. It serves as:

- **Configuration language** — editor settings, theme, defaults
- **Extension language** — plugins, commands, hooks
- **Scripting language** — automation, composable transformations
- **REPL environment** — live experimentation against editor state

## Embedded Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Rust Process                        │
│  ┌───────────────────────────────────────────────┐   │
│  │            Janet Virtual Machine               │   │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────┐  │   │
│  │  │  Core     │  │  Parser  │  │  Compiler  │  │   │
│  │  │  Runtime  │  │          │  │            │  │   │
│  │  └──────────┘  └──────────┘  └────────────┘  │   │
│  │  ┌────────────────────────────────────────┐    │   │
│  │  │         Janet Environment               │    │   │
│  │  │  - Magma API functions                  │    │   │
│  │  │  - Plugin namespaces                    │    │   │
│  │  │  - Global configuration state           │    │   │
│  │  │  - User-defined functions               │    │   │
│  │  └────────────────────────────────────────┘    │   │
│  └───────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

## Environment Initialization

The Janet environment is initialized in phases:

### Phase 1: Core Bootstrap
- Initialize Janet VM
- Register Rust→Janet bridge functions (`buffer/`, `window/`, `event/`, etc.)
- Set up global constants and editor state references

### Phase 2: Default Configuration
- Load `builtins/init.janet` (embedded default config)
- Set up vim keybindings (default mode)
- Register default hooks and commands

### Phase 3: User Configuration
- Load `~/.magma/init.janet`
- Load all plugins from `~/.magma/plugins/`
- Apply user overrides

### Phase 4: REPL Ready
- Open `*repl*` buffer
- Start REPL event loop
- Emit `:editor-ready` event

## Global Namespace Structure

```
magma/                  # Root namespace for all editor APIs
  buffer/               # Buffer manipulation
  window/               # Window management
  event/                # Pub/sub event system
  command/              # Command registration and dispatch
  keymap/               # Keybinding management
  hook/                 # Hook chain management
  fs/                   # File system operations
  editor/               # Editor-level operations
  repl/                 # REPL management
  render/               # Rendering control
  mode/                 # Editing mode management

math/                   # Standard Janet modules
string/
array/
table/
fiber/
...
```

## REPL

The REPL is a regular buffer named `*repl*` with special behavior:

- **Input**: Type Janet expressions in the buffer
- **Evaluation**: `ctrl-j` or `ctrl-enter` evaluates the expression at cursor
- **Output**: Result is printed below the input
- **History**: Previous expressions are navigable with `ctrl-p`/`ctrl-n`
- **Multi-line**: Incomplete expressions trigger continued input

```
┌──────────────────────────────────────────────┐
│  *repl*                                       │
│  > (+ 1 2 3)                                  │
│  => 6                                         │
│  > (buffer/current)                           │
│  => <buffer:main.rs>                          │
│  > (event/on :cursor-moved                    │
│  |   (fn [ev] (print "moved!")))              │
│  => <subscription:42>                         │
│  > _                                          │
└──────────────────────────────────────────────┘
```

## Plugin System

Plugins are simply Janet files loaded into the runtime:

### Loading

```janet
# Plugin: ~/.magma/plugins/my-plugin.janet
(def- state @{})

(defn my-command []
  (print "Hello from my plugin!"))

(command/define "my-command" my-command)

(event/on :editor-ready
  (fn [&] (print "My plugin loaded!")))
```

### Plugin Lifecycle

```
Discover ──→ Parse ──→ Resolve Deps ──→ Sandbox Eval ──→ Register ──→ Cache
                │                                                        │
                └───────── Hot Reload Path ──────────────────────────────┘
                               │
                    1. Call unload hook
                    2. Remove registrations
                    3. Eval new file
                    4. Re-register
```

### Plugin Registry

```rust
struct PluginRecord {
    name: String,
    path: PathBuf,
    exports: HashMap<String, JanetFunction>,
    commands: Vec<CommandId>,
    hooks: Vec<HookId>,
    keybindings: Vec<(KeySequence, CommandId)>,
    unload_hook: Option<JanetFunction>,
}
```

### Hot Reload

```janet
# Reload a plugin
(editor/reload "my-plugin")

# Watch a file and auto-reload
(fs/watch "~/.magma/plugins/my-plugin.janet"
  (fn [path] (editor/reload "my-plugin")))
```

## Modes System

Magma uses a mode system inspired by both Vim and Emacs:

### Major Modes

Major modes define the primary editing behavior:

- `vim-mode` — Vim emulation (default)
- `emacs-mode` — Emacs-style keybindings
- `none-mode` — Raw editor, no special bindings

### Minor Modes

Minor modes layer on additional behavior:

```janet
(defn markdown-mode []
  (keymap/push-layer :markdown)
  (event/on :buffer-changed markdown-preview))

(defn git-mode []
  (keymap/push-layer :git)
  (event/on :before-save git-pre-commit-check))
```

### Mode Stack

```
Buffer-local ──┐
Minor mode 2 ──┤
Minor mode 1 ──┤
Major mode ────┤
Global ────────┘
                │
                ▼
          Key lookup (first match wins)
```

## Safety and Error Handling

- Each plugin is evaluated in a fresh environment (sandboxed)
- Plugin crashes are caught by Janet's `protect` mechanism
- A crash in one plugin does not affect others or the editor
- Infinite loops are prevented by a 5-second eval timeout
- Memory is managed by Janet's garbage collector
- Rust-side memory is never exposed to Janet directly

## Performance Considerations

- Janet is single-fibered for event processing — handlers must not block
- Long-running operations should spawn new fibers via `(fiber/new ...)`
- Heavy text processing should delegate to Rust primitives
- The REPL eval timeout prevents hung editor on accidental infinite loops
