# Dart plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "dart" "treesitter")
(semantic/register-provider "dart" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "dart"
  ["class" "extends" "implements" "with" "mixin" "abstract" "interface"
   "enum" "extension" "typedef" "library" "import" "export" "part" "of"
   "void" "int" "double" "num" "bool" "String" "var" "final" "const"
   "late" "static" "get" "set" "operator" "async" "await" "yield"
   "try" "catch" "finally" "throw" "rethrow" "if" "else" "for" "while"
   "do" "switch" "case" "default" "break" "continue" "return" "new"
   "this" "super" "is" "as" "in" "show" "hide" "external" "factory"
   "null" "true" "false"]
  "//")

(syntax/map-extension "dart" "dart")

# ── Activation + LSP ─────────────────────────────────────────────────────────
# The core extension map predates this plugin, so the plugin routes its
# own extensions: its major mode wins over the core's pick.  The dart
# language server ships with the dart SDK and serves single files, so it
# starts with the first dart buffer rather than waiting for a project.

(def dart-exts @["dart"])

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
      (when (index-of ext dart-exts)
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "dart-mode")]
          ((get entry :activate))
          (editor/run-command "dart-mode"))
        (lsp/start "dart" (option/get "dart-lsp-command") "language-server")))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "dart-mode"
  {:parent "prog"
   :layers [:dart-mode]
   :setup (fn []
            (def width (option/get "dart-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
