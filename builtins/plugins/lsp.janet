(var *lsp-servers* @{})
(var *buffer-languages* @{})
(var *lsp-progress* @{})
# Buffers that have received didOpen for their server (idempotence across
# focus/attach events).  Only marked once the server is ready — see
# send-did-open — or the one didOpen the server never saw locks the buffer out.
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

(defn- buffer-extension [buffer-id]
  (def path (buffer/path buffer-id)) (def name (or path (buffer/name buffer-id)))
  (def parts (string/split "." name))
  (if (> (length parts) 1) (last parts) nil))

(defn- language-for [buffer-id]
  (def ext (buffer-extension buffer-id)) (when ext (get *lang-servers* (keyword ext))))

(defn diagnostics [buffer-id] (buffer/diagnostics buffer-id))
(defn- language-id-for [buffer-id] (get *buffer-languages* buffer-id))

(defn send-did-open [buffer-id]
  # Only a ready server has a live handle; lsp/did-open against a still-starting
  # server silently drops, and marking the buffer attached then would suppress
  # the retry on `lsp-initialized`.
  (def lang-id (get *buffer-languages* buffer-id))
  (when (and lang-id (= :ready (get *lsp-servers* lang-id))
             (not (get *attached-buffers* buffer-id)))
    (lsp/did-open lang-id buffer-id)
    (lsp/semantic-tokens lang-id buffer-id)
    (put *attached-buffers* buffer-id true)))

(def *missing-reported* @{})

(defn- resolve-command
  "Command override for a language: the `<lang>-lsp-command` editor option
  wins (the per-language config packs set these), then the built-in table.
  Resolution goes through `toolchain/find-extended` and returns the resolved
  *path*, so a server installed outside `$PATH` (keg-only Homebrew, Xcode)
  is both found and spawnable.  A command that is missing everywhere reports
  once and returns nil — a raw spawn failure is the wrong way for a user to
  learn a server is missing."
  [lang-id cmd]
  (def override (option/get (string lang-id "-lsp-command")))
  (def final-cmd (or override cmd))
  (if-let [path (toolchain/find-extended final-cmd)]
    path
    (do
      (unless (get *missing-reported* final-cmd)
        (put *missing-reported* final-cmd true)
        (editor/echo (string "LSP: '" final-cmd
                             "' not found — language server disabled for "
                             lang-id) "error"))
      nil)))

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
    # Now that the handle is live, sync every buffer of this language.
    (each buf-id (keys *buffer-languages*)
      (when (= lang-id (get *buffer-languages* buf-id))
        (send-did-open buf-id)))))

# A server that crashed (or failed its handshake) must be forgotten here:
# attach-if-supported refuses to start a language that has a table entry,
# so without the reset the language never comes back.  The kernel bounds
# re-attachment with a restart budget before emitting this event.
(event/on "lsp-server-exited"
  (fn [ev]
    (def lang-id (get ev :language))
    (when lang-id
      (put *lsp-servers* lang-id nil)
      (each buf-id (keys *buffer-languages*)
        (when (= lang-id (get *buffer-languages* buf-id))
          # didOpen must fire again once the replacement is ready.
          (put *attached-buffers* buf-id nil)
          (attach-if-supported buf-id))))))

# lsp-go-to-definition is a Rust command: goto-definition's fallback calls
# it from inside a Janet command, and dispatching a Janet command there
# nests fibers, which Janet does not support.

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
        (buffer/set-cursor buf-id offset)))))

(event/on "lsp-references"
  (fn [ev]
    # References land in the search/quickfix list via the kernel; keep a
    # subscriber so observers (and tests) can react to the result.
    (def locations (get ev :locations))
    (when locations (editor/log-message (string "lsp: " locations)))))

(command/define "lsp-hover"
  (fn [] (def buf-id (buffer/current)) (def lang-id (language-id-for buf-id)) (when lang-id (lsp/hover lang-id))))

# Hover text lands in the pointer-dwell card (or a status message when no
# card is showing) via the kernel; no handler is needed here.

(command/define "lsp-code-actions"
  (fn [] (def buf-id (buffer/current)) (def lang-id (language-id-for buf-id)) (when lang-id (lsp/code-actions lang-id))))

(command/define "lsp-references"
  (fn [] (def buf-id (buffer/current)) (def lang-id (language-id-for buf-id)) (when lang-id (lsp/references lang-id))))

# Code-action results open the kernel's quick-fix picker; applying an
# action routes through the shared workspace-edit applier.

(command/define "lsp-completion"
  (fn [] (def buf-id (buffer/current)) (def lang-id (language-id-for buf-id)) (when lang-id (lsp/completion lang-id))))

# lsp-completion-items results are merged into the completion popup by the
# kernel (server items lead, duplicates drop); no handler is needed here.

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
    # clamp: responses are routinely shorter than the truncation width
    (def method (get ev :method)) (def result (or (get ev :result) ""))
    (editor/log-message (string "[lsp/" method "] " (string/slice result 0 (min 200 (length result)))))
    # Hover and definition results answer a question the user asked —
    # they surface as transient messages, not just the log.
    (when (or (= method "textDocument/hover") (= method "textDocument/definition"))
      (editor/echo (string/slice result 0 (min 160 (length result)))))))

(defn attach-from-event [ev]
  (when-let [id (scan-number (get ev :buffer-id))]
    (attach-if-supported id)
    (send-did-open id)))

(event/on "buffer-created" attach-from-event)
(event/on "buffer-focused" attach-from-event)

(print "lsp.janet loaded")
