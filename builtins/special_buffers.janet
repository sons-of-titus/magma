# Special buffer management — Sprint 2
# Loaded after init.janet so *colon-plugins* and command/define are available.

# ── *scratch* initialisation ──────────────────────────────────────────────────
#
# The *scratch* buffer is created at startup by Rust with MajorMode::Prog.
# We insert a welcome comment on editor-ready so the user sees something useful.

(event/on "editor-ready"
  (fn [_data]
    (def scratch (buffer/get-by-name "*scratch*"))
    (when scratch
      (def welcome ";; This is *scratch* — a temporary space for notes and Janet evaluation.\n;; Changes here are not saved.\n\n")
      (when (= (buffer/len scratch) 0)
        (buffer/set-read-only scratch false)
        (buffer/insert scratch 0 welcome)
        (buffer/set-read-only scratch false)))))

# ── *Messages* helpers ────────────────────────────────────────────────────────

(defn messages/open
  "Open or switch to the *Messages* buffer."
  []
  (def key (buffer/find-or-create "*Messages*"))
  (buffer/set-read-only key true)
  (buffer/set-ephemeral key true)
  (def win-id (get (window/current) :id))
  (when win-id
    (window/set-buffer win-id key)))

(put *colon-plugins* "messages"
  (fn [_] (messages/open)))

# ── *Warnings* helpers ────────────────────────────────────────────────────────

(defn warnings/open
  "Open or switch to the *Warnings* buffer."
  []
  (def key (buffer/find-or-create "*Warnings*"))
  (buffer/set-read-only key true)
  (buffer/set-ephemeral key true)
  (def win-id (get (window/current) :id))
  (when win-id
    (window/set-buffer win-id key)))

(put *colon-plugins* "warnings"
  (fn [_] (warnings/open)))

# Handle warning-emitted — show a one-liner in messages
(event/on "warning-emitted"
  (fn [data]
    (def text (get data :text ""))
    (when (> (length text) 0)
      (editor/log-message (string "Warning: " text)))))

# ── *janet-output* helpers ────────────────────────────────────────────────────

(defn janet-output/write
  "Append text and a newline to the *janet-output* buffer."
  [text]
  (def key (buffer/find-or-create "*janet-output*"))
  (buffer/set-ephemeral key true)
  (buffer/set-read-only key false)
  (def end (buffer/len key))
  (buffer/insert key end (string text "\n"))
  (buffer/set-read-only key false))

(put *colon-plugins* "janet-output"
  (fn [_]
    (def key (buffer/find-or-create "*janet-output*"))
    (buffer/set-ephemeral key true)
    (def win-id (get (window/current) :id))
    (when win-id
      (window/set-buffer win-id key))))

# ── *Buffer List* ─────────────────────────────────────────────────────────────
#
# :buffers opens a read-only, ephemeral buffer listing all open buffers.
# `return` on a line switches to that buffer.

(defn buffer-list/open
  "Open the *Buffer List* buffer."
  []
  (def key (buffer/find-or-create "*Buffer List*"))
  (buffer/set-ephemeral key true)
  (buffer/set-read-only key false)
  # Clear and repopulate
  (def old-len (buffer/len key))
  (when (> old-len 0) (buffer/delete key 0 old-len))
  (def lines
    (map (fn [bkey]
           (def name (buffer/name bkey))
           (def len  (buffer/len bkey))
           (def mode (buffer/major-mode bkey))
           (string (string/format "%-4d" bkey)
                   (string/format "%-30s" name)
                   (string/format "%-12s" mode)
                   (string len) " bytes\n"))
         (editor/buffer-list)))
  (buffer/insert key 0 (string/join lines ""))
  (buffer/set-read-only key true)
  (def win-id (get (window/current) :id))
  (when win-id
    (window/set-buffer win-id key)))

(put *colon-plugins* "buffers"
  (fn [_] (buffer-list/open)))

(put *colon-plugins* "ls"
  (fn [_] (buffer-list/open)))
