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
| `:lsp` | Language-server launch policy (`:markers`, `:single-file`, `:server-key`, bundled `:command`, pinned `:install`) |
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

The server binary resolves in priority order: the `nova-lsp-command`
option (seed it in `config.janet`), a server the plugin bundles, one a
previous download landed, a system install, and — when the manifest pins
one — a download.  Attach, didOpen/didChange, hover, completion, rename,
and diagnostics are the editor's.

### Bundling the server

Ship the executable in the plugin and point `:command` at it, relative to
the plugin directory:

```janet
:lsp {"nova" {:markers ["project.nova"] :command "servers/nova-lsp"}}
```

A platform suffix picks the build per OS (`servers/nova-lsp.macos`,
`.linux`, `.exe`) exactly like grammars do.  Bundling suits small
single-file servers; big ones should be pinned instead.

### Pinning a download

```janet
:lsp {"nova" {:markers ["project.nova"]
              :install {:url "https://example.com/nova-lsp.tar.gz"
                        :sha256 "9f2…"
                        :format "tar.gz"
                        :bin "nova-lsp"}}}
```

When the server is installed nowhere, the editor asks the user before
acting; on accept it fetches the payload, verifies the checksum before
executing anything, unpacks (`raw`, `gz`, `tar.gz`), and starts the
server — announcing the download and the outcome as notifications.  The
`servers-offer-install` option turns the asking off entirely (no prompt,
no download).  A manifest that declares `:install` must also declare the
`:network` capability.

## Management verbs

`:plugin-list`, `:plugin-info <name>`, `:plugin-enable <name>`,
`:plugin-reload <name>`, `:plugin-unload <name>`, `:plugin-build <name>`
(precompile the entry to a `main.jimage`).

## Authoritative reference

The full manifest schema, trust model, grammar ABI rules, and template
substitution live in [plugins-templates.md](../plugins-templates.md);
the complete Janet API is [docs/api/janet-api.md](../api/janet-api.md).
