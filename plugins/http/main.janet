# Http plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "http" "treesitter")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "http"
  ["GET" "POST" "PUT" "DELETE" "PATCH" "HEAD" "OPTIONS" "HTTP"]
  "#")

(each ext ["http" "rest"]
  (syntax/map-extension ext "http"))

# ── Request parsing ──────────────────────────────────────────────────────────
# A file holds one or more requests separated by `###` lines.  A request
# is a request line (`METHOD url`), optional `Name: value` header lines,
# a blank line, then an optional body.  `@name = value` lines anywhere in
# the file define variables referenced as {{name}}.

(def http-methods
  @["GET" "POST" "PUT" "DELETE" "PATCH" "HEAD" "OPTIONS"])

(defn- split-segments
  "Split source text into requests at `###` separator lines.  Each
   segment is the array of its lines."
  [text]
  (def segs @[])
  (var cur @[])
  (each line (string/split "\n" text)
    (if (string/has-prefix? "###" (string/trim line))
      (do (array/push segs cur) (set cur @[]))
      (array/push cur line)))
  (array/push segs cur)
  segs)

(defn- segment-at
  "The request segment containing byte offset `off`, or nil when the
   cursor sits between requests (on a separator or trailing blank lines)."
  [text off]
  (def line-num
    (+ 1 (length (string/find-all "\n" (string/slice text 0 off)))))
  (var seg-line 1)
  (var found nil)
  (each seg (split-segments text)
    (when (and (nil? found) (>= line-num seg-line) (<= line-num (+ seg-line (length seg))))
      (set found seg))
    (+= seg-line (+ (length seg) 1)))
  found)

(defn- collect-variables
  "The `@name = value` lines of the whole file, name → value."
  [text]
  (def vars @{})
  (each line (string/split "\n" text)
    (def t (string/trim line))
    (when (string/has-prefix? "@" t)
      (def eq (string/find "=" t))
      (when (and eq (> eq 1))
        (put vars
             (string/trim (string/slice t 1 eq))
             (string/trim (string/slice t (+ eq 1)))))))
  vars)

(defn- substitute
  "Expand {{name}} references from the file's variables."
  [s vars]
  (var out (string s))
  (eachp [k v] vars
    (set out (string/replace-all (string "{{" k "}}") v out)))
  out)

(defn- parse-request
  "Parse one request segment into {:method :url :headers :body}."
  [lines]
  (var method nil)
  (var url nil)
  (def headers @{})
  (def body @[])
  (var seen-blank false)
  (each raw lines
    (def line (string/trim raw))
    (if (= (length line) 0)
      (set seen-blank true)
      (cond
        # Anything after the first blank line is the body.
        seen-blank (array/push body raw)
        (nil? method)
        (do
          (def parts (string/split " " line))
          (set method (first parts))
          (set url (string/trim (string/join (slice parts 1) " "))))
        (string/has-prefix? "#" line) nil
        # Header lines carry a colon; a stray body line before a blank
        # line is tolerated as body.
        (string/find ":" line)
        (do
          (def ci (string/find ":" line))
          (put headers
               (string/trim (string/slice line 0 ci))
               (string/trim (string/slice line (+ ci 1)))))
        (array/push body raw))))
  {:method method :url url :headers headers :body (string/join body "\n")})

# ── Response output ──────────────────────────────────────────────────────────

(defn http-output/write
  "Append text to the *http-response* buffer, creating it on first use."
  [text]
  (def buf (buffer/find-or-create "*http-response*"))
  (buffer/set-ephemeral buf true)
  (buffer/set-read-only buf false)
  (buffer/insert buf (buffer/len buf) text)
  (buffer/set-read-only buf true))

(defn- clip-body
  [body]
  (def max-chars (scan-number (option/get "http-max-output")))
  (if (and max-chars (> (length body) max-chars))
    (string (string/slice body 0 max-chars) "\n… (truncated)")
    body))

(defn- await-response
  "Route one pending request's response into *http-response*."
  [id url]
  (net/on-response
    id
    (fn [res]
      (if (get res :error)
        (http-output/write (string "✗ " url "\n  error: " (get res :error) "\n\n"))
        (http-output/write
          (string "← " (get res :status) " " url "\n\n"
                  (clip-body (get res :body)) "\n\n"))))))

# ── Sending ──────────────────────────────────────────────────────────────────

(defn http/send
  "Send the request containing the cursor of the focused buffer."
  []
  (def buf (buffer/current))
  (unless buf (editor/echo "http: no buffer"))
  (when buf
    (def text (buffer/slice buf 0 (buffer/len buf)))
    (def off (or (buffer/cursor buf) 0))
    (def seg (segment-at text off))
    (if (nil? seg)
      (editor/echo "http: cursor is not inside a request")
      (do
        (def req (parse-request seg))
        (def vars (collect-variables text))
        (def method (get req :method))
        (def url (substitute (or (get req :url) "") vars))
        (if (or (nil? method) (= (length url) 0))
          (editor/echo "http: request needs a `METHOD url` line")
          (do
            (def headers @{})
            (eachp [k v] (get req :headers)
              (put headers (substitute k vars) (substitute v vars)))
            (http-output/write (string "→ " method " " url "\n"))
            (def id (net/http-request method url (substitute (get req :body) vars) headers))
            (await-response id url)))))))

(command/define "http-send-request" (fn [&] (http/send)))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "http-mode"
  {:parent "prog"
   :layers [:http-mode]
   :setup (fn []
            (def width (option/get "http-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

(keymap/set "ctrl-return" "http-send-request" "http-mode")
(keymap/set "meta-r" "http-send-request" "http-mode")

# ── Colon verbs ──────────────────────────────────────────────────────────────

(defn colon-http
  "Fetch a URL and show the response: :http <url>"
  [arg]
  (def url (string/trim (string arg)))
  (if (= (length url) 0)
    (print "http: provide a URL")
    (do
      (http-output/write (string "→ GET " url "\n"))
      (def id (net/http-get url))
      (await-response id url))))
