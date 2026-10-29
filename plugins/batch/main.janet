# Batch plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "batch" "treesitter")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "batch"
  ["echo" "set" "if" "else" "for" "goto" "call" "shift" "errorlevel"
   "exist" "defined" "not" "equ" "neq" "lss" "leq" "gtr" "geq" "pause"
   "cd" "copy" "del" "move" "md" "rd" "ren" "cls" "exit" "start"
   "setlocal" "endlocal" "rem" "enableextensions" "enabledelayedexpansion"]
  "rem")

(syntax/map-extension "bat" "batch")
(syntax/map-extension "cmd" "batch")

# ── Activation ───────────────────────────────────────────────────────────────
# The core extension map predates this plugin, so the plugin routes its
# own extensions: its major mode wins over the core's pick.

(def batch-exts @["bat" "cmd"])

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
      (when (index-of ext batch-exts)
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "batch-mode")]
          ((get entry :activate))
          (editor/run-command "batch-mode"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "batch-mode"
  {:parent "prog"
   :layers [:batch-mode]
   :setup (fn []
            (def width (option/get "batch-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
