# R plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "r" "treesitter")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "r"
  ["function" "if" "else" "for" "while" "repeat" "break" "next"
   "return" "switch" "in" "TRUE" "FALSE" "NULL" "NA" "NaN" "Inf"
   "library" "require" "source" "print" "cat" "length" "mean" "median"
   "sum" "min" "max" "apply" "sapply" "lapply" "mapply" "tapply"]
  "#")

(syntax/map-extension "r" "r")
(syntax/map-extension "R" "r")

# ── Activation ───────────────────────────────────────────────────────────────
# The core extension map predates this plugin, so the plugin routes its
# own extensions: its major mode wins over the core's pick.

(def r-exts @["r" "R"])

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
      (when (index-of ext r-exts)
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "r-mode")]
          ((get entry :activate))
          (editor/run-command "r-mode"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "r-mode"
  {:parent "prog"
   :layers [:r-mode]
   :setup (fn []
            (def width (option/get "r-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
