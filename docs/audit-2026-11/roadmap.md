# Roadmap — November 2026 round (post "continue until all fixed")

Dependency-ordered.  Effort: S ≤ half day · M ≤ 2 days · L = sprint+.
Everything from the first register that could be fixed in-round was
fixed (see `findings.md` status column); what remains is below.
"Not re-audited" areas carry October's scores until the next round
verifies them.

## 1 — Immediate

| Item | Origin | Effort | Why |
|---|---|---|---|
| CI pipeline running the AGENTS.md gates | gap | M | Tests, warnings, clippy, file-size limits, janet-api coverage still rely on human memory; this is the single highest-leverage missing piece. |
| Re-audit LSP lifecycle / tasks+debugger / VCS / security | quota casualty | M×4 | October's scores there are unverified; semantic tokens, folding, task-build, and the agent landed after that audit. |
| Full LSP shutdown on buffer close (not just switch) | PM-02 residue | S | Switch now shuts servers down; a closed buffer still leaves its language in `buffer_languages` until quit. |

## 2 — Editor quality

| Item | Effort | Notes |
|---|---|---|
| Per-project LSP namespace + restart policy | M | Builds on the switch shutdown. |
| Request cancellation on keystroke/buffer close | M | |
| Rename end-to-end test (multi-file, undo across files) | M | `applyEdit` now groups per file; no test proves a real rename. |
| Word-motion paragraph semantics (blank-line hopping, camelCase) | S | Current `is_word_char` runs are correct but plain. |

## 3 — Project model scale

| Item | Effort |
|---|---|
| Incremental tree update on watch events (no full re-walk) | L |
| Bounded FS-event channel (backpressure instead of unbounded) | M |
| Session fidelity: pane geometry (directions/ratios), tool-window state, run-config selection | L |
| Per-root index swap on switch completion (warm-index diff against the walk) | S |

## 4 — UX / accessibility (carried from October, unverified)

Settings search + validation, keymap conflict detector, screen-reader
semantics, GPU-path parity for CJK, per-surface dirty flags in the shell
projection pass before the next dock lands.

## Deliberate differences from JetBrains (keep)

- **Sync open-file** (documented design): correctness over a loading
  shimmer; revisit only with real user data.
- **CPU-first rendering**: the GPU path stays opt-in until it matches
  the CPU path's attribute coverage.
- **Command-registry plugin surface** instead of a UI-extension SDK:
  correct for the current scale; the WASI extension crate remains
  experimental.
