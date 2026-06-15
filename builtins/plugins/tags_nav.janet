# Magma tags — navigation: tag stack, jump-to-definition, word-at-cursor.
# Depends on tags_gen.janet (load-tags, *tag-cache*).

# ── Tag stack ─────────────────────────────────────────────────────────────────

(var *tag-stack* @[])   # [{:buf slab :pos byte-offset} ...]

# ── Jump to tag ────────────────────────────────────────────────────────────────

(defn- jump-to-tag-entry
  "Open file and jump to line for a tag entry {:file :line :kind}."
  [entry]
  (def file (get entry :file))
  (def line-no (get entry :line))
  (def content (try (editor/fs-read file) ([e] (print "Cannot open " file ": " e) nil)))
  (unless content (break))
  (def name (last (string/split "/" file)))
  (def buf (or (do
                 (var found nil)
                 (each b (buffer/list)
                   (when (= (get b :name) name) (set found (get b :slab)) (break)))
                 found)
               (buffer/create name content)))
  (def win (window/current))
  (when win (window/set-buffer win buf))
  (when line-no
    (def offset (buffer/line-start-offset buf (- line-no 1)))
    (when offset (buffer/set-cursor buf offset))))

(defn tags/jump
  "Jump to the definition of `tag-name`, pushing current position to the stack."
  [tag-name]
  (def tbl (load-tags))
  (unless tbl (break))
  (def entries (get tbl tag-name))
  (unless entries
    (print "Tag not found: " tag-name)
    (break))
  (def cur-buf (buffer/current))
  (when cur-buf
    (def cur-pos (buffer/cursor cur-buf))
    (array/push *tag-stack* {:buf cur-buf :pos cur-pos}))
  (jump-to-tag-entry (first entries)))

(defn tags/pop
  "Return to the previous position before the last tag jump (ctrl-t)."
  []
  (when (empty? *tag-stack*)
    (print "Tag stack empty")
    (break))
  (def entry (array/pop *tag-stack*))
  (def buf-key (get entry :buf))
  (def pos (get entry :pos))
  (def win (window/current))
  (when win (window/set-buffer win buf-key))
  (buffer/set-cursor buf-key pos))

# ── Word-at-cursor ────────────────────────────────────────────────────────────

(defn- tags-word-char? [c]
  (or (and (>= c 97) (<= c 122))
      (and (>= c 65) (<= c 90))
      (and (>= c 48) (<= c 57))
      (= c 95)))

(defn- tags-word-at-cursor [buf]
  "Return the word under cursor as a string, or nil."
  (def text (buffer/slice buf 0 (buffer/len buf)))
  (def cursor (buffer/cursor buf))
  (def n (length text))
  (var wstart cursor)
  (while (> wstart 0)
    (if (tags-word-char? (in text (- wstart 1)))
      (-= wstart 1)
      (break)))
  (var wend cursor)
  (while (< wend n)
    (if (tags-word-char? (in text wend))
      (+= wend 1)
      (break)))
  (when (= wstart wend) (break nil))
  (string/slice text wstart wend))

(defn tags/jump-at-cursor
  "Jump to the tag named by the word under the cursor."
  []
  (def buf (buffer/current))
  (unless buf (break))
  (def word (tags-word-at-cursor buf))
  (unless word (print "No word under cursor") (break))
  (tags/jump word))

# ── Commands and keymaps ──────────────────────────────────────────────────────

(command/define "tag-jump-cursor" (fn [&] (tags/jump-at-cursor)))
(command/define "tag-pop"         (fn [&] (tags/pop)))
(command/define "tag-generate"    (fn [&] (tags/generate)))

(keymap/set "ctrl-]" "tag-jump-cursor" "vim")
(keymap/set "ctrl-t" "tag-pop" "vim")

# ── Colon plugin registration ─────────────────────────────────────────────────

(put *colon-plugins* "tag"
  (fn [rest]
    (def name (string/trim rest))
    (if (> (length name) 0)
      (tags/jump name)
      (print ":tag requires a tag name"))))

(put *colon-plugins* "ctags" (fn [_] (tags/generate)))

(print "tags loaded")
