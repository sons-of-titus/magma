# --- Scroll commands -------------------------------------------------
#
# Line scrolls move the cursor (sticky view-follow does the rest).  Page and
# half-page scrolls move the VIEWPORT — the cursor rides along, keeping its
# screen row — the GUI convention.  Both stay vim-compatible: the cursor
# never leaves the window.

(defn- scroll-by-lines [n]
  (def buf (buffer/current))
  (when buf
    (def n-lines (buffer/line-count buf))
    (def top (window/scroll-top))
    (def dims (window/dimensions))
    (def visible (max 1 (- (get dims :height) 1)))
    (def new-top (min (max (+ top n) 0) (max 0 (- n-lines 1))))
    (window/set-scroll-top new-top)
    # The cursor rides the same distance, clamped to the document; the
    # viewport sync leaves the view alone while the cursor stays inside it.
    (def cur-line (buffer/line-number buf))
    (def target (min (max (+ cur-line n) 0) (- n-lines 1)))
    (def offset (buffer/line-start-offset buf target))
    (when offset (buffer/set-cursor buf offset))
    (when (< target new-top) (window/set-scroll-top target))
    (when (>= target (+ new-top visible)) (window/set-scroll-top (max 0 (- target visible -1))))))

(command/define "scroll-line-down" (fn [&] (scroll-by-lines 1)))
(command/define "scroll-line-up" (fn [&] (scroll-by-lines -1)))
(command/define "scroll-half-page-down" (fn [&] (scroll-by-lines (max 1 (math/floor (/ ((window/dimensions) :height) 2))))))
(command/define "scroll-half-page-up" (fn [&] (scroll-by-lines (- (max 1 (math/floor (/ ((window/dimensions) :height) 2)))))))
(command/define "scroll-page-down" (fn [&] (scroll-by-lines (max 1 (- ((window/dimensions) :height) 2)))))
(command/define "scroll-page-up" (fn [&] (scroll-by-lines (- (max 1 (- ((window/dimensions) :height) 2))))))
