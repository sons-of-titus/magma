# ============================================================
# Magma — Vim editing mode (pure Janet dispatch)
#
# editor/on-input handles operator/motion/char-capture/count.
# Rust never branches on mode names.
# ============================================================

# ── Vim state ──────────────────────────────────────────────
(var *v-op* nil)
(var *v-count* 0)
(var *v-op-start* nil)
(var *v-cc-fn* nil)

# ── Tables ─────────────────────────────────────────────────
(def *v-operators* @{"d" "delete-line" "c" "change-line" "y" "yank-line"})

(def *v-motions* @{
  "h" "cursor-left"   "j" "cursor-down"    "k" "cursor-up"    "l" "cursor-right"
  "w" "move-word-forward"  "b" "move-word-back"  "e" "move-word-end"
  "0" "line-start"   "$" "line-end"   "^" "move-to-first-char"
  "G" "goto-buffer-end"   "%" "goto-matching-brace"
  "}" "move-paragraph-forward"   "{" "move-paragraph-back"
  ")" "move-sentence-forward"    "(" "move-sentence-back"
})

(def *v-cc* @{
  "f" "find-forward"   "t" "find-till-forward"
  "F" "find-backward"  "T" "find-till-backward"
  "r" "replace-char"   "m" "set-mark"
  "q" "vim-macro-toggle"   "@" "play-macro"
  "'" "jump-to-mark"   "`" "jump-to-mark-char"
})

# ── Mode transitions ──────────────────────────────────────
(defn set-vim-mode [name &opt accepts-text]
  (editor/set-mode name {:accepts-text (or accepts-text false)})
  (case name
    "command" (minibuffer/open ":" "command")
    "search" (minibuffer/open "/" "search")
    (minibuffer/close))
  (each l ["vim" "insert" "visual" "replace" "command" "search"]
    (keymap/pop-layer l))
  (keymap/push-layer (if (= name "normal") "vim" name)))

# ── Misc helpers ───────────────────────────────────────────
(defn- cur-buf [] (buffer/current))

(defn- run-motion [cmd count]
  (def b (cur-buf))
  (when (and b cmd) (for i 0 (max count 1) (editor/run-command cmd))))

(defn- apply-op [op lo hi]
  (def b (cur-buf)) (unless b (break))
  (when (= lo hi) (break))
  (def [s e] (if (< lo hi) [lo hi] [hi lo]))
  (case op
    "d" (do (buffer/delete b s e) (buffer/set-cursor b s))
    "c" (do (buffer/delete b s e) (editor/set-mode "insert" {:accepts-text true}))
    "y" nil
    (print "vim: unknown operator " op)))

(defn- apply-op-sel [op]
  (def b (cur-buf)) (unless b (break))
  (def sel (selection/get)) (unless sel (break))
  (def a (get sel :anchor))
  (def cur (buffer/cursor b))
  (def [lo hi] (if (< a cur) [a cur] [cur a]))
  (when (= lo hi) (break))
  (case op
    "d" (do (buffer/delete b lo hi) (buffer/set-cursor b lo)
            (selection/clear) (set-vim-mode "normal" false))
    "c" (do (buffer/delete b lo hi) (selection/clear)
            (editor/set-mode "insert" {:accepts-text true}))
    "y" (do (def txt (buffer/slice b lo hi))
            (register/set "\"" txt)
            (selection/clear) (set-vim-mode "normal" false))
    (print "vim: unknown operator " op)))

(defn- double-op [op]
  (def b (cur-buf)) (unless b (break))
  (case op
    "d" (editor/run-command "delete-line")
    "c" (do (editor/run-command "delete-line") (editor/set-mode "insert" {:accepts-text true}))
    "y" (editor/run-command "yank-line")))

(command/define "vim-enter-insert"
  (fn [&] (editor/run-command "enter-insert-mode")))
(command/define "vim-exit-insert"
  (fn [&] (editor/run-command "exit-insert-mode")))
(command/define "vim-enter-visual"
  (fn [&] (set-vim-mode "visual" false)))
(command/define "vim-exit-visual"
  (fn [&] (selection/clear) (set-vim-mode "normal" false)))
(command/define "vim-enter-replace"
  (fn [&] (editor/run-command "enter-replace-mode")))
(command/define "vim-exit-replace"
  (fn [&] (editor/run-command "exit-replace-mode")))
(command/define "vim-enter-command"
  (fn [&] (editor/run-command "enter-command-mode")))
(command/define "vim-exit-command"
  (fn [&] (editor/clear-completions) (set-vim-mode "normal" false)))
(command/define "vim-enter-search"
  (fn [&] (set-vim-mode "search" false)))
(command/define "vim-exit-search"
  (fn [&] (set-vim-mode "normal" false)))

# ── Visual mode operators ─────────────────────────────────
(command/define "vim-visual-delete"
  (fn [&] (apply-op-sel "d")))
(command/define "vim-visual-change"
  (fn [&] (apply-op-sel "c")))
(command/define "vim-visual-yank"
  (fn [&] (apply-op-sel "y")))

# ── on-input interceptor ─────────────────────────────────
(defn vim-on-input [key]
  (when (= (editor/mode-name) "normal")
    (cond
      *v-cc-fn*
      (do (editor/run-command *v-cc-fn* key)
          (set *v-cc-fn* nil)
          (editor/consume-input))

      *v-op*
      (cond
        (= key "esc")
        (do (set *v-op* nil) (set *v-count* 0) (editor/consume-input))
        (= key *v-op*)
        (do (double-op *v-op*) (set *v-op* nil) (set *v-count* 0) (editor/consume-input))
        (get *v-motions* key)
        (let [cmd (get *v-motions* key)
              start *v-op-start*]
          (run-motion cmd *v-count*)
          (def b (cur-buf))
          (when b (apply-op *v-op* start (buffer/cursor b)))
          (set *v-op* nil) (set *v-count* 0)
          (editor/consume-input))
        (peg/match ~(* (range "09")) key)
        (do (set *v-count* (+ (* *v-count* 10) (scan-number key)))
            (editor/consume-input))
        (do (set *v-op* nil) (set *v-count* 0)))

      (and (= key "0") (= *v-count* 0))
      (set *v-count* 0)

      (peg/match ~(* (range "09")) key)
      (do (set *v-count* (+ (* *v-count* 10) (scan-number key)))
          (editor/consume-input))

      (get *v-operators* key)
      (do (def b (cur-buf))
          (set *v-op* key)
          (set *v-op-start* (when b (buffer/cursor b)))
          (editor/consume-input))

      (get *v-cc* key)
      (do (set *v-cc-fn* (get *v-cc* key))
          (editor/consume-input))

      (and (> *v-count* 0) (get *v-motions* key))
      (do (run-motion (get *v-motions* key) *v-count*)
          (set *v-count* 0)
          (editor/consume-input))

      (set *v-count* 0))))

(editor/on-input "vim-on-input")

# ── Activate ────────────────────────────────────────────
(keymap/push-layer "vim")
(print "Vim editing mode loaded")
