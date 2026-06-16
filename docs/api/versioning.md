# Magma API Versioning

## Version number

Magma exposes a single version constant that all Janet code (including
extensions) can read:

```
magma-api-version  →  [0 24 0]
```

The version is a three-element tuple `[MAJOR MINOR PATCH]` following
semantic versioning semantics.

## Versioning policy

| Bump | When | Effect on extensions |
|------|------|---------------------|
| **Patch** (e.g. 0.24.0 → 0.24.1) | Backward-compatible additions — new functions, new optional arguments, new events | Extensions do not need to change |
| **Minor** (e.g. 0.24.0 → 0.25.0) | Additions with deprecations — old names log warnings but still work for one minor version | Extensions should migrate before the next major bump |
| **Major** (e.g. 0.24.0 → 1.0.0) | Breaking removals — old names deleted, required arguments changed, behaviour changed | Extensions that relied on removed APIs will break |

## Checking compatibility from Janet

Use `(magma/api-compat? major minor)` to check whether the running API
version meets a plugin's minimum requirement:

```janet
(if (magma/api-compat? 0 24)
  (print "This API is available")
  (print "Plugin requires at least Magma 0.24"))
```

The check returns `true` when the running MAJOR version is greater than
the requested one, or when MAJOR matches and the running MINOR is greater
than or equal to the requested one.  PATCH is ignored — all 0.24.x
releases are mutually compatible.

## Migration policy

When a function is renamed or removed:

1. The old name is **deleted immediately** — no aliases, no shims, no
   backwards-compatibility wrappers.  This follows the hard rule from
   `AGENTS.md`: *No feature flags or backwards-compat shims when you can
   just change the code.*

2. Every call site within Magma's own builtins is updated in the same
   commit that renames or removes the function.

3. The deprecation window described above (old names log warnings for one
   minor version) applies only to **public API functions** that external
   extensions might use.  Internal functions used only by Magma's own
   builtins do not get a deprecation window.
