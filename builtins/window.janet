# Window and workspace management — Sprint 8.
# Provides directional window focus, layout save/restore, and resize bindings.
# Loaded after init.janet so *colon-plugins* and command/define are available.

# ── Directional focus helper ───────────────────────────────────────────────────

(defn- window/focus-direction
  "Focus the nearest window in `dir` (:h left, :j down, :k up, :l right)."
  [dir]
  (let [cur (window/current)]
    (when cur
      (let [cx (get cur :x)
            cy (get cur :y)
            cw (get cur :width)
            ch (get cur :height)
            ccx (+ cx (/ cw 2))
            ccy (+ cy (/ ch 2))]
        (var best-id nil)
        (var best-dist nil)
        (each w (window/list)
          (when (not= (get w :id) (get cur :id))
            (let [wx (get w :x)
                  wy (get w :y)
                  ww (get w :width)
                  wh (get w :height)
                  wcx (+ wx (/ ww 2))
                  wcy (+ wy (/ wh 2))
                  qualifies
                  (cond
                    (= dir :h) (< wcx cx)
                    (= dir :l) (> wcx (+ cx cw))
                    (= dir :k) (< wcy cy)
                    (= dir :j) (> wcy (+ cy ch))
                    false)]
              (when qualifies
                (let [dx (- wcx ccx)
                      dy (- wcy ccy)
                      dist (+ (* dx dx) (* dy dy))]
                  (when (or (nil? best-dist) (< dist best-dist))
                    (set best-id (get w :id))
                    (set best-dist dist)))))))
        (when best-id
          (window/focus best-id))))))

# ── Window focus commands ─────────────────────────────────────────────────────

(command/define "window-focus-left"
  (fn [&] (window/focus-direction :h)))

(command/define "window-focus-right"
  (fn [&] (window/focus-direction :l)))

(command/define "window-focus-up"
  (fn [&] (window/focus-direction :k)))

(command/define "window-focus-down"
  (fn [&] (window/focus-direction :j)))

(command/define "window-next"
  (fn [&]
    (let [wins (window/list)
          cur (window/current)]
      (when (and cur (> (length wins) 1))
        (let [cur-id (get cur :id)
              ids (map (fn [w] (get w :id)) wins)
              idx (find-index (fn [id] (= id cur-id)) ids)
              next-idx (if idx (% (+ idx 1) (length ids)) 0)]
          (window/focus (get ids next-idx)))))))

# ── Layout colon verbs ────────────────────────────────────────────────────────

(put *colon-plugins* "layout"
  (fn [rest]
    (let [parts (string/split " " (string/trim rest))
          sub   (get parts 0 "")
          name  (string/trim (get parts 1 ""))]
      (cond
        (= sub "save")
          (do
            (editor/save-layout name)
            (editor/log-message (string "Layout saved: " name)))
        (= sub "restore")
          (do
            (editor/restore-layout name)
            (editor/log-message (string "Layout restored: " name)))
        (= sub "list")
          (let [ls (editor/layout-list)]
            (editor/log-message
              (if (= (length ls) 0)
                "No saved layouts."
                (string "Layouts: " (string/join ls ", ")))))
        (editor/log-message
          "Usage: :layout save <name> | :layout restore <name> | :layout list")))))
