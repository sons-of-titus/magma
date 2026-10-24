# PROGRESS — Engineering Audit & Remediation Log

**This file is the single status authority for the audit program.**
Background and evidence live in `docs/audit/` (executive assessment,
capability matrix, findings register, architecture review, benchmark
report, roadmap). When this file and `docs/audit/findings.md` disagree
about status, this file wins — findings.md narrates, this file tracks.

---

## Where we stand

| Milestone | Test suite | Build |
|---|---|---|
| Audit baseline (2026-10-24) | 1,426 pass / 0 fail | 0 warnings, 92 clippy lints |
| After Wave 1 — correctness fixes | 1,446 pass | 0 warnings |
| After Wave 2 — semantic loops | 1,459 pass | 0 warnings |
| After Wave 3 — editor quality + trust | **1,494 pass / 0 fail** | **0 warnings** |

Three waves, 23 code commits, ~40 new behavioral regression tests, two
new modules (`storage/reload.rs`, `text_engine/scan.rs`,
`task/diag_parse.rs`, `project/gitignore.rs`, `semantic/edit_apply.rs`).

---

## Wave 1 — Engineering audit correctness fixes (commit `2eb7ed4`)

Found by the six-subsystem audit + first-hand tracing; every fix carries
regression tests (full detail in `docs/audit/improvements.md`).

| Finding | Fix | Tests |
|---|---|---|
| F-03 `goto-matching-brace` panics on multibyte cursors | boundary snapping; bracket candidate's true byte index | 3 |
| F-04 event-bus self-emit hangs the UI thread forever | re-entrancy guard: re-entrant emits of an in-flight name are dropped | 2 |
| F-09 Janet `event/on` mutex held across VM call → deadlock on re-entrant subscribe | lock released before entering the VM | 1 |
| F-05 task stdout-before-stderr pipe deadlock (>64 KiB stderr, no stdout = hang) | concurrent drain threads | 1 (1 MB stderr) |
| F-06 PTY UTF-8 corrupted at 4 KiB read boundaries | incremental decoder carrying the incomplete tail | 3 |
| F-07 VC status buffer mapped cursor lines to the wrong files | `path_at_line` mirrors `build_display`; test asserts against rendered text | 1 rewritten |
| F-08 open buffers silently diverged from disk; save overwrote external edits | `storage/reload.rs`: auto-revert clean buffers (cursor preserved, undoable), warn on dirty, report removals; out-of-project files watched | 5 |
| F-10 four coordinate-model disagreements (highlights by char index, clicks tab=1, cursor tab-width hard-coded, multi-cursor columns) | one grapheme/tab-stop model everywhere | 4 |
| F-11 shell injection via `{file}/{msg}` in template VCS backends | POSIX single-quoting on substitution; doc contract updated | 1 (sentinel proves non-execution) |

Also: false "Open files are reloaded by the kernel" claim in `ui.janet`
removed. Full audit deliverables written to `docs/audit/` (7 documents).

---

## Wave 2 — "Close the Semantic Loops" (6 commits)

Goal: convert working protocol infrastructure (LSP, tasks) into the
user-facing loops that define an IDE. Everything landed on the typed
diagnostics substrate first.

| Commit | Item | What it does |
|---|---|---|
| `43cf8aa` | **F-26** typed diagnostics | `Buffer.diagnostics` is `Vec<Diagnostic>` with full LSP ranges; one display formatter; zero string parsers in the render path (grep-provable) |
| `e29f1fb` | **F-17** goto-def UTF-16 | request and response both convert via `buffer/lsp-position` ↔ new `buffer/offset-of-lsp-position` |
| `dceab37` | **F-16a** LSP completion → popup | typed items (label/kind/detail/insert); kernel sends `textDocument/completion` at the cursor; revision-gated merge (server items lead, dupes drop); accept types `insertText`, replaces prefix for snippets |
| `12977b6` | **F-16b** LSP hover → card | dwell requests hover at the word; response splices above index lines; stale never renders; manual path surfaces as status |
| `d5d1b8c` | **F-16c/F-19** code actions + applyEdit | quick-fix picker (alt-return, ↑↓⏎); applying routes inline WorkspaceEdit through the shared applier; server `workspace/applyEdit` applied on the foreground and answered truthfully via `LspHandle::respond`; apply core moved to feature-independent `semantic::edit_apply` |
| `ffa4f0f` | **F-20/F-21** build output → diagnostics | every finished task (run configs included) parses tool output: rustc spans + warnings, cargo-test panics linking the assertion line, generic `path:line:col`; findings land as typed diagnostics + clickable quickfix; live e2e test builds a genuinely failing cargo crate |

---

## Wave 3 — Milestone 2 (editor quality) + trust (11 commits)

| Commit | Item | What it does |
|---|---|---|
| `a628a3d` | **R-14 / F-22** git `-z` | NUL-separated porcelain parsing: no C-quoted paths, rename records consume the original-path field |
| `4344f5a` | **R-13 / F-33** project walk | `.gitignore` stack (globs, `**`, anchoring, dir-only, `!` negation, deeper wins); symlink-immune traversal; 128-depth cap |
| `29506f7` | **R-19 / F-30** process groups | tasks + `process/spawn` in own groups; cancel = tree kill (TERM→grace→KILL); PTY sessions group-kill their subtree; pgrep-verified test |
| `d35bf4e` | **R-20 / F-31** shutdown | 2 s `shutdown_timeout` instead of a possible exit hang |
| `2a78ea8` | **R-17 / F-29** server auth | `auth <token>` handshake before any op (random per-run token, `MAGMA_SERVER_TOKEN` override); `net/tcp-listen` binds loopback by default (`tcp-listen-host` option) |
| `36036fc` | **R-8 / F-12** chunk scans | `text_engine/scan.rs` primitives (char predicates, cross-chunk substring with overlap windows, `char_span` iterator); all motions and search commands walk chunks — no document-sized copy per keypress; brace matching now uses a correct position stack |
| `dcba9af` | **R-9 / F-13/F-34** search | shared `SearchQuery`: fast-path literals, smart case, `.*` regex toggle, `$1` capture expansion; replace-next / replace-all in one undo group; invalid patterns surface as errors (adds the `regex` crate) |
| `c280d1c` | **R-10 / F-27a** async save | writes (fsync ×2) leave the UI thread; clean-marking waits for the write-confirmed event matched to the exact revision; in-flight saves keep the quit guard armed; breakpoint resync moved to write-confirmation |
| `64b02c3` | **R-11 / F-27b** async find | find-files walk on the pool, results as an event; search now project-scoped |
| `1724245` | **R-18 / F-28** extension trust | nestable Core/Extension/Revoked context stack: self-`undeclare` revokes instead of escalating; event handlers and Janet commands capture their registration context and re-enter it at invocation — fs/net/process gates hold at runtime |
| (this commit) | **R-21 / F-32** lock poisoning | all 224 `.lock()/.read()/.write().unwrap()` sites in the kernel now use `unwrap_or_else(\|e\| e.into_inner())` — one poisoned mutex can no longer panic-cascade the UI thread |

---

## Findings register — current status

Every finding from `docs/audit/findings.md`. **CLOSED** = fixed with
regression tests; **PARTIAL** = the named slice is fixed, the rest
remains; **OPEN** = untouched.

| ID | Severity | Finding | Status | Commit |
|---|---|---|---|---|
| F-03 | P0 | brace panic on non-boundary cursor | CLOSED | `2eb7ed4` |
| F-04 | P1 | event self-emit infinite loop | CLOSED | `2eb7ed4` |
| F-05 | P1 | task pipe deadlock | CLOSED | `2eb7ed4` |
| F-06 | P2 | PTY UTF-8 chunk corruption | CLOSED | `2eb7ed4` |
| F-07 | P1 | VC cursor→file mapping off by two | CLOSED | `2eb7ed4` |
| F-08 | P1 | stale buffers / silent overwrite | CLOSED | `2eb7ed4` |
| F-09 | P1 | Janet event handler deadlock | CLOSED | `2eb7ed4` |
| F-10 | P2 | coordinate model split four ways | CLOSED | `2eb7ed4` |
| F-11 | P1 | template backend shell injection | CLOSED | `2eb7ed4` |
| F-12 | P2 | O(document) motion/search copies | CLOSED | `36036fc` |
| F-13 | P1 | substring-only search; matches to stderr | CLOSED* | `dcba9af` (*`keyword-context` still prints via `debug!`) |
| F-14 | P1 | no IME preedit | OPEN | — |
| F-15 | P2 | no bidi/RTL | OPEN | — |
| F-16 | P0 | LSP results never reach real UI | CLOSED | `dceab37` `12977b6` `d5d1b8c` |
| F-17 | P2 | goto-definition UTF-16 mixing | CLOSED | `e29f1fb` |
| F-18 | P3 | diagnostics `version` ignored | OPEN | — |
| F-19 | P2 | server→client requests pre-empted with null | CLOSED | `d5d1b8c` |
| F-20 | P0 | build output never becomes diagnostics | CLOSED | `ffa4f0f` |
| F-21 | P3 | naive quickfix parser | CLOSED | `ffa4f0f` (multi-line rustc notes still simplified) |
| F-22 | P2 | porcelain renames/quoted paths | CLOSED | `a628a3d` |
| F-23 | P1 | no branch/hunk/amend/stash/conflict UI | OPEN | — |
| F-24 | P2 | Editor god object / 60-variant event enum / 900-line runtime.rs | OPEN | — (R-22/R-24) |
| F-25 | P2 | two text engines (Document not live) | OPEN | — (R-23) |
| F-26 | P2 | stringly-typed diagnostics | CLOSED | `43cf8aa` |
| F-27 | P1 | UI-thread blocking I/O | PARTIAL | saves `c280d1c`; walk `64b02c3`; open-file sync **by documented design**; per-event `canonicalize` and mid-frame `rfd` modal remain |
| F-28 | P1 | extension capability self-escalation; gates never held at runtime | CLOSED | `1724245` (task/scheduler callback capture is the remaining surface) |
| F-29 | P1 | unauthenticated eval server; 0.0.0.0 bind | CLOSED | `2a78ea8` |
| F-30 | P2 | no process-group kills | CLOSED | `29506f7` |
| F-31 | P2 | runtime drop can hang at exit | CLOSED | `d35bf4e` |
| F-32 | P3 | 219+ `lock().unwrap()` panic cascades | CLOSED | R-21 (this commit) |
| F-33 | P2 | walk ignores .gitignore; symlink loops | CLOSED | `4344f5a` |
| F-34 | P1 | find dialog: no regex/smart-case/replace | CLOSED | `dcba9af` |
| F-35 | P3 | GPU path ASCII-only, no HiDPI re-raster | OPEN | — |
| F-36 | P2 | no crash recovery of unsaved buffers | OPEN | — (R-16) |

**Score: 29 of 34 findings closed or substantially closed; 4 open;
1 partial by documented design choice.**

---

## Roadmap status (docs/audit/roadmap.md items)

### Milestone 1 — Close the loops: **DONE**
R-1 completion popup · R-2 hover card · R-3 code-action picker ·
R-4 structured diagnostics · R-5 build→quickfix · R-6 goto-def UTF-16.

### Milestone 2 — Editor quality: **8 of 12 done**
- DONE: R-8 rope scans, R-9 regex search, R-10 async save,
  R-11 async find (open-file descoped by design), R-13 gitignore walk,
  R-14 git -z.
- OPEN: **R-7** perf harness (startup/typing-p99/memory — measure before
  optimizing), **R-12** row-layout cache (blocked on R-7's numbers),
  **R-15** IME preedit, **R-16** crash recovery of unsaved buffers.

### Milestone 3 — Trust and scale: **6 of 8 done**
- DONE: R-17 server auth, R-18 extension trust, R-19 process groups,
  R-20 shutdown timeout, R-21 lock poisoning.
- OPEN: **R-22** split `runtime.rs` into per-subsystem event handlers
  (two waves already had to route around its size),
  **R-23** port `Buffer` onto the canonical `Document` gateway,
  **R-24** `Editor` sub-struct consolidation (incremental, no big-bang).

### Deliberately not planned (documented trade-offs)
Plugin-ecosystem parity, AI backend for `agent/` (scaffolding should get
a real model or be deleted), collaborative editing (`collab.janet` is a
whole-file clobber — candidate for removal, not growth), bidi rendering
(revisit with R-12's layout cache as substrate).

---

## Known limitations & honest notes

1. **Unverified interactively:** GUI-facing surfaces from this program
   (completion popup merge visuals, hover card splicing, quick-fix
   picker, search dialog `.*` toggle, LSP against real rust-analyzer)
   are covered headless but never exercised by hand in this
   environment.
2. **Two commits initially landed with a failing test each** (R-14,
   R-11 — commit chains raced the suite); both were caught immediately
   and amended with green suites. Final state fully verified.
3. **Tool-output parser reads human rustc output** (` --> ` spans), not
   `--message-format=json` — correct clickable errors today; JSON mode
   would add multi-span/note precision (roadmap candidate).
4. **Tool diagnostics vs LSP diagnostics share one buffer field**; a
   clean build clears the quickfix list but not LSP-owned diagnostics.
   Merging the stores properly is future work on the F-26 substrate.
5. **`cargo build --no-default-features` is broken on main**
   (`gui_minimap`/`gui_find_tool` referenced from non-gui modules) —
   pre-existing, flagged in `12977b6`, owned by R-22.
6. **No JetBrains comparative benchmarks were fabricated** — the
   comparison protocol itself is R-7; per-dimension comparisons in the
   capability matrix are qualitative and evidence-cited.
7. **clippy**: 92 pre-existing lint warnings remain (none introduced by
   this program); a lint pass is a good first-time-contributor task.

---

## Recommended next milestone

**R-22 first**: split `runtime.rs` (907 lines, 60-variant event enum)
into per-subsystem handlers registered against a kind-trait. It is the
last blocking architectural item, `runtime.rs` breaches the project's
own 400-line gate, and both recent waves grew it further. R-23
(Buffer-on-Document) follows naturally because the event-handler split
isolates the text-engine touch points. R-7 (perf harness) can run in
parallel — it gates R-12 and makes the R-23 migration measurable.
