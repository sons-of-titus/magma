# ~/.magma/plugins/vim.janet
# Vim emulation mode — default major mode for Magma
#
# This plugin implements Vim-style modal editing with normal, insert,
# visual, and command-line modes. It is loaded by default but can be
# replaced or modified by the user.

############################################################################
# MODE STATE
############################################################################

(def- MODE :normal)            # :normal, :insert, :visual, :visual-line, :visual-block
(def- LAST-COMMAND nil)        # For dot-repeat (.)
(def- YANK-REGISTER "")        # Yank buffer
(def- SEARCH-STRING "")        # Last search pattern
(def- SEARCH-DIRECTION :forward)

# Mode stack for visual/normal transitions
(def- MODE-STACK @[])

############################################################################
# MODE TRANSITIONS
############################################################################

(defn set-mode [new-mode]
  (when (not= MODE new-mode)
    (event/emit :mode-exited {:mode MODE :kind :major})
    (set MODE new-mode)
    (event/emit :mode-entered {:mode MODE :kind :major})))

(defn enter-insert-mode []
  (keymap/push-layer :insert)
  (set-mode :insert))

(defn enter-insert-mode-append []
  (cursor-right)
  (enter-insert-mode))

(defn enter-insert-mode-line-start []
  (cursor-line-start)
  (enter-insert-mode))

(defn enter-insert-mode-line-end []
  (cursor-line-end)
  (enter-insert-mode))

(defn enter-insert-mode-append-end []
  (cursor-line-end)
  (enter-insert-mode))

(defn open-line-below []
  (cursor-line-end)
  (buffer/insert (buffer/current) (buffer/cursor (buffer/current)) "\n")
  (enter-insert-mode))

(defn open-line-above []
  (cursor-line-start)
  (buffer/insert (buffer/current) (buffer/cursor (buffer/current)) "\n")
  (cursor-up)
  (enter-insert-mode))

(defn exit-insert-mode []
  (keymap/pop-layer :insert)
  (cursor-left)                # Consistent with Vim behavior
  (set-mode :normal))

(defn enter-visual-mode []
  (def start (buffer/cursor (buffer/current)))
  (push MODE-STACK {:mode :visual :start start :end start})
  (keymap/push-layer :visual)
  (set-mode :visual))

(defn enter-visual-line-mode []
  (def start (buffer/cursor (buffer/current)))
  (def line-start (*line-start* (buffer-current-line)))
  (def line-end (*line-end* (buffer-current-line)))
  (push MODE-STACK {:mode :visual-line :start line-start :end line-end})
  (keymap/push-layer :visual)
  (set-mode :visual))

(defn exit-visual-mode []
  (keymap/pop-layer :visual)
  (pop MODE-STACK)
  (set-mode :normal))

(defn escape []
  (case MODE
    :normal nil
    :insert (exit-insert-mode)
    :visual (exit-visual-mode)
    :command-line (exit-command-line-mode)
    (set-mode :normal)))

############################################################################
# CURSOR MOTION COMMANDS
############################################################################

(defn cursor-left []
  (def buf (buffer/current))
  (def pos (max 0 (- (buffer/cursor buf) 1)))
  (buffer/cursor buf pos))

(defn cursor-right []
  (def buf (buffer/current))
  (def pos (min (buffer/len buf) (+ (buffer/cursor buf) 1)))
  (buffer/cursor buf pos))

(defn cursor-up []
  (def buf (buffer/current))
  (def current-line (buffer-current-line))
  (when (> current-line 0)
    (def target-line (- current-line 1))
    (def target-col (buffer-current-col))
    (move-to-line-col buf target-line target-col)))

(defn cursor-down []
  (def buf (buffer/current))
  (def current-line (buffer-current-line))
  (when (< current-line (- (buffer/line-count buf) 1))
    (def target-line (+ current-line 1))
    (def target-col (buffer-current-col))
    (move-to-line-col buf target-line target-col)))

(defn cursor-line-start []
  (def buf (buffer/current))
  (def line-num (buffer-current-line))
  (def line-start (get (buffer/line-start-offsets buf) line-num))
  (buffer/cursor buf (or line-start 0)))

(defn cursor-line-end []
  (def buf (buffer/current))
  (def line-num (buffer-current-line))
  (def line (buffer/line buf line-num))
  (def line-start (get (buffer/line-start-offsets buf) line-num))
  (buffer/cursor buf (+ (or line-start 0) (length (or line "")))))

(defn cursor-first-nonblank []
  (def buf (buffer/current))
  (def line-num (buffer-current-line))
  (def line (buffer/line buf line-num))
  (def line-start (get (buffer/line-start-offsets buf) line-num))
  (def first-nonblank (or (string/find-first-not " \t" (or line "")) 0))
  (buffer/cursor buf (+ (or line-start 0) first-nonblank)))

(defn cursor-buffer-start []
  (buffer/cursor (buffer/current) 0))

(defn cursor-buffer-end []
  (buffer/cursor (buffer/current) (buffer/len (buffer/current))))

(defn word-forward [&opt count]
  (set count (or count 1))
  (def buf (buffer/current))
  (var pos (buffer/cursor buf))
  (def text (buffer/slice buf 0 (buffer/len buf)))
  (repeat count
    (set pos (next-word-boundary text pos)))
  (buffer/cursor buf pos))

(defn word-backward [&opt count]
  (set count (or count 1))
  (def buf (buffer/current))
  (var pos (buffer/cursor buf))
  (def text (buffer/slice buf 0 (buffer/len buf)))
  (repeat count
    (set pos (prev-word-boundary text pos)))
  (buffer/cursor buf pos))

(defn word-end-forward [&opt count]
  (set count (or count 1))
  (def buf (buffer/current))
  (var pos (buffer/cursor buf))
  (def text (buffer/slice buf 0 (buffer/len buf)))
  (repeat count
    (set pos (next-word-end text pos)))
  (buffer/cursor buf pos))

############################################################################
# TEXT OBJECT OPERATIONS
############################################################################

(defn delete-char-forward [&opt count]
  (set count (or count 1))
  (def buf (buffer/current))
  (def pos (buffer/cursor buf))
  (buffer/delete buf pos (min (+ pos count) (buffer/len buf))))

(defn delete-char-backward [&opt count]
  (set count (or count 1))
  (def buf (buffer/current))
  (def pos (buffer/cursor buf))
  (def start (max 0 (- pos count)))
  (buffer/delete buf start pos)
  (buffer/cursor buf start))

(defn delete-line [&opt count]
  (set count (or count 1))
  (def buf (buffer/current))
  (def current-line (buffer-current-line))
  (def line-start (get (buffer/line-start-offsets buf) current-line))
  (def line-end (if (< (+ current-line count) (buffer/line-count buf))
                  (get (buffer/line-start-offsets buf) (+ current-line count))
                  (buffer/len buf)))
  (def deleted (buffer/slice buf line-start line-end))
  (set YANK-REGISTER deleted)
  (buffer/delete buf line-start line-end))

(defn yank-line []
  (def buf (buffer/current))
  (def current-line (buffer-current-line))
  (def line-start (get (buffer/line-start-offsets buf) current-line))
  (def line-end (if (< (+ current-line 1) (buffer/line-count buf))
                  (get (buffer/line-start-offsets buf) (+ current-line 1))
                  (buffer/len buf)))
  (set YANK-REGISTER (buffer/slice buf line-start line-end)))

(defn yank-word []
  (def buf (buffer/current))
  (def text (buffer/slice buf 0 (buffer/len buf)))
  (def pos (buffer/cursor buf))
  (def word-end (next-word-end text pos))
  (set YANK-REGISTER (buffer/slice buf pos word-end)))

(defn paste-after []
  (def buf (buffer/current))
  (def pos (buffer/cursor buf))
  (when (> (length YANK-REGISTER) 0)
    (buffer/insert buf pos YANK-REGISTER)))

(defn paste-before []
  (def buf (buffer/current))
  (def pos (buffer/cursor buf))
  (when (> (length YANK-REGISTER) 0)
    (buffer/insert buf pos YANK-REGISTER)))

(defn delete-word []
  (def buf (buffer/current))
  (def text (buffer/slice buf 0 (buffer/len buf)))
  (def pos (buffer/cursor buf))
  (def word-end (next-word-end text pos))
  (set YANK-REGISTER (buffer/slice buf pos word-end))
  (buffer/delete buf pos word-end))

(defn delete-to-line-end []
  (def buf (buffer/current))
  (def pos (buffer/cursor buf))
  (def line-num (buffer-current-line))
  (def line (buffer/line buf line-num))
  (def line-start (get (buffer/line-start-offsets buf) line-num))
  (def line-end (+ line-start (length (or line ""))))
  (set YANK-REGISTER (buffer/slice buf pos line-end))
  (buffer/delete buf pos line-end))

(defn delete-to-line-start []
  (def buf (buffer/current))
  (def pos (buffer/cursor buf))
  (def line-num (buffer-current-line))
  (def line-start (get (buffer/line-start-offsets buf) line-num))
  (set YANK-REGISTER (buffer/slice buf line-start pos))
  (buffer/delete buf line-start pos))

############################################################################
# VISUAL MODE OPERATIONS
############################################################################

(defn visual-select-left []
  (def state (last MODE-STACK))
  (def buf (buffer/current))
  (def pos (max 0 (- (buffer/cursor buf) 1)))
  (buffer/cursor buf pos)
  (put state :end pos))

(defn visual-select-right []
  (def state (last MODE-STACK))
  (def buf (buffer/current))
  (def pos (min (buffer/len buf) (+ (buffer/cursor buf) 1)))
  (buffer/cursor buf pos)
  (put state :end pos))

(defn delete-selection []
  (def state (last MODE-STACK))
  (def buf (buffer/current))
  (def start (min (state :start) (state :end)))
  (def end (max (state :start) (state :end)))
  (set YANK-REGISTER (buffer/slice buf start end))
  (buffer/delete buf start end)
  (buffer/cursor buf start)
  (exit-visual-mode))

(defn yank-selection []
  (def state (last MODE-STACK))
  (def buf (buffer/current))
  (def start (min (state :start) (state :end)))
  (def end (max (state :start) (state :end)))
  (set YANK-REGISTER (buffer/slice buf start end))
  (exit-visual-mode))

(defn replace-selection []
  (def state (last MODE-STACK))
  (def buf (buffer/current))
  (def start (min (state :start) (state :end)))
  (def end (max (state :start) (state :end)))
  (buffer/delete buf start end)
  (buffer/insert buf start YANK-REGISTER)
  (exit-visual-mode))

############################################################################
# SEARCH
############################################################################

(defn search-forward [pattern]
  (set SEARCH-STRING pattern)
  (set SEARCH-DIRECTION :forward)
  (def buf (buffer/current))
  (def text (buffer/slice buf 0 (buffer/len buf)))
  (def pos (buffer/cursor buf))
  (def match (string/find pattern text pos))
  (when match
    (buffer/cursor buf match)))

(defn search-backward [pattern]
  (set SEARCH-STRING pattern)
  (set SEARCH-DIRECTION :backward)
  (def buf (buffer/current))
  (def text (buffer/slice buf 0 (buffer/len buf)))
  (def pos (buffer/cursor buf))
  (def match (string/rfind pattern text (- pos 1)))
  (when match
    (buffer/cursor buf match)))

(defn search-next []
  (if (= SEARCH-DIRECTION :forward)
    (search-forward SEARCH-STRING)
    (search-backward SEARCH-STRING)))

(defn search-prev []
  (if (= SEARCH-DIRECTION :forward)
    (search-backward SEARCH-STRING)
    (search-forward SEARCH-STRING)))

############################################################################
# HELPER FUNCTIONS
############################################################################

(defn buffer-current-line []
  (def buf (buffer/current))
  (def pos (buffer/cursor buf))
  (def offsets (buffer/line-start-offsets buf))
  (var line 0)
  (each o offsets
    (when (<= o pos)
      (++ line)))
  (max 0 (- line 1)))

(defn buffer-current-col []
  (def buf (buffer/current))
  (def line-num (buffer-current-line))
  (def line-start (get (buffer/line-start-offsets buf) line-num))
  (- (buffer/cursor buf) (or line-start 0)))

(defn move-to-line-col [buf line col]
  (def line-start (get (buffer/line-start-offsets buf) line))
  (def line-text (buffer/line buf line))
  (def line-len (length (or line-text "")))
  (buffer/cursor buf (+ (or line-start 0) (min col line-len))))

(defn next-word-boundary [text pos]
  (def len (length text))
  (when (>= pos len) (break len))
  (var i pos)
  (while (and (< i len) (not (word-char? (get text i))))
    (++ i))
  (while (and (< i len) (word-char? (get text i)))
    (++ i))
  i)

(defn prev-word-boundary [text pos]
  (when (<= pos 0) (break 0))
  (var i (- pos 1))
  (while (and (> i 0) (not (word-char? (get text i))))
    (-- i))
  (while (and (> i 0) (word-char? (get text (- i 1))))
    (-- i))
  i)

(defn next-word-end [text pos]
  (def len (length text))
  (def next-boundary (next-word-boundary text pos))
  (var i next-boundary)
  (while (and (< i len) (word-char? (get text i)))
    (++ i))
  (when (= i next-boundary)
    (set i (+ i 1)))
  (- i 1))

(defn word-char? [c]
  (or (and (>= c "a") (<= c "z"))
      (and (>= c "A") (<= c "Z"))
      (and (>= c "0") (<= c "9"))
      (= c "_")))

(defn buffer/line-start-offsets [buf]
  (def lines (buffer/lines buf))
  (var offset 0)
  (def offsets @[])
  (each line lines
    (array/push offsets offset)
    (set offset (+ offset (length line) 1)))  # +1 for newline
  offsets)

############################################################################
# COMMAND REGISTRATION
############################################################################

# Normal mode commands
(command/define "cursor-left" cursor-left
  {:doc "Move cursor left"})
(command/define "cursor-right" cursor-right
  {:doc "Move cursor right"})
(command/define "cursor-up" cursor-up
  {:doc "Move cursor up"})
(command/define "cursor-down" cursor-down
  {:doc "Move cursor down"})
(command/define "cursor-line-start" cursor-line-start
  {:doc "Move cursor to beginning of line"})
(command/define "cursor-line-end" cursor-line-end
  {:doc "Move cursor to end of line"})
(command/define "cursor-first-nonblank" cursor-first-nonblank
  {:doc "Move cursor to first non-whitespace character"})
(command/define "cursor-buffer-start" cursor-buffer-start
  {:doc "Move cursor to beginning of buffer"})
(command/define "cursor-buffer-end" cursor-buffer-end
  {:doc "Move cursor to end of buffer"})
(command/define "word-forward" word-forward
  {:doc "Move cursor forward by one word"})
(command/define "word-backward" word-backward
  {:doc "Move cursor backward by one word"})
(command/define "word-end-forward" word-end-forward
  {:doc "Move cursor to end of word"})
(command/define "enter-insert-mode" enter-insert-mode
  {:doc "Enter insert mode"})
(command/define "enter-insert-mode-append" enter-insert-mode-append
  {:doc "Enter insert mode after cursor"})
(command/define "enter-insert-mode-line-start" enter-insert-mode-line-start
  {:doc "Enter insert mode at line start"})
(command/define "enter-insert-mode-line-end" enter-insert-mode-line-end
  {:doc "Enter insert mode at line end"})
(command/define "enter-insert-mode-append-end" enter-insert-mode-append-end
  {:doc "Enter insert mode at end of line"})
(command/define "open-line-below" open-line-below
  {:doc "Open a new line below and enter insert mode"})
(command/define "open-line-above" open-line-above
  {:doc "Open a new line above and enter insert mode"})
(command/define "delete-char-forward" delete-char-forward
  {:doc "Delete character under cursor"})
(command/define "delete-char-backward" delete-char-backward
  {:doc "Delete character before cursor"})
(command/define "delete-line" delete-line
  {:doc "Delete current line"})
(command/define "delete-word" delete-word
  {:doc "Delete word forward"})
(command/define "delete-to-line-end" delete-to-line-end
  {:doc "Delete from cursor to end of line"})
(command/define "delete-to-line-start" delete-to-line-start
  {:doc "Delete from cursor to beginning of line"})
(command/define "yank-line" yank-line
  {:doc "Yank current line"})
(command/define "yank-word" yank-word
  {:doc "Yank word under cursor"})
(command/define "paste-after" paste-after
  {:doc "Paste after cursor"})
(command/define "paste-before" paste-before
  {:doc "Paste before cursor"})
(command/define "enter-visual-mode" enter-visual-mode
  {:doc "Enter visual selection mode"})
(command/define "exit-insert-mode" exit-insert-mode
  {:doc "Exit insert mode"})
(command/define "escape" escape
  {:doc "Generic escape handler"})
(command/define "search-forward" search-forward
  {:doc "Search forward for a pattern"})
(command/define "search-backward" search-backward
  {:doc "Search backward for a pattern"})
(command/define "search-next" search-next
  {:doc "Jump to next search match"})
(command/define "search-prev" search-prev
  {:doc "Jump to previous search match"})

# Visual mode commands
(command/define "visual-select-left" visual-select-left
  {:doc "Extend visual selection left"})
(command/define "visual-select-right" visual-select-right
  {:doc "Extend visual selection right"})
(command/define "delete-selection" delete-selection
  {:doc "Delete visual selection"})
(command/define "yank-selection" yank-selection
  {:doc "Yank visual selection"})
(command/define "replace-selection" replace-selection
  {:doc "Replace visual selection with yanked text"})
(command/define "exit-visual-mode" exit-visual-mode
  {:doc "Exit visual selection mode"})

############################################################################
# NORMAL MODE KEYBINDINGS
############################################################################

(defn setup-vim-keybindings []

  # Basic motion
  (keymap/set "h" "cursor-left" :vim)
  (keymap/set "j" "cursor-down" :vim)
  (keymap/set "k" "cursor-up" :vim)
  (keymap/set "l" "cursor-right" :vim)
  (keymap/set "w" "word-forward" :vim)
  (keymap/set "b" "word-backward" :vim)
  (keymap/set "e" "word-end-forward" :vim)
  (keymap/set "0" "cursor-line-start" :vim)
  (keymap/set "^" "cursor-first-nonblank" :vim)
  (keymap/set "$" "cursor-line-end" :vim)
  (keymap/set "gg" "cursor-buffer-start" :vim)
  (keymap/set "G" "cursor-buffer-end" :vim)

  # Scrolling
  (keymap/set "ctrl-d" "page-down-half" :vim)
  (keymap/set "ctrl-u" "page-up-half" :vim)
  (keymap/set "ctrl-f" "page-down" :vim)
  (keymap/set "ctrl-b" "page-up" :vim)

  # Insert mode entry
  (keymap/set "i" "enter-insert-mode" :vim)
  (keymap/set "I" "enter-insert-mode-line-start" :vim)
  (keymap/set "a" "enter-insert-mode-append" :vim)
  (keymap/set "A" "enter-insert-mode-line-end" :vim)
  (keymap/set "o" "open-line-below" :vim)
  (keymap/set "O" "open-line-above" :vim)

  # Deletion
  (keymap/set "x" "delete-char-forward" :vim)
  (keymap/set "X" "delete-char-backward" :vim)
  (keymap/set "dd" "delete-line" :vim)
  (keymap/set "dw" "delete-word" :vim)
  (keymap/set "d$" "delete-to-line-end" :vim)
  (keymap/set "d0" "delete-to-line-start" :vim)

  # Yank and paste
  (keymap/set "yy" "yank-line" :vim)
  (keymap/set "yw" "yank-word" :vim)
  (keymap/set "p" "paste-after" :vim)
  (keymap/set "P" "paste-before" :vim)

  # Undo/redo
  (keymap/set "u" "undo" :vim)
  (keymap/set "ctrl-r" "redo" :vim)

  # Search
  (keymap/set "/" "search-forward" :vim)
  (keymap/set "?" "search-backward" :vim)
  (keymap/set "n" "search-next" :vim)
  (keymap/set "N" "search-prev" :vim)

  # Visual mode
  (keymap/set "v" "enter-visual-mode" :vim)
  (keymap/set "V" "enter-visual-line-mode" :vim)

  # Escape
  (keymap/set "esc" "escape" :vim)
  (keymap/set "ctrl-c" "escape" :vim)
  (keymap/set "ctrl-[" "escape" :vim))

############################################################################
# INSERT MODE KEYBINDINGS
############################################################################

(defn setup-insert-keybindings []
  (keymap/set "esc" "exit-insert-mode" :insert)
  (keymap/set "ctrl-c" "exit-insert-mode" :insert)
  (keymap/set "ctrl-[" "exit-insert-mode" :insert)
  (keymap/set "ctrl-h" "delete-char-backward" :insert))

############################################################################
# VISUAL MODE KEYBINDINGS
############################################################################

(defn setup-visual-keybindings []
  (keymap/set "h" "visual-select-left" :visual)
  (keymap/set "j" "visual-select-down" :visual)
  (keymap/set "k" "visual-select-up" :visual)
  (keymap/set "l" "visual-select-right" :visual)
  (keymap/set "d" "delete-selection" :visual)
  (keymap/set "x" "delete-selection" :visual)
  (keymap/set "y" "yank-selection" :visual)
  (keymap/set "p" "replace-selection" :visual)
  (keymap/set "esc" "exit-visual-mode" :visual)
  (keymap/set "ctrl-c" "exit-visual-mode" :visual)
  (keymap/set "ctrl-[" "exit-visual-mode" :visual))

############################################################################
# INITIALIZATION
############################################################################

(setup-vim-keybindings)
(setup-insert-keybindings)
(setup-visual-keybindings)

# Push the vim layer onto the active stack
(keymap/push-layer :vim)

(print "Vim mode loaded")
