# Language Plugins and Project Templates

How Magma plugins contribute language support, project templates, build
toolchains, and project detection — and how the New Project wizard ties
them together.

The split follows the repository's load-bearing wall: **Janet owns** every
declaration (manifests, templates, toolchains, language metadata); **Rust
owns** the registries, validation, generation, process execution, and
event dispatch; **Slint owns** only the presentation of the New Project
wizard.  Nothing Slint-specific leaks into the Janet API.

---

## Plugin package format

A plugin is a directory with a `manifest.janet` calling `plugin/define`
and an entry script.  The full surface:

```text
plugins/rust/
├── manifest.janet                  plugin/define — identity + contributions
├── main.janet                      entry: modes, hooks, commands, LSP wiring
├── config.janet                    option defaults (loaded by main.janet)
├── syntax.scm                      tree-sitter highlight query for <lang>-mode
├── grammar/                        optional: grammar shared libraries
│   └── libtree-sitter-rust.dylib
└── project-templates/
    ├── binary/
    │   ├── template.janet          template/define — metadata
    │   ├── Cargo.toml              project files, {{variable}} placeholders
    │   └── src/main.rs
    └── library/…
```

`plugin/define` spec keys:

| Key | Meaning |
|-----|---------|
| `:name`, `:version`, `:entry` | Identity; entry defaults to `main.janet` |
| `:deps` | Dependencies, resolved depth-first before load |
| `:languages` | Language names the plugin serves |
| `:extensions` | `{lang [ext …]}` — file extensions mapped to `<lang>-mode` |
| `:toolchain` | One `toolchain/register` spec (see below) |
| `:capabilities` | `[:filesystem :network :process-execution :editor-access]` |

Discovery scans `<cwd>/plugins` and the user plugin root
(`plugins-root` option, default `~/.config/magma/plugins`); a directory is
a plugin exactly when it holds `manifest.janet`.  A bad manifest warns and
is skipped — one broken plugin never prevents its siblings from loading,
and a broken template manifest never prevents its plugin from loading.

### Identifiers

Plugin, language, template, and toolchain identifiers are namespaced:
templates and toolchains use `<plugin>.<name>` (lowercase letters, digits,
`.`, `-`, `_`); a duplicate id from a *different* plugin is a registration
error, while the same plugin re-registering replaces its entry.

---

## Language registration

`plugin/define` registers each `:languages` entry through
`language/define`, which accepts the conventional sibling files
(`syntax.scm`, `config.janet`) or a full spec:

```janet
(language/define
  {:id "nova"
   :extensions ["nova" "nva"]
   :grammar (string plugin-dir "/grammar/libtree-sitter-nova.dylib")
   :syntax  (string plugin-dir "/highlights.scm")
   :comments {:line "//" :block ["/*" "*/"]}
   :indentation {:size 4 :style "spaces"}})
```

What registration wires, and where it lives:

- **Registry entry** (Rust): name, extensions, asset paths, owner.
- **Grammar loading** (Rust): a `:grammar` shared library is loaded
  immediately through the tree-sitter registry and **ABI-validated** — an
  incompatible grammar is a clear warning, and the language falls back to
  the native highlighter.  Failures never block plugin loading.
- **Query validation** (Rust): when a grammar is loaded, the highlight
  query is compile-checked at registration; errors are reported instead
  of appearing as silently missing colors.
- **Extension mapping** (Janet): each extension maps to `<id>-mode` in the
  major-mode map.  First registration wins; conflicts are reported, never
  silently overridden.  The built-in extension map is unaffected unless a
  plugin language declares the same extension.
- **Highlight query** (Janet): `:syntax` becomes the mode's tree-sitter
  query (the `syntax` highlight layer transfers to the tree-sitter lane).
- **Options** (Janet): `:comments` and `:indentation` seed
  `<id>-comment-prefix` / `-comment-open` / `-comment-close` /
  `<id>-indent-width` / `-indent-style` defaults — set only when the user
  has not configured them.

Injections and folds queries are reserved for the query-based features
that consume them today (highlighting only); declaring them is not yet
meaningful.

---

## Project templates

A template is a directory of files copied into every generated project.
`template.janet` declares metadata:

```janet
(template/define
  {:id "rust.binary"
   :name "Binary Application"
   :description "Create a Rust executable project."
   :category "application"          ; wizard grouping label
   :language "rust"
   :source "."                       ; relative to the template directory
   :toolchain "rust.cargo"           ; optional, shown with availability
   :variables {:project_name {:type "project-name" :required true}
               :edition {:type "choice" :default "2021"
                         :choices ["2021" "2024"]}
               :use_tokio {:type "boolean" :default "false"}}})
```

Variable types: `text`, `project-name` (letters, digits, `-`, `_`),
`choice` (validated against `:choices`), `boolean`.  Each may be
`:required` with a `:default` and a `:description` shown in the wizard.
`project_name` is always provided to substitution — declaring it only
controls requiredness in the wizard.

### Generation

The wizard runs the `new-project` command
(`(new-project template dest name vars open)`): `dest` is the final
project directory (the wizard joins the picked folder and project name),
`name` doubles as the `project_name` variable, `vars` is a JSON object,
`open` (default true) opens the project when done.

The engine, on the background pool:

- Substitutes `{{variable}}` in file contents and path segments — a
  single pass, no shell, no evaluation.  Unknown variables and unclosed
  placeholders are errors.
- Validates every declared input (requiredness, choice membership,
  boolean literals, project-name characters) before touching the disk.
- Refuses symlinks anywhere in the template, refuses rendered path
  segments that are empty, `.`, `..`, or contain separators, and refuses
  destinations that are relative, filesystem roots, non-empty, or
  overlap the template source (symlink-resolved comparison).
- Rolls back everything it created when generation fails or is cancelled
  (`new-project-cancel`), so a half-rendered project never survives.
- `template.janet` itself is metadata, not project content.

---

## Toolchains and project actions

A toolchain declaration names a build system, how to recognize its
projects, and what can run in them:

```janet
(toolchain/register
  {:id "rust.cargo"
   :language "rust"
   :detect ["Cargo.toml"]
   :actions {"build"  {:program "cargo" :args ["build"]}
             "test"   {:program "cargo" :args ["test"]}
             "run"    {:program "cargo" :args ["run"] :debuggable true}
             "clean"  {:program "cargo" :args ["clean"]}
             "format" {:program "cargo" :args ["fmt"]}
             "lint"   {:program "cargo" :args ["clippy"]}}
   :debug {:adapter "codelldb"}}      ; optional: this project type's adapter
```

Actions are structured program/argument lists with optional project
relative `:cwd` and `:env` — plugins cannot interpolate untrusted values
into shell strings and cannot bypass the host's process-execution
policies (the task scheduler owns every spawn).

**Detection is separate from execution.**  When a project opens, built-in
marker detection runs first; a registered toolchain whose markers match
names the project type when the built-in table did not, and its actions
*fill the gaps* in the seeded run configurations — saved
`.magma/run-configs.json` and built-in defaults always win.  A project
type is declared even when its toolchain is not installed: the wizard
checks each template's toolchain programs and explains exactly what is
missing before anything is created.

Run/debug integration: `run` actions marked `:debuggable true` become
launch configurations; the existing `debug-config` command derives DAP
launch arguments from them, resolving the adapter through the language's
built-in candidate list first, then the toolchain's `:debug :adapter`
declaration, or an explicit override.  Languages without a debugger
simply declare no debuggable action; a missing adapter binary surfaces
the existing actionable error when debugging is attempted.

---

## Project detection and import

Detection needs no wizard: opening any folder runs the marker walk
(`Cargo.toml`, `go.mod`, `package.json`, `Makefile`, `pyproject.toml`,
`CMakeLists.txt`, `pom.xml`, `build.gradle[.kts]`, then every registered
toolchain's `:detect` markers in registration order).  The detected
language seeds run configurations from saved config, built-in defaults,
and toolchain actions.  Nested projects classify by their own root's
markers; the walk itself skips `node_modules`, `target`, dot-dirs, and
gitignored paths.

---

## The New Project wizard

Entries: **File → New Project…**, the launcher's **New Project…** button
(switches to the editor window), and the `new-project-dialog` command.
One screen: searchable template list (fed by the registry — no
per-language UI code), template details with toolchain availability,
destination + name fields, dynamic variable inputs (text / choice /
boolean), and a footer that explains exactly why Create is disabled.
Escape closes (when idle), Return creates, arrows move the selection.
Generation shows a "Creating project" progress chip; a failure reopens
the form with the error; success opens the generated project through the
normal project-open pipeline (walk, session, task detection — all
background).

---

## Grammar artifacts and trust model

Plugin grammars are native shared libraries that execute **in the editor
process with full privileges**.  Loading one is gated on you having
installed the plugin: there is no sandbox around the Janet runtime, and
this document does not claim one.  What the loader does enforce:

- The grammar must export `tree_sitter_<language>` (or the generic
  `tree_sitter_language`) and pass an ABI compatibility check against the
  linked tree-sitter runtime — a mismatch is a clean, reported error, not
  a silent parser failure.
- Grammar paths resolve inside the plugin package (relative paths are
  plugin-relative; the destination-safety rules above keep template
  output from ever touching the plugin directory).
- Static grammars keep working without repackaging: languages without a
  `:grammar` simply use whatever grammar is registered (or none).

All plugin declarations are untrusted input: manifests, template specs,
and toolchain actions are validated at registration with actionable
errors, and identifiers are resource-limited (template ids ≤128 chars,
templates capped at 20k files and 32 nesting levels).

---

## Testing

- `src/tests/plugin_templates_tests.rs` — registry validation, the
  rendering engine (substitution, path escapes, symlinks, rollback,
  cancellation), toolchain detection and run-config merging, and the
  `new-project` command surface including the background completion path.
- `crates/scripting/tests/plugin_templates_janet_tests.rs` — the full
  manifest pipeline against fixture plugins: language + extensions +
  toolchain + templates from one manifest, extension-conflict policy,
  end-to-end generation through the command registry, toolchain-driven
  detection with saved-config precedence, unload/reload cleanup, and
  malformed-plugin isolation.
- `crates/slint-shell/src/new_project.rs` — wizard validation rules.
