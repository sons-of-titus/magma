# Magma tags — ctags file generation and parsing.

# ── Tag cache ─────────────────────────────────────────────────────────────────

(var *tag-cache* nil)   # parsed tags table or nil (invalidated on :ctags)

# ── Find tags file ────────────────────────────────────────────────────────────

(defn- find-tags-file
  "Walk up from cwd to find a `tags` file. Returns path or nil."
  []
  (def candidates @["tags" "TAGS"])
  (var result nil)
  (each name candidates
    (def content (try (editor/fs-read name) ([_] nil)))
    (when content
      (set result name)
      (break)))
  (unless result
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
  "Extract a line number from a ctags address field."
  [address fields-str]
  (when fields-str
    (def line-match (peg/match '(* (thru "line:") (capture :d+)) fields-str))
    (when line-match
      (break (scan-number (first line-match)))))
  (when (peg/match '(* :d+ -1) address)
    (break (scan-number address)))
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
        (def fields-str (if (> (length parts) 3) (string/join (array/slice parts 3) "\t") nil))
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

(defn load-tags
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

# ── Tag generation ────────────────────────────────────────────────────────────

(defn tags/generate
  "Run ctags recursively in the current directory."
  []
  (print "Generating tags...")
  (def out (editor/shell "ctags -R --fields=+n --output-format=e-ctags . 2>&1"))
  (set *tag-cache* nil)
  (if (= (length (string/trim out)) 0)
    (print "tags file updated")
    (print out)))
