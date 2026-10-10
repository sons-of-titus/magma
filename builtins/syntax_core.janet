# Magma built-in syntax highlighting — keyword/regex matching.

(def *syntax-rules* (table/new 64))

# Tables populated by syntax_lang_ext.janet at load time via put.
(def *extension-language* (table))
(def *comment-markers* (table))

(defn define-syntax
  [name keywords]
  (def kw-table @{})
  (each kw keywords (put kw-table kw true))
  (put *syntax-rules* name kw-table))

# --- Highlight helpers -------------------------------------------------

(defn file-extension
  [path]
  (when path
    (var i (- (length path) 1))
    (var dot nil)
    (while (>= i 0)
      (when (= (string/slice path i (+ i 1)) ".")
        (set dot i)
        (set i -1))
      (-= i 1))
    (when dot (string/slice path (+ dot 1)))))

(defn word-char?
  [c]
  (def code (string/bytes c))
  (and code (not (zero? (length code)))
       (or (and (>= (code 0) 97)  (<= (code 0) 122))
           (and (>= (code 0) 65)  (<= (code 0) 90))
           (and (>= (code 0) 48)  (<= (code 0) 57))
           (= (code 0) 95)
           (= (code 0) 47)
           (= (code 0) 45)
           (= (code 0) 63)
           (= (code 0) 33))))

(defn skip-while
  [pred text pos n]
  (var p pos)
  (while (and (< p n) (pred (string/slice text p (+ p 1))))
    (set p (+ p 1)))
  p)

(defn highlight-keywords
  [text keywords ranges]
  (def n (length text))
  (var pos 0)
  (while (< pos n)
    (set pos (skip-while (fn [c] (not (word-char? c))) text pos n))
    (when (>= pos n) (break))
    (def start pos)
    (set pos (skip-while word-char? text pos n))
    (def word (string/slice text start pos))
    (when (in keywords word)
      (array/push ranges [start (+ start (length word)) "keyword-face"])))
  ranges)

(defn highlight-comments
  [text marker ranges]
  (def n (length text))
  (def mlen (length marker))
  (var pos 0)
  (while (< pos n)
    (when (<= (+ pos mlen) n)
      (def ch (string/slice text pos (+ pos mlen)))
      (when (= ch marker)
        (def start pos)
        (set pos (skip-while (fn [c] (not= c "\n")) text pos n))
        (array/push ranges [start pos "comment-face"])
        (set pos (- pos 1))))
    (set pos (+ pos 1)))
  ranges)

(defn highlight-strings
  [text ranges]
  (def n (length text))
  (var pos 0)
  (while (< pos n)
    (def ch (string/slice text pos (+ pos 1)))
    (when (= ch "\"")
      (def start pos)
      (set pos (+ pos 1))
      (while (and (< pos n)
                  (not= (string/slice text pos (+ pos 1)) "\""))
        (when (= (string/slice text pos (+ pos 1)) "\\")
          (set pos (+ pos 1)))
        (set pos (+ pos 1)))
      (when (< pos n)
        (set pos (+ pos 1))
        (array/push ranges [start pos "string-face"])))
    (set pos (+ pos 1)))
  ranges)

(defn highlight-numbers
  [text ranges]
  (def n (length text))
  (var pos 0)
  (while (< pos n)
    (def ch (string/slice text pos (+ pos 1)))
    (when (and (or (= ch "0") (= ch "1") (= ch "2") (= ch "3") (= ch "4")
                   (= ch "5") (= ch "6") (= ch "7") (= ch "8") (= ch "9"))
               (or (= pos 0) (not (word-char? (string/slice text (- pos 1) pos)))))
      (def start pos)
      (set pos (+ pos 1))
      (while (and (< pos n)
                  (let [c (string/slice text pos (+ pos 1))]
                    (or (= c "0") (= c "1") (= c "2") (= c "3") (= c "4")
                        (= c "5") (= c "6") (= c "7") (= c "8") (= c "9")
                        (= c ".") (= c "x") (= c "X")
                        (= c "e") (= c "E"))))
        (set pos (+ pos 1)))
      (array/push ranges [start pos "constant-face"])
      (set pos (- pos 1)))
    (set pos (+ pos 1)))
  ranges)

# --- Dirty-range highlighting (docs/core-redesign.md §7.1) ----------------
#
# A buffer-changed event carries the transaction: revision-before/after and
# "start:old-end:new-end" edit spans.  When the cached layer matches the
# event's before-revision, only the affected line region is re-scanned and
# the cached ranges are spliced (drop intersecting, shift following) instead
# of re-deriving the whole buffer.

(def *hl-cache* @{})

(defn- parse-edits
  [s]
  (def out @[])
  (when (or (nil? s) (empty? s)) (break out))
  (each part (string/split "," s)
    (def f (string/split ":" part))
    (when (= 3 (length f))
      (def st (scan-number (f 0)))
      (def oe (scan-number (f 1)))
      (def ne (scan-number (f 2)))
      (when (and st oe ne) (array/push out [st oe ne]))))
  out)

(defn- run-passes
  [text kw-table comment-marker]
  (var ranges @[])
  (set ranges (highlight-keywords text kw-table ranges))
  (when comment-marker
    (set ranges (highlight-comments text comment-marker ranges)))
  (set ranges (highlight-strings text ranges))
  (set ranges (highlight-numbers text ranges))
  ranges)

(defn- full-highlight
  [buf-id kw-table comment-marker]
  (def text (buffer/slice buf-id 0 (buffer/len buf-id)))
  # Canonical order: sorted by start offset, same as the dirty splice —
  # layers are deterministic regardless of how they were produced.
  (def ranges (sort (array/concat @[] (run-passes text kw-table comment-marker))
                    (fn [a b] (< (a 0) (b 0)))))
  (buffer/set-highlights-layer buf-id "syntax" ranges)
  (put *hl-cache* [buf-id "syntax"]
       @{:rev (buffer/revision buf-id) :ranges ranges}))

(defn- line-start-of
  [buf-id offset]
  (def line (buffer/line-of buf-id offset))
  (or (buffer/line-start-offset buf-id line) 0))

(defn- line-end-of
  [buf-id offset]
  (def line (buffer/line-of buf-id offset))
  (def next-start (buffer/line-start-offset buf-id (+ line 1)))
  (if next-start
    (- next-start 1)
    (buffer/len buf-id)))

(defn- dirty-highlight
  [buf-id kw-table comment-marker rev-after edits]
  (var s nil)
  (var max-new-end 0)
  (var delta 0)
  (each [st oe ne] edits
    (when (or (nil? s) (< st s)) (set s st))
    (when (> ne max-new-end) (set max-new-end ne))
    (+= delta (- ne oe)))
  (set s (line-start-of buf-id s))
  (def e (line-end-of buf-id max-new-end))
  (def old-e (- e delta))
  (def cache (get *hl-cache* [buf-id "syntax"]))
  (def kept @[])
  (each [rs re face] (cache :ranges)
    (cond
      (<= re s) (array/push kept [rs re face])
      (>= rs old-e) (array/push kept [(+ rs delta) (+ re delta) face])))
  (def text (buffer/slice buf-id s e))
  (def fresh (run-passes text kw-table comment-marker))
  # Pass ranges are immutable tuples; rebuild them offset by the region start.
  (def offset-fresh @[])
  (each r fresh
    (array/push offset-fresh @[(+ (r 0) s) (+ (r 1) s) (r 2)]))
  (def ranges (sort (array/concat @[] kept offset-fresh) (fn [a b] (< (a 0) (b 0)))))
  (buffer/set-highlights-layer buf-id "syntax" ranges)
  (put *hl-cache* [buf-id "syntax"] @{:rev rev-after :ranges ranges}))

# --- API ----------------------------------------------------------------

(defn syntax/highlight-buffer
  [buf-id]
  (def path (buffer/path buf-id))
  (def ext (file-extension path))
  (def lang (get *extension-language* ext))
  (def kw-table (when lang (get *syntax-rules* lang)))
  (if (and lang kw-table)
    (full-highlight buf-id kw-table (get *comment-markers* lang))))

(command/define "syntax-highlight"
  (fn [& args]
    (def buf-id (if (> (length args) 0) (args 0) (buffer/current)))
    (when buf-id (syntax/highlight-buffer buf-id))))

(defn- highlight-on-event [data]
  (def raw (get data :buffer-id))
  (when raw
    (def buf-id (scan-number raw))
    (when buf-id
      (def path (buffer/path buf-id))
      (def ext (file-extension path))
      (def lang (get *extension-language* ext))
      (def kw-table (when lang (get *syntax-rules* lang)))
      (when (and lang kw-table)
        # buffer-created payloads carry no revisions — scan-number(nil)
        # throws, so absent keys must degrade to nil, not kill the handler.
        (def rev-before (when-let [r (get data :revision-before)] (scan-number r)))
        (def rev-after (when-let [r (get data :revision-after)] (scan-number r)))
        (def edits (parse-edits (get data :edits)))
        (def cache (get *hl-cache* [buf-id "syntax"]))
        (def take-dirty (and rev-before rev-after cache
                             (= (cache :rev) rev-before)
                             (not (empty? edits))))
        (if take-dirty
          (dirty-highlight buf-id kw-table (get *comment-markers* lang) rev-after edits)
          (full-highlight buf-id kw-table (get *comment-markers* lang)))))))

(event/on "buffer-changed" highlight-on-event)
(event/on "buffer-created" highlight-on-event)

(print "syntax core loaded")
