# Plugin System Architecture

## Layers

| Layer | Lives in | Owns |
|-------|----------|------|
| Manifest pipeline | `builtins/plugin_system.janet` | `plugin/define`, discovery, dependency resolution, capability validation |
| Registries (Rust) | `crates/extension`, `crates/scripting/src/plugin_api.rs` | Plugin records, capability enforcement (`ExecContext`), load/unload |
| Language surface | `builtins/language.janet`, `crates/scripting/src/language_api.rs` | Language registration, grammar load + ABI validation, file-type maps |
| Mode activation | `builtins/major_modes.janet` | Extension and filename registries, `buffer-created` activation |
| LSP attach | `builtins/plugins/lsp_attach.janet` | Launch policies from `:lsp` specs, server-key sharing, marker matching |
| LSP client | `builtins/plugins/lsp.janet`, `crates/semantic/src/lsp` | Server state, requests, responses |
| Formatting | `builtins/formatter.janet` | Formatter registry, format-on-save, presence checks, failure reporting |

## Startup flow

1. Builtins load in fixed order (`crates/scripting/src/loader.rs`).
2. `plugin/bootstrap` discovers plugins from `<cwd>/plugins` and the
   user root (`plugins-root` option, default `~/.config/magma/plugins`),
   evaluating each `manifest.janet`.  Manifest evaluation registers
   languages, file types, launch policies, toolchains, and templates;
   a bad manifest warns and is skipped.
3. Every discovered plugin is enabled (dependencies first, depth-first),
   except names in `plugins-disabled`.  Installed means enabled.

## Capability model

`crates/extension` defines the capability set (filesystem, network,
process-execution, editor-access).  A plugin's entry script evaluates
under `ExecContext::Extension(name)`, and every privileged call checks
the declaring manifest at invocation time.  Unknown capabilities are
denied; the core context is unrestricted.  `plugin/define` validates
consistency eagerly — launching a language server requires
`:process-execution`.

## Loading and unloading

`plugin/load` evaluates the entry script (or a precompiled `main.jimage`,
sniffed by its `MGIMG1` magic) under the capability context.  `plugin/unload`
removes registry-level contributions — languages, templates, toolchains —
and emits `plugin-unloaded`.  Commands, event handlers, and keymaps a
plugin registered stay active until the session ends; the scripting
runtime has no per-plugin teardown of script-level registrations.

## Packaging

A plugin entry can be compiled to a `main.jimage` (`plugin/build`,
`:plugin-build`) — marshaled bytecode pinned to the Janet version — and
loads through the same path as source.

## Reference

The manifest schema and trust model: [plugins-templates.md](../plugins-templates.md).
The Janet API: [api/janet-api.md](api/janet-api.md).
