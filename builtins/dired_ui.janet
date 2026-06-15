# Dired colour layer.
#
# Applies a "dired" highlight layer to the *dired* buffer whenever it gains
# focus or its content changes.  The layer is recomputed from the raw text,
# so it stays correct after refresh.
#
# Entry types detected by suffix in the Name column:
#   name/   → directory       → type-face     (blue)
#   name@   → symlink         → operator-face (sky)
#   [D] …   → marked delete   → error-face    (red)
#   perms with x → executable → string-face   (green)
# Header / separator rows use keyword-face / comment-face.

(def- HEADER 4)   # lines before first entry (matches dired.rs HEADER_LINES)

(defn- dired-highlights [buf]
  "Return a highlight tuple array for the *dired* buffer."
  (let [text  (buffer/slice buf 0 (buffer/len buf))
        lines (string/split "\n" text)
        out   @[]]
    (var byte 0)
    (loop [i :range [0 (length lines)]]
      (let [line (get lines i)
            len  (length line)
            lend (+ byte len)]   # exclusive end (newline not included)
        (cond
          # Row 0: directory path → highlight entire line in blue
          (= i 0)
          (array/push out [byte lend "type-face"])

          # Row 2: column headers → keyword (mauve/bold)
          (= i 2)
          (array/push out [byte lend "keyword-face"])

          # Row 3: separator line → dim
          (= i 3)
          (array/push out [byte lend "comment-face"])

          # Entry rows (i >= HEADER)
          (>= i HEADER)
          (cond
            # Marked for deletion: line starts with "[D]"
            (string/has-prefix? "[D]" (string/trim line))
            (array/push out [byte lend "error-face"])

            # Directory: name column ends with "/"
            (string/has-suffix? "/" (string/trim line))
            (array/push out [byte lend "type-face"])

            # Symlink: name column ends with "@"
            (string/has-suffix? "@" (string/trim line))
            (array/push out [byte lend "operator-face"])

            # Executable: perms string contains "x" in owner position (col 4)
            (do
              (def perms-start (+ byte 4))  # skip "[D] " or "    "
              (def maybe-exec
                (and (> (length line) 4)
                     (= "x" (string/slice line 3 4))))
              maybe-exec)
            (array/push out [byte lend "string-face"])))

        # Advance byte counter: +1 for the newline
        (+= byte (+ len 1))))
    out))

(defn- apply-dired-highlights []
  (let [buf (buffer/get-by-name "*dired*")]
    (when buf
      (buffer/set-highlights-layer buf "dired" (dired-highlights buf)))))

# Re-colour whenever the dired buffer gains focus or is refreshed.
(event/on "buffer-focused"
  (fn [data]
    (let [buf (get data :buffer-id nil)]
      (when buf
        (when (= (buffer/name buf) "*dired*")
          (apply-dired-highlights))))))

# Also re-colour right after a dired refresh (the command emits buffer-after-save
# when it rewrites the buffer, or we can hook buffer-modified).
(event/on "buffer-after-save"
  (fn [data]
    (let [buf (get data :buffer-id nil)]
      (when (and buf (= (buffer/name buf) "*dired*"))
        (apply-dired-highlights)))))
