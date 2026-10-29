# Ini plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "ini" "treesitter")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "ini"
  ["true" "false" "yes" "no" "on" "off" "null"]
  ";")

(each ext ["ini" "cfg" "conf" "properties" "env"]
  (syntax/map-extension ext "ini"))

# ── Activation ───────────────────────────────────────────────────────────────
# The core extension map predates this plugin, so the plugin routes its
# own extensions: its major mode wins over the core's pick.

(def ini-exts @["ini" "cfg" "conf" "properties" "env"])

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
      (when (index-of ext ini-exts)
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "ini-mode")]
          ((get entry :activate))
          (editor/run-command "ini-mode"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "ini-mode"
  {:parent "text"
   :layers [:ini-mode]
   :setup (fn []
            (def width (option/get "ini-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
