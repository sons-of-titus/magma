# --- Scroll commands moved from Rust builtin.rs ---------------------
# These are defined in Janet using the buffer/ API.

(command/define "scroll-line-down"
  (fn [&]
    (def buf (buffer/current))
    (when buf
      (def n-lines (buffer/line-count buf))
      (def cur-line (buffer/line-number buf))
      (def target (min (+ cur-line 1) (- n-lines 1)))
      (def offset (buffer/line-start-offset buf target))
      (when offset (buffer/set-cursor buf offset)))))

(command/define "scroll-line-up"
  (fn [&]
    (def buf (buffer/current))
    (when buf
      (def cur-line (buffer/line-number buf))
      (def target (max (- cur-line 1) 0))
      (def offset (buffer/line-start-offset buf target))
      (when offset (buffer/set-cursor buf offset)))))

(command/define "scroll-half-page-down"
  (fn [&]
    (def buf (buffer/current))
    (when buf
      (def n-lines (buffer/line-count buf))
      (def cur-line (buffer/line-number buf))
      (def half (max 12 (math/floor (/ n-lines 4))))
      (def target (min (+ cur-line half) (- n-lines 1)))
      (def offset (buffer/line-start-offset buf target))
      (when offset (buffer/set-cursor buf offset)))))

(command/define "scroll-half-page-up"
  (fn [&]
    (def buf (buffer/current))
    (when buf
      (def n-lines (buffer/line-count buf))
      (def cur-line (buffer/line-number buf))
      (def half (max 12 (math/floor (/ n-lines 4))))
      (def target (max (- cur-line half) 0))
      (def offset (buffer/line-start-offset buf target))
      (when offset (buffer/set-cursor buf offset)))))

(command/define "scroll-page-down"
  (fn [&]
    (def buf (buffer/current))
    (when buf
      (def n-lines (buffer/line-count buf))
      (def cur-line (buffer/line-number buf))
      (def page (max 1 (math/floor (/ n-lines 2))))
      (def target (min (+ cur-line page) (- n-lines 1)))
      (def offset (buffer/line-start-offset buf target))
      (when offset (buffer/set-cursor buf offset)))))

(command/define "scroll-page-up"
  (fn [&]
    (def buf (buffer/current))
    (when buf
      (def n-lines (buffer/line-count buf))
      (def cur-line (buffer/line-number buf))
      (def page (max 1 (math/floor (/ n-lines 2))))
      (def target (max (- cur-line page) 0))
      (def offset (buffer/line-start-offset buf target))
      (when offset (buffer/set-cursor buf offset)))))
