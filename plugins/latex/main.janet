# Latex plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "latex" "treesitter")
(semantic/register-provider "latex" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "latex"
  ["begin" "end" "section" "subsection" "subsubsection" "paragraph"
   "documentclass" "usepackage" "title" "author" "date" "maketitle"
   "textbf" "textit" "emph" "cite" "ref" "label" "includegraphics"
   "footnote" "itemize" "enumerate" "table" "figure" "equation"
   "centering" "input" "include" "bibliography" "citep" "citet"
   "caption" "item" "providecommand" "newcommand" "renewcommand"
   "usemintedstyle" "setlength" "addcontentsline"]
  "%")

(syntax/map-extension "tex" "latex")
(syntax/map-extension "sty" "latex")
(syntax/map-extension "cls" "latex")

# ── Activation + LSP ─────────────────────────────────────────────────────────
# The core extension map predates this plugin, so the plugin routes its
# own extensions: its major mode wins over the core's pick.  texlab is
# project-scoped (compile targets, bibliography); it starts with the first
# latex buffer of an opened project.

(def latex-exts @["tex" "sty" "cls"])

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
      (when (index-of ext latex-exts)
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "latex-mode")]
          ((get entry :activate))
          (editor/run-command "latex-mode"))
        (lsp/start "latex" (option/get "latex-lsp-command"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "latex-mode"
  {:parent "text"
   :layers [:latex-mode]
   :setup (fn []
            (def width (option/get "latex-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
