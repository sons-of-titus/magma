# Code folding — behavior over the core's fold mechanism.
#
# Core owns the data and rendering: BufferView fold ranges, the fold-aware
# visible-row walk, and the gutter's :folding column.  This file owns the
# language knowledge: per-mode fold queries, recomputing foldable ranges
# when a fresh tree lands, and the user-facing commands.
#
# A fold query (conventionally the plugin's folds.scm) marks foldable nodes
# with the @fold capture:
#
#   (function_declaration body: (_) @fold)
#
# Foldable ranges and folded ranges are distinct things: the query only
# reports what CAN fold; collapsing happens explicitly (toggle-fold,
# fold-all) through the same byte ranges.

# Per-mode fold queries: mode-name → query source.
(def *ts-mode-fold-queries* @{})

# Register `query` as the tree-sitter fold query for a major mode,
# replacing any earlier registration.  Requires the same grammar setup as
# ts/set-mode-query — a mode without a grammar never gets a tree, so the
# query would never run.
(defn ts/set-mode-fold-query
  [mode query]
  (put *ts-mode-fold-queries* mode (string query)))

# Register a major mode's fold query from a query file on disk
# (conventionally the plugin's folds.scm).  A missing file is ignored.
(defn ts/load-mode-fold-query
  [mode path]
  (def f (try (file/open path :r) ([_] nil)))
  (when f
    (def query (file/read f :all))
    (file/close f)
    (when (and query (> (length query) 0))
      (ts/set-mode-fold-query mode query))))

# ── Recompute ────────────────────────────────────────────────────────────────

# Run the mode's fold query and publish the foldable ranges.  Fold starts
# snap to their line start: the renderer keys folds by line-start offset.
# A failed query clears the ranges rather than leaving stale candidates.
(defn ts/recompute-folds
  [buf-id]
  (def mode (buffer/major-mode buf-id))
  (def query (get *ts-mode-fold-queries* mode))
  (unless query (break))
  (def result (try
    (ts/query-tree buf-id query)
    ([e]
      (buffer/set-fold-ranges buf-id [])
      (break))))
  (def ranges @[])
  (when result
    (each [s e capture] result
      (when (= capture "fold")
        # Only spans of two or more lines fold meaningfully — a one-line
        # range has nothing to hide.  This keeps grammars whose generic
        # nodes (lisp lists, expression groups) match every call from
        # marking single-line forms foldable.
        (def start-line (buffer/line-of buf-id s))
        (when (> (buffer/line-of buf-id (max (dec e) 0)) start-line)
          (def start (or (buffer/line-start-offset buf-id start-line) s))
          (array/push ranges [start e])))))
  (buffer/set-fold-ranges buf-id ranges))

(event/on "ts-tree-updated"
  (fn [data]
    (when-let [buf-id (scan-number (string (get data :buffer-id)))]
      (ts/recompute-folds buf-id))))

# Edits re-run the query through the lane-scheduled parse above, but a
# freshly opened file never edits: its first (and only) tree comes from
# ts/highlight-buffer on buffer-created.  Loader order puts this file
# before symbols.janet, so recomputing on created would run before any
# language is set — first focus is the earliest point the tree exists.
(event/on "buffer-focused"
  (fn [data]
    (when-let [buf-id (scan-number (string (get data :buffer-id)))]
      (ts/recompute-folds buf-id))))

# ── Commands ─────────────────────────────────────────────────────────────────

# Foldable range starting exactly at `offset`, or nil.
(defn- fold-range-at
  [buf-id offset]
  (find |(= (first $) offset) (buffer/fold-ranges buf-id)))

# Active fold starting exactly at `offset`, or nil.
(defn- active-fold-at
  [buf-id offset]
  (find |(= (first $) offset) (buffer/folds buf-id)))

(defn- line-start-of-cursor
  [buf-id]
  (def pos (buffer/cursor buf-id))
  (buffer/line-start-offset buf-id (buffer/line-of buf-id pos)))

(command/define "toggle-fold"
  (fn [&]
    (def buf (buffer/current))
    (unless buf (break))
    (def line-start (line-start-of-cursor buf))
    (if-let [folded (active-fold-at buf line-start)]
      (buffer/unfold buf (first folded) (last folded))
      (when-let [range (fold-range-at buf line-start)]
        (buffer/fold buf (first range) (last range))))))

(command/define "fold-all"
  (fn [&]
    (def buf (buffer/current))
    (unless buf (break))
    (each range (buffer/fold-ranges buf)
      (buffer/fold buf (first range) (last range)))))

(command/define "unfold-all"
  (fn [&]
    (def buf (buffer/current))
    (unless buf (break))
    (buffer/unfold-all buf)))

# The keymap carries flat chords only, so the vim z-prefix collapses onto
# a modifier: za toggles, zM closes all, zR opens all.  The GUI's key
# translation names keyboard alt "meta-" (terminal frontends send alt-),
# so both prefixes are bound.
(each prefix ["alt" "meta"]
  (keymap/set (string prefix "-z") "toggle-fold" :editor)
  (keymap/set (string prefix "-m") "fold-all" :editor)
  (keymap/set (string prefix "-r") "unfold-all" :editor))

# ── Gutter click ─────────────────────────────────────────────────────────────
#
# Clicking the :folding column toggles the fold starting on that line.  The
# event carries 0-based lines already mapped through the fold-aware row
# walk, so the line number is directly a buffer line.

(event/on "gutter-clicked"
  (fn [ev]
    (when (= (get ev :column) ":folding")
      (when-let [buf-id (scan-number (string (get ev :buf)))]
        (when-let [line (scan-number (string (get ev :line)))]
          (when-let [line-start (buffer/line-start-offset buf-id line)]
            (if-let [folded (active-fold-at buf-id line-start)]
              (buffer/unfold buf-id (first folded) (last folded))
              (when-let [range (fold-range-at buf-id line-start)]
                (buffer/fold buf-id (first range) (last range))))))))))

(print "folds loaded")
