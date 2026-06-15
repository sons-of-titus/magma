# Configuration Guide

## Overview

All Magma configuration is done via Janet files. The primary entry point is `~/.magma/init.janet`. There is no separate config format — everything is code.

## Directory Structure

```
~/.magma/
  init.janet              # Main configuration (loaded first)
  user.janet              # User overrides (loaded last)
  plugins/
    *.janet               # Plugin files
  themes/
    *.janet               # Theme definitions
```

## Configuration File

### Settings

```janet
# ~/.magma/init.janet

# Editor settings using a config table
(def- config
  {:theme :dark            # :dark, :light, :custom
   :tab-width 4            # Width of a tab character
   :indent-width 4         # Width of one indent level
   :expand-tab false       # true = spaces, false = tabs
   :cursor-style :block    # :block, :bar, :underline
   :line-numbers true      # Show line numbers
   :scrolloff 5            # Context lines around cursor
   :undo-limit 10000       # Max undo operations
   :clipboard :system      # :system, :internal, :none
   :encoding :utf-8
   :font-size 14})

# Access settings
(print config:theme)       # => :dark
```

## Keybindings

### Global Keybindings

```janet
# Bind a key sequence to a command
(keymap/set "ctrl-s" "save-buffer")
(keymap/set "ctrl-o" "open-file")
(keymap/set "ctrl-z" "undo")

# Bind a multi-key sequence
(keymap/set "ctrl-x ctrl-s" "save-buffer")
(keymap/set "ctrl-x ctrl-c" "quit")
```

### Mode-Specific Keybindings

```janet
# Vim normal mode
(keymap/set "space f" "open-file" :vim)
(keymap/set "space s" "save-buffer" :vim)

# Insert mode
(keymap/set "jk" "exit-insert-mode" :insert)
(keymap/set "ctrl-s" "save-buffer" :insert)

# Visual mode
(keymap/set "ctrl-c" "yank-selection" :visual)
```

### Buffer-Local Keybindings

```janet
# Bind only for a specific buffer
(keymap/set "ctrl-enter" "eval-buffer" (buffer/current))
```

### Unsetting Keybindings

```janet
# Remove a keybinding
(keymap/unset "ctrl-s")

# Remove from specific layer
(keymap/unset "ctrl-s" :vim)
```

## Hooks

### Event Hooks

```janet
# Run code before saving
(event/on :before-save
  (fn [ev]
    (def buf (:buffer-id ev))
    (when (buffer/modified? buf)
      (trim-trailing-whitespace buf))))

# Run code after loading a file
(event/on :buffer-created
  (fn [ev]
    (print "Created: " (:name ev))))

# Observe cursor movement
(event/on :cursor-moved
  (fn [ev]
    (update-status-line (:window-id ev))))
```

### Hook Chains

```janet
# Before hooks (can modify event data)
(hook/add :before-save :ensure-newline
  (fn [ev]
    (def buf (:buffer-id ev))
    (ensure-trailing-newline buf)
    ev))  # Return event for chain

# After hooks (read-only)
(hook/add :after-save :notify
  (fn [ev]
    (print "Saved: " (:path ev))))
```

## Custom Commands

### Simple Commands

```janet
(command/define "hello"
  (fn [& args]
    (print "Hello from Magma!"))
  {:doc "Print a greeting"})
```

### Commands with Arguments

```janet
(command/define "insert-date"
  (fn [& args]
    (def buf (or (get args :buffer) (buffer/current)))
    (def date (os/date))
    (buffer/insert buf (buffer/cursor buf) date))
  {:args [{:name :buffer :type :buffer :optional true}]
   :doc "Insert the current date at cursor"})
```

## Themes

### Built-in Themes

```janet
# Set built-in theme
(set theme :dark)
(set theme :light)
```

### Custom Themes

```janet
# ~/.magma/themes/solarized.janet

(def theme/solarized
  {:background "#002b36"
   :foreground "#839496"
   :cursor "#93a1a1"
   :selection "#073642"
   :line-numbers "#586e75"
   :comment "#586e75"
   :string "#2aa198"
   :keyword "#859900"
   :function "#b58900"
   :type "#268bd2"
   :constant "#d33682"
   :error "#dc322f"
   :statusline {:background "#073642"
                :foreground "#93a1a1"}}})

# Apply it
(editor/theme :solarized)
```

## Modes

### Major Modes

```janet
# Switch to Emacs mode
(mode/set-major :emacs)

# Switch to no mode (raw editor)
(mode/set-major :none)

# Current major mode
(print (mode/major))
```

### Minor Modes

```janet
# Enable minor mode
(mode/enable-minor :markdown)

# Disable minor mode
(mode/disable-minor :markdown)

# List active minor modes
(mode/minors)
```

## Plugins

### Loading Plugins

```janet
# Import from plugins directory
(import plugins/git :as git)
(import plugins/lsp :as lsp)
(import plugins/snippets :as snippets)
```

### Conditional Loading

```janet
# Only load for certain file types
(when (= (buffer/extension) ".rs")
  (import plugins/rust :as rust))

# Only load if feature is available
(when (try (require plugins/docker) true (fn [] false))
  (print "Docker plugin loaded"))
```

## Complete Example

```janet
# ~/.magma/init.janet

# ── Settings ──
(def- config
  {:theme :dark
   :tab-width 2
   :indent-width 2
   :expand-tab true
   :line-numbers true
   :scrolloff 3})

# ── Plugins ──
(import plugins/lsp :as lsp)
(import plugins/git :as git)
(import plugins/snippets :as snippets)

# ── Keybindings ──
# Leader key
(keymap/set "space space" "command-palette" :vim)
(keymap/set "space f" "open-file" :vim)
(keymap/set "space b" "buffer-next" :vim)
(keymap/set "space s" "save-buffer" :vim)

# Insert mode helpers
(keymap/set "jk" "exit-insert-mode" :insert)
(keymap/set "ctrl-s" "save-buffer" :insert)

# ── Hooks ──
(event/on :before-save
  (fn [ev]
    (trim-trailing-whitespace (:buffer-id ev))))

(event/on :cursor-moved
  (fn [ev]
    (render/status-line
      (string/format " %s  Ln %d"
        (buffer/name (:buffer-id ev))
        (+ (window/cursor-row (:window-id ev)) 1)))))

# ── Commands ──
(command/define "repl-toggle"
  (fn [& args]
    (def repl-buf (find-repl-buffer))
    (if repl-buf
      (window/set-buffer (window/current) repl-buf)
      (repl/start)))
  {:doc "Toggle REPL buffer"})
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MAGMA_HOME` | `~/.magma` | Magma configuration directory |
| `MAGMA_THEME` | `dark` | Default theme |
| `MAGMA_PLUGINS` | `~/.magma/plugins` | Plugin directory |

## Tips

1. **Use `def-`** for module-private state to avoid polluting the global namespace
2. **Add `unload-hooks`** to plugins that use timers or file watchers
3. **Test in the REPL** before adding to init.janet
4. **Keep init.janet small** and move complex logic to plugins
5. **Use `print`** for debugging — output appears in the REPL or status line
