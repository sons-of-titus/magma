# Getting Started with Magma

## Installation

### Prerequisites

- **Rust** 1.75+ (install via [rustup](https://rustup.rs/))
- **Janet** 1.34+ (optional, embedded version is included)

### Build from Source

```bash
git clone https://github.com/your-org/magma.git
cd magma
cargo build --release
./target/release/magma
```

### Quick Start

```bash
magma                           # Open with empty buffer
magma file.txt                  # Open file
magma file1.txt file2.txt       # Open multiple files
magma +100 file.txt             # Open at line 100
```

## First Launch

When you first launch Magma, you'll see:

```
┌────────────────────────────────────────────┐
│  *repl*                               - □ × │
│                                              │
│  Magma v0.1.0                               │
│  Type (help) for assistance                 │
│  > _                                         │
│                                              │
│                                              │
│                                              │
│  *repl*  [NORMAL]  Ln 3 Col 3              │
└────────────────────────────────────────────┘
```

You start in the REPL buffer in **normal mode** (Vim-style, press `i` to insert, `esc` to go back to normal).

## Basic Usage

### REPL (Read-Eval-Print Loop)

The REPL is your primary interface for live interaction:

```janet
> (+ 1 2 3)
=> 6

> (buffer/current)
=> <buffer:1 *repl*>

> (buffer/create "scratch")
=> <buffer:2 scratch>

> (event/on :cursor-moved
  (fn [ev]
    (print "Moved!")))
=> <subscription:42>
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `i` | Enter insert mode |
| `esc` | Return to normal mode |
| `ctrl-s` | Save buffer |
| `ctrl-o` | Open file |
| `ctrl-p` | Command palette |
| `ctrl-` `` ` `` | Toggle REPL |
| `:w` | Save (in command mode) |
| `:q` | Quit |

## Navigation

### Vim-style Motion

| Key | Action |
|-----|--------|
| `h` `j` `k` `l` | Cursor movement |
| `w` `b` | Word forward/backward |
| `0` `$` | Line start/end |
| `gg` `G` | Buffer start/end |
| `ctrl-d` `ctrl-u` | Half-page scroll |

### Insert Mode

| Key | Action |
|-----|--------|
| `i` | Insert at cursor |
| `a` | Insert after cursor |
| `o` | Open line below |
| `esc` `ctrl-c` | Return to normal mode |

## Basic Editing

```janet
# In insert mode, type normally.
# In normal mode, use Vim keys.

# Delete operations (normal mode)
x      # Delete character
dd     # Delete line
dw     # Delete word

# Yank and paste (normal mode)
yy     # Yank line
p      # Paste after cursor
P      # Paste before cursor

# Undo/Redo
u      # Undo
ctrl-r # Redo
```

## Configuration

Configuration is done through `~/.magma/init.janet`:

```janet
# ~/.magma/init.janet

# Settings
(set theme :dark)
(set tab-width 2)

# Keybindings
(keymap/set "ctrl-s" "save-buffer")

# Hooks
(event/on :buffer-saved
  (fn [ev]
    (print "Saved: " (:path ev))))
```

See the [Configuration Guide](configuration.md) for details.

## Opening Files

```janet
# Command line
magma myfile.txt

# From within Magma (via REPL)
> (command/run "open-file" {:path "myfile.txt"})
=> nil

# Or using the keybinding
# Press ctrl-o in normal mode, then type the path
```

## Saving Files

```janet
# Via keybinding
# Press ctrl-s in any mode

# Via command
> (command/run "save-buffer")
=> nil

# Save as...
> (command/run "save-buffer" {:path "newfile.txt"})
=> nil
```

## Windows

```janet
# Split window horizontally
ctrl-w s

# Split window vertically
ctrl-w v

# Switch windows
ctrl-w w

# Close window
ctrl-w q
```

## Getting Help

```janet
# List all commands
> (command/list)
=> @["save-buffer" "open-file" "undo" ...]

# Describe a command
> (describe command/save-buffer)
=> {:name "save-buffer" :doc "Save current buffer to disk" ...}

# List all events
> (event/list-events)
=> @{:cursor-moved 3 :key-pressed 5 ...}

# View all keybindings
> (keymap/list)
=> {:vim @[...] :global @[...]}
```

## Next Steps

- Read the [Configuration Guide](configuration.md) to customize Magma
- Learn [Plugin Development](plugin-development.md) to extend the editor
- Browse the [API Reference](../api/janet-api.md) for all available functions
- Explore the [Architecture](../architecture/overview.md) docs for deep understanding
