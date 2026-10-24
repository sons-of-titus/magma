(var *lsp-servers* @{})
(var *buffer-languages* @{})
(var *lsp-progress* @{})
# Buffers whose language server is still initializing — didOpen is sent
# when the `lsp-initialized` event confirms the server is ready.
(var *pending-attach* @{})
# Buffers that have received didOpen for their server (idempotence across
# focus/attach events).
(var *attached-buffers* @{})

(def *lang-servers*
  {:rs    {:command "rust-analyzer" :args @[] :language-id "rust"}
   :py    {:command "pyright" :args @["--stdio"] :language-id "python"}
   :janet {:command "janet-lsp" :args @[] :language-id "janet"}
   :js    {:command "typescript-language-server" :args @["--stdio"] :language-id "javascript"}
   :ts    {:command "typescript-language-server" :args @["--stdio"] :language-id "typescript"}
   :go    {:command "gopls" :args @[] :language-id "go"}
   :lua   {:command "lua-language-server" :args @[] :language-id "lua"}
   :json  {:command "vscode-json-languageserver" :args @["--stdio"] :language-id "json"}
   :clj   {:command "clojure-lsp" :args @[] :language-id "clojure"}
   :c     {:command "clangd" :args @[] :language-id "c"}
   :cpp   {:command "clangd" :args @[] :language-id "cpp"}
   :rb    {:command "solargraph" :args @["stdio"] :language-id "ruby"}
   :sh    {:command "bash-language-server" :args @["start"] :language-id "bash"}
   :java  {:command "jdtls" :args @[] :language-id "java"}
   :kt    {:command "kotlin-language-server" :args @[] :language-id "kotlin"}
   :swift {:command "sourcekit-lsp" :args @[] :language-id "swift"}
   :hs    {:command "haskell-language-server" :args @[] :language-id "haskell"}
   :ex    {:command "elixir-ls" :args @[] :language-id "elixir"}
   :erl   {:command "erlang_ls" :args @[] :language-id "erlang"}
   :nim   {:command "nimlangserver" :args @[] :language-id "nim"}
   :zig   {:command "zls" :args @[] :language-id "zig"}
   :nix   {:command "nil" :args @[] :language-id "nix"}
   :tf    {:command "terraform-ls" :args @["serve"] :language-id "terraform"}
   :yaml  {:command "yaml-language-server" :args @["--stdio"] :language-id "yaml"}
   :toml  {:command "taplo" :args @["lsp" "stdio"] :language-id "toml"}
   :html  {:command "vscode-html-languageserver" :args @["--stdio"] :language-id "html"}
   :css   {:command "vscode-css-languageserver" :args @["--stdio"] :language-id "css"}
   :sql   {:command "sql-language-server" :args @["up" "--method" "stdio"] :language-id "sql"}})

(defn- json-escape [s]
  (->> s (string/replace-all "\\" "\\\\") (string/replace-all "\"" "\\\"")
    (string/replace-all "\n" "\\n") (string/replace-all "\r" "\\r") (string/replace-all "\t" "\\t")))

(defn- buffer-extension [buffer-id]
  (def path (buffer/path buffer-id)) (def name (or path (buffer/name buffer-id)))
  (def parts (string/split "." name))
  (if (> (length parts) 1) (last parts) nil))

(defn- language-for [buffer-id]
  (def ext (buffer-extension buffer-id)) (when ext (get *lang-servers* (keyword ext))))

(defn diagnostics [buffer-id] (buffer/diagnostics buffer-id))
(defn- language-id-for [buffer-id] (get *buffer-languages* buffer-id))

(defn- buffer-uri [buffer-id]
  (def path (buffer/path buffer-id))
  (if path (string "file://" path) (string "untitled:" (buffer/name buffer-id))))

(defn send-did-open [buffer-id]
  (def lang-id (get *buffer-languages* buffer-id))
  (when (and lang-id (get *lsp-servers* lang-id) (not (get *attached-buffers* buffer-id)))
    (lsp/did-open lang-id buffer-id)
    (put *attached-buffers* buffer-id true)))

(def *missing-reported* @{})

(defn- resolve-command
  "Command override for a language: the `<lang>-lsp-command` editor option
  wins (the per-language config packs set these), then the built-in table.
  A command that is not on PATH reports once and returns nil — a raw spawn
  failure is the wrong way for a user to learn a server is missing."
  [lang-id cmd]
  (def override (option/get (string lang-id "-lsp-command")))
  (def final-cmd (or override cmd))
  (unless (toolchain/find final-cmd)
    (unless (get *missing-reported* final-cmd)
      (put *missing-reported* final-cmd true)
      (editor/echo (string "LSP: '" final-cmd
                           "' not found on PATH — language server disabled for "
                           lang-id) "error"))
    nil)
  final-cmd)

(defn attach-if-supported [buffer-id]
  (def cfg (language-for buffer-id))
  (when cfg
    (def lang-id (get cfg :language-id))
    (put *buffer-languages* buffer-id lang-id)
    (unless (get *lsp-servers* lang-id)
      (def cmd (resolve-command lang-id (get cfg :command)))
      (when cmd
        (def args (get cfg :args))
        (put *lsp-servers* lang-id :starting)
        (lsp/start lang-id cmd (splice args))))))

(event/on "lsp-initialized"
  (fn [ev]
    (def lang-id (get ev :language))
    (put *lsp-servers* lang-id :ready)
    (put *pending-attach* lang-id nil)
    # Sync every buffer already attached to this language.
    (each buf-id (keys *buffer-languages*)
      (when (= lang-id (get *buffer-languages* buf-id))
        (send-did-open buf-id)))))

(command/define "lsp-go-to-definition"
  (fn []
    (def buf-id (buffer/current)) (def lang-id (language-id-for buf-id))
    (when lang-id
      (def uri (buffer-uri buf-id)) (def pos (buffer/cursor buf-id))
      # LSP wants (line, UTF-16 code units) — never a byte column.
      (def [line col] (buffer/lsp-position buf-id pos))
      (lsp/request lang-id "textDocument/definition"
        (string/format "{\"textDocument\":{\"uri\":\"%s\"},\"position\":{\"line\":%d,\"character\":%d}}" uri line col)))))

(event/on "lsp-definition"
  (fn [ev]
    (def path (get ev :path)) (def sl (get ev :start-line)) (def sc (get ev :start-col))
    (when path
      (command/run "open-file" :path path)
      (when-let [buf-id (buffer/current)]
        # The server's columns are UTF-16 code units — convert, never add
        # them to a byte line start.
        (def offset (buffer/offset-of-lsp-position buf-id
                      (scan-number sl) (scan-number sc)))
        (buffer/cursor buf-id offset)))))

(command/define "lsp-hover"
  (fn [] (def buf-id (buffer/current)) (def lang-id (language-id-for buf-id)) (when lang-id (lsp/hover lang-id))))

(event/on "lsp-hover"
  (fn [ev] (def contents (get ev :contents)) (when contents (editor/log-message (string "[hover] " contents)))))

(command/define "lsp-code-actions"
  (fn [] (def buf-id (buffer/current)) (def lang-id (language-id-for buf-id)) (when lang-id (lsp/code-actions lang-id))))

(event/on "lsp-code-actions"
  (fn [ev]
    (def actions (get ev :actions))
    (when (and actions (> (length actions) 0)) (editor/log-message (string "[code-actions] " actions)))))

(command/define "lsp-completion"
  (fn [] (def buf-id (buffer/current)) (def lang-id (language-id-for buf-id)) (when lang-id (lsp/completion lang-id))))

(event/on "lsp-completion-items"
  (fn [ev]
    (def items (get ev :items))
    (when (and items (> (length items) 0)) (editor/log-message (string "[completion] " items)))))

(command/define "lsp-rename"
  (fn [& new-name]
    (def buf-id (buffer/current)) (def lang-id (language-id-for buf-id))
    (when lang-id
      (lsp/rename lang-id (if (empty? new-name) "new_name" (first new-name))))))

(event/on "lsp-rename-result"
  (fn [ev] (def edit (get ev :edit)) (when edit (lsp/apply-edit edit) (editor/log-message "[rename] applied"))))

(event/on "lsp-progress"
  (fn [ev]
    (def token (get ev :token)) (def message (get ev :message))
    (put *lsp-progress* token {:message message :percentage (get ev :percentage)})
    (when (and message (> (length message) 0)) (editor/log-message (string "[lsp] " message)))))

(event/on "lsp-response"
  (fn [ev]
    (def method (get ev :method)) (def result (get ev :result))
    (editor/log-message (string "[lsp/" method "] " (slice result 0 200)))
    # Hover and definition results answer a question the user asked —
    # they surface as transient messages, not just the log.
    (when (or (= method "textDocument/hover") (= method "textDocument/definition"))
      (editor/echo (slice result 0 160)))))

(defn attach-from-event [ev]
  (when-let [id (scan-number (get ev :buffer-id))]
    (attach-if-supported id)
    (send-did-open id)))

(event/on "buffer-created" attach-from-event)
(event/on "buffer-focused" attach-from-event)

# Incremental didChange (docs/core-redesign.md §8): one range change per
# edit of the transaction, emitted back-to-front so every range and every
# inserted-text slice is valid against the final buffer; the version is the
# canonical document revision.
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
          uri (or rev "0") (did-change-payload buf-id (get ev :edits)))))))

(print "lsp.janet loaded")
