# LSP document sync — URI building and negotiated didChange.
#
# Split from lsp.janet (file-size gate): lifecycle, attach, and command
# verbs live there; this file owns everything that ships buffer text to
# the server.  Runs after lsp.janet (loader order) so the shared attach
# tables exist.

# Percent-encode the reserved bytes the kernel's path_to_uri encodes.
# didOpen sends the encoded URI — didChange and every other per-document
# message must reference the same string, or the server answers for a
# document it never opened.
(defn- uri-encode-path [path]
  (->> path
       (string/replace-all "%" "%25")
       (string/replace-all " " "%20")
       (string/replace-all "#" "%23")
       (string/replace-all "?" "%3F")))

(defn buffer-uri [buffer-id]
  (def path (buffer/path buffer-id))
  (if path (string "file://" (uri-encode-path path))
      (string "untitled:" (buffer/name buffer-id))))

(defn- escape-json-char [c]
  (case (first c)
    34 "\\\""
    92 "\\\\"
    8 "\\b"
    9 "\\t"
    10 "\\n"
    12 "\\f"
    13 "\\r"
    (string/format "\\u%04X" (first c))))

# Every byte not matched by the escape class passes through verbatim,
# including raw UTF-8 above 0x7F — JSON strings carry UTF-8 natively.
(def json-escape-peg
  ~(any (+ (/ (<- (range "\x00\x1F" "\"\"" "\\\\")) ,escape-json-char)
          (<- (range "\x20\x21" "\x23\x5B" "\x5D\x7E" "\x80\xFF")))))

(defn- json-escape [s]
  (peg/match json-escape-peg s))

# One range change per edit of the transaction, emitted back-to-front so
# every range and every inserted-text slice is valid against the final
# buffer; the version is the canonical document revision
# (docs/core-redesign.md §8).
(defn- parse-edit-spans
  [s]
  (def out @[])
  (when (and s (not (empty? s)))
    (each part (string/split "," s)
      (def f (string/split ":" part))
      (when (= 3 (length f))
        (def st (scan-number (f 0)))
        (def oe (scan-number (f 1)))
        (def ne (scan-number (f 2)))
        (when (and st oe ne) (array/push out [st oe ne])))))
  out)

(defn- full-text-change [buf-id]
  (string/format "{\"text\":\"%s\"}"
    (json-escape (buffer/slice buf-id 0 (buffer/len buf-id)))))

(defn- range-changes [buf-id edits-str]
  (def spans (parse-edit-spans edits-str))
  (when (not (empty? spans))
    (def parts @[])
    (var i (- (length spans) 1))
    (while (>= i 0)
      (def [st oe ne] (spans i))
      (def p1 (buffer/lsp-position buf-id st))
      (def p2 (buffer/lsp-position buf-id ne))
      (def text (buffer/slice buf-id st ne))
      (array/push parts (string/format
        "{\"range\":{\"start\":{\"line\":%d,\"character\":%d},\"end\":{\"line\":%d,\"character\":%d}},\"rangeLength\":%d,\"text\":\"%s\"}"
        (p1 0) (p1 1) (p2 0) (p2 1) (- oe st) (json-escape text)))
      (-= i 1))
    (string/join parts ",")))

# Incremental range changes only when the server declared kind 2 in its
# initialize result.  Full text is valid against kinds 1 and 2 alike, so
# anything else — including servers that never negotiated — syncs whole.
(defn- content-changes [lang-id buf-id edits-str]
  (or (and (= 2 (lsp/sync-kind lang-id)) (range-changes buf-id edits-str))
      (full-text-change buf-id)))

(event/on "buffer-changed"
  (fn [ev]
    (def buf-id (scan-number (get ev :buffer-id)))
    (def lang-id (get *buffer-languages* buf-id))
    (when (and lang-id buf-id)
      (def uri (buffer-uri buf-id))
      (def rev (get ev :revision-after))
      (lsp/notify lang-id "textDocument/didChange"
        (string/format "{\"textDocument\":{\"uri\":\"%s\",\"version\":%s},\"contentChanges\":[%s]}"
          uri (or rev "0") (content-changes lang-id buf-id (get ev :edits))))
      # Fresh tokens for the fresh text.  The client supersedes an older
      # in-flight request for the same buffer, so this stays safe to fire
      # on every change; servers debounce on their side.
      (lsp/semantic-tokens lang-id buf-id))))

(print "lsp-sync loaded")
