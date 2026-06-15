# Magma tags plugin — ctags-based jump-to-definition.
#
# Supports universal-ctags (preferred) and exuberant-ctags.
# Tags file format: {name}\t{file}\t{address}[;"\t{fields}]
#
# Commands:
#   :ctags            — regenerate tags file in the current directory
#   :tag <name>       — jump to tag by name
#   ctrl-]            — jump to tag under cursor
#   ctrl-t            — pop tag stack (return to previous location)

# ── Tag stack ─────────────────────────────────────────────────────────────────

(var *tag-stack* @[])   # [{:buf slab :pos byte-offset} ...]
(var *tag-cache* nil)   # parsed tags table or nil (invalidated on :ctags)

# ── Find tags file ────────────────────────────────────────────────────────────

(defn- find-tags-file
  "Walk up from cwd to find a `tags` file. Returns path or nil."
  []
  (def candidates @["tags" "TAGS"])
  (var result nil)
  # Try candidates in cwd first, then git root
  (each name candidates
    (def content (try (editor/fs-read name) ([_] nil)))
    (when content
      (set result name)
      (break)))
  (unless result
    # Try in git root
    (def root (string/trim (editor/shell "git rev-parse --show-toplevel 2>/dev/null")))
    (when (> (length root) 0)
      (each name candidates
        (def path (string root "/" name))
        (def content (try (editor/fs-read path) ([_] nil)))
        (when content
          (set result path)
          (break)))))
  result)

# ── Parse ctags file ──────────────────────────────────────────────────────────

(defn- parse-address
  "Extract a line number from a ctags address field.
   Address is either a number, /pattern/, or has a line: field."
  [address fields-str]
  # Prefer explicit line: field (universal ctags --fields=+n)
  (when fields-str
    (def line-match (peg/match '(* (thru "line:") (capture :d+)) fields-str))
    (when line-match
      (break (scan-number (first line-match)))))
  # Bare numeric address
  (when (peg/match '(* :d+ -1) address)
    (break (scan-number address)))
  # Can't determine line without opening the file
  nil)

(defn- parse-tags
  "Parse a ctags file into a table of {name → [{:file :line :kind} ...]}."
  [content]
  (def tbl @{})
  (each line (string/split "\n" content)
    (unless (or (string/has-prefix? "!" line) (= (length line) 0))
      (def parts (string/split "\t" line))
      (when (>= (length parts) 3)
        (def tag-name (get parts 0))
        (def file     (get parts 1))
        (def address  (get parts 2))
        # Fields after address (kind, line:, etc.)
        (def fields-str (if (> (length parts) 3) (string/join (array/slice parts 3) "\t") nil))
        # Strip ;\" suffix from address
        (def clean-addr
          (if (string/find ";\"" address)
            (string/slice address 0 (string/find ";\"" address))
            address))
        (def line-no (parse-address clean-addr fields-str))
        (def kind
          (if fields-str
            (do
              (def m (peg/match '(* (capture (between 1 2 :a)) :s) fields-str))
              (if m (first m) "?"))
            "?"))
        (def entry {:file file :line line-no :kind kind})
        (def existing (get tbl tag-name))
        (if existing
          (array/push existing entry)
          (put tbl tag-name @[entry])))))
  tbl)

(defn- load-tags
  "Load and cache the tags file. Returns parsed table or nil."
  []
  (when *tag-cache*
    (break *tag-cache*))
  (def tags-path (find-tags-file))
  (unless tags-path
    (print "No tags file found. Run :ctags to generate one.")
    (break nil))
  (def content (try (editor/fs-read tags-path) ([e] (print "tags read error: " e) nil)))
  (unless content (break nil))
  (def tbl (parse-tags content))
  (set *tag-cache* tbl)
  tbl)

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
  # Save current position before jumping
  (def cur-buf (buffer/current))
  (when cur-buf
    (def cur-pos (buffer/cursor cur-buf))
    (array/push *tag-stack* {:buf cur-buf :pos cur-pos}))
  # Jump to first matching entry (TODO: show picker for multiple)
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

(defn tags/generate
  "Run ctags recursively in the current directory."
  []
  (print "Generating tags...")
  (def out (editor/shell "ctags -R --fields=+n --output-format=e-ctags . 2>&1"))
  (set *tag-cache* nil)   # invalidate cache
  (if (= (length (string/trim out)) 0)
    (print "tags file updated")
    (print out)))

# ── Commands ──────────────────────────────────────────────────────────────────

(command/define "tag-jump-cursor" (fn [&] (tags/jump-at-cursor)))
(command/define "tag-pop"         (fn [&] (tags/pop)))
(command/define "tag-generate"    (fn [&] (tags/generate)))

# ctrl-] — jump to definition in normal mode
(keymap/set "ctrl-]" "tag-jump-cursor" "vim")
# ctrl-t — pop tag stack in normal mode
(keymap/set "ctrl-t" "tag-pop" "vim")

# ── Colon plugin registration ─────────────────────────────────────────────────

(put *colon-plugins* "tag"
  (fn [rest]
    (def name (string/trim rest))
    (if (> (length name) 0)
      (tags/jump name)
      (print ":tag requires a tag name"))))

(put *colon-plugins* "ctags" (fn [_] (tags/generate)))

(print "tags.janet loaded")
