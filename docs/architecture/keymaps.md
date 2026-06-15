# Keymap System Design

## Overview

Magma's keymap system is layered, fully dynamic, and modifiable at runtime. It follows a precedence-based lookup: buffer-local → mode layers → global.

## Layered Architecture

```
┌──────────────────────────────────────────────┐
│              Key Lookup Order                 │
│                                                │
│  ┌────────────────────────────────────────┐   │
│  │  1. Buffer-local keymap                │   │  Highest priority
│  │     (per-buffer overrides)             │   │
│  └────────────────────────────────────────┘   │
│                      ▼                        │
│  ┌────────────────────────────────────────┐   │
│  │  2. Minor mode keymaps (stacked)       │   │
│  │     - markdown-mode                    │   │
│  │     - git-mode                         │   │
│  │     - org-mode                         │   │
│  └────────────────────────────────────────┘   │
│                      ▼                        │
│  ┌────────────────────────────────────────┐   │
│  │  3. Major mode keymap                  │   │
│  │     - vim-mode (default)               │   │
│  │     - emacs-mode                       │   │
│  │     - none-mode                        │   │
│  └────────────────────────────────────────┘   │
│                      ▼                        │
│  ┌────────────────────────────────────────┐   │
│  │  4. Global keymap                      │   │  Lowest priority
│  │     (always available)                 │   │
│  └────────────────────────────────────────┘   │
└──────────────────────────────────────────────────┘
```

First match wins. If no match is found at any layer, the key is inserted (in insert mode) or ignored (in normal mode).

## Key Sequences

Keys are represented as strings using modifier notation:

```
Key Sequence         Meaning
────────────         ───────
"a"                  Literal 'a'
"ctrl-s"             Control + s
"meta-x"             Meta/Alt + x
"shift-enter"        Shift + Enter
"ctrl-x ctrl-s"      Two-key chord
"g g"                Two sequential 'g' presses
"ctrl-x ctrl-f"      Multi-key sequence
"esc"                Escape
"tab"                Tab
"backspace"          Backspace
"return"             Enter
"space"              Space
"up"                 Up arrow
"down"               Down arrow
"left"               Left arrow
"right"              Right arrow
"home"               Home
"end"                End
"page-up"            Page Up
"page-down"          Page Down
"f1" ... "f12"       Function keys
```

## Default Vim Keybindings

Since Magma defaults to vim-mode, the following keymaps are preloaded:

### Normal Mode (default)

```
Key                  Action
───                  ──────
"h"                  cursor-left
"j"                  cursor-down
"k"                  cursor-up
"l"                  cursor-right
"w"                  word-forward
"b"                  word-backward
"e"                  word-end-forward
"0"                  line-start
"^"                  line-first-nonblank
"$"                  line-end
"ctrl-d"             page-down-half
"ctrl-u"             page-up-half
"ctrl-f"             page-down
"ctrl-b"             page-up
"gg"                 buffer-start
"G"                  buffer-end
"i"                  enter-insert-mode
"I"                  enter-insert-mode-line-start
"a"                  enter-insert-mode-append
"A"                  enter-insert-mode-line-end
"o"                  open-line-below
"O"                  open-line-above
"x"                  delete-char-forward
"X"                  delete-char-backward
"dd"                 delete-line
"dw"                 delete-word
"d$"                 delete-to-line-end
"d0"                 delete-to-line-start
"yy"                 yank-line
"yw"                 yank-word
"p"                  paste-after
"P"                  paste-before
"u"                  undo
"ctrl-r"             redo
"."                  repeat-last-change
"/"                  search-forward
"?"                  search-backward
"n"                  search-next
"N"                  search-prev
"v"                  enter-visual-mode
"V"                  enter-visual-line-mode
"ctrl-v"             enter-visual-block-mode
"ctrl-s"             save-buffer
"ctrl-w s"           split-window-h
"ctrl-w v"           split-window-v
"ctrl-w w"           window-next
"ctrl-w q"           close-window
"dd"                 delete-line
"yy"                 yank-line
"ctrl-p"             command-palette
"ctrl-`"             repl-toggle
"esc"                escape
```

### Insert Mode

```
Key                  Action
───                  ──────
"ctrl-h"             backspace
"ctrl-w"             delete-word-backward
"ctrl-u"             delete-to-line-start
"ctrl-j"             newline
"ctrl-s"             save-buffer
"ctrl-c"             exit-insert-mode
"esc"                exit-insert-mode
```

### Visual Mode

```
Key                  Action
───                  ──────
"h/j/k/l"            extend-selection-direction
"w/b/e"              extend-selection-word
"d"                  delete-selection
"x"                  delete-selection
"y"                  yank-selection
"p"                  replace-selection
">"                  indent-right
"<"                  indent-left
"u"                  lowercase-selection
"U"                  uppercase-selection
"esc"                exit-visual-mode
```

## Janet API

### Setting Keybindings

```janet
# Set a global keybinding
(keymap/set "ctrl-s" "save-buffer")

# Set a keybinding in a specific layer
(keymap/set "ctrl-s" "save-buffer" :global)           # explicit global
(keymap/set "ctrl-s" "save-buffer" :markdown)          # minor mode layer
(keymap/set "ctrl-s" "save-buffer" :vim)               # major mode layer
(keymap/set "ctrl-s" "save-buffer" (buffer/current))   # buffer-local
```

### Unsetting Keybindings

```janet
(keymap/unset "ctrl-s")
(keymap/unset "ctrl-s" :markdown)
```

### Layer Management

```janet
# Push a new layer onto the stack
(keymap/push-layer :markdown)
=> nil

# Pop a layer from the stack
(keymap/pop-layer :markdown)
=> nil

# List all active layers
(keymap/list-layers)
=> @[:vim :markdown :git]
```

### Inspection

```janet
# List all keybindings in a layer
(keymap/list :global)
=> @[{"ctrl-s" "save-buffer"} {"ctrl-o" "open-file"}]

# List all keybindings across all layers
(keymap/list)
=> {:global @[...] :vim @[...] :markdown @[...]}

# Describe what a key sequence does
(keymap/describe "ctrl-s")
=> "save-buffer"

# Lookup with full layer resolution
(keymap/describe "ctrl-s")
=> "save-buffer"
```

## Vim Mode as a Janet Plugin

Vim mode is implemented entirely in Janet, loaded by default from `builtins/init.janet`:

```janet
# Default vim-mode setup (simplified)
(def- mode-stack @[])
(def- mode :normal)

(defn enter-insert-mode []
  (set mode :insert)
  (keymap/push-layer :insert))

(defn exit-insert-mode []
  (set mode :normal)
  (keymap/pop-layer :insert))

# Register commands
(command/define "enter-insert-mode" enter-insert-mode {})
(command/define "exit-insert-mode" exit-insert-mode {})

# Normal mode keybindings (partial)
(keymap/set "h" "cursor-left" :vim)
(keymap/set "j" "cursor-down" :vim)
(keymap/set "k" "cursor-up" :vim)
(keymap/set "l" "cursor-right" :vim)
(keymap/set "i" "enter-insert-mode" :vim)
(keymap/set "I" "enter-insert-mode-line-start" :vim)
(keymap/set "a" "enter-insert-mode-append" :vim)
(keymap/set "x" "delete-char-forward" :vim)
(keymap/set "u" "undo" :vim)

# Insert mode keybindings
(keymap/set "esc" "exit-insert-mode" :insert)
(keymap/set "ctrl-c" "exit-insert-mode" :insert)
```

Users can override or extend:

```janet
# In ~/.magma/init.janet
(keymap/set "ctrl-s" "save-buffer")        # Works in all modes

# Override vim's jk to exit insert mode
(keymap/unset "jk" :insert)
(keymap/set "jk" "exit-insert-mode" :insert)

# Custom leader key
(keymap/set "space space" "command-palette" :vim)
(keymap/set "space f" "open-file" :vim)
(keymap/set "space b" "buffer-next" :vim)

# Disable vim mode entirely
(mode/set-major :none)
```

## Key Event Processing

When a key is pressed:

```
Key Press
    │
    ▼
[Key Sequence Buffer] ←── Accumulate keys into sequence buffer
    │
    ├── Prefix of any binding? → Wait for more keys (timeout: 1s)
    │
    ├── Exact match? → Execute command
    │
    └── No match? → 
         ├── Insert mode → Insert character
         └── Normal mode → Ignore (or beep)
```

### Timeout

If a key sequence is a prefix of a longer binding, the editor waits (configurable timeout, default 1 second) for the next key. If no key arrives within the timeout, the buffered keys are processed individually.
