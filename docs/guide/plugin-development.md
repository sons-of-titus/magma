# Plugin Development Guide

A Magma plugin is a directory with a `manifest.janet` that calls
`plugin/define`.  The manifest declares everything declarative —
languages, file-type mappings, tree-sitter grammars and queries,
language-server launch policies, formatters live in the entry script's
one-line registrations, toolchains, project templates, capabilities —
and the entry script (`main.janet`) holds only what is genuinely
imperative: major modes and custom commands.

## Minimal plugin

```
~/.config/magma/plugins/nova/
├── manifest.janet
├── main.janet
└── config.janet          option defaults, loaded by main.janet
```

```janet
# manifest.janet
(plugin/define
  {:name "nova"
   :version "0.1.0"
   :languages ["nova"]
   :extensions {"nova" ["nova"]}
   :capabilities [:editor-access]})
```

```janet
# main.janet
(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(major-mode/define "nova-mode"
  {:parent "prog"
   :layers [:nova-mode]
   :setup (fn []
            (def width (option/get "nova-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
```

That is a working language plugin: `.nova` files open in `nova-mode`
(mode activation is driven by the manifest's `:extensions` — a plugin
never installs its own buffer-created handler), and the mode command is
registered automatically.

## What the manifest contributes

| Key | Effect |
|-----|--------|
| `:extensions` / `:filenames` | File-type → mode activation |
| `:lsp` | Language-server launch policy (`:markers`, `:single-file`, `:server-key`) |
| `syntax.scm`, `folds.scm` | Tree-sitter highlight/fold queries for the mode |
| `grammars/tree-sitter-<lang>.dylib` | Grammar, loaded and ABI-validated at registration |
| `:toolchain` | Build/test/run/debug actions with project detection |
| `project-templates/` | New Project wizard entries |
| `:capabilities` | What the plugin's handlers may do at invocation time |

A manifest that starts a language server must declare
`:process-execution` — this is validated at define time.

## Formatter

One line in `main.janet`:

```janet
(formatter/register "nova" {:command "nova-fmt" :args @["-"]})
```

This defines `nova-format-buffer`, honors the `nova-format-on-save`
option, resolves the binary outside `$PATH`, and reports missing
binaries and failures instead of silently returning the input.

## Language server

Nothing to write.  Declare the launch policy in the manifest:

```janet
:lsp {"nova" {:markers ["project.nova"]}}
```

The server command comes from the `nova-lsp-command` option (seed it in
`config.janet`); attach, didOpen/didChange, hover, completion, rename,
and diagnostics are the editor's.

## Management verbs

`:plugin-list`, `:plugin-info <name>`, `:plugin-enable <name>`,
`:plugin-reload <name>`, `:plugin-unload <name>`, `:plugin-build <name>`
(precompile the entry to a `main.jimage`).

## Authoritative reference

The full manifest schema, trust model, grammar ABI rules, and template
substitution live in [plugins-templates.md](../plugins-templates.md);
the complete Janet API is [docs/api/janet-api.md](../api/janet-api.md).
