# LSP document sync — URI building and incremental didChange.
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

(defn- json-escape [s]
  (->> s (string/replace-all "\\" "\\\\") (string/replace-all "\"" "\\\"")
    (string/replace-all "\n" "\\n") (string/replace-all "\r" "\\r") (string/replace-all "\t" "\\t")))

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

(defn- did-change-payload
  [buf-id edits-str]
  (def spans (parse-edit-spans edits-str))
  (def changes
    (if (empty? spans)
      # No edit detail (e.g. a view-switch ping): full-text sync.
      (string/format "{\"text\":\"%s\"}"
        (json-escape (buffer/slice buf-id 0 (buffer/len buf-id))))
      (do
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
        (string/join parts ","))))
  changes)

(event/on "buffer-changed"
  (fn [ev]
    (def buf-id (scan-number (get ev :buffer-id)))
    (def lang-id (get *buffer-languages* buf-id))
    (when (and lang-id buf-id)
      (def uri (buffer-uri buf-id))
      (def rev (get ev :revision-after))
      (lsp/notify lang-id "textDocument/didChange"
        (string/format "{\"textDocument\":{\"uri\":\"%s\",\"version\":%s},\"contentChanges\":[%s]}"
          uri (or rev "0") (did-change-payload buf-id (get ev :edits))))
      # Fresh tokens for the fresh text.  The client supersedes an older
      # in-flight request for the same buffer, so this stays safe to fire
      # on every change; servers debounce on their side.
      (lsp/semantic-tokens lang-id buf-id))))

(print "lsp-sync loaded")
