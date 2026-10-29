# Plan — Package Plugins as Janet Images

Status: research complete, implementation not started.
Every claim in "Validated findings" was probed live against the embedded VM
(evil-janet 1.42.0-dev) on 2026-10-29.

---

## Goal

A plugin should be shippable as a single precompiled Janet image
(`main.jimage`) instead of — or alongside — its `main.janet` source.  The
editor reads the image at `plugin/load` time: no parser, faster load, and a
single-file artifact for distribution.  Sources stay in the repository; the
image is a build artifact.

## Validated findings (from live probes)

1. The embedded VM exposes everything needed: `compile`, `parse-all`,
   `marshal`, `unmarshal`, `slurp`, `spit`.
2. A full round trip works: a chunk defining a helper and calling a C
   function (`plugin-state/set`) compiles to real bytecode, marshals to
   170 bytes, unmarshals, runs, and its side effect is observable.
3. `(compile x)` takes **forms, not source strings** — `(compile "code")`
   compiles a string constant (silently a no-op).  Source must go through
   `(parse-all src)` first, then splice into `(compile ~(do ,;forms) env)`.
4. Bytecode referencing C functions (`option/set`, `command/define`, …)
   cannot marshal raw: `no registry value and cannot marshal <cfunction …>`.
5. Both `marshal` lookups are **tables**, not functions — the signature is
   `(marshal x &opt reverse-lookup buffer no-cycles)` and
   `(unmarshal buffer &opt lookup)`.  With a reverse table (value → symbol)
   and a forward table (symbol → value) built from the environment, C calls
   become symbolic references resolved live at load time.

### The validated recipe

```janet
(def env (curenv))
(def fwd @{}) (def rreg @{})
(eachp [k b] env
  (when (get b :value)
    (put fwd k (get b :value))
    (put rreg (get b :value) k)))
(def f (compile ~(do ,;(parse-all src)) env))
(def img (marshal f rreg))          # write to disk with a version header
(def g (unmarshal img fwd))
(g)                                 # runs the plugin entry
```

## Design

### Image file format

```
MGIMG1:<janet-version>\n<marshaled bytes>
```

- Magic distinguishes an image from source the moment the file is opened.
- The Janet version line turns a VM-version mismatch into a clean
  "rebuild this plugin" error instead of a cryptic unmarshal panic.
  (Images are bytecode-format-pinned; there is no cross-version guarantee.)

### Manifest contract (unchanged shape)

`manifest.janet` stays source — it is needed before any code runs
(discovery, dependencies, capabilities) and is human-auditable.  `:entry`
simply points at whichever file ships:

```janet
(plugin/define {:name "rust" :entry "main.janet" ...})   # dev: source
(plugin/define {:name "rust" :entry "main.jimage" ...})  # shipped: image
```

### Capability enforcement — unchanged

Rust `plugin/load` pushes the plugin's `ExecContext::Extension(name)` around
the eval, exactly as it does today for source.  The image path runs inside
that same window, so handlers registered by an image capture the plugin's
capability context at registration.  An image is code with the same trust
level as source; the capability system remains the gate.

### Load flow

Rust sniffs only the 7-byte magic; all binary handling stays in Janet
(architecture rule: behavior in Janet, state in Rust).

```
plugin/load (Rust)
  ├─ read <root>/<entry>
  ├─ starts with "MGIMG1" → eval_entry("(plugin/load-image \"<abs path>\")")
  └─ otherwise            → eval_entry(source)                [today's path]

plugin/load-image (Janet, inside the pushed capability context)
  ├─ slurp file
  ├─ header check: magic + janet/version match, else error with fix hint
  ├─ build forward table from (curenv)
  ├─ (unmarshal bytes fwd) → function
  └─ call it
```

### Build flow

```
plugin/build (Janet)   → colon verb :plugin-build <name>
  ├─ plugin/status name → root + entry (must be .janet)
  ├─ read source, parse-all, compile against (curenv)
  ├─ marshal with reverse table
  ├─ prepend header, spit to <stem>.jimage next to the entry
  └─ print a reminder to flip :entry to the image for shipping
```

---

## Needed changes (file by file)

### 1. `builtins/plugin_images.janet` (new, ~90 lines)

`plugin_system.janet` is already at 208 lines — over the ~200 Janet limit,
so the image machinery gets its own file (one abstraction per file).

- `plugin/load-image path` — header validation, forward-table build,
  unmarshal, call.  Errors carry the plugin name and the fix hint
  ("rebuild with :plugin-build after a VM upgrade").
- `plugin/build name` — compile + marshal + header + write `<stem>.jimage`.
  Returns the image path; errors if the entry is not `.janet`.
- `colon-plugin-build` — `:plugin-build <name>` verb.
- Event-free and state-free: pure behavior over the existing registries.

### 2. `crates/scripting/src/plugin_api.rs` (+ ~12 lines)

In `c_plugin_load`, after reading the file:

```rust
if content.as_bytes().starts_with(b"MGIMG1") {
    // eval a one-line snippet instead of the content itself; the
    // capability context push in with_context is shared by both paths
    let snippet = format!(r#"(plugin/load-image "{}")"#, path.display());
    eval via eval_entry(&snippet)
} else {
    eval_entry(&content)                       // today's path
}
```

`eval_entry` already extracts the real Janet error — image load failures
surface in the editor status message like source failures do.

### 3. `crates/scripting/src/loader.rs` (+ 4 lines)

`load_builtin("builtins/plugin_images.janet", include_str!(…))` immediately
after `builtins/plugin_system.janet`, so both exist before
`<plugin-bootstrap>` runs.

### 4. Tests

`crates/scripting/tests/plugin_system_janet_tests.rs`:

- `plugin_build_creates_loadable_image` — temp plugin dir, entry source
  with a `plugin-state/set` side effect; `plugin/build`; set
  `:entry "main.jimage"` via re-register; `plugin/load`; assert `:ok`,
  `:loaded` true, and the side effect landed.
- `image_rejects_wrong_vm_version` — hand-write a `MGIMG1:0.0.0\n` file;
  `plugin/load` must return `:error` and set an error status message
  mentioning rebuild.
- `image_caps_still_enforced` — image whose chunk calls `(fs/cwd)`; plugin
  registered without `:filesystem`; `plugin/load` returns `:error`
  (context capture works through the image path).
- `plugin_build_rejects_non_janet_entry` — `:entry "main.jimage"` already;
  `plugin/build` errors instead of double-compiling.
- Bundled e2e: extend `bundled_rust_plugin_…` with a build→reload cycle.

Pure Rust: none needed beyond the existing registry tests (the Rust change
is a 7-byte prefix branch covered by the Janet tests).

### 5. Docs — `docs/api/janet-api.md`

- New section `plugin/` → add `(plugin/build name)`, `(plugin/load-image
  path)`, the image file format, and the `:plugin-build` verb.
- One sentence under the plugin directory table: "entry may be a compiled
  image (`main.jimage`); see plugin/build".

### 6. Sprint gates checklist (AGENTS.md)

- [ ] `cargo build --features janet` — no errors, no new warnings
- [ ] `cargo test --features janet` — zero failures
- [ ] `docs/api/janet-api.md` updated for any new C function (none are
      added — `plugin/load-image`/`plugin/build` are Janet-level)
- [ ] No file over limit: `plugin_images.janet` new file ~90 lines;
      `plugin_system.janet` unchanged at 208 (already at its boundary —
      next feature there forces the manifest/discovery split)

---

## Risks and known limits

- **Version pinning** — images break on VM upgrades; the header makes it
  loud and the fix is one `:plugin-build`.  CI should build images as a
  release step, never commit them as a primary artifact.
- **Symbol renames** — an image references env bindings by symbol; renaming
  a C function (e.g. `option/set`) breaks built images.  Acceptable: the
  same rename breaks source plugins identically at load.
- **Debugging** — stack traces from an image lose source context.  Sources
  stay in the repo; `:plugin-build` is a release step, not a dev step.
- **Env embedding** — the image serializes the compile env's binding
  *structure* with values replaced by symbols.  Measured small for real
  chunks (170 bytes for a two-binding chunk); a pathological env could
  bloat it — measure once on the bundled rust plugin during
  implementation.
- **Not a security boundary** — an image is arbitrary bytecode with the
  same VM power as source.  The capability system stays the gate; the
  header check is integrity/diagnostics, not trust.

## Open decision (for the user)

Default dev workflow keeps `:entry "main.janet"`.  Whether a future
`plugins/` release script should rewrite manifests to images automatically,
or manifests ship pointing at images committed to the repo, is a packaging
policy call — no code depends on it.
