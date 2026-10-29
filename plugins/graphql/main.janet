# Graphql plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "graphql" "treesitter")
(semantic/register-provider "graphql" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "graphql"
  ["query" "mutation" "subscription" "fragment" "on" "type" "input"
   "interface" "union" "enum" "scalar" "schema" "extend" "implements"
   "directive" "true" "false" "null"]
  "#")

(syntax/map-extension "graphql" "graphql")
(syntax/map-extension "gql" "graphql")

# ── Activation + LSP ─────────────────────────────────────────────────────────
# The core extension map predates this plugin, so the plugin routes its
# own extensions: its major mode wins over the core's pick.  The graphql
# language server is schema-backed; it starts with the first graphql
# buffer rather than waiting for a project to open.

(def graphql-exts @["graphql" "gql"])

(defn- path-extension
  "File extension of `path` (after the last dot), or nil."
  [path]
  (when-let [i (last (string/find-all "." path))]
    (string/slice path (+ i 1))))

(event/on "buffer-created"
  (fn [data]
    (def path (get data :path ""))
    (when (> (length path) 0)
      (def ext (path-extension path))
      (when (index-of ext graphql-exts)
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "graphql-mode")]
          ((get entry :activate))
          (editor/run-command "graphql-mode"))
        (lsp/start "graphql" (option/get "graphql-lsp-command"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "graphql-mode"
  {:parent "prog"
   :layers [:graphql-mode]
   :setup (fn []
            (def width (option/get "graphql-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
