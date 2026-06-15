# Magma — Agent Working Instructions

Rules for AI agents (and humans) contributing to this repository.
These are constraints, not suggestions.  A pull request that violates any rule
here will be sent back regardless of whether the code itself is correct.

---

## Architecture

The Rust / Janet split is the load-bearing wall of this project.

**Rust owns:** state (`Editor` struct), buffer storage (rope), event bus,
command registry, keymap manager, rendering surface, and the Janet FFI bridge.
Rust exposes all of this through stable, typed APIs.

**Janet owns:** every user-facing behavior — keybindings, major modes,
language support, UI formatting, colon-mode verbs, and configuration.

If you find yourself adding knowledge of a specific language, file type, or
workflow to Rust, you are putting it in the wrong place.  It belongs in a
Janet extension.

---

## Sprint Completion Gates

**A sprint is not done until all three gates pass.**

### Gate 1 — Tests

Every new Rust primitive (C function, command, event) and every new Janet macro
must have tests.  "It compiles" and "I tried it manually" are not tests.

**Location:** `src/tests/` — one file pair per feature area, declared in `src/tests/mod.rs`.

| Kind | File | When |
|------|------|------|
| Pure Rust (no Janet VM) | `src/tests/<feature>_tests.rs` | Buffer fields, event counts, command registry, slab state |
| Janet API | `src/tests/<feature>_janet_tests.rs` under `#[cfg(feature = "janet")]` | C functions, Janet macros, event handlers |

File names reflect the **feature area**, not the sprint number.  Sprint numbers
rot; feature names stay meaningful.  Example: the Extension Foundation tests
live in `extension_foundation_tests.rs` and `extension_foundation_janet_tests.rs`.

Janet tests **must** acquire `janet_bridge::JANET_VM_LOCK` at the top of every
test function:

```rust
let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
```

Use `unwrap_or_else(|e| e.into_inner())`, never bare `unwrap()`.  A panic in
one test poisons the mutex and cascades failures to every test that follows.

**What to assert:** observable Rust-side state after Janet runs — field values,
mode names, option maps, event subscriber counts.  Do not assert only that
`janet_bridge::eval` returned `"ok"`.

**Nested fiber constraint:** Janet does not support `janet_continue` called
from within a fiber C-callback that was itself started by a direct
`janet_continue` (i.e., not via `janet_dostring`).  This means: if an event
handler fires and calls `editor/run-command "some-janet-command"`, a second
fiber would be created inside the first, which silently fails.  The fix is to
register the target command as a Rust command (`ed.commands.register_fn`) in
the test, so no second fiber is needed.  For production code, prefer invoking
Rust commands from event handlers, not Janet-defined commands.

**Pass bar:** `cargo test --features janet` → zero failures.

### Gate 2 — Documentation

Every Janet C function added to the bridge must be documented before the sprint
closes.

**File:** `docs/api/janet-api.md`

Entry format:

```
### `(module/fn-name arg …)` → return-type

One sentence.  What it does, not how.
```

Document the contract.  Do not mention implementation details, file paths, or
internal struct names.

### Gate 3 — File size

No source file may grow beyond ~400 lines (Rust) or ~200 lines (Janet) without
being split at a natural abstraction boundary.

**One abstraction per file.**

| Situation | Correct split |
|-----------|---------------|
| New group of related Janet C functions | New `src/janet_bridge/<topic>_api.rs`, registered in `mod.rs` |
| New editor subsystem (project, process, LSP) | New `src/<subsystem>/mod.rs` |
| Janet init script growing large | Split topic into `builtins/<topic>.janet`, `require` it from `init.janet` |

Do not add significant logic to a file that is already near the size limit.
Refactor first, then add.

---

## Code Style

- **No comments that explain what the code does.**  Well-named identifiers do
  that.  Only comment the *why*: a hidden constraint, a non-obvious invariant,
  a workaround for a specific upstream bug.
- **No speculative abstractions.**  Three similar lines is better than a
  premature helper.  Add the abstraction when the fourth call site appears.
- **No error handling for impossible cases.**  Trust Rust's type system and
  internal invariants.  Only validate at true boundaries (user input, disk I/O,
  FFI).
- **No feature flags or backwards-compat shims** when you can just change the
  code.

---

## Commit Checklist

Before opening a PR:

- [ ] `cargo build --features janet` — no errors, no new warnings
- [ ] `cargo test --features janet` — zero failures
- [ ] `docs/api/janet-api.md` updated for every new C function
- [ ] No file exceeds the size limit without a documented reason
- [ ] Sprint completion gates (above) all pass if this closes a sprint
