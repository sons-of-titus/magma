# Help buffer — Sprint 2
# Loaded after init.janet and special_buffers.janet.

# ── *Help* helpers ────────────────────────────────────────────────────────────

(defn help/show
  "Display `text` in the *Help* buffer."
  [text]
  (editor/show-help text))

(defn help/close
  "Close the *Help* buffer (focus the previous window buffer)."
  []
  (def win-tbl (window/current))
  (def win-id (get win-tbl :id))
  (def cur-buf (get win-tbl :buffer))
  (def buf-name (when cur-buf (buffer/name cur-buf)))
  (when (= buf-name "*Help*")
    # Switch back to *scratch* or the first available non-help buffer
    (def all-bufs (editor/buffer-list))
    (def other (find (fn [k] (not= (buffer/name k) "*Help*")) all-bufs))
    (when (and other win-id)
      (window/set-buffer win-id other))))

# ── help keymap layer ─────────────────────────────────────────────────────────
#
# When *Help* is focused, `q` closes it.

(command/define "help-close"
  (fn [&] (help/close)))

# ── :help colon verb ──────────────────────────────────────────────────────────

(defn- help-text-for-topic
  [topic]
  (case topic
    "" (string "Magma Help\n"
               "==========\n\n"
               "Type :help <topic> for help on a topic.\n\n"
               "Topics:\n"
               "  buffers    Buffer management\n"
               "  commands   Command reference\n"
               "  events     Built-in events\n"
               "  keymaps    Keymap layers\n"
               "  modes      Major and minor modes\n")
    "buffers"  "Buffer Management\n=================\n\nbuffer/current, buffer/list, buffer/create, buffer/find-or-create,\nbuffer/get-by-name, buffer/set-read-only, buffer/read-only?,\nbuffer/set-ephemeral, buffer/ephemeral?\n"
    "commands" "Commands are registered with command/define and executed via\neditor/run-command or colon-execute.\n"
    "events"   "Built-in events: buffer-created, buffer-focused, buffer-closed,\nbuffer-before-save, buffer-after-save, major-mode-changed,\nbuffer-read-only, buffer-message-appended, warning-emitted,\nhelp-shown, editor-ready\n"
    "keymaps"  "Keymap layers: global, vim, insert, visual, command, replace, search\n"
    "modes"    "Major modes: fundamental, text, prog, and extension-defined modes.\n"
    (string "No help for '" topic "'\n")))

(put *colon-plugins* "help"
  (fn [rest]
    (def topic (string/trim rest))
    (help/show (help-text-for-topic topic))))
