# Groovy plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "groovy" "treesitter")
(semantic/register-provider "groovy" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "groovy"
  ["def" "if" "else" "for" "while" "in" "return" "switch" "case"
   "break" "continue" "import" "package" "new" "try" "catch" "finally"
   "throw" "class" "interface" "trait" "extends" "implements" "public"
   "private" "protected" "static" "final" "abstract" "void" "boolean"
   "int" "long" "double" "float" "String" "true" "false" "null" "it"
   "as" "this" "super" "assert" "synchronized" "volatile" "transient"
   "native" "enum"]
  "//")

(syntax/map-extension "groovy" "groovy")
(syntax/map-extension "gradle" "groovy")
(syntax/map-extension "jenkinsfile" "groovy")

# ── Activation + LSP ─────────────────────────────────────────────────────────
# The core extension map predates this plugin, so the plugin routes its
# own extensions: its major mode wins over the core's pick.  Jenkinsfile
# has no extension, so the plugin matches the basename too.

(def groovy-exts @["groovy" "gradle" "jenkinsfile"])
(def groovy-names @["Jenkinsfile"])

(defn- path-extension
  "File extension of `path` (after the last dot), or nil."
  [path]
  (when-let [i (last (string/find-all "." path))]
    (string/slice path (+ i 1))))

(defn- file-basename
  "Last path segment, with backslash separators normalized."
  [path]
  (def norm (string/replace-all "\\" "/" path))
  (if-let [i (last (string/find-all "/" norm))]
    (string/slice norm (+ i 1))
    norm))

(event/on "buffer-created"
  (fn [data]
    (def path (get data :path ""))
    (when (> (length path) 0)
      (def ext (path-extension path))
      (when (or (index-of ext groovy-exts)
                (index-of (file-basename path) groovy-names))
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "groovy-mode")]
          ((get entry :activate))
          (editor/run-command "groovy-mode"))
        (lsp/start "groovy" (option/get "groovy-lsp-command"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "groovy-mode"
  {:parent "prog"
   :layers [:groovy-mode]
   :setup (fn []
            (def width (option/get "groovy-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
