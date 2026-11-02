# Magma Engineering Audit — November 2026 (round 2)

A fresh, evidence-based audit of the Magma editor benchmarked against
JetBrains IDEs, run two weeks after the October audit
(`docs/audit/`) declared all 34 of its findings closed.  This round
re-verified a sample of those closures against the current code and then
hunted for new defects — concentrating on the code added since the
October audit (GUI chrome, folding, semantic tokens, search commands,
project switching, output pane).

Companion documents: `findings.md` (register with evidence and fix
status), `improvements.md` (implemented fixes), `roadmap.md` (remaining
work, dependency-ordered).

---

## Executive assessment

### What the verification found

The October audit's closures are **real**.  Spot-checked fixes (F-03
boundary snap, F-10 coordinate model, F-33 walk safety, F-36 crash
recovery, R-23 live Document facade, F-05 concurrent pipe drain) hold up
under re-reading; the text engine, undo/save interaction, generation-
stamped project-switch pipeline, and crash-safe index store are the
strongest parts of the codebase, and each is pinned by behavior tests.
The baseline was green: `cargo test --features janet --workspace`
→ **2,028 passed / 0 failed**, build with zero warnings.

The October round, however, closed the *letter* of some findings rather
than the *spirit*:

- F-12 ("O(document) per keypress") was closed with a chunk-scan layer,
  but `cursor-left/right` still copied the whole document prefix per
  keypress, and the find bar — which runs `search-buffer` on **every
  keystroke** — did one whole-document copy *per match* per keystroke
  (O(matches × document); ~10 GB of copies for a full match set on a
  1 MB file).
- F-34 ("find dialog regex/smart-case/replace") closed the *engine*;
  every product entry point hardcodes `regex_mode = false`, so regex
  mode and capture-group replacement are unreachable from the UI.
- F-36's crash recovery, F-08's auto-revert and the watcher were all
  correctly implemented at the handler level — but the watcher's event
  filter dropped every Remove event (`canonicalize` fails on deleted
  paths), so external deletions never reached any of it in production.

### New findings this round

30 new findings, all with file:line evidence (`findings.md`).  The most
serious, all now fixed unless noted:

- **EC-01 (P1, reproduced)** — `BufferView::transform_range` compared a
  range in post-edit coordinates against the next edit's pre-edit range,
  corrupting highlights/folds on any multi-edit transaction while the
  cursor (correct anchor transform) stayed right.
- **EC-06 (P1)** — multi-cursor typing broke after the first keystroke:
  extra-cursor offsets were never updated, so the second keystroke
  inserted at stale offsets and corrupted text; N cursors also produced
  N undo entries.
- **PM-01 (P1)** — the watcher dropped every Remove event (above).
- **GUI-1 (P1)** — the pane blit never rendered `italic`, `underline`,
  `strikethrough`, or `dim` (the builtin theme ships faces using dim and
  strikethrough), and `surface_signature` didn't hash those bits, so
  attribute-only changes never re-blitted (stale screen).
- **GUI-2 / PM-05 (P2)** — the Output pane re-cloned and re-parsed the
  entire uncapped task log on every keystroke; `session.json` was the
  one persistence path written non-atomically.

### Where Magma stands vs. JetBrains

Kernel and text engine are the credible part: transactions, undo with
revision-addressed save points, rope scans, UTF-16 LSP positions, and a
concurrency model where background threads never touch the editor are at
the level a mature IDE's core would demand.  The product layer remains
behind: language-intelligence depth (full LSP lifecycle edge cases,
semantic refactoring UX), project-model scale (incremental VFS, debounced
watcher), and UX polish (session fidelity, settings validation) are the
gaps that separate it from daily-driver status.  The October capability
matrix's overall shape stands; this round's findings move "editor
experience" up (word motions, search, multi-cursor, output pane) and
document the watcher/VFS layer as the next architectural priority.

### Suitability

- **Personal use / polyglot editing:** workable now; the editor core
  defects fixed this round (multi-cursor corruption, stale highlights,
  search keystroke cost) were the sharpest edges.
- **Daily professional development:** not yet — rename/refactoring UX,
  debugger depth, and session fidelity are below JetBrains bar.
- **Broader production adoption:** blocked on CI (none exists), LSP
  lifecycle hardening (PM-02 class), and the watcher/VFS scale items.

### Three biggest risks

1. **No CI** — the gates AGENTS.md demands (build warnings, tests,
   file-size limits) run only when a human remembers; nothing prevents
   regression merges.
2. **Watcher/VFS model (PM-01 fixed; PM-03/PM-07 open)** — no debounce,
   per-event O(files) foreground work, watchers/PTYs/terminals leak
   across project switches.
3. **Closed-but-unreachable product surface** — *(resolved this round)*
   regex search, capture replacement (EC-07), and the typing undo session
   (EC-10) are wired to real user paths; the pattern to keep watching is
   engine capability landing without a product entry point.

### What this round changed

17 regression tests, 14 fixes across text engine, editing commands,
search, rendering, output pane, watcher, session persistence, and
find-files — **2,045 passed / 0 failed** after the fixes, no new clippy
warnings.  Details in `improvements.md`; remaining gaps in `roadmap.md`.

### Method and limits

Everything reported was verified by reading the current implementation
with file:line evidence; EC-01 was additionally reproduced against the
repo's own text crate.  Two planned subsystem deep-dives (LSP lifecycle,
tasks/debugger/VCS/security) could not be re-run this round due to
agent-quota limits — those areas are marked *not re-audited* in the
roadmap and carry over October's scores.
