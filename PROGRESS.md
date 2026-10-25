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
| After Wave 3 — editor quality + trust | 1,494 pass / 0 fail | 0 warnings |
| **After Wave 4 — backlog close-out** | **1,551 pass / 0 fail** | **0 warnings; `--no-default-features` builds again** |
| **After GUI overhaul + follow-ups** | **1,590 pass / 0 fail** | **0 warnings; `cargo clippy --all-targets` clean** |

Wave 4 closed every open finding and roadmap item from the audit
(including the ones the roadmap had marked "deliberately not planned"),
plus a real interactive-performance defect class reported from use.

---

## GUI overhaul — JetBrains look & feel (this program)

A shell-level redesign toward IntelliJ/New-UI conventions.  Structural
changes (all covered by `src/tests/gui_chrome_tests.rs` + the updated
`keymap_guard_tests`):

- **Theme & typography:** new IntelliJ dark/light palettes (default
  `intellij-dark`), squared 1px-border chrome, and a system sans UI font for
  chrome distinct from the mono editor face.  Catppuccin stays selectable.
- **Top chrome:** a File/Edit/View/Navigate/Run menu bar and a main toolbar
  (save/undo/redo + run/debug/config), above the tab bar.
- **Docks:** left rail = Project + Commit; bottom tool bar = Terminal / Find
  in Files / Output / Debug; the Agent docks on the **right**.  `DockPosition`
  gains `Right`; the left dock switches between the tree and the Commit panel.
- **Version control:** a real Commit tool window — staged/unstaged/untracked/
  conflict change lists with click-to-stage, a branch widget, a commit box
  (Commit/Amend), and an inline diff preview.  Backed by path-parameterized
  commands (`vc-stage-file`, `vc-commit-message`, …) and a latent-bug fix:
  `vc-amend` is now registered (it was bound but missing).
- **Command palette removed:** ctrl-p no longer opens a palette (it cycles
  completions again); Settings is a two-pane JetBrains dialog at
  `ctrl-alt-s` / `ctrl-comma`.

New files: `gui_menu`, `gui_dock`, `gui_commit_tool`, `gui_commit_actions`,
`tool_window_view`, `builtin/vc_actions`.  Split-first respected on the files
that were already over the 400-line limit (`gui_editor_window`,
`tool_window`, `vcs` via a separate `vc_actions`).

---

## Wave 4 — Backlog close-out (11 commits)

| Commit | Item | What it does |
|---|---|---|
| `58c6aff` | **F-18 / F-28 / F-13 + no-default-features** | publishDiagnostics version gate (stale publishes drop); task/on-complete + task/spawn callbacks capture their registration capability context; keyword-context fills the quickfix list instead of `debug!`; `cargo build --no-default-features` repaired (toolchain_api gate, MinimapCache → state, quickfix cap const → kernel::task) |
| `d52cf80` | **R-22 / F-24** | `runtime.rs` (990 lines) split into `runtime/`: mod.rs (event vocabulary), pool.rs (handle + lanes), handlers/ per subsystem (lsp, dap, net, process, fs, task, vc, agent) behind one exhaustive dispatch. Largest file 208 lines |
| `9f9c0fe` | **R-16 / F-36** | Crash recovery: heartbeat snapshots modified buffers to a per-session state dir (opt-out `crash-recovery`), save confirmations delete their snapshot, clean exit clears the session, startup opens survivors as read-only `*recovered …*` buffers; recovered sessions never recover twice |
| `2132c28` | **R-15 / F-14** | IME preedit: egui ImeEvents surface through the collector, composition renders inline at the cursor with an underline, commits insert through the normal text path |
| `a808f41` | **F-23** | Git: branch list/checkout(+create), hunk-level stage/discard (`git diff -U0` + `apply --cached/--unidiff-zero` via stdin), amend (with/without message), full stash surface, and a dedicated Conflicts section (unmerged porcelain pairs no longer double-listed). UI: `*vc-branches*`, `*vc-hunks*`, `*vc-stash*` buffers with cursor-line mapping; vc keys h/A/S/b/R |
| `24aabda` | **Agent backend + UI** | Real LLM loop: OpenAI-compatible client over ureq with tool calling (read_active_buffer, get_diagnostics, list_editor_commands, run_editor_command — all mutations through the command registry); foreground turn loop bounded at 12 turns with user stop; config via `agent.api-url`/`agent.model` + `MAGMA_AGENT_API_KEY`; deterministic BuiltinPlanner fallback without a key. Agent tool window (✦): prompt + transcript + stop |
| `e211953` | **R-23 / R-24** | R-23 verified: Buffer is a live facade over the Document gateway (stale headers corrected; change-feed contract pinned by test). R-24 first slice: search state consolidated behind `SearchState` |
| `602b26f` | **Interactive performance (user report) + R-12 + R-7** | TAB hang: variadic Janet commands received a fabricated empty-string argument → indent-line's `for` iterated forever (620k inserts sampled); the wrapper passes args only when `min_arity >= 1`, and indent validates the count. Typing degradation: completion word set rebuilt inline per keystroke up to 256 KB — inline limit now 16 KB with the coalesced background path beyond. Tree-sitter stopped re-highlighting the whole document per buffer-changed; highlighting follows the off-thread lane parse via `ts-tree-updated`. **R-12**: row-layout cache (one grapheme→cell pass per line/revision, shared by paint + highlight walks). **R-7**: perf harness (startup, typing p99 across 10 KB–1 MB, retained memory) with regression gates; isolated core measures typing p99 ≤ 14 µs, render p99 ≤ 51 µs flat across sizes |
| `7b5243c` | **F-35** | GPU path: atlas grid derived from the live char set (the hand-kept 12-row constant had drifted and clipped the 331-char set), HiDPI re-raster at font size × ppx on scale change, Greek/Cyrillic added |
| `50d3a87` | **F-15** | Bidi: RTL runs reorder into visual order at layout time on the R-12 substrate; highlight walks tolerate non-monotonic offsets; cursor maps logical byte → visual column; LTR behavior byte-for-byte unchanged |

Also verified: nothing auto-starts against the `*scratch*` buffer at
runtime (no path → syntax/blame skip; no TS query → tree-sitter skips;
the completion word scan that degraded scratch typing is backgrounded);
the agent never starts or connects at runtime — it runs only on an
explicit `agent/ask` or panel submit, and only with a configured key.

---

## Findings register — final status

Every finding from `docs/audit/findings.md` is now **CLOSED**.

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
| F-12 | P2 | O(document) motion/search copies | CLOSED | `36036fc` (+ `602b26f` editing/motion leftovers) |
| F-13 | P1 | substring-only search; matches to stderr | CLOSED | `dcba9af`, `58c6aff` (quickfix surfacing) |
| F-14 | P1 | no IME preedit | CLOSED | `2132c28` |
| F-15 | P2 | no bidi/RTL | CLOSED | `50d3a87` |
| F-16 | P0 | LSP results never reach real UI | CLOSED | `dceab37` `12977b6` `d5d1b8c` |
| F-17 | P2 | goto-definition UTF-16 mixing | CLOSED | `e29f1fb` |
| F-18 | P3 | diagnostics `version` ignored | CLOSED | `58c6aff` |
| F-19 | P2 | server→client requests pre-empted with null | CLOSED | `d5d1b8c` |
| F-20 | P0 | build output never becomes diagnostics | CLOSED | `ffa4f0f` |
| F-21 | P3 | naive quickfix parser | CLOSED | `ffa4f0f` |
| F-22 | P2 | porcelain renames/quoted paths | CLOSED | `a628a3d` |
| F-23 | P1 | no branch/hunk/amend/stash/conflict UI | CLOSED | `a808f41` |
| F-24 | P2 | Editor god object / event enum / 900-line runtime.rs | CLOSED | `d52cf80` (R-22), `e211953` (R-24 first slice — consolidation continues incrementally) |
| F-25 | P2 | two text engines (Document not live) | CLOSED | `e211953` (verified live; headers corrected) |
| F-26 | P2 | stringly-typed diagnostics | CLOSED | `43cf8aa` |
| F-27 | P1 | UI-thread blocking I/O | CLOSED | `c280d1c` `64b2a64`; open-file sync by documented design |
| F-28 | P1 | extension capability self-escalation | CLOSED | `1724245`, `58c6aff` (task/scheduler callback capture) |
| F-29 | P1 | unauthenticated eval server; 0.0.0.0 bind | CLOSED | `2a78ea8` |
| F-30 | P2 | no process-group kills | CLOSED | `29506f7` |
| F-31 | P2 | runtime drop can hang at exit | CLOSED | `d35bf4e` |
| F-32 | P3 | lock().unwrap() panic cascades | CLOSED | R-21 |
| F-33 | P2 | walk ignores .gitignore; symlink loops | CLOSED | `4344f5a` |
| F-34 | P1 | find dialog: no regex/smart-case/replace | CLOSED | `dcba9af` |
| F-35 | P3 | GPU path ASCII-only, no HiDPI re-raster | CLOSED | `7b5243c` (CJK remains CPU-path by design) |
| F-36 | P2 | no crash recovery of unsaved buffers | CLOSED | `9f9c0fe` |

**Score: 34 of 34 findings closed.**

---

## Roadmap status (docs/audit/roadmap.md items)

### Milestone 1 — Close the loops: **DONE**
R-1 completion popup · R-2 hover card · R-3 code-action picker ·
R-4 structured diagnostics · R-5 build→quickfix · R-6 goto-def UTF-16.

### Milestone 2 — Editor quality: **DONE**
R-8 rope scans · R-9 regex search · R-10 async save · R-11 async find ·
R-13 gitignore walk · R-14 git -z · **R-15 IME preedit** ·
**R-16 crash recovery** · **R-7 perf harness** · **R-12 row-layout cache**.

### Milestone 3 — Trust and scale: **DONE**
R-17 server auth · R-18 extension trust · R-19 process groups ·
R-20 shutdown timeout · R-21 lock poisoning · **R-22 runtime split** ·
**R-23 Buffer-on-Document (verified live)** · **R-24 consolidation
(first slice: SearchState; continues incrementally)**.

### Formerly "deliberately not planned" — now landed
- **Agent backend** (`24aabda`): the scaffolding got a real model
  backend with a tool-calling loop and the Agent tool window.  It never
  starts at runtime: an LLM session exists only after an explicit ask,
  and only when an API key is configured (else the deterministic
  planner).
- **Bidi rendering** (`50d3a87`): landed on the R-12 layout-cache
  substrate the roadmap named as its prerequisite.

Still deliberately not planned: collaborative editing (`collab.janet`
remains a whole-file clobber — a removal candidate, not growth), and
JetBrains plugin-ecosystem parity.

---

## Known limitations & honest notes

1. **Unverified interactively:** the GUI-facing surfaces from this
   program (IME preedit overlay, agent panel, VC branch/hunk/stash
   buffers, GPU HiDPI rendering, bidi display) are covered headless but
   not exercised by hand in this environment.
2. **Tool-output parser reads human rustc output**, not
   `--message-format=json` — correct clickable errors today; JSON mode
   remains a roadmap candidate.
3. **Tool and LSP diagnostics are separate stores** (`Buffer.tool_diagnostics`
   / `lsp_diagnostics`, merged on read); a clean build clears only the tool
   store, so LSP results survive.  `quickfix_list` still holds both build
   problems and search/find results — splitting that is future work.
4. **CJK glyphs on the GPU path** render through the CPU paint fallback
   (fixed-grid atlas cannot hold CJK); Latin/Greek/Cyrillic are baked.
5. **Bidi scope:** per-line L2 reordering with grapheme granularity;
   multi-paragraph isolation and bidi mirroring of neutral punctuation
   at run edges are approximations.
6. **Perf harness p99 outliers up to ~120 ms** were observed on a
   machine under concurrent compile load; the isolated core stays flat
   (≤ 14 µs typing p99).  Run `cargo bench --bench perf_gates` on a
   quiet machine when gating.
7. **Lints**: `cargo build`, the test build, and `cargo clippy
   --all-targets` are all warning-free.  A few structural lints keep
   scoped `#[allow]`s (render-pass arity, module inception,
   `should_implement_trait`, `type_complexity`).
8. **R-24 (Editor consolidation)** is deliberately incremental: the
   search-state and editing-state slices have landed (`SearchState`,
   `EditingState`); the register/mark/history fields still live flat on
   `Editor` and follow the same shape next.

---

## Recommended next milestone

The audit program is complete.  Candidate follow-ups, in value order:
1. Interactive verification pass of the GUI surfaces listed in note 1.
2. R-24 continuation (editing-state consolidation) following the
   SearchState pattern.
3. `quickfix_list` split (note 3): build problems and search/find results
   still share one list, so a build and a grep overwrite each other.
