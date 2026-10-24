# Magma Engineering Audit — October 2026

Evidence-based audit of the Magma editor (Rust kernel + Janet userland)
benchmarked against JetBrains IDEs (RustRover/IntelliJ as the primary
reference). Companion documents: `capability-matrix.md`, `findings.md`,
`architecture.md`, `benchmark.md`, `improvements.md`, `roadmap.md`.

---

## Deliverable A — Executive Assessment

### Engineering maturity

Magma is a **competent, honest editor core wrapped around an unfinished
IDE**. The load-bearing infrastructure — transactional rope text engine
with anchor-transforming cursors, single-owner threading model with a
typed background-event channel, real VT-100 terminal emulator, real DAP
client, real LSP client with request correlation — is better than its
size suggests, and the codebase is unusually free of fakery: when
something is a stub, it is usually labeled one. 1,426 tests passed before
this audit touched anything, and they assert behavior (real PTYs, real
processes, real event pumps), not mocks.

What keeps it from being a JetBrains-class product is not the kernel but
the **last mile**: language-intelligence results that never reach a real
UI (hover/code-actions/completion log to a message buffer), build output
that never becomes diagnostics, a VCS status buffer whose cursor mapping
was off by two lines, open buffers that silently went stale on external
file changes, and coordinate-model inconsistencies (tabs/wide glyphs)
between renderer, mouse mapper, and highlight pass.

### Strongest aspects

1. **Text engine** — transactions, undo groups with vim save semantics,
   revision-tagged change feed, UTF-16 LSP position conversion (tested).
2. **Concurrency discipline** — background threads never touch the
   Editor; everything arrives as typed events with staleness gates
   (revision for LSP, generation for search, supersede lanes).
3. **Terminal emulator** — real grid/scroll-regions/alt-screen/SGR,
   regression-tested against vim/htop behavior patterns.
4. **Test culture** — 1,400+ behavioral tests including end-to-end
   scenarios with deadline-bounded event pumps.

### Weakest aspects

1. **Language intelligence integration** — protocol clients exist; the
   user-facing layer (completion popup fed by LSP, hover UI, quick-fix
   picker, semantic highlighting) is missing or logs to a buffer.
2. **Editor↔world consistency** — external file changes, build
   diagnostics, debugger output are not woven back into the editor the
   way JetBrains weaves them.
3. **UI-thread I/O** — `save-buffer` (two fsyncs), `find-files` (full
   tree walk), `open-file` (sync read) all run inline on the frame
   thread.

### Three biggest architectural risks

1. **The `Editor` god object** (77 public fields) plus a 60-variant
   `BackgroundEvent` mega-enum in a 900-line `runtime.rs` — every new
   subsystem couples to both.
2. **Two text engines coexist**: the canonical `Document` gateway is
   test-only; live `Buffer` still runs the old path
   (`text_engine/document.rs` admits this in its header).
3. **Stringly-typed diagnostics** — `"[E] line 12:3 msg"` strings are
   parsed back apart in three render sites; structured diagnostics live
   in `semantic` but the render path never sees them.

### Five biggest user-facing deficiencies (pre-fix)

1. Open buffers silently diverge from disk; saving overwrites external
   edits without warning (**F-08**, fixed).
2. The VCS status buffer maps cursor lines to the wrong files —
   stage/unstage/diff act on the wrong path (**F-07**, fixed).
3. Highlights/selection paint at wrong columns on tab-indented or
   wide-glyph lines; clicks land on wrong offsets (**F-10**, fixed).
4. A stderr-heavy build task deadlocks the task system (**F-05**,
   fixed).
5. Non-ASCII terminal output corrupts at 4 KiB read boundaries (**F-06**,
   fixed).

### Why it does not match JetBrains yet

JetBrains' moat here is not editor speed — Magma's O(visible) frame
pipeline is benchmarked flat from 1k to 100k lines. The moat is
**integrated semantics**: diagnostics that flow from compiler to editor
to quick-fix, refactors that understand code, a project model that
indexes and watches everything, and ten years of UI polish. Magma has
built the transports (LSP, DAP, tasks, watcher) but not yet the
integrations.

### Suitability

- **Personal use on small/medium projects**: yes, today, with LSP
  diagnostics and the terminal (post-fixes).
- **Daily professional development**: not yet — missing semantic
  refactoring flows, build→diagnostic integration, IME, crash recovery.
- **Broader production adoption**: no — extension system is
  self-escalatable, server mode is unauthenticated, no hot reload.

---

## Deliverable H — Final Verification

### What was inspected

Every subsystem directory under `src/kernel/` (38 kloc of non-test Rust),
the Janet bridge (297 C functions across 52 modules), all 41 builtin
`.janet` files, the test suite (95 test files), and the benchmark suite.
Six parallel subsystem audits (rendering, LSP, terminal/process,
project/VC/session, debug/extension/bridge, runtime/concurrency) plus
first-hand tracing of the edit path, render path, command dispatch,
event pump, and save path.

### What was actually executed

- `cargo build --features janet` — clean, 0 warnings (before and after).
- `cargo test --features janet` — 1,426 passed / 0 failed before changes;
  1,455+ passed / 0 failed after (includes 17 new regression tests).
- `cargo clippy --features janet` — 92 lint warnings (pre-existing; none
  introduced by the audit's changes).
- `cargo bench --bench frame_pipeline` — O(visible) frame cost confirmed
  flat across 1k/10k/100k-line documents.

Not executed (environment limits): interactive GUI sessions, real
rust-analyzer round-trips, real debug-adapter sessions beyond the
existing fake-adapter tests, high-DPI/multi-monitor behavior, IME input.
Findings that depend on those are marked accordingly in `findings.md`.

### Findings fixed in this pass

F-03 (brace panic), F-04 (event re-entrancy hang), F-05 (task pipe
deadlock), F-06 (PTY UTF-8 corruption), F-07 (VC cursor mapping),
F-08 (external file changes), F-09 (Janet event deadlock), F-10
(coordinate model), F-11 (template shell injection), plus the stale
`ui.janet` claim. Details and tests: `improvements.md`.

### Findings not fixed and why

The remaining register (`findings.md`) is dominated by *feature build-out*
(LSP UI, build→quickfix, hunk staging, IME, bidi) rather than small
correctness fixes — each is a scoped project, not a patch. Two
deliberately deferred "fixes": making `save-buffer` fully async (would
weaken the quit guard's dirty-buffer check semantics) and splitting the
`Editor` god object (a migration, not a change; see `architecture.md`).

### Regressions or risks introduced

- The event-bus re-entrancy guard drops a re-entrant emit of an
  in-flight event name. Legitimate self-re-entrant chains now lose the
  nested event; no shipped builtin relies on that pattern (verified by
  full-suite green).
- Template VCS backends now auto-quote `{file}/{dir}/{msg}`; templates
  that quoted placeholders themselves would double-quote. The documented
  examples never did; doc updated.

### Remaining gap to JetBrains

Widest gaps, in order: semantic feature UI, compiler/IDE feedback loops,
refactoring depth, plugin ecosystem, and IME/accessibility. Narrowest
gaps: terminal emulation, editor core correctness, rendering
scalability.

### Recommended next milestone

**"Close the loops"**: (1) LSP completion → completion popup, hover →
hover card, code actions → picker that applies edits; (2) every task run
parses quickfix output, not just `make`; (3) diagnostics as structured
data end-to-end. All three reuse infrastructure that already works.
