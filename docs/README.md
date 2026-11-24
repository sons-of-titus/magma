# Magma

**A programmable text editor — Rust kernel, Janet userland, live at runtime.**

Magma is not an application with plugins. It is a live runtime environment where the editor itself is modifiable while running. Janet is the "user OS layer", Rust is the "kernel".

## Design

```
┌──────────────────────────────────────────┐
│            RENDERER (GUI)                 │
├──────────────────────────────────────────┤
│            RUST CORE (Kernel)             │
│  Buffer  Window  Events  Commands  Keys  │
├──────────────────────────────────────────┤
│          JANET RUNTIME (User)             │
│  Scripting  Plugins  Config  Modes  Keys │
└──────────────────────────────────────────┘
```

**Rust** = state + primitives + safety. **Janet** = behavior + configuration + composition. Never hardcode editor behavior in Rust. Never push UI logic into Rust core.

## Quick Start

See [Building](building.md) for per-OS prerequisites, then:

```bash
cargo xtask grammars --all   # syntax-highlighting libraries (not committed)
cargo build --release
./target/release/magma
```

A welcome window opens; pick a folder and the main editor window follows.

Open a folder from the welcome screen (or `:open <dir>`); `ctrl-shift-n` jumps
to a file, `ctrl-alt-s` opens Settings, and `:` still runs commands.  See
[Getting Started](guide/getting-started.md).

## Documentation

```
docs/
├── README.md                          # This file
├── architecture/
│   ├── overview.md                    # High-level architecture
│   ├── gui-architecture.md            # GUI chrome and shell
│   ├── rust-core.md                   # Rust kernel layer
│   ├── concurrency.md                 # Background runtime
│   └── core-redesign.md               # Transaction/revision engine
├── api/
│   └── janet-api.md                   # Full Janet API reference
├── guide/
    ├── getting-started.md             # First steps
    ├── configuration.md               # Configuration guide
    ├── options.md                     # Every option, typed
    └── plugin-development.md          # Plugin development guide
```

## Key Features

- **Native GUI IDE shell** — project tree, tabs, dialogs, tool windows, quickfix
- **Non-modal editing** — text always inserts; modal editing (vim and friends) is community-extension territory, with the mode/keymap substrate built in
- **Navigate fast** — Go to File / Go to Symbol (`ctrl-shift-n` / `ctrl-shift-o`) and a project tree
- **Safe by default** — guarded quit, opt-in auto-save, trash-conscious deletes
- **Live programmability** — commands, keybindings, and behavior change at runtime
- **Event-driven** — central event bus with hook chains
- **Rope-based buffers** — efficient text operations (O(log n))
- **Layered keymaps** — global → mode → buffer-local precedence

## Philosophy

| Rust (Kernel) | Janet (Userland) |
|---|---|
| State management | Behavior definition |
| Text primitives | Configuration |
| Memory safety | Extension logic |
| Event dispatch | Mode implementation |
| Rendering abstraction | UI customization |

## License

MIT
