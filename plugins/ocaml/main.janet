# Ocaml plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "ocaml" "treesitter")
(semantic/register-provider "ocaml" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "ocaml"
  ["let" "in" "rec" "if" "then" "else" "match" "with" "when" "begin"
   "end" "type" "of" "and" "module" "sig" "struct" "functor" "open"
   "include" "try" "exception" "function" "fun" "mutable" "ref" "true"
   "false" "not" "ignore" "as" "private" "virtual" "inherit"
   "constraint" "lazy" "assert"]
  "(*")

(syntax/map-extension "ml" "ocaml")
(syntax/map-extension "mli" "ocaml")

# ── Activation + LSP ─────────────────────────────────────────────────────────
# The core extension map predates this plugin, so the plugin routes its
# own extensions: its major mode wins over the core's pick.  ocamllsp is
# project-scoped (dune/merlin configuration); it starts with the first
# OCaml buffer of an opened project.

(def ocaml-exts @["ml" "mli"])

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
      (when (index-of ext ocaml-exts)
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "ocaml-mode")]
          ((get entry :activate))
          (editor/run-command "ocaml-mode"))
        (lsp/start "ocaml" (option/get "ocaml-lsp-command"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "ocaml-mode"
  {:parent "prog"
   :layers [:ocaml-mode]
   :setup (fn []
            (def width (option/get "ocaml-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
