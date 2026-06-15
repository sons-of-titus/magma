# Buffer Model Design

## Overview

Buffers are the fundamental text container in Magma. They are designed for efficient mutation, undo/redo, and multi-window viewing. The core data structure is a **rope** — a binary tree of string chunks.

```
┌──────────────────────────────────────────────┐
│                  Buffer                       │
│  ┌────────────────────────────────────────┐  │
│  │               Rope                      │  │
│  │         ┌──────────────────┐            │  │
│  │         │      Node        │            │  │
│  │    ┌────┴─────┐      ┌────┴─────┐      │  │
│  │    │  Node    │      │  Node    │      │  │
│  │  ┌─┴─┐  ┌─┴─┐  ┌─┴─┐  ┌─┴─┐      │  │
│  │  │ L │  │ L │  │ L │  │ L │      │  │
│  │  │"he"│  │"ll"│  │"o "│  │"wo"│      │  │
│  │  └───┘  └───┘  └───┘  └───┘      │  │
│  └────────────────────────────────────────┘  │
│  ┌────────────────────────────────────────┐  │
│  │             UndoTree                    │  │
│  │   root ──→ Insert{0,"hel"}             │  │
│  │             └── Insert{3,"lo"}         │  │
│  │             └── Delete{0,3,"hel"}      │  │
│  └────────────────────────────────────────┘  │
│  ┌────────────────────────────────────────┐  │
│  │              Marks                      │  │
│  │   Mark{id:1, pos:0, sticky:true}       │  │
│  │   Mark{id:2, pos:5, sticky:false}      │  │
│  └────────────────────────────────────────┘  │
│                                              │
│  Metadata:                                   │
│  - name: "main.rs"                           │
│  - path: "/src/main.rs"                      │
│  - modified: true                            │
│  - major-mode: :rust                         │
│  - encoding: :utf-8                          │
└──────────────────────────────────────────────┘
```

## Rope Data Structure

The rope is a balanced binary tree where:

- **Leaf nodes** contain short (4–64 byte) UTF-8 string chunks
- **Branch nodes** contain pointers to children, total length, and height
- **Operations** are O(log n) for insert, delete, and slice

```rust
pub struct Rope {
    root: Node,
    len: usize,
    line_cache: LineCache,
}

enum Node {
    Leaf { text: SmallStr },    // 4-64 byte inline string
    Branch {
        left: Box<Node>,
        right: Box<Node>,
        len: usize,
        height: u8,
    },
}
```

### Rope Operations

All operations are O(log n):

| Operation | Description |
|-----------|-------------|
| `slice(start, end)` | Extract substring as `Cow<str>` |
| `insert(offset, text)` | Insert text at position |
| `delete(start, end)` | Remove range of text |
| `char_at(offset)` | Get character at position |
| `line(n)` | Get nth line (via line cache) |
| `line_count()` | Total number of lines |
| `line_start(n)` | Byte offset of nth line start |
| `len()` | Total byte length |

### Line Cache

The line cache maps line numbers to byte offsets. It is lazily updated:

```rust
pub struct LineCache {
    lines: Vec<usize>,       // line → byte offset
    dirty: bool,             // invalidated on mutation
}

impl LineCache {
    // Rebuild from rope (O(n)) — only when needed
    fn rebuild(&mut self, rope: &Rope);

    // Invalidate on mutation
    fn invalidate(&mut self);
}
```

The cache is rebuilt on first access after a mutation. This trades O(n) rebuild cost for not paying it on every edit.

## Undo System

The undo tree stores operations as a tree, not a linear stack. This allows branching undo history.

```rust
pub struct UndoTree {
    groups: Vec<UndoGroup>,
    cursor: usize,           // Current position (allows redo)
    saved_at: Option<usize>, // For tracking modified status
}

pub struct UndoGroup {
    ops: Vec<UndoOp>,
    timestamp: Instant,
    merge_id: u64,          // Adjacent groups with same merge_id are merged
}

pub enum UndoOp {
    Insert { offset: usize, text: SmallStr },
    Delete { offset: usize, text: SmallStr },
}
```

### Merging

Adjacent edits within a short time window (200ms) are merged into a single undo group:

```janet
# This series of inserts becomes ONE undo step:
(buffer/insert buf 0 "hel")
(buffer/insert buf 3 "lo")
;; Undo removes both in one step
```

### Undo/Redo

```rust
impl UndoTree {
    pub fn record(&mut self, op: UndoOp) {
        // Merge with last group if recent and adjacent
        // Otherwise create new group
    }

    pub fn undo(&mut self, rope: &mut Rope) -> Result<()> {
        let group = self.groups[self.cursor];
        self.cursor -= 1;
        // Reverse all ops in group
        for op in group.ops.rev() {
            match op {
                Insert { offset, text } => rope.delete(offset, offset + text.len()),
                Delete { offset, text } => rope.insert(offset, &text),
            }
        }
    }

    pub fn redo(&mut self, rope: &mut Rope) -> Result<()> {
        self.cursor += 1;
        let group = self.groups[self.cursor];
        for op in &group.ops {
            match op {
                Insert { offset, text } => rope.insert(*offset, text),
                Delete { offset, text } => rope.delete(*offset, *offset + text.len()),
            }
        }
    }
}
```

## Marks

Marks are persistent positions within a buffer that survive edits:

```rust
pub struct Mark {
    pub id: MarkId,
    pub pos: usize,
    pub sticky: bool,       // Sticky marks follow insertions at their position
}

impl Mark {
    // Adjust position after an insert before/at the mark
    fn adjust_for_insert(&mut self, offset: usize, len: usize) {
        if self.sticky && self.pos >= offset {
            self.pos += len;
        } else if !self.sticky && self.pos > offset {
            self.pos += len;
        }
    }

    // Adjust position after a delete before the mark
    fn adjust_for_delete(&mut self, start: usize, end: usize) {
        if self.pos > start {
            self.pos = if self.pos <= end { start } else { self.pos - (end - start) };
        }
    }
}
```

## Buffer API (Janet)

### Creating and Managing Buffers

```janet
# Create a new buffer
(buffer/create "scratch")
=> <buffer:42>

# List all buffers
(buffer/list)
=> @[<buffer:1> <buffer:42>]

# Get/set current buffer
(buffer/current)
=> <buffer:1>

(buffer/current buf-id)
=> <buffer:1>
```

### Text Manipulation

```janet
(def buf (buffer/current))

# Insert text at offset
(buffer/insert buf 0 "Hello, Magma!")
=> nil

# Delete a range
(buffer/delete buf 5 7)
=> nil

# Replace a range
(buffer/replace buf 0 5 "Hi")
=> nil

# Read text (returns string slice)
(buffer/slice buf 0 5)
=> "Hi, M"

# Get a specific line
(buffer/line buf 0)
=> "Hi, Magma!"

# Get all lines
(buffer/lines buf)
=> @["Hi, Magma!"]

# Buffer length
(buffer/len buf)
=> 12
```

### Undo/Redo

```janet
(buffer/undo buf)
=> nil

(buffer/redo buf)
=> nil

(buffer/modified? buf)
=> true
```

### Marks

```janet
# Create a mark at current cursor position
(def m (buffer/mark buf 5))
=> <mark:1>

# Create a sticky mark (follows insertions)
(def m (buffer/mark buf 5 true))
=> <mark:2>

# Go to mark (returns position)
(buffer/goto buf m)
=> 8
```

### Introspection

```janet
(describe buffer)
=> {:name "scratch" :path nil :modified false :len 12 :lines 1}

(buffer/name buf)
=> "scratch"

(buffer/path buf)
=> nil

(buffer/major-mode buf)
=> :fundamental
```

## Safety Guarantees

1. **No raw string access** — The buffer API only returns slices and requires offset-based mutation
2. **Bounds checking** — All operations validate offsets; out-of-bounds returns an error
3. **UTF-8 safety** — All operations operate on byte offsets but validate UTF-8 boundaries
4. **No dangling marks** — Marks are owned by the buffer and removed when the buffer is deleted
5. **Undo memory bounded** — Undo history is bounded by configurable max groups or memory limit
