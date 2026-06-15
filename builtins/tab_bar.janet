# Magma tab bar renderer.

(defn- user-buffer? [bid cur]
  "True if buf should appear in the tab bar: focused OR not a *…* special."
  (or (= bid cur)
      (let [n (buffer/name bid)]
        (not (and (string/has-prefix? n "*")
                  (string/has-suffix? n "*"))))))

(defn- buf-label [bid]
  (let [path (buffer/path bid)]
    (if path
      (last (string/split "/" path))
      (buffer/name bid))))

(defn- fill-tab [y w face]
  (surface/set-text 0 y (string/repeat " " w) face))

(defn- write-tab [x y text face]
  (surface/set-text x y text face)
  (+ x (length text)))

(defn draw-tab-bar []
  (let [sz (surface/size)]
    (unless sz (break))
    (let [w    (get sz :width 80)
          bufs (filter |(user-buffer? $ (buffer/current)) (editor/buffer-list))
          cur  (buffer/current)]

      (fill-tab 0 w "tab-bar")

      (var x 0)
      (each bid bufs
        (when (>= x w) (break))
        (let [label (buf-label bid)
              sel   (= bid cur)
              face  (if sel "tab-active" "tab-inactive")
              tab   (string " " label " ")]
          (when (> x 0)
            (set x (write-tab x 0 "│" "tab-sep")))
          (set x (write-tab x 0 tab face)))))))
