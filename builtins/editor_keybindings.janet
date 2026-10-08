# Default keybindings — one non-modal layer.
#
# Magma ships without modal editing: buffers always accept text, and every
# action lives on a chord or in the palette.  The keymap layer registry and
# `editor/set-mode` remain as extension substrate — a community vim plugin
# would push its own layers on top, exactly like the dialogs do.

# ── Helper ────────────────────────────────────────────────────────────────────

(defn enter-editor-mode
  "Reset to the default editing context (one keymap layer)."
  [&]
  (each l ["insert" "visual" "replace" "command" "search" "terminal"]
    (keymap/pop-layer l))
  (keymap/push-layer "editor"))

(event/on "editor-ready" (fn [_] (enter-editor-mode)))

# ── Selection helpers ─────────────────────────────────────────────────────────

(defn- select-move [cmd]
  (def b (buffer/current))
  (when b
    (when (nil? (selection/get))
      (selection/set (buffer/cursor b) "char"))
    (editor/run-command cmd)))

(command/define "select-left"      (fn [&] (select-move "cursor-left")))
(command/define "select-right"     (fn [&] (select-move "cursor-right")))
(command/define "select-up"        (fn [&] (select-move "cursor-up")))
(command/define "select-down"      (fn [&] (select-move "cursor-down")))
(command/define "select-line-start" (fn [&] (select-move "line-start")))
(command/define "select-line-end"  (fn [&] (select-move "line-end")))

(command/define "editor-delete"
  (fn [&]
    (if (selection/get)
      (editor/run-command "delete-selection")
      (editor/run-command "delete-char"))))

(command/define "editor-backspace"
  (fn [&]
    (if (selection/get)
      (editor/run-command "delete-selection")
      (editor/run-command "backspace"))))

# editor-escape is a Rust command now (esc clears any active selection).

# Clipboard — real commands for the chords below.  With no selection,
# copy/cut operate on the current line (modern-editor convention).
(command/define "editor-copy"
  (fn [&]
    (if (selection/get)
      (editor/run-command "yank-selection")
      (editor/run-command "yank-line"))))

(command/define "editor-cut"
  (fn [&]
    (if (selection/get)
      (editor/run-command "cut-selection")
      (do
        (editor/run-command "yank-line")
        (editor/run-command "delete-line")))))

(command/define "editor-paste"
  (fn [&]
    (def b (buffer/current))
    (when b
      (def text (clipboard/get))
      (when text
        (def pos (buffer/cursor b))
        (buffer/insert b pos text)
        (buffer/set-cursor b (+ pos (length text)))))))

(command/define "select-all"
  (fn [&]
    (def b (buffer/current))
    (when b
      (selection/set 0 "char")
      (buffer/set-cursor b (buffer/len b)))))

# ── Default layer ─────────────────────────────────────────────────────────────

# Movement
(keymap/set "left"  "cursor-left"  :editor)
(keymap/set "right" "cursor-right" :editor)
(keymap/set "up"    "cursor-up"    :editor)
(keymap/set "down"  "cursor-down"  :editor)
(keymap/set "home"  "line-start"   :editor)
(keymap/set "end"   "line-end"     :editor)
(keymap/set "page-up"   "scroll-page-up"   :editor)
(keymap/set "page-down" "scroll-page-down" :editor)

# Selection (shift + movement)
(keymap/set "shift-left"  "select-left"  :editor)
(keymap/set "shift-right" "select-right" :editor)
(keymap/set "shift-up"    "select-up"    :editor)
(keymap/set "shift-down"  "select-down"  :editor)
(keymap/set "shift-home"  "select-line-start" :editor)
(keymap/set "shift-end"   "select-line-end"   :editor)
(keymap/set "esc" "editor-escape" :editor)

# Editing
(keymap/set "backspace" "editor-backspace" :editor)
(keymap/set "delete"    "editor-delete"    :editor)
(keymap/set "return"    "newline"          :editor)
(keymap/set "tab"       "indent-line"      :editor)
(keymap/set "shift-tab" "deindent-line"    :editor)

# Clipboard + selection
(keymap/set "ctrl-c" "editor-copy" :editor)
(keymap/set "ctrl-x" "editor-cut"  :editor)
(keymap/set "ctrl-v" "editor-paste" :editor)
(keymap/set "ctrl-a" "select-all"   :editor)

# Completion — works while typing, no mode entry required.
# ctrl-p stays with the palette (GUI-reserved); cycling back is one
# keymap line away for any community layout.
(keymap/set "ctrl-n"     "completion-next"    :editor)
(keymap/set "ctrl-space" "completion-trigger" :editor)

# Navigation
(keymap/set "ctrl-]" "goto-definition" :editor)
(keymap/set "shift-k" "lsp-hover"      :editor)
(keymap/set "ctrl-o" "mark-ring-jump-back"    :editor)
(keymap/set "ctrl-i" "mark-ring-jump-forward" :editor)

# Session (global)
(keymap/set "ctrl-s" "save-buffer")
(keymap/set "ctrl-z" "undo")
(keymap/set "ctrl-shift-z" "redo")
(keymap/set "ctrl-w" "close-buffer")

(keymap/push-layer "editor")
(print "Standard editor keybindings loaded")
