# --- Indent commands --------------------------------------------------
# Moved from Rust builtin.rs.  Uses `magma/trim` defined in
# magma_utils.janet before this file is loaded.

(command/define "indent-line"
  (fn [& args]
    (def buf (buffer/current))
    (when buf
      (def count (if (> (length args) 0) (args 0) 1))
      (def line (buffer/line-number buf))
      (for i 0 count
        (def start (buffer/line-start-offset buf line))
        (when start (buffer/insert buf start "    "))))))

(command/define "deindent-line"
  (fn [& args]
    (def buf (buffer/current))
    (when buf
      (def count (if (> (length args) 0) (args 0) 1))
      (def line (buffer/line-number buf))
      (for i 0 count
        (def start (buffer/line-start-offset buf line))
        (when start
          (def text (buffer/slice buf start (+ start 4)))
          (var to-remove 0)
          (each c text
            (if (or (= c " ") (= c "\t"))
              (set to-remove (+ to-remove (length c)))
              (break)))
          (when (> to-remove 0)
            (buffer/delete buf start (+ start to-remove))))))))

(command/define "autoindent-line"
  (fn [&]
    (def buf (buffer/current))
    (when buf
      (def line (buffer/line-number buf))
      (def start (buffer/line-start-offset buf line))
      (unless start (break))
      (def end-offset (buffer/line-start-offset buf (+ line 1)))
      (def line-len (if end-offset (- end-offset start) (- (buffer/len buf) start)))
      (def text (buffer/slice buf start (+ start line-len)))
      (unless (not= text "") (break))
      (def trimmed (magma/trim text))
      (def indent-len (- (length text) (length trimmed)))
      (def indent-str (string/repeat "    " (math/floor (/ indent-len 4))))
      (buffer/delete buf start (+ start (length text)))
      (buffer/insert buf start indent-str)
      (buffer/insert buf (+ start (length indent-str)) trimmed)
      (buffer/set-cursor buf (+ start (length indent-str))))))

# --- Insert mode helpers --------------------------------------------
# Moved from Rust builtin.rs

(command/define "delete-to-bol"
  (fn [&]
    (def buf (buffer/current))
    (when buf
      (def pos (buffer/cursor buf))
      (def line (buffer/line-number buf))
      (def start (buffer/line-start-offset buf line))
      (when (and start (< start pos))
        (buffer/delete buf start pos)))))

(command/define "delete-to-eol"
  (fn [&]
    (def buf (buffer/current))
    (when buf
      (def pos (buffer/cursor buf))
      (def line (buffer/line-number buf))
      (def next-start (buffer/line-start-offset buf (+ line 1)))
      (def end-off (if next-start next-start (buffer/len buf)))
      (when (< pos end-off)
        (buffer/delete buf pos end-off)))))
