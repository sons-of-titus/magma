# Rust Core (Kernel Layer)

## Module Breakdown

```
magma-core/
├── lib.rs                    # Module exports + global initialization
├── state/
│   ├── mod.rs                # Editor struct (single source of truth)
│   └── id.rs                 # Typed IDs: BufferId, WindowId, CommandId, MarkId
├── buffer/
│   ├── mod.rs                # Buffer struct + public mutation API
│   ├── rope.rs               # Rope (B-tree of string chunks)
│   ├── undo.rs               # UndoTree (grouped operations, tree structure)
│   └── marks.rs              # Mark (sticky/persistent cursor positions)
├── window/
│   ├── mod.rs                # Window, Viewport, Cursor
│   └── layout.rs             # LayoutTree (splits, ratios, frame management)
├── event/
│   ├── mod.rs                # EventBus, Subscription, EventId
│   ├── queue.rs              # Lock-free MPSC queue + serialization
│   └── hook.rs               # Hook chain (before/transform/after)
├── command/
│   ├── mod.rs                # CommandRegistry, Command trait
│   ├── builtin.rs            # Core built-in commands
│   └── args.rs               # ArgSpec, ArgValue (typed argument system)
├── keymap/
│   ├── mod.rs                # KeymapManager, KeySequence, KeyBinding
│   └── layer.rs              # Layered lookup (global → mode → buffer-local)
├── fs/
│   ├── mod.rs                # FileSystem trait (read/write/watch)
│   └── disk.rs               # Native filesystem implementation
├── render/
│   ├── mod.rs                # RenderTrait (draw, dimension, polling)
│   └── surface.rs            # Surface (2D character grid abstraction)
├── lsp/
│   └── mod.rs                # LspBridge trait (optional, pluggable)
├── janet_bridge/
│   ├── mod.rs                # init_janet(), register_janet_types()
│   ├── buffer_api.rs         # Janet functions → buffer system
│   ├── event_api.rs          # Janet functions → event bus
│   ├── command_api.rs        # Janet functions → command registry
│   ├── keymap_api.rs         # Janet functions → keymap manager
│   ├── window_api.rs         # Janet functions → window system
│   └── types.rs              # Janet<->Rust type marshalling
└── util/
    ├── id_pool.rs            # Reusable ID allocator
    └── path.rs               # Normalized path helper
```

## Editor State

The `Editor` struct is the single source of truth for all editor state:

```rust
pub struct Editor {
    pub buffers: Slab<Buffer>,
    pub windows: WindowTree,
    pub events: EventBus,
    pub commands: CommandRegistry,
    pub keymaps: KeymapManager,
    pub plugins: PluginRegistry,
    pub fs: Box<dyn FileSystem>,
    pub renderer: Option<Box<dyn RenderTrait>>,
    pub running: bool,
}
```

All access is guarded by `Arc<RwLock<Editor>>`. The lock hierarchy is:

1. **Janet eval fiber** — no lock held (sequential)
2. **Buffer operations** — read lock on editor, write lock on specific buffer
3. **Save/sync operations** — brief write lock on editor
4. **Event dispatch** — read lock on editor

## Subsystem Responsibilities

### Buffer System
- Manages text content via rope data structure
- Owns undo/redo history
- Manages marks (persistent cursor positions)
- Exposes safe mutation API: `insert`, `delete`, `replace`
- Never exposes raw strings as primary model — always slices

### Window System
- Manages layout tree (horizontal/vertical splits)
- Maps windows to buffers via `BufferId`
- Handles viewport calculations (scroll offset, visible range)
- Supports arbitrary nesting of split frames

### Event Bus
- Central pub/sub event dispatch
- Lock-free MPSC ingress queue
- Serialized processing on single fiber
- Before/transform/after hook chains

### Command Registry
- Maps command names to implementations
- Supports both Rust and Janet commands
- Typed argument system with optional args and defaults
- Interactive prompting for missing required args

### Keymap Manager
- Layered keymap system: global → mode → buffer-local
- Supports key sequence chaining (e.g., `ctrl-x ctrl-s`)
- Fully mutable at runtime from Janet

### File System
- Trait-based abstraction for I/O
- Default disk implementation
- Optional file watching for hot reload triggers

### Renderer
- Trait-based rendering abstraction
- Supports TUI (via ratatui or similar) and GUI backends
- Surface abstraction: 2D character grid with styling
- No UI logic in core — only draw commands

## Janet Bridge

The bridge connects Rust and Janet via Janet's C-API:

```rust
// Initialization
pub fn init_janet(editor: Arc<RwLock<Editor>>) -> JanetVM {
    janet_init();
    register_types();
    register_functions();
    register_globals();
    load_init_janet();
}

// Type marshalling
// Rust types are wrapped as Janet abstract types
// IDs are passed as Janet integers
// Callbacks are registered as Janet functions
```

### Bridge Safety Rules

1. **No pointers across boundary** — all editor references are typed IDs
2. **Janet holds GC roots** for any objects that must survive across calls
3. **All cross-boundary calls** hold at minimum a read lock on the editor
4. **No Janet callback executes** while the write lock is held
5. **Deleted objects** return nil with an error message
6. **Panic safety** — Janet `protect` wrappers catch Rust panics

## Built-in Commands

The Rust core provides only essential built-in commands:

- `save-buffer` — Save buffer to disk
- `open-file` — Open file into buffer
- `quit` — Exit the editor
- `eval` — Evaluate Janet expression
- `buffer-next` — Switch to next buffer
- `buffer-prev` — Switch to previous buffer
- `split-window-h` — Split window horizontally
- `split-window-v` — Split window vertically
- `close-window` — Close current window
- `scroll-up` — Scroll window up
- `scroll-down` — Scroll window down

All other behavior comes from Janet.

## Rendering Abstraction

```rust
pub trait RenderTrait {
    fn draw(&mut self, surface: &Surface);
    fn poll_event(&mut self) -> Option<InputEvent>;
    fn dimensions(&self) -> (u16, u16);  // width, height
    fn set_title(&mut self, title: &str);
    fn close(&mut self);
}

pub struct Surface {
    pub width: u16,
    pub height: u16,
    cells: Vec<Cell>,
}

pub struct Cell {
    pub char: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}
```

The renderer receives a `Surface` and draws it. No UI logic exists in the core — the rendering layer is purely a display mechanism.
