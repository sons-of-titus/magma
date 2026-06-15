# Magma

**A programmable text editor — Rust kernel, Janet userland, live at runtime.**

Magma is not an application with plugins. It is a live runtime environment where the editor itself is modifiable while running. Janet is the "user OS layer", Rust is the "kernel".

## Design

```
┌──────────────────────────────────────────┐
│            RENDERER (TUI/GUI)             │
├──────────────────────────────────────────┤
│            RUST CORE (Kernel)             │
│  Buffer  Window  Events  Commands  Keys  │
├──────────────────────────────────────────┤
│          JANET RUNTIME (User)             │
│  REPL  Plugins  Config  Modes  Keymaps   │
└──────────────────────────────────────────┘
```

**Rust** = state + primitives + safety. **Janet** = behavior + configuration + composition. Never hardcode editor behavior in Rust. Never push UI logic into Rust core.

## Quick Start

```bash
cargo build --release
./target/release/magma
```

## Documentation

```
docs/
├── README.md                          # This file
├── architecture/
│   ├── overview.md                    # High-level architecture
│   ├── rust-core.md                   # Rust kernel layer
│   ├── janet-runtime.md               # Janet user layer
│   ├── events.md                      # Event system
│   ├── commands.md                    # Command system
│   ├── buffers.md                     # Buffer model (rope-based)
│   ├── keymaps.md                     # Keymap system (vim defaults)
│   ├── plugins.md                     # Plugin system
│   ├── repl.md                        # REPL architecture
│   └── memory.md                      # Memory ownership rules
├── api/
│   └── janet-api.md                   # Full Janet API reference
├── examples/
│   ├── init.janet                     # Example configuration
│   └── vim.janet                      # Vim keymap implementation
└── guide/
    ├── getting-started.md             # First steps
    ├── configuration.md               # Configuration guide
    └── plugin-development.md          # Plugin development guide
```

## Key Features

- **Vim keymaps by default** — fully configurable, complete Vim emulation in Janet
- **Live REPL** — evaluate Janet expressions that immediately affect editor state
- **No restart required** — commands, keybindings, plugins change at runtime
- **Full introspection** — all editor state is inspectable at runtime
- **Hot reload** — reload plugins without restarting
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
