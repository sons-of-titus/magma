# --- Multi-cursor commands (high-level logic in Janet) -----------------

(defn find-next-occurrence
  "Find the next occurrence of `word` starting from `from-pos` in `text`.
   Returns the byte offset or nil."
  [text word from-pos]
  (def remaining (string/slice text from-pos))
  (def idx (string/find word remaining))
  (when idx (+ from-pos idx)))

(defn- word-char? [c]
  (or (and (>= c 97) (<= c 122))   # a-z
      (and (>= c 65) (<= c 90))    # A-Z
      (and (>= c 48) (<= c 57))    # 0-9
      (= c 95)))                   # _

(defn word-under-cursor
  "Get the word (alphanumeric + underscore) under the cursor position.
   Returns [word-start word-end word] or nil."
  [buf]
  (def text (buffer/slice buf 0 (buffer/len buf)))
  (def cursor (buffer/cursor buf))
  (def n (length text))
  (var wstart cursor)
  (while (> wstart 0)
    (if (word-char? (in text (- wstart 1)))
      (-= wstart 1)
      (break)))
  (var wend cursor)
  (while (< wend n)
    (if (word-char? (in text wend))
      (+= wend 1)
      (break)))
  (when (= wstart wend) (break))
  [wstart wend (string/slice text wstart wend)])

(command/define "multi-cursor-next-occurrence"
  (fn [&]
    (def buf (buffer/current))
    (when buf
      (def text (buffer/slice buf 0 (buffer/len buf)))
      (def result (word-under-cursor buf))
      (when result
        (def [wstart wend word] result)
        (def remaining (string/slice text wend))
        (def idx (string/find word remaining))
        (when idx
          (editor/run-command "cursor-add" (string (+ wend idx))))))))

(command/define "multi-cursor-clear"
  (fn [&] (editor/run-command "cursor-clear")))



# --- Completion insertion (tab/ctrl-n accept in insert mode) -----------

(command/define "completion-next"
  (fn [&]
    (editor/run-command "completion-trigger")
    (editor/run-command "completion-next")))

(command/define "completion-prev"
  (fn [&]
    (editor/run-command "completion-trigger")
    (editor/run-command "completion-prev")))

(command/define "snippet-expand"
  (fn [&]
    (editor/run-command "snippet-expand")))
