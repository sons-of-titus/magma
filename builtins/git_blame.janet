# Sprint 11b — Git Blame EOL Decorations
# Spawns `git blame --porcelain` on buffer save/focus; parses output;
# sets EndOfLine decorations in the "git-blame" layer.

(def- *blame-enabled* @{})   # buf-id → bool

(defn- blame-enabled? [buf]
  (get *blame-enabled* buf false))

(defn git-blame/enable [buf]
  "Enable git blame annotations for a buffer."
  (put *blame-enabled* buf true))

(defn git-blame/disable [buf]
  "Disable git blame annotations and clear decorations for a buffer."
  (put *blame-enabled* buf false)
  (buffer/decor-clear-layer buf "git-blame"))

(defn- run-blame [buf path]
  (when (not (blame-enabled? buf)) (break))
  (buffer/decor-clear-layer buf "git-blame")
  (var current-commit @{})   # commit-hash → {:author "..." :time "..."}
  (var line-commits @[])     # list of [line-num commit-hash]

  (let [pid (process/spawn "git" ["blame" "--porcelain" path])]
    (event/on "process-output"
      (fn [data]
        (when (= (get data :id 0) pid)
          (let [s (get data :line "")]
            # Porcelain format: first token on the commit-header lines is the hash,
            # then author/time lines, then the actual source line prefixed with TAB.
            (cond
              # Line entry header: "<hash> <orig-line> <final-line>"
              (and (> (length s) 40)
                   (string/find " " s))
              (let [parts (string/split " " s)
                    hash  (get parts 0 "")]
                (when (= (length hash) 40)
                  (array/push line-commits [(- (scan-number (get parts 2 "0")) 1) hash])
                  (put current-commit hash (get current-commit hash @{}))))
              # "author <name>" line
              (string/has-prefix? "author " s)
              (let [hash  (get (last line-commits) 1 "")
                    entry (get current-commit hash @{})]
                (put entry :author (string/trim (string/slice s 7)))
                (put current-commit hash entry))
              # "author-time <unix>" line
              (string/has-prefix? "author-time " s)
              (let [hash  (get (last line-commits) 1 "")
                    entry (get current-commit hash @{})
                    ts    (scan-number (string/trim (string/slice s 12)) 0)
                    now   (os/time)
                    diff  (- now ts)
                    age   (cond
                            (< diff 3600)   (string (math/floor (/ diff 60)) "m ago")
                            (< diff 86400)  (string (math/floor (/ diff 3600)) "h ago")
                            (string (math/floor (/ diff 86400)) "d ago"))]
                (put entry :age age)
                (put current-commit hash entry)))))))

    (event/once (string "process-exit:" pid)
      (fn [_]
        (each [line-num hash] line-commits
          (let [info   (get current-commit hash {})
                author (get info :author "?")
                age    (get info :age "?")
                text   (string author "  " age)]
            (buffer/decor-set-eol buf "git-blame" line-num text "comment-face")))))))

(event/on "buffer-focused"
  (fn [data]
    (let [buf  (get data :buffer-id nil)
          path (when buf (buffer/path buf))]
      (when (and buf path (blame-enabled? buf))
        (run-blame buf path)))))

(event/on "buffer-after-save"
  (fn [data]
    (let [buf  (get data :buffer-id nil)
          path (when buf (buffer/path buf))]
      (when (and buf path (blame-enabled? buf))
        (run-blame buf path)))))

# Colon verb: :git-blame on|off
(when (and (table? *colon-plugins*) (not (nil? *colon-plugins*)))
  (put *colon-plugins* "git-blame"
    (fn [args]
      (let [sub  (get args 0 "")
            buf  (buffer/current)
            path (when buf (buffer/path buf))]
        (cond
          (= sub "on")
          (do
            (git-blame/enable buf)
            (when path (run-blame buf path)))
          (= sub "off")
          (git-blame/disable buf)
          (editor/log-message "Usage: :git-blame on|off"))))))
