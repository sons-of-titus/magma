# Scala plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "scala" "treesitter")
(semantic/register-provider "scala" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "scala"
  ["def" "val" "var" "lazy" "object" "class" "trait" "case" "extends"
   "with" "for" "yield" "if" "then" "else" "while" "do" "match"
   "return" "new" "import" "export" "package" "sealed" "implicit"
   "override" "abstract" "final" "private" "protected" "type" "this"
   "super" "null" "true" "false" "require" "given" "using" "enum"
   "end" "throw" "try" "catch" "finally"]
  "//")

(syntax/map-extension "scala" "scala")
(syntax/map-extension "sc" "scala")

# ── Activation + LSP ─────────────────────────────────────────────────────────
# The core extension map predates this plugin, so the plugin routes its
# own extensions: its major mode wins over the core's pick.  Metals is
# project-scoped (import the build first); it starts with the first scala
# buffer of an opened project.

(def scala-exts @["scala" "sc"])

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
      (when (index-of ext scala-exts)
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "scala-mode")]
          ((get entry :activate))
          (editor/run-command "scala-mode"))
        (lsp/start "scala" (option/get "scala-lsp-command"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "scala-mode"
  {:parent "prog"
   :layers [:scala-mode]
   :setup (fn []
            (def width (option/get "scala-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
