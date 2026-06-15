# Magma built-in syntax highlighting — keyword/regex matching.

(def- *syntax-rules* (table/new 32))

# Store each keyword list as a table {keyword true} for O(1) membership
# testing via `(in kw-table word)`.  Using a tuple/array with `in` only
# works for integer indices; string membership requires a table.
(defn- define-syntax
  [name keywords]
  (def kw-table @{})
  (each kw keywords (put kw-table kw true))
  (put *syntax-rules* name kw-table))

# --- Language definitions ----------------------------------------------

(define-syntax "janet"
  ["defn" "defn-" "def" "def-" "var" "let" "if" "when" "unless"
   "do" "each" "for" "while" "loop" "break"
   "fn" "case" "match" "try" "catch" "protect"
   "import" "require" "native" "declare"
   "and" "or" "not" "in" "not-in"
   "true" "false" "nil"
   "string/trim" "string/slice" "string/split" "string/find"
   "string/replace" "string/replace-all" "string/repeat"
   "buffer/create" "buffer/insert" "buffer/delete" "buffer/slice"
   "buffer/current" "buffer/len" "buffer/line-count" "buffer/set-cursor"
   "buffer/cursor" "buffer/set-highlights" "buffer/clear-highlights"
   "editor/set-mode" "option/get" "option/set"
   "keymap/set" "command/define"
   "math/floor" "os/shell"
   "print" "debug/log"
   "table/new" "array/new"])

(define-syntax "rust"
  ["fn" "let" "mut" "const" "static" "pub" "use" "mod" "struct"
   "enum" "impl" "trait" "where" "as" "in" "for" "while" "loop"
   "if" "else" "match" "return" "unsafe" "async" "await" "move"
   "ref" "self" "Self" "super" "crate"
   "true" "false" "Some" "None" "Ok" "Err"
   "u8" "u16" "u32" "u64" "usize"
   "i8" "i16" "i32" "i64" "isize"
   "f32" "f64" "bool" "char" "String" "&str"
   "Vec" "HashMap" "Box" "Arc" "Rc" "Cell" "RefCell"
   "Result" "Option"])

(define-syntax "python"
  ["def" "class" "return" "if" "elif" "else" "for" "while"
   "import" "from" "as" "try" "except" "finally" "with"
   "yield" "lambda" "pass" "break" "continue"
   "True" "False" "None"
   "self" "cls"
   "print" "len" "range" "int" "str" "float" "list" "dict" "set"
   "is" "in" "not" "and" "or"])

# --- Highlight helpers -------------------------------------------------

(defn- file-extension
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

(defn- guess-language
  [buf-id]
  (def path (buffer/path buf-id))
  (def ext (file-extension path))
  (case ext
    "janet" "janet"
    "rs" "rust"
    "py" "python"
    nil))

(defn- word-char?
  [c]
  (def code (string/bytes c))
  (and code (not (zero? (length code)))
       (or (and (>= (code 0) 97)  (<= (code 0) 122))
           (and (>= (code 0) 65)  (<= (code 0) 90))
           (and (>= (code 0) 48)  (<= (code 0) 57))
           (= (code 0) 95)        # _
           (= (code 0) 47)        # /
           (= (code 0) 45)        # -
           (= (code 0) 63)        # ?
           (= (code 0) 33))))     # !

(defn- skip-while
  "Advance pos while (pred (slice pos (+ pos 1))) is truthy."
  [pred text pos n]
  (var p pos)
  (while (and (< p n) (pred (string/slice text p (+ p 1))))
    (set p (+ p 1)))
  p)

(defn- highlight-keywords
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

(defn- highlight-comments
  [text marker ranges]
  (def n (length text))
  (def mlen (length marker))
  (var pos 0)
  (while (< pos n)
    (def ch (string/slice text pos (+ pos mlen)))
    (when (= ch marker)
      (def start pos)
      (set pos (skip-while (fn [c] (not= c "\n")) text pos n))
      (array/push ranges [start pos "comment-face"])
      (set pos (- pos 1)))
    (set pos (+ pos 1)))
  ranges)

(defn- highlight-strings
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

(defn- highlight-numbers
  [text ranges]
  (def n (length text))
  (var pos 0)
  (while (< pos n)
    (def ch (string/slice text pos (+ pos 1)))
    # Only start a number if ch is a digit AND it is not preceded by a word
    # character (which would make it part of an identifier like "v2" or
    # "table/3").  The previous code ran (set pos (- pos 1)) for ALL digits,
    # including non-standalone ones, causing pos to stay frozen and the loop
    # to spin forever.
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
      # Step back so the outer (set pos (+ pos 1)) lands on the char after
      # the number rather than skipping an extra character.
      (set pos (- pos 1)))
    (set pos (+ pos 1)))
  ranges)

# --- API ----------------------------------------------------------------

(defn syntax/highlight-buffer
  "Highlight a buffer by its ID.  Returns nil."
  [buf-id]
  (def lang (guess-language buf-id))
  (def kw-table (when lang (get *syntax-rules* lang)))
  (unless (and lang kw-table)
    (buffer/clear-highlights buf-id)
    (break))
  (def text (buffer/slice buf-id 0 (buffer/len buf-id)))
  (var ranges @[])
  (set ranges (highlight-keywords text kw-table ranges))
  (def comment-marker (case lang "janet" "#" "rust" "//" "python" "#" nil))
  (when comment-marker
    (set ranges (highlight-comments text comment-marker ranges)))
  (set ranges (highlight-strings text ranges))
  (set ranges (highlight-numbers text ranges))
  (buffer/set-highlights-layer buf-id "syntax" ranges))

(command/define "syntax-highlight"
  "Highlight the current buffer using syntax rules."
  (fn [& args]
    (def buf-id (if (> (length args) 0) (args 0) (buffer/current)))
    (when buf-id (syntax/highlight-buffer buf-id))))

(defn- highlight-on-event [data]
  (def raw (get data :buffer-id))
  (when raw
    (def buf-id (scan-number raw))
    (when buf-id (syntax/highlight-buffer buf-id))))

(event/on "buffer-changed" highlight-on-event)
(event/on "buffer-created" highlight-on-event)

(print "syntax loaded")
