# Deliverable G — Remaining Roadmap

Dependency-aware; priorities follow findings (`findings.md`). Effort:
S ≤2 days, M ≤2 weeks, L >2 weeks.

## Milestone 1 — Close the loops (highest value, infra exists)

| Item | Fixes | Depends on | Effort | Acceptance |
|---|---|---|---|---|
| R-1 LSP completion → popup | F-16 | — | S | typing shows server items merged with word completion; accept inserts `insertText` |
| R-2 LSP hover → hover card | F-16 | — | S | dwell shows server markdown, not index guess |
| R-3 Code actions → picker that applies | F-16, F-19 | `applyEdit` handling (S) | M | ⌥⏎ lists actions; applying edits buffer via `lsp/apply-edit` |
| R-4 Structured diagnostics end-to-end | F-26 | — | M | `Buffer.diagnostics` is `Vec<Diagnostic>`; zero parsers in render path |
| R-5 Build output → quickfix for all tasks | F-20, F-21 | R-4 | S | failing `cargo build`/`cargo test` yields clickable errors |
| R-6 goto-definition UTF-16 fix | F-17 | — | S | correct target on non-ASCII lines (test with `héllo`) |

## Milestone 2 — Editor quality parity ✅ (R-8..R-11, R-13/R-14, R-17..R-20 done — see findings.md)

| Item | Fixes | Effort |
|---|---|---|
| R-7 Perf harness (startup, typing p99, memory) | benchmark.md | M |
| R-8 O(document) motion/search slices → rope scans | F-12 | M |
| R-9 Regex + smart-case + in-file replace UI | F-13, F-34 | M |
| R-10 Async save with in-flight state on IoState | F-27 | S |
| R-11 `find-files`/`open-file` off the UI thread | F-27 | S |
| R-12 Row layout cache (measure first) | architecture.md | M |
| R-13 .gitignore-aware project walk + symlink guard | F-33 | S |
| R-14 git porcelain `-z` parsing; renames/quotes | F-22 | S |
| R-15 IME preedit overlay | F-14 | M |
| R-16 Crash recovery (unsaved buffer snapshots) | F-36 | M |

## Milestone 3 — Trust and scale ✅ (R-17..R-21 done in the same wave; R-22..R-24 remain)

| Item | Fixes | Effort |
|---|---|---|
| R-17 `--server` auth token; `tcp-listen` binds loopback by default | F-29 | S |
| R-18 Extension capabilities: enforce or delete | F-28 | M |
| R-19 Process groups for terminal/task kills | F-30 | S |
| R-20 `shutdown_timeout` + join strategy | F-31 | S |
| R-21 Lock poisoning policy (`unwrap_or_else` sweep) | F-32 | S |
| R-22 Split `runtime.rs`; per-subsystem event handlers | F-24 | M |
| R-23 Buffer-on-Document migration | F-25 | M |
| R-24 Editor sub-struct consolidation | F-24 | L, incremental |

## Deliberately not planned

Full JetBrains plugin-ecosystem parity, AI assistant integration
(`agent/` remains scaffolding and should either get a real model backend
or be deleted), collaborative editing (current `collab.janet` is a
whole-file clobber and should be removed rather than grown), and bidi
rendering (correct, but tiny user base vs. cost right now; revisit with
R-12's layout cache which is its natural substrate).

## Strategy (per the mission's classification)

- **Must match to be credible:** diagnostics loop, completion, hover,
  navigation, debugger basics, terminal, git file ops → Milestones 1–2.
- **Must be reliable, not parity:** refactoring (LSP rename is enough if
  previewed), indexing, project model.
- **Chance to be better than JetBrains:** startup weight, Janet
  scriptability, terminal quality, single-binary simplicity.
- **Defer:** ecosystem, AI, collab. **Not relevant:** JVM-specific tooling.
