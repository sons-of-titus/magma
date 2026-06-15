# Magma Architecture Overview

Magma is a programmable text editor designed as a live runtime environment. It follows a kernel/userland separation model: **Rust is the kernel**, **Janet is the userland**.

## Core Philosophy

```
Magma is not an application with plugins.
It is a live runtime where the editor itself is modifiable while running.
```

- **Rust Core (Kernel)**: Owns state, memory, and lifecycle. Provides deterministic primitives. Never contains user logic.
- **Janet Runtime (Userland)**: All behavior, configuration, extensions. Fully programmable at runtime via REPL and script files.

## System Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                      RENDERER (TUI/GUI)                           │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────────┐   │
│  │ Terminal  │  │  Web     │  │  Native  │  │  ...           │   │
│  │ Widget    │  │  Canvas  │  │  Window  │  │                │   │
│  └────┬──────┘  └────┬─────┘  └────┬─────┘  └───────┬────────┘   │
└───────┼──────────────┼─────────────┼─────────────────┼───────────┘
        │              │             │                  │
┌───────┴──────────────┴─────────────┴──────────────────┴───────────┐
│                        RUST CORE (Kernel)                          │
│                                                                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │
│  │  Buffer  │  │  Window  │  │  Event   │  │   Command         │   │
│  │  System  │  │  System  │  │  Bus     │  │   Registry        │   │
│  ├──────────┤  ├──────────┤  ├──────────┤  ├──────────────────┤   │
│  │  Rope    │  │  Layout  │  │ Pub/Sub  │  │  Dispatch Table   │   │
│  │  Engine  │  │  Tree    │  │ Queue    │  │  Arg Resolution   │   │
│  ├──────────┤  ├──────────┤  ├──────────┤  ├──────────────────┤   │
│  │  Undo    │  │  Split   │  │ Filter   │  │  Built-in Cmds   │   │
│  │  Tree    │  │  Frame   │  │ Chain    │  │                  │   │
│  ├──────────┤  ├──────────┤  ├──────────┤  ├──────────────────┤   │
│  │  Mark    │  │  View    │  │  Tick    │  │  MIDI System     │   │
│  │  System  │  │  Port    │  │  Loop    │  │                  │   │
│  └──────────┘  └──────────┘  └──────────┘  └──────────────────┘   │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                  JANET BRIDGE (C-API FFI)                    │   │
│  │  janet_wrap_* / janet_unwrap_* / janet_scan_* / janet_cfun  │   │
│  └──────────────────────────┬──────────────────────────────────┘   │
└─────────────────────────────┼──────────────────────────────────────┘
                              │
┌─────────────────────────────┴──────────────────────────────────────┐
│                      JANET RUNTIME (User Layer)                     │
│                                                                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │
│  │  REPL    │  │  Plugin  │  │  Config  │  │  Modes Engine    │   │
│  │  Loop    │  │  Loader  │  │  System  │  │  (major/minor)   │   │
│  ├──────────┤  ├──────────┤  ├──────────┤  ├──────────────────┤   │
│  │  Eval    │  │  Hot     │  │  init    │  │  Vim Mode (def)  │   │
│  │  Print   │  │  Reload  │  │  .janet  │  │  Emacs Mode      │   │
│  ├──────────┤  ├──────────┤  ├──────────┤  ├──────────────────┤   │
│  │  History │  │  Dep     │  │  Env     │  │  Scope Stack     │   │
│  │          │  │  Graph   │  │  Vars    │  │                  │   │
│  └──────────┘  └──────────┘  └──────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## Key Principles

### Separation of Concerns

| Layer | Responsibility | Does NOT do |
|-------|---------------|-------------|
| **Rust** | State management, text primitives, event dispatch, rendering abstraction, file I/O | Hardcode behavior, define keybindings, implement editing modes |
| **Janet** | Keybindings, commands, hooks, modes, UI customization, plugin logic | Manage memory, perform text operations directly, control rendering |

### Runtime Properties

- **Live evaluation**: Any Janet expression evaluated in the REPL immediately affects editor state
- **No restart required**: Commands, keybindings, plugins, and UI behavior change without recompilation
- **Full introspection**: All editor objects, commands, hooks, and keymaps are inspectable at runtime
- **Hot reload**: Plugins can be reloaded without restarting; function redefinitions take effect immediately

### Directory Layout

```
~/.magma/
  init.janet          # Entry point configuration
  plugins/
    git.janet         # Git integration plugin
    lsp.janet         # Language server protocol plugin  
    vim.janet         # Vim emulation mode (loaded by default)
    emacs.janet       # Emacs emulation mode (alternative)
    rust.janet        # Rust-specific tooling
  themes/
    dark.janet        # Theme definitions
    light.janet
  user.janet          # User-specific overrides (not in VCS)
```

### Architecture Invariants

1. The Janet VM runs on a single fiber — all event handlers are serialized
2. Editor state is behind `Arc<RwLock<Editor>>` — Janet holds IDs, never pointers
3. No Rust→Janet call ever holds the write lock (prevents deadlock)
4. All cross-boundary values are marshalled through typed IDs
5. The REPL is a regular buffer with special keybindings
6. Plugins are sandboxed in fresh environments
7. A plugin crash never crashes the editor
