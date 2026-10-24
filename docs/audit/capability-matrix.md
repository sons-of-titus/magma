# Deliverable B — Feature and Capability Matrix

Scores 0–5 (0 absent, 1 rudimentary, 2 functional-but-deficient,
3 competent, 4 professional, 5 excellent). "Fixed" marks defects repaired
during this audit; the score reflects the post-fix state.

| Subsystem | Status | Score | Conf. | JetBrains ref | Key evidence | Most important gap | Priority |
|---|---|---|---|---|---|---|---|
| Text engine (rope, txn, undo) | Working | 4 | High | All | `text_engine/{document,transaction,undo}.rs`; 100+ property tests | `Document` gateway not yet the live path | P2 |
| Cursor/selection/multi-cursor | Working | 3.5 | High | All | anchor-transform cursors `view.rs:53`; grapheme motion | Column-rect selection, IME | P2 |
| Search/replace (buffer) | Partial | 2.5 | High | All | substring only `search.rs`; no regex, no smart-case | Regex + replace-all preview | P1 |
| Project-wide search | Working | 3 | High | All | `rg --vimgrep` bg job, generation-gated `gui_find_tool.rs:88` | Result grouping, file filters | P2 |
| Syntax highlighting | Working | 3 | High | All | tree-sitter incremental, per-buffer trees, TS lane | Grammar coverage = 12 langs, no semantic tokens | P2 |
| Code folding | Working | 3 | Med | All | fold-aware viewport walk `editor_view.rs:120` | Only manual ranges + indent heuristics | P3 |
| Rendering pipeline | Working | 3.5 | High | All | O(visible) per frame, benchmarked flat 1k→100k | No line-layout cache; alloc per line per frame | P2 |
| Repaint/idle discipline | Working | 4 | High | All | request-on-change + blink heartbeat; regression tests | 400 ms blink wakeups in normal mode | P3 |
| Glyph/atlas (GPU path) | Partial | 2 | High | All | ASCII-only atlas `gpu_atlas.rs:22`, opt-in flag | CJK/emoji tofu, no HiDPI re-raster | P2 |
| Unicode/graphemes | Working | 4 | High | All | grapheme layout, widths, tab stops `text_layout.rs` | No bidi/RTL | P2 |
| IME | Absent | 0 | High | All | no preedit handling anywhere | CJK input unusable | P1 |
| Mouse mapping | Working (fixed) | 3.5 | High | All | tab-stop walk `mouse.rs:307` | drag past EOL auto-scroll is edge-rough | P3 |
| LSP client core | Working | 4 | High | RustRover | correlation, timeouts, crash drain `semantic/lsp.rs` | diagnostics version ignored | P2 |
| LSP integration/UI | Partial | 2 | High | RustRover | 27 servers `plugins/lsp.janet`; hover/actions log-only | completion/hover/quick-fix UI | **P0** |
| Refactoring | Partial | 1.5 | High | IntelliJ | rename applies WorkspaceEdit; no preview/conflicts | find-usages UX, safe delete | P1 |
| Project model/index | Working | 4 | High | All | persistent JSONL, content-hash incremental `index/store.rs` | no .gitignore filter in walk | P2 |
| Session restore | Working | 3 | High | All | per-project session.json round-trip tests | no split layout, no crash recovery | P2 |
| File watching/reload | Working (fixed) | 3.5 | High | All | auto-revert + warn `storage/reload.rs` | no diff-on-conflict picker | P3 |
| Git integration | Partial | 2.5 | High | IntelliJ | file-level ops, async, arg-array safe | hunk staging, branches UI, conflicts | P1 |
| Terminal | Working | 4.5 | High | All | full VT/alt-screen/scrollback, PTY, tested | no reflow on resize, no mouse mode | P3 |
| Tasks/build | Partial | 3 | High | All | real cancel, deps, streaming (deadlock fixed) | quickfix only for `make` | **P0** |
| Run configurations | Working | 3.5 | High | All | persistence, build-gating, tested `run_config/` | output→diagnostics link | P1 |
| Debugger (DAP) | Working | 3.5 | High | RustRover/PyCharm | real client, anchor breakpoints, vars/watches | no conditionals/logpoints, response crossing | P2 |
| Extension system | Partial | 2 | High | IntelliJ | capability gates fs/net/process only | self-escalation, no sandbox/install | P1 |
| Janet bridge | Working | 3.5 | High | — | 297 cfuns, id-based marshalling, fiber boundaries | handler error detail lost | P2 |
| Settings/persistence | Working | 3 | Med | All | options + workspace persist; themes | no settings search/validation UI | P3 |
| Keymaps | Working | 3.5 | High | All | layered, buffer-local, guard tests | no conflict detector, no import | P3 |
| Accessibility | Absent | 0.5 | Med | All | keyboard-only works; no SR semantics | screen reader support | P2 |
| Server mode (`--server`) | Partial | 2.5 | High | — | clean single-owner protocol | unauthenticated eval = local RCE | P1 |
| Security posture | Mixed | 2.5 | High | — | template injection fixed; `net/tcp-listen` binds 0.0.0.0 | auth tokens, sandboxing | P1 |

**Overall**: kernel 4/5, editor experience 3/5, IDE integration 2/5,
ecosystem 1.5/5 — a credible editor foundation that has not yet finished
becoming an IDE.
