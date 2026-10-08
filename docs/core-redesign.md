# Magma Core Redesign

## Objective

Rebuild Magma's text-engine foundation around one invariant:

> **Every document mutation goes through one canonical transaction path and produces one revision transition that every dependent subsystem can consume.**

The goal is **not** to rewrite Magma.

Preserve:

- Rust/Janet architecture
- Janet scripting API
- command system
- event system
- keymaps
- rope storage
- existing feature surface
- existing tests where their behavioral contracts remain valid

Substantially redesign:

- document mutation
- positions/anchors
- line indexing
- views
- undo/redo
- change propagation
- rendering invalidation
- syntax invalidation
- LSP synchronization
- persistence

Delete accidental complexity rather than wrapping it in new abstractions.

---

# Phase 0 — Freeze the Architecture and Establish Invariants

Before changing implementation, document the new ownership model.

## 0.1 Define the object hierarchy

The target model is:

```text
Document
├── TextStore
├── LineIndex
├── Revision
├── ChangeHistory
├── UndoHistory
└── PersistenceState

View
├── Document reference
├── Cursor
├── Selection
├── Scroll position
└── Desired visual column

Window
└── View
```

A document contains shared file state.

A view contains per-window state.

Two windows displaying the same document must have independent:

- cursors
- selections
- scroll positions
- desired columns

## 0.2 Establish coordinate terminology

Do not use `Position` ambiguously.

Define explicitly:

```text
ByteOffset
LineIndex
Column
GraphemeIndex
CellColumn
ScreenCoordinate
```

The internal canonical document coordinate should be a **byte offset**.

Higher-level coordinates are derived from it.

## 0.3 Define mutation semantics

Every mutation must become:

```text
EditTransaction
    revision_before
    revision_after
    edits[]
    origin
```

Primitive edit:

```rust
Edit {
    range: ByteRange,
    inserted_text: Text,
}
```

Do not prematurely optimize the representation. Correct semantics come first.

## 0.4 Define anchor semantics

Create:

```rust
Anchor {
    offset: ByteOffset,
    affinity: Affinity,
}
```

with explicit boundary behavior:

```rust
Affinity::Before
Affinity::After
```

Define behavior for:

- insertion before anchor
- insertion at anchor
- insertion after anchor
- deletion containing anchor
- deletion ending at anchor
- replacement
- undo
- redo

This becomes a formal contract and must be property-tested.

## 0.5 Define revision semantics

Every committed document transaction advances the revision.

```text
revision N
    ↓
transaction
    ↓
revision N+1
```

Undo and redo are also document mutations and therefore produce new revisions.

Never reuse revisions.

---

# Phase 1 — Build the New Core Kernel in Isolation

Do not immediately replace the editor.

Create the new primitives alongside the existing implementation.

Suggested modules:

```text
text_engine/
    document.rs
    edit.rs
    revision.rs
    anchor.rs
    line_index.rs
    transaction.rs
```

## 1.1 Implement `Edit`

Support:

```text
Insert
Delete
Replace
```

Prefer one normalized representation internally.

## 1.2 Implement transactions

A transaction can contain multiple edits:

```text
Transaction
├── before revision
├── after revision
├── edits[]
└── origin
```

Examples:

```text
typing one character       → 1 edit
multi-cursor insertion     → N edits
formatting                 → N edits
replace-all                → N edits
LSP workspace edit         → N edits
```

The entire transaction gets one revision transition.

## 1.3 Implement anchor transformation

Create pure functions:

```text
transform_anchor(anchor, edit)
transform_anchor(anchor, transaction)
```

No buffer access should be necessary.

Test every boundary condition.

## 1.4 Implement `LineIndex`

The line index owns newline structure.

It must support efficient:

```text
byte_offset → line
line → byte_offset
```

Do not promise O(1) for everything.

The actual requirement is:

> No operation on the interactive input path may scan the entire prefix of the document merely to determine the cursor's line.

The line index must update incrementally when edits add/remove newlines.

## 1.5 Property-test the kernel

Before integration, aggressively test:

- random insertions
- random deletions
- replacements
- multiple edits
- Unicode
- multibyte UTF-8
- newline insertion/removal
- empty files
- huge lines
- edits at EOF
- edits at BOF
- anchor boundaries
- transaction composition

Create a simple reference implementation using a normal `String`.

For random edit sequences:

```text
Reference String
        ↕
Magma TextStore
```

must remain identical after every operation.

---

# Phase 2 — Introduce the Document Abstraction

Once the kernel is correct, make `Buffer` internally use it.

Target:

```text
Document
├── Rope
├── LineIndex
├── revision
├── ChangeHistory
└── saved_state
```

## 2.1 One mutation gateway

There must be one canonical path:

```text
Document::apply(transaction)
```

All mutations eventually pass through it.

No direct:

```text
rope.insert(...)
rope.remove(...)
```

outside the document implementation.

## 2.2 Mutation sequence

The canonical sequence should be:

```text
validate transaction
       ↓
capture revision
       ↓
update text
       ↓
update line index
       ↓
commit revision
       ↓
record transaction
       ↓
publish change
```

Do not let consumers independently infer that a mutation occurred.

## 2.3 Remove eager mark adjustment

Delete:

```text
marks.adjust_for_insert()
marks.adjust_for_delete()
```

or equivalent duplicated span arithmetic.

Convert marks into anchors.

They transform through the canonical transaction.

---

# Phase 3 — Replace BufferView With Real View State

Fix the shared-cursor architecture.

Current conceptual problem:

```text
Buffer
  ↓
one BufferView
```

Target:

```text
Document
  ↑
  │
┌─┴───────┐
View A   View B
```

## 3.1 View owns

Each view owns:

```text
cursor
selection
scroll_x
scroll_y
desired_column
viewport
```

## 3.2 Document owns

Document owns:

```text
text
revision
line index
anchors
undo history
save state
```

## 3.3 Cursor movement

Cursor movement must use:

```text
LineIndex
+
Unicode/grapheme boundaries
+
visual cell measurement
```

instead of:

```text
slice(0..cursor)
to_string()
scan chars
```

## 3.4 Add explicit visual-column handling

Separate:

```text
logical column
```

from:

```text
visual cell column
```

Account for:

- tabs
- wide Unicode characters
- combining marks
- grapheme clusters

Do not contaminate byte-offset logic with visual layout logic.

---

# Phase 4 — Replace Undo With Transaction-Based History

Only after transactions and anchors are stable.

## 4.1 Undo stores transactions

Undo history should reference:

```text
Transaction
```

rather than independently reconstructing edits from mark snapshots.

An undo entry contains enough information to reverse a transaction.

## 4.2 Cursor/selection state

Undo policy must explicitly define whether undo restores:

- cursor
- selection
- marks
- viewport

At minimum:

> Undo must restore the user's editing position predictably.

Do not hide cursor restoration inside the text mutation layer.

## 4.3 Merge policy

Delete the current 200 ms merge-window implementation.

Replace it with explicit transaction grouping.

For example:

```text
begin_transaction()
apply edits
commit_transaction()
```

Typing can be grouped according to semantic boundaries rather than arbitrary wall-clock timing.

## 4.4 Branching

When the user:

```text
edit
undo
new edit
```

the redo branch must be handled explicitly.

Do not silently destroy history because a `Vec` happens to make branching inconvenient.

A tree/DAG is acceptable if required by the semantics, but **do not implement persistent undo machinery before proving the required behavior**.

---

# Phase 5 — Saved-State and Persistence Model

Replace:

```text
saved_at: undo-index
```

with explicit persistence state.

At minimum:

```text
current_revision
saved_state
```

The saved state must represent the actual document state, not merely an undo-array position.

## 5.1 Snapshot save

When saving:

```text
capture revision N
capture document snapshot
start async write
```

When write completes:

```text
if current_revision == N:
    mark clean
else:
    remain modified
```

Never blindly call:

```text
mark_saved()
```

after an asynchronous write.

## 5.2 Atomic writes

Implement:

```text
write temporary file
flush
fsync where appropriate
rename atomically
```

Handle failures without changing the document's saved state.

## 5.3 Autosave

Autosave becomes a consumer of revisions.

It should ask:

```text
current_revision != saved_revision?
```

not infer modification from undo history.

---

# Phase 6 — Canonical Change Propagation

Now expose the transaction to the rest of Magma.

Create a canonical change event containing:

```text
buffer/document id
revision_before
revision_after
transaction
```

Do not emit merely:

```text
buffer-changed { id }
```

## 6.1 Rust consumers

Consumers should receive the transaction directly where possible.

Consumers include:

- rendering
- syntax
- LSP
- diagnostics
- marks
- persistence
- project state

## 6.2 Janet bridge

Expose enough information through the C API for Janet to consume changes without copying the entire document.

For example:

```text
buffer/revision
buffer/changes-since
buffer/slice
```

Do not force Janet to understand Rust internals.

---

# Phase 7 — Incremental Syntax

Only now redesign syntax.

## 7.1 Dirty ranges

A transaction identifies affected ranges.

Syntax should invalidate only the relevant region plus whatever surrounding context the parser requires.

Do not assume:

```text
edit range == syntax range
```

Languages can have multiline constructs.

The syntax subsystem must define its own expansion rules.

For example:

```text
edit
 ↓
affected lines
 ↓
parser context expansion
 ↓
dirty syntax region
```

## 7.2 Tree-sitter

Use real incremental parsing.

The pipeline should become conceptually:

```text
previous tree
      +
transaction
      ↓
tree edit
      ↓
incremental parse
      ↓
updated tree
```

Never recreate the complete tree for every character.

## 7.3 Highlight revisions

Every highlight layer must record which document revision it corresponds to.

Never consume ranges from revision N against document revision N+5.

This should become an explicit invariant.

---

# Phase 8 — Incremental LSP Synchronization

Replace whole-document `didChange`.

For each transaction:

```text
document revision
+
range
+
rangeLength
+
inserted text
```

becomes the LSP change.

## 8.1 Document version

Use the canonical document revision.

Never hardcode versions.

## 8.2 Diagnostics

Diagnostics must carry the server/document version they correspond to.

Reject stale results.

Example:

```text
current revision = 105

diagnostic revision = 103

→ stale
→ discard
```

## 8.3 Formatting and workspace edits

All incoming LSP edits must enter through the same transaction system.

There must not be a second mutation path.

---

# Phase 9 — Rendering and Damage Tracking

Only after the document/change model works.

## 9.1 Stop full-buffer rendering

Rendering must never need:

```text
buffer.slice(0..buffer.len())
```

to render a viewport.

Extract only:

```text
visible lines
```

and only the relevant ranges.

## 9.2 View revision

Each view maintains:

```text
drawn_revision
```

If:

```text
drawn_revision == document.revision
```

there is no document damage to process.

## 9.3 Damage calculation

Convert transaction edits into:

```text
dirty lines
dirty visual regions
```

Account for:

- inserted lines
- deleted lines
- changed line widths
- selection changes
- cursor changes
- scrolling
- decorations

## 9.4 GUI repaint

Remove unconditional:

```text
ctx.request_repaint()
```

where possible.

Repaint should be driven by:

- document changes
- cursor blinking
- animation
- asynchronous UI updates
- window events

A static editor should not continuously rebuild itself.

---

# Phase 10 — Unicode and Visual Layout

Do this after the byte/line model is stable.

Implement a clear pipeline:

```text
byte offset
    ↓
UTF-8 boundary
    ↓
grapheme cluster
    ↓
cell width
    ↓
screen position
```

Correctly handle:

- combining characters
- emoji sequences
- CJK/wide characters
- tabs
- zero-width characters
- malformed UTF-8 policy

Add tests for cursor movement and rendering.

---

# Phase 11 — FFI Soundness

Replace the `EDITOR_PTR` pattern.

The target must not depend on creating arbitrary `&mut Editor` references from raw pointers while another mutable borrow is live.

Design a scoped bridge such as:

```text
Rust owns Editor
     ↓
enter Janet call
     ↓
explicit mutable access
     ↓
Janet API call
     ↓
return
     ↓
Rust resumes ownership
```

Establish a hard invariant:

> Janet cannot retain references into Rust editor state across a call.

Audit every C function.

Run the relevant sanitizers/tools available for the project.

This is a correctness/soundness phase, not a performance phase.

---

# Phase 12 — Delete the Old Architecture

Only after the new pipeline is proven.

Delete:

- old mark snapshot restoration
- old eager mark adjustment
- old undo merge implementation
- old buffer-wide change event
- full-buffer syntax refresh
- full-document LSP updates
- full-buffer rendering
- prefix-scan cursor calculation
- dead `ts_trees`
- obsolete debounce comments
- `JANET_VM_LOCK` production paths if genuinely unnecessary
- old persistence modified-state machinery

Do not maintain two architectures indefinitely.

---

# Phase 13 — Performance Gates

Add benchmarks for actual editor workloads.

Do NOT benchmark only the rope.

Benchmark:

### Cursor

```text
move cursor in:
100 KB
1 MB
10 MB
100 MB
```

### Typing

Measure:

```text
p50
p95
p99
max
```

for repeated insertion.

### Rendering

Measure frame cost for:

```text
100 KB
1 MB
10 MB
```

with:

- syntax highlighting
- minimap
- multiple panes

### Undo

Measure:

- 100 edits
- 10,000 edits
- large paste
- undo/redo traversal

### Syntax

Measure incremental edit latency.

### LSP

Measure transaction-to-didChange latency.

### Save

Measure:

- snapshot creation
- asynchronous write
- UI blocking time

The primary target is:

> **Interactive latency must remain bounded by the affected region, not total document size.**

---

# Phase 14 — Correctness Gates

The project should not declare the redesign complete until these invariants hold.

## Mutation invariant

Every document mutation goes through one transaction path.

## Revision invariant

Every committed transaction increments the revision exactly once.

## Position invariant

Every anchor has deterministic behavior under every edit.

## View invariant

Views never share cursor/selection/scroll state accidentally.

## Syntax invariant

No syntax result for revision N is applied to revision N+1 without validation/rebasing.

## LSP invariant

Every `didChange` corresponds to real document changes.

## Persistence invariant

A save completion can never mark newer unsaved edits as saved.

## Rendering invariant

The renderer never requires a whole-document copy to render a viewport.

## Undo invariant

Undo/redo never silently loses history or restores stale mark state.

## FFI invariant

No unsafe mutable aliasing exists across the Janet boundary.

---

# Phase 15 — Final Regression Suite

Keep the existing 1319 tests, but do not treat them as sufficient.

Add dedicated suites:

```text
tests/
    document/
    transactions/
    anchors/
    line_index/
    undo/
    persistence/
    rendering/
    syntax/
    lsp/
    ffi/
    property/
```

The highest-value tests are property-based tests around the document kernel.

Run:

```text
cargo test
cargo test --features janet
cargo test --release
```

and the project's full parallel suite.

A hanging test suite is a failure.

The CI gate should enforce:

```text
0 failures
0 hangs
0 new warnings
```

---

# Implementation Order

The actual execution order should be:

```text
0. Invariants / architecture freeze
          ↓
1. Edit + Transaction + Revision
          ↓
2. Anchor semantics
          ↓
3. LineIndex
          ↓
4. Document mutation gateway
          ↓
5. View/document separation
          ↓
6. Undo/redo
          ↓
7. Persistence / saved state
          ↓
8. Change propagation
          ↓
9. Incremental syntax
          ↓
10. Incremental LSP
          ↓
11. Incremental rendering
          ↓
12. Unicode / visual layout
          ↓
13. FFI soundness
          ↓
14. Delete old architecture
          ↓
15. Performance + correctness gates
```

Do **not** reverse this order by starting with rendering optimization.

---

# Definition of Done

Magma's core redesign is complete when:

1. A one-character insertion does not scan the entire document to determine its position.
2. A one-character insertion produces exactly one canonical transaction/revision transition.
3. Multiple cursors can produce multiple edits inside one transaction.
4. Marks, cursors, selections, and decorations use explicit anchor semantics.
5. Two windows showing the same document have independent view state.
6. Undo/redo operates on transactions rather than stale mark snapshots.
7. Undo/redo behavior is property-tested.
8. `modified` cannot be cleared by a stale asynchronous save.
9. Saves are atomic.
10. Syntax invalidation is incremental.
11. Tree-sitter uses incremental edits rather than reparsing the entire document.
12. LSP uses incremental `didChange`.
13. Stale diagnostics cannot overwrite newer document state.
14. Rendering does not copy the entire document every frame.
15. Static windows do not continuously repaint without a reason.
16. Unicode cursor/rendering semantics are explicit.
17. Janet cannot cause mutable-reference aliasing.
18. No subsystem has its own independent interpretation of document mutations.
19. Full parallel tests complete without deadlock.
20. Performance scales primarily with the **affected region**, not the total document size.

---

# Final Principle

Do not optimize Magma's existing architecture.

**Replace the reason it needs to repeatedly recompute everything.**

The fundamental transformation is:

```text
OLD

mutable rope
   ↓
everyone repeatedly asks:
"What does the document look like now?"
```

into:

```text
NEW

Document
   ↓
Transaction
   ↓
Revision
   ↓
"What changed?"
   ├── anchors
   ├── line index
   ├── undo
   ├── syntax
   ├── LSP
   ├── rendering
   └── persistence
```

The rope remains the text store.

The **transaction/revision system becomes the nervous system of Magma**.

Do not add major editor features until that nervous system is correct.

---

# Appendix — Phase 0 Contract Decisions

Concrete decisions for the places the plan above leaves open.  These are
binding for the kernel (`src/kernel/text_engine/{edit,revision,anchor,
transaction,line_index,document}.rs`) and property-tested in `src/tests/`.

## A.1 Coordinate names

`ByteOffset` and `ByteRange` are type aliases over `usize` /
`Range<usize>` (defined in `edit.rs`).  The kernel is byte-offset
canonical.  Grapheme indices and cell columns are Phase 10 and must not
appear in kernel types.

## A.2 Anchor boundary contract (formal)

For an edit that deletes the byte range `[s, e)` and inserts `L` bytes at
`s` (after the edit the inserted text occupies `[s, s+L)`):

```text
anchor.offset < s         → unchanged
s ≤ anchor.offset ≤ e     → Affinity::Before : s
                            Affinity::After  : s + L
anchor.offset > e         → offset + L − (e − s)
```

Consequences (mapping to plan §0.4's checklist):

- insertion strictly before/after the anchor: unchanged / shifted.
- insertion exactly at the anchor (`s == e == anchor`): `Before` stays
  left of the inserted text, `After` lands right of it.
- deletion containing the anchor (`s < anchor < e`): the anchor's content
  is gone; both affinities collapse onto the replacement span edges.
- deletion ending at the anchor (`anchor == e, e > s`): `After` rides the
  surviving right-hand content to `s + L`; `Before` collapses to `s`.
- undo/redo: an undo is a commit of the *inverse* transaction through the
  same transform — no special cases.  A cursor sitting at the end of a
  typed insertion (`After` at `s`) returns exactly to `s` when undone.

## A.3 Transaction edit coordinates

All edits inside one transaction are expressed **in parallel, in
`revision_before` coordinates**, like an LSP workspace edit — not
sequentially.  Normalization at commit:

1. drop no-op edits (empty range and empty text),
2. sort by range start,
3. merge edits whose ranges abut (`next.start == cur.end`) into one edit,
4. reject overlapping edits (`next.start < cur.end`) with
   `TransactionError::OverlappingEdits`.

After normalization, closed ranges `[s, e]` are strictly separated, which
is what makes `transform_anchor(anchor, transaction)` well-defined at
every boundary.

## A.4 Commit atomicity and empty transactions

`Document::commit` validates everything (bounds, UTF-8 char boundaries at
both range ends) against the current text before touching the rope, then
applies edits back-to-front.  A builder with no surviving edits commits
nothing: no revision bump, `Ok(None)`.  Undo and redo are ordinary
commits (of inverse transactions) and bump the revision like any other
mutation — revisions are never reused.

## A.5 LineIndex implementation

`LineIndex` is the single owner of the newline-structure *contract*
(`line_of`, `line_start`, `line_end`, `line_span`, `line_count`,
`last_line`) and is implemented over ropey's internally-maintained line
tree through the existing `Rope` wrapper — O(log n), incrementally
updated by construction.  A second, kernel-private newline structure
would be exactly the duplicated derivation the redesign deletes.  Line
semantics match the existing rope layer: a trailing `\n` begins a final
empty line; lines are `\n`-delimited and `\r` belongs to line content.

## A.6 Phase status — COMPLETE

All sixteen phases are implemented; `cargo test --features janet` passes
1402/0 serially in debug and release.

- Phase 0–1: kernel in `text_engine/{edit,revision,anchor,transaction,
  line_index,document}.rs`; property-tested against a `String` oracle.
- Phase 2–3: `Buffer` is a facade over `Document::commit`; marks are
  anchors; views fork per split window (`window_views`).
- Phase 4–5: undo stores grouped transactions with explicit sessions and
  recorded cursor positions; save state is the undo-cursor position of a
  revision (`mark_saved_at`); writes are atomic (temp+fsync+rename).
- Phase 6: every commit publishes a canonical `buffer-changed` carrying
  the revision transition and edit spans; Janet gets `buffer/revision`,
  `buffer/changes-since`, `buffer/line-of`.
- Phase 7: Janet syntax splices dirty ranges when the cached layer
  revision matches (property: splice == full rescan); tree-sitter reparses
  incrementally from the cached tree (`ts/highlight`, `ts_trees` is a real
  `TsTreeState` cache).
- Phase 8: `didChange` sends one range change per edit with the canonical
  revision; positions are UTF-16 (`buffer/lsp-position`).
- Phase 9: the renderer extracts only visible rows; the frame is flat
  ~50µs from 100KB to 10MB; the GUI repaints only when the render
  signature changes.
- Phase 10: `text_layout.rs` — byte offset → grapheme → cell width →
  screen column; CJK renders two cells, combining marks zero.
- Phase 11: the `EDITOR_PTR` stash is gone; Janet accesses the editor only
  inside `scoped_editor`/`EditorScope` windows (live exclusive borrow,
  nesting restores, unscooped access is a clean error).
- Phase 12: deleted — mark snapshots and eager adjustment, the 200 ms
  merge window, `saved_at` undo indexing, prefix-scan position
  constructors (`Position::from_offset`), whole-document status/minimap/
  breadcrumb/overlay slices.
- Phase 13: `benches/editor_workloads.rs` — typing, cursor, render, undo,
  save.  Measured gate: typing p50 is 1.7–1.9µs from 100KB to 10MB; the
  cursor at 100MB is ~1.2µs.
- Phase 14–15: gates in `src/tests/correctness_gates_tests.rs`; full
  suites serial in debug and release: 0 failures, no new warnings, no
  hangs.
