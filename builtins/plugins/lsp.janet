(var *lsp-servers* @{})
(var *buffer-languages* @{})
(var *lsp-progress* @{})

(def *lang-servers*
  {:rs    {:command "rust-analyzer" :args @[] :language-id "rust"}
   :py    {:command "pyright" :args @["--stdio"] :language-id "python"}
   :janet {:command "janet-lsp" :args @[] :language-id "janet"}
   :js    {:command "typescript-language-server" :args @["--stdio"] :language-id "javascript"}
   :ts    {:command "typescript-language-server" :args @["--stdio"] :language-id "typescript"}
   :go    {:command "gopls" :args @[] :language-id "go"}
   :lua   {:command "lua-language-server" :args @[] :language-id "lua"}
   :json  {:command "vscode-json-languageserver" :args @["--stdio"] :language-id "json"}
   :clj   {:command "clojure-lsp" :args @[] :language-id "clojure"}})

(defn- json-escape [s]
  (->> s (string/replace-all "\\" "\\\\") (string/replace-all "\"" "\\\"")
    (string/replace-all "\n" "\\n") (string/replace-all "\r" "\\r") (string/replace-all "\t" "\\t")))

(defn- file-extension [buffer-id]
  (def path (buffer/path buffer-id)) (def name (or path (buffer/name buffer-id)))
  (def parts (string/split "." name))
  (if (> (length parts) 1) (last parts) nil))

(defn- language-for [buffer-id]
  (def ext (file-extension buffer-id)) (when ext (get *lang-servers* (keyword ext))))

(defn diagnostics [buffer-id] (buffer/diagnostics buffer-id))
(defn- language-id-for [buffer-id] (get *buffer-languages* buffer-id))

(defn- ensure-server [language-id command args]
  (var server (get *lsp-servers* (keyword language-id)))
  (when server (lsp/start language-id command (splice args)) (put *lsp-servers* (keyword language-id) true)))

(defn- buffer-uri [buffer-id]
  (def path (buffer/path buffer-id))
  (if path (string "file://" path) (string "untitled:" (buffer/name buffer-id))))

(defn attach-if-supported [buffer-id]
  (def cfg (language-for buffer-id))
  (when cfg
    (def lang-id (get cfg :language-id)) (def cmd (get cfg :command)) (def args (get cfg :args))
    (def uri (buffer-uri buffer-id)) (def text (buffer/slice buffer-id 0 (buffer/len buffer-id)))
    (put *buffer-languages* buffer-id lang-id)
    (lsp/start lang-id cmd (splice args))
    (lsp/notify lang-id "textDocument/didOpen"
      (string/format "{\"textDocument\":{\"uri\":\"%s\",\"languageId\":\"%s\",\"version\":1,\"text\":\"%s\"}}"
        uri lang-id (json-escape text)))))

(command/define "lsp-go-to-definition"
  (fn []
    (def buf-id (buffer/current)) (def lang-id (language-id-for buf-id))
    (when lang-id
      (def uri (buffer-uri buf-id)) (def pos (buffer/cursor buf-id))
      (def line (buffer/line-number buf-id pos))
      (def col (- pos (buffer/line-start-offset buf-id line)))
      (lsp/request lang-id "textDocument/definition"
        (string/format "{\"textDocument\":{\"uri\":\"%s\"},\"position\":{\"line\":%d,\"character\":%d}}" uri line col)))))

(event/on "lsp-definition"
  (fn [ev]
    (def uri (get ev :uri)) (def sl (get ev :start-line)) (def sc (get ev :start-col))
    (when uri
      (command/run "open-file" {:path uri})
      (def buf-id (buffer/current))
      (def line-start (buffer/line-start-offset buf-id (scan-number sl)))
      (buffer/cursor buf-id (+ line-start (scan-number sc))))))

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
    (when lang-id (lsp/rename lang-id (or new-name "new_name")))))

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
    (editor/log-message (string "[lsp/" method "] " (slice result 0 200)))))

(event/on "buffer-created" (fn [ev] (attach-if-supported (get ev :buffer-id))))

(event/on "buffer-changed"
  (fn [ev]
    (def buf-id (get ev :buffer-id)) (def lang-id (get *buffer-languages* buf-id))
    (when lang-id
      (def uri (buffer-uri buf-id))
      (def text (buffer/slice buf-id 0 (buffer/len buf-id)))
      (lsp/notify lang-id "textDocument/didChange"
        (string/format "{\"textDocument\":{\"uri\":\"%s\",\"version\":2},\"contentChanges\":[{\"text\":\"%s\"}]}"
          uri (json-escape text))))))

(defn- lsp-colon [arg]
  (case arg
    "gd" (command/run "lsp-go-to-definition") "definition" (command/run "lsp-go-to-definition")
    "hover" (command/run "lsp-hover") "actions" (command/run "lsp-code-actions")
    "rename" (command/run "lsp-rename")
    (editor/log-message (string "Unknown LSP command: " arg))))

(put *colon-plugins* "lsp" lsp-colon)
(print "lsp.janet loaded")
