# LSP client — request/response surface.
#
# Servers are launched by lsp_attach.janet, which owns the launch
# policies declared in plugin manifests (:lsp spec keys).  This file is
# the client: it tracks per-language server state, answers requests, and
# applies results.  Which server serves which language is decided by the
# language registry, never here.

(var *lsp-servers* @{})
(var *buffer-languages* @{})
(var *lsp-progress* @{})

# Buffers that have received didOpen for their server (idempotence across
# focus/attach events).  Only marked once the server is ready — see
# send-did-open — or the one didOpen the server never saw locks the buffer out.
(var *attached-buffers* @{})

(defn language-id-for [buffer-id] (get *buffer-languages* buffer-id))

(defn diagnostics [buffer-id] (buffer/diagnostics buffer-id))

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

(event/on "lsp-initialized"
  (fn [ev]
    (def lang-id (get ev :language))
    (put *lsp-servers* lang-id :ready)
    # Now that the handle is live, sync every buffer of this language.
    (each buf-id (keys *buffer-languages*)
      (when (= lang-id (get *buffer-languages* buf-id))
        (send-did-open buf-id)))))

# A server that crashed (or failed its handshake) must be forgotten here:
# ensure-server refuses to start a language that has a state entry, so
# without the reset the language never comes back.  The kernel bounds
# re-attachment with a restart budget before emitting this event; shared
# server-key groups get the same treatment in lsp_attach.janet.
(event/on "lsp-server-exited"
  (fn [ev]
    (def lang-id (get ev :language))
    (when lang-id (put *lsp-servers* lang-id nil))))

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
  (fn [new-name]
    (def buf-id (buffer/current)) (def lang-id (language-id-for buf-id))
    (when lang-id
      (lsp/rename lang-id (if (empty? new-name) "new_name" new-name)))))

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
