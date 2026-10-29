# Powershell plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "powershell" "treesitter")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "powershell"
  ["if" "elseif" "else" "switch" "foreach" "for" "while" "do" "until"
   "break" "continue" "return" "throw" "try" "catch" "finally" "trap"
   "function" "filter" "param" "begin" "process" "end" "in" "exit"
   "class" "enum" "using" "workflow" "configuration" "namespace"
   "module" "data" "dynamicparam" "default" "hidden" "static"
   "true" "false" "null"]
  "#")

(syntax/map-extension "ps1" "powershell")
(syntax/map-extension "psm1" "powershell")
(syntax/map-extension "psd1" "powershell")

# ── Activation ───────────────────────────────────────────────────────────────
# The core extension map predates this plugin, so the plugin routes its
# own extensions: its major mode wins over the core's pick.

(def powershell-exts @["ps1" "psm1" "psd1"])

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
      (when (index-of ext powershell-exts)
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "powershell-mode")]
          ((get entry :activate))
          (editor/run-command "powershell-mode"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "powershell-mode"
  {:parent "prog"
   :layers [:powershell-mode]
   :setup (fn []
            (def width (option/get "powershell-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
