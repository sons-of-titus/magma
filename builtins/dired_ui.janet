# Dired highlight layer.
#
# Applies a "dired" highlight layer to the *dired* buffer.
# Uses dedicated dired faces from init.janet so users can theme them.

(def- HEADER 4)

(defn- dired-highlights [buf]
  (let [text  (buffer/slice buf 0 (buffer/len buf))
        lines (string/split "\n" text)
        out   @[]]
    (var byte 0)
    (loop [i :range [0 (length lines)]]
      (let [line (get lines i)
            len  (length line)
            lend (+ byte len)]
        (cond
          (= i 0)
          (array/push out [byte lend "dired-path-face"])

          (= i 2)
          (array/push out [byte lend "dired-header-face"])

          (= i 3)
          (array/push out [byte lend "comment-face"])

          (>= i HEADER)
          (let [stripped (string/trim line)]
            (cond
              (string/has-prefix? "[" stripped)
              (array/push out [byte lend "dired-marked-face"])

              (string/has-suffix? "/" stripped)
              (array/push out [byte lend "dired-directory-face"])

              (string/has-suffix? "@" stripped)
              (array/push out [byte lend "dired-symlink-face"])

              (do
                (def maybe-exec
                  (and (> (length line) 4)
                       (= "x" (string/slice line 3 4))))
                maybe-exec)
              (array/push out [byte lend "dired-executable-face"])))

          (string/has-prefix? "  Marks" (string/trim line))
          (array/push out [byte lend "dired-flagged-face"])

          (string/has-prefix? "  [filter" (string/trim line))
          (array/push out [byte lend "dired-filter-face"]))

        (+= byte (+ len 1))))
    out))

(defn- apply-dired-highlights []
  (let [buf (buffer/get-by-name "*dired*")]
    (when buf
      (buffer/set-highlights-layer buf "dired" (dired-highlights buf)))))

(event/on "buffer-focused"
  (fn [data]
    (def raw (get data :buffer-id))
    (when raw
      (def buf (scan-number raw))
      (when (and buf (= (buffer/name buf) "*dired*"))
        (apply-dired-highlights)))))

(event/on "buffer-after-save"
  (fn [data]
    (def raw (get data :buffer-id))
    (when raw
      (def buf (scan-number raw))
      (when (and buf (= (buffer/name buf) "*dired*"))
        (apply-dired-highlights)))))

# ── Wrapper commands for keybindings that need fixed args ─────────────

(command/define "dired-mark-copy"
  (fn [&] (editor/run-command "dired-mark-with-type" "copy")))

(command/define "dired-mark-move"
  (fn [&] (editor/run-command "dired-mark-with-type" "move")))

# ── Dired keybindings (layer "dired") ──────────────────────────────────
# Pushed automatically by the `dired` command in Rust, popped on file-open
# or dired-close.

(keymap/set "return"    "dired-open-at-cursor"         "dired")
(keymap/set "l"         "dired-open-at-cursor"         "dired")
(keymap/set "h"         "dired-parent"                 "dired")
(keymap/set "j"         "cursor-down"                  "dired")
(keymap/set "k"         "cursor-up"                    "dired")
(keymap/set "d"         "dired-mark"                   "dired")
(keymap/set "c"         "dired-mark-copy"              "dired")
(keymap/set "m"         "dired-mark-move"              "dired")
(keymap/set "u"         "dired-unmark-all"             "dired")
(keymap/set "U"         "dired-invert-marks"           "dired")
(keymap/set "x"         "dired-execute-deletion"       "dired")
(keymap/set "g"         "dired-refresh"                "dired")
(keymap/set "r"         "dired-refresh"                "dired")
(keymap/set "q"         "dired-close"                  "dired")
(keymap/set "."         "dired-toggle-hidden"          "dired")
(keymap/set "s"         "dired-toggle-sort"            "dired")
(keymap/set "S"         "dired-toggle-sort-reverse"    "dired")
