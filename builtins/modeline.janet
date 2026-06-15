# Magma modeline — mode helpers, git branch detection, and modeline renderer.
# Depends on the Catppuccin palette (C) and surface helpers from ui.janet.

# ── Mode helpers ──────────────────────────────────────────────────────────────

(defn- mode-pill-face [mode]
  (cond
    (= mode "NORMAL")                        "ml-normal"
    (= mode "INSERT")                        "ml-insert"
    (string/has-prefix? "VISUAL" mode)       "ml-visual"
    (= mode "REPLACE")                       "ml-replace"
    (= mode "COMMAND")                       "ml-command"
    (= mode "SEARCH")                        "ml-search"
    (= mode "TERMINAL")                      "ml-term"
    "ml-normal"))

(defn- mode-sep-face [mode]
  (cond
    (= mode "NORMAL")                        "ml-sep-normal"
    (= mode "INSERT")                        "ml-sep-insert"
    (string/has-prefix? "VISUAL" mode)       "ml-sep-visual"
    (= mode "REPLACE")                       "ml-sep-replace"
    (= mode "COMMAND")                       "ml-sep-command"
    (= mode "SEARCH")                        "ml-sep-search"
    (= mode "TERMINAL")                      "ml-sep-term"
    "ml-sep-normal"))

(defn- mode-abbrev [mode]
  (cond
    (= mode "VISUAL LINE")  "V·LINE"
    (= mode "VISUAL BLOCK") "V·BLCK"
    mode))

# ── Git branch (cached) ───────────────────────────────────────────────────────

(var *ml-branch* nil)

(defn detect-branch []
  (let [root (project/root)]
    (set *ml-branch*
      (if (nil? root)
        nil
        (let [head (string root "/.git/HEAD")]
          (if (not (project/path-exists? head))
            nil
            (try
              (let [s (string/trim (editor/fs-read head))]
                (if (string/has-prefix? "ref: refs/heads/" s)
                  (string/slice s 16)
                  (string/slice s 0 7)))
              ([_] nil))))))))

# ── Surface helpers ───────────────────────────────────────────────────────────

(defn- write-at [x y text face]
  "Write text to surface at (x,y), return next x position."
  (surface/set-text x y text face)
  (+ x (length text)))

(defn- fill [y w face]
  (surface/set-text 0 y (string/repeat " " w) face))

# ── Modeline renderer ─────────────────────────────────────────────────────────

(defn draw-modeline []
  (let [sz (surface/size)]
    (unless sz (break))
    (let [w    (get sz :width  80)
          h    (get sz :height 24)
          row  (- h 1)
          mode (editor/mode)]

      (when (or (= mode "COMMAND") (= mode "SEARCH")) (break))

      (fill row w "ml-base")

      (var x 0)
      (let [abbr  (mode-abbrev mode)
            pface (mode-pill-face mode)
            sface (mode-sep-face mode)]
        (set x (write-at x row "▌" pface))
        (set x (write-at x row (string " " abbr " ") pface))
        (set x (write-at x row "▌" sface)))

      (let [buf (buffer/current)]
        (when buf
          (let [name (buffer/name buf)]
            (set x (write-at x row (string " " name " ") "ml-filename"))
            (set x (write-at x row "▌" "ml-sep-right")))))

      (let [buf (buffer/current)]
        (when buf
          (let [line   (+ 1 (buffer/line-number buf))
                lstart (buffer/line-start-offset buf (buffer/line-number buf))
                col    (if lstart (- (buffer/cursor buf) lstart) 0)
                pos    (string line ":" col)
                rsep   "▐"
                rtext  (string " " pos " ")
                rlen   (+ (length rsep) (length rtext))
                rx     (- w rlen)]
            (when (> rx x)
              (write-at rx row rsep "ml-sep-right")
              (write-at (+ rx (length rsep)) row rtext "ml-filename"))))))))
