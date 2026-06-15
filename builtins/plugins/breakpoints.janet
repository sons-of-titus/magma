# Breakpoint gutter extension (Sprint 11c).
#
# Tracks breakpoints per buffer and toggles them via gutter column clicks.
# The `:breakpoints` column is shown automatically when the first breakpoint
# is set and hidden when the last one is cleared.

(def *breakpoints* @{})   # {buf-key -> set-of-line-numbers}

(defn- bp-count []
  (var total 0)
  (each lines *breakpoints*
    (set total (+ total (length lines))))
  total)

(defn bp/set [buf line]
  (let [lines (get *breakpoints* buf @{})]
    (put lines line true)
    (put *breakpoints* buf lines))
  (gutter/sign-set ":breakpoints" buf line "● " "gutter-breakpoint" 100)
  (gutter/show-column ":breakpoints"))

(defn bp/clear [buf line]
  (when-let [lines (get *breakpoints* buf)]
    (put lines line nil)
    (when (empty? lines)
      (put *breakpoints* buf nil)))
  (gutter/sign-clear-line ":breakpoints" buf line)
  (when (zero? (bp-count))
    (gutter/hide-column ":breakpoints")))

(defn bp/toggle [buf line]
  (if (get-in *breakpoints* [buf line])
    (bp/clear buf line)
    (bp/set buf line)))

(defn bp/list [buf]
  (keys (get *breakpoints* buf @{})))

(command/define "bp/toggle-at-cursor"
  (fn []
    (let [buf (editor/focused-buffer)
          line (buffer/line-number buf)]
      (bp/toggle buf line))))

(event/on "gutter-clicked"
  (fn [data]
    (when (= (get data :column) ":breakpoints")
      (bp/toggle
        (scan-number (get data :buf))
        (scan-number (get data :line))))))
