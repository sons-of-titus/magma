# Memory Ownership Rules

## Ownership Hierarchy

```
┌─────────────────────────────────────────────────────────┐
│                    EDITOR (Singleton)                     │
│  Arc<RwLock<Editor>>                                     │
│                                                          │
│  ├── BufferSlab (Slab<Buffer>)                           │
│  │    ├── Rope (owns all text Leaf chunks)               │
│  │    ├── UndoTree (owns undo operations)                │
│  │    └── Marks (owned Vec<Mark>)                        │
│  │                                                       │
│  ├── WindowTree (owns layout tree)                       │
│  │    └── Windows (own viewport state)                   │
│  │                                                       │
│  ├── EventBus (owns subscriber list)                     │
│  │    └── Janet GC roots for callback functions          │
│  │                                                       │
│  ├── CommandRegistry (owns command table)                 │
│  │    ├── Rust command implementations                   │
│  │    └── Janet command function references               │
│  │                                                       │
│  ├── KeymapManager (owns layered keymaps)                │
│  │    └── Key bindings (key-seq → command-name)          │
│  │                                                       │
│  ├── PluginRegistry (owns plugin records)                │
│  │    └── Plugin environments + metadata                 │
│  │                                                       │
│  └── Renderer (trait object, owned)                      │
└─────────────────────────────────────────────────────────┘
```

## Core Rule: IDs, Not Pointers

**No reference crosses the Janet↔Rust boundary.** All editor objects are identified by typed numeric IDs:

| Object Type | ID Type | Example |
|-------------|---------|---------|
| Buffer | `BufferId(u64)` | `42` |
| Window | `WindowId(u64)` | `7` |
| Command | `CommandId(u64)` | `12` |
| Mark | `MarkId(u64)` | `3` |
| Subscription | `SubscriptionId(u64)` | `55` |
| Hook | `HookId(u64)` | `8` |

```rust
pub struct BufferId(pub u64);
pub struct WindowId(pub u64);
pub struct CommandId(pub u64);
pub struct MarkId(pub u64);
pub struct SubscriptionId(pub u64);
pub struct HookId(pub u64);
```

## Locking Rules

### Lock Hierarchy

```
WriteLock(Editor)
    │
    ▼
ReadLock(Editor)
    │
    ▼
ReadLock(Buffer) ◄── Buffer-level operations
```

### Invariants

1. **Janet main fiber holds no locks by default** — all event processing is sequential
2. **Read lock is sufficient for most operations** — viewing, inspecting, navigating
3. **Write lock is required for mutations** — editing text, changing windows
4. **Write lock is released before calling Janet callbacks** — prevents ABA/deadlock
5. **Buffer-level read lock** — used when iterating buffer content for rendering

### Deadlock Prevention

```rust
// SAFE: No lock held, calling into Janet
fn process_key(editor: &Arc<RwLock<Editor>>, key: KeyEvent) {
    let cmd_name = {
        let reader = editor.read().unwrap();
        reader.keymaps.resolve(&key)
    };

    if let Some(cmd) = cmd_name {
        // Release read lock before executing (command may call Janet)
        editor.read().unwrap().commands.run(&cmd);
    }
}

// UNSAFE: Would deadlock if command tries to read editor
fn wrong_process_key(editor: &Arc<RwLock<Editor>>, key: KeyEvent) {
    let reader = editor.read().unwrap();
    let cmd = reader.keymaps.resolve(&key);
    reader.commands.run(&cmd);  // DEADLOCK: reader held during Janet call
}
```

## Janet GC Integration

### GC Roots

Janet's garbage collector must know about references held by Rust:

```rust
// Objects that Janet can reference must be rooted:
pub struct JanetBridge {
    // GC roots for registered callbacks
    event_handlers: Vec<JanetFunction>,
    command_functions: Vec<JanetFunction>,
    hook_functions: Vec<JanetFunction>,

    // The shared Janet environment
    env: *mut JanetTable,  // Boxed, Janet-managed
}
```

### Reference Counting

Objects shared between Rust and Janet use `Arc` for ownership:

```rust
pub struct SharedBuffer {
    inner: Arc<RwLock<BufferInner>>,
}

impl JanetBridge {
    fn buffer_get(env: &mut Env, buf_id: BufferId) -> Option<SharedBuffer> {
        // Returns an Arc-wrapped reference that Janet can hold
        env.editor.buffers.get(buf_id).map(|b| b.shared())
    }
}
```

## Lifetime Rules by Subsystem

### Buffer System

- `Editor` owns all `Buffer`s in a `Slab<Buffer>` (contiguous, indexable storage)
- `Slab` provides O(1) access by `BufferId`
- Deleting a buffer frees its rope, undo tree, and marks
- `BufferId` reuse is prevented by generation counters

### Window System

- `WindowTree` owns the tree of `Window` nodes
- Each `Window` holds a `BufferId` (not a reference)
- Closing a window does not delete its buffer
- `WindowId` uses the same slab mechanism with generation counters

### Event System

- `EventBus` owns subscriber entries containing Janet function references
- Subscriber functions are rooted in Janet's GC
- Removing a subscription unroots the function, allowing GC

### Command System

- `CommandRegistry` owns command entries
- Rust commands are `Box<dyn Fn>` — owned by the registry
- Janet commands are `JanetFunction` references — rooted in GC

### Keymap System

- `KeymapManager` owns layered keymaps
- Keymaps are stored as `HashMap<String, CommandName>` (strings only)
- No Janet references in keymaps — commands are looked up by name at execution time

## Buffer-Level Locking

For concurrent access patterns:

```rust
impl Buffer {
    // Read-only operations use the read lock
    pub fn slice(&self, start: usize, end: usize) -> Cow<str> {
        self.rope.slice(start, end)
    }

    // Write operations use the write lock on the inner data
    pub fn insert(&mut self, offset: usize, text: &str) {
        self.rope.insert(offset, text);
        self.undo.record(UndoOp::Insert { offset, text: text.into() });
        self.marks.adjust_for_insert(offset, text.len());
        self.line_cache.invalidate();
    }
}
```

## Ownership by Operation

| Operation | Locks Required | Duration |
|-----------|---------------|----------|
| Read buffer text | Editor: Read | Brief (microseconds) |
| Edit buffer text | Editor: Read, Buffer: Write | Brief |
| Save to disk | Editor: Write | Brief |
| Render frame | Editor: Read | Per frame |
| Event dispatch | Editor: Read | Per event |
| Plugin reload | Editor: Write | Moderate |
| Key lookup | Editor: Read | Brief |
| Command execution | Editor: Read | Variable |
| REPL eval | Editor: Read, Janet: Eval | Up to 5s |

## Slab Implementation

```rust
pub struct Slab<T> {
    entries: Vec<Slot<T>>,
    free: Vec<usize>,
    generation: u64,
}

enum Slot<T> {
    Occupied { value: T, generation: u64 },
    Vacant { next_free: Option<usize> },
}

impl<T> Slab<T> {
    pub fn insert(&mut self, value: T) -> IdWithGeneration { ... }
    pub fn get(&self, id: IdWithGeneration) -> Option<&T> { ... }
    pub fn get_mut(&mut self, id: IdWithGeneration) -> Option<&mut T> { ... }
    pub fn remove(&mut self, id: IdWithGeneration) -> Option<T> { ... }
}

pub struct IdWithGeneration {
    index: usize,
    generation: u64,
}
```

The generation counter prevents use-after-free: if a buffer is deleted and its slot is reused, the generation will not match, and the stale ID will return `None`.

## Memory Budgets

| System | Budget | Behavior at Limit |
|--------|--------|-------------------|
| Undo history | 10,000 operations | Oldest groups evicted (ring buffer) |
| Plugin cache | 100 plugins | Least recently used unloaded |
| Event queue | 1,000 events | New events block (backpressure) |
| Key sequence buffer | 10 keys | Buffer flushed, previous keys discarded |
| Rope | Unlimited (system memory) | OOM handled at OS level |
