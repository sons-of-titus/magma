# ~/.magma/init.janet
# Magma main configuration file
# This file is loaded after built-in defaults and before plugins.

############################################################################
# EDITOR SETTINGS
############################################################################

(def- config
  {:theme :dark
   :tab-width 4
   :indent-width 4
   :expand-tab false            # true = spaces, false = tabs
   :font-size 14
   :line-numbers true
   :cursor-style :block         # :block, :bar, :underline
   :mouse-support true
   :scrolloff 5                 # lines of context around cursor
   :undo-limit 10000
   :clipboard :system           # :system, :internal, :none
   :encoding :utf-8
   :dangerous-permissions false # allow os/shell etc. in plugins
   :repl-timeout 5})            # seconds

############################################################################
# PLUGINS (loaded from ~/.magma/plugins/)
############################################################################

(import plugins/git :as git)
(import plugins/lsp :as lsp)
(import plugins/snippets :as snippets)

# Conditional loading
(when (= ((config :theme)) :light)
  (import plugins/themes/light :as theme))

############################################################################
# KEYBINDINGS
############################################################################

# --- Global Keybindings ---

(keymap/set "ctrl-s" "save-buffer")
(keymap/set "ctrl-o" "open-file")
(keymap/set "ctrl-z" "undo")
(keymap/set "ctrl-y" "redo")
(keymap/set "ctrl-p" "command-palette")
(keymap/set "ctrl-`" "repl-toggle")
(keymap/set "ctrl-q" "quit")

# --- Vim Mode Customizations ---

# Leader key bindings (space as leader)
(keymap/set "space space" "command-palette" :vim)
(keymap/set "space f" "open-file" :vim)
(keymap/set "space b" "buffer-next" :vim)
(keymap/set "space B" "buffer-prev" :vim)
(keymap/set "space s" "save-buffer" :vim)
(keymap/set "space w v" "split-window-v" :vim)
(keymap/set "space w s" "split-window-h" :vim)
(keymap/set "space w q" "close-window" :vim)
(keymap/set "space w w" "window-next" :vim)

# Enhanced navigation in normal mode
(keymap/set "ctrl-j" "cursor-down" :vim)
(keymap/set "ctrl-k" "cursor-up" :vim)
(keymap/set "ctrl-h" "cursor-left" :vim)
(keymap/set "ctrl-l" "cursor-right" :vim)

# Insert mode overrides
(keymap/set "ctrl-s" "save-buffer" :insert)
(keymap/set "jk" "exit-insert-mode" :insert)
(keymap/set "kj" "exit-insert-mode" :insert)

# Visual mode additions
(keymap/set "ctrl-c" "yank-selection" :visual)
(keymap/set "ctrl-x" "delete-selection" :visual)

############################################################################
# GLOBAL HOOKS
############################################################################

# Auto-trim trailing whitespace on save
(event/on :before-save
  (fn [ev]
    (def buf (:buffer-id ev))
    (when (buffer/modified? buf)
      (trim-trailing-whitespace buf)
      (ensure-trailing-newline buf))))

# Auto-indent on file open
(event/on :buffer-created
  (fn [ev]
    (lsp/attach-if-supported (:buffer-id ev))))

# Cursor position tracking in status line
(event/on :cursor-moved
  (fn [ev]
    (render/status-line (format-status-line (:window-id ev)))))

############################################################################
# CUSTOM COMMANDS
############################################################################

# --- Buffer operations ---

(command/define "buffer-next"
  (fn [& args]
    (def buffers (buffer/list))
    (def current (buffer/current))
    (def idx (index-of buffers current))
    (def next-idx (mod (+ (or idx 0) 1) (length buffers)))
    (window/set-buffer (window/current) (get buffers next-idx)))
  {:doc "Switch to the next buffer in the list"})

(command/define "buffer-prev"
  (fn [& args]
    (def buffers (buffer/list))
    (def current (buffer/current))
    (def idx (index-of buffers current))
    (def prev-idx (mod (- (or idx 0) 1) (length buffers)))
    (window/set-buffer (window/current) (get buffers prev-idx)))
  {:doc "Switch to the previous buffer in the list"})

# --- Text operations ---

(command/define "trim-trailing-whitespace"
  (fn [& args]
    (def buf (or (get args :buffer) (buffer/current)))
    (def text (buffer/slice buf 0 (buffer/len buf)))
    (def trimmed (string/trim-trailing text))
    (when (not= text trimmed)
      (buffer/replace buf 0 (buffer/len buf) trimmed)))
  {:args [{:name :buffer :type :buffer :optional true}]
   :doc "Remove trailing whitespace from the buffer"})

(command/define "ensure-trailing-newline"
  (fn [& args]
    (def buf (or (get args :buffer) (buffer/current)))
    (def len (buffer/len buf))
    (when (and (> len 0) (not= "\n" (buffer/slice buf (- len 1) len)))
      (buffer/insert buf len "\n")))
  {:args [{:name :buffer :type :buffer :optional true}]
   :doc "Ensure the buffer ends with a newline"})

# --- Editor operations ---

(command/define "repl-toggle"
  (fn [& args]
    (def repl-buf (find-repl-buffer))
    (if repl-buf
      (window/set-buffer (window/current) repl-buf)
      (repl/start)))
  {:doc "Toggle the REPL buffer"})

(command/define "command-palette"
  (fn [& args]
    (def commands (command/list))
    (def palette-buf (buffer/create "*palette*"))
    (buffer/insert palette-buf 0 (string/join commands "\n"))
    (window/set-buffer (window/current) palette-buf))
  {:doc "Open the command palette"})

############################################################################
# STATUS LINE
############################################################################

(defn format-status-line [window-id]
  (def buf (window/buffer window-id))
  (def name (buffer/name buf))
  (def path (buffer/path buf))
  (def modified (if (buffer/modified? buf) " ●" ""))
  (def mode (string/upper (name (buffer/major-mode buf))))
  (def row (window/cursor-row window-id))
  (def col (window/cursor-col window-id))
  (def percent (math/floor (* 100 (/ row (buffer/line-count buf)))))
  (string/format " %s%s  [%s]  Ln %d Col %d  %d%%"
    name modified mode (+ row 1) (+ col 1) percent))

############################################################################
# MINOR MODE EXAMPLES
############################################################################

# Markdown preview mode
(defn markdown-mode []
  (keymap/push-layer :markdown)
  (keymap/set "ctrl-p" "markdown-preview" :markdown)
  (event/on :buffer-changed markdown-auto-preview))

# Enable markdown mode for .md files
(event/on :buffer-created
  (fn [ev]
    (def name (buffer/name (:buffer-id ev)))
    (when (string/has-suffix? ".md" name)
      (markdown-mode))))

############################################################################
# HELPER FUNCTIONS
############################################################################

(defn index-of [arr elem]
  (var i 0)
  (while (< i (length arr))
    (when (= (get arr i) elem)
      (break i))
    (++ i))
  nil)

(defn find-repl-buffer []
  (var result nil)
  (each buf (buffer/list)
    (when (= "*repl*" (buffer/name buf))
      (set result buf)))
  result)

(print "Magma initialized: " (editor/version))
