# Formatter registry — plugins declare, this file operates.
#
# A plugin hands over its external formatter in one line:
#   (formatter/register "rust" {:command "rustfmt" :args ["--edition" "2021"]})
# and gets the rest: the `<lang>-format-buffer` command, format-on-save
# behind the `<lang>-format-on-save` option, a resolved binary (outside
# `$PATH` still works), and failures a user can actually see — a missing
# binary reports once per session instead of silently returning the input.

(var *formatters* @{})

# One loud report per formatter binary per session; later failures of the
# same binary only log.  A save hook firing every keystroke batch must not
# become a message flood.
(var *reported* @{})

(defn- mode-language [mode-name]
  (when (and mode-name (string/has-suffix? "-mode" mode-name))
    (string/slice mode-name 0 (- (length mode-name) 5))))

(defn- resolve-command
  "The `<lang>-formatter-command` option wins over the manifest spec, and
   resolution goes through toolchain/find-extended so a formatter outside
   $PATH is both found and spawnable.  nil means missing — report once."
  [lang]
  (def spec (get *formatters* lang))
  (def cmd (or (option/get (string lang "-formatter-command"))
               (get spec :command)))
  (if-let [path (toolchain/find-extended cmd)]
    {:command path :args (get spec :args @[])}
    (do
      (unless (get *reported* cmd)
        (put *reported* cmd true)
        (editor/echo
          (string "Formatter '" cmd "' not found — formatting disabled for "
                  lang " (install it or set " lang "-formatter-command)")
          "error"))
      nil)))

(defn formatter/format
  "Run `lang`'s formatter over `text`.  Returns the text unchanged when the
   formatter is missing, fails, or produces nothing — and says so the
   first time each failure happens."
  [lang text]
  (if (empty? (get *formatters* lang))
    text
    (let [resolved (resolve-command lang)]
      (if (nil? resolved)
        text
        (let [result (process/run (get resolved :command)
                                  (get resolved :args) text)
              code (get result :exit-code -1)]
          (cond
            (= code 0) (let [out (get result :stdout "")]
                         (if (empty? out) text out))
            (do
              (unless (get *reported* lang)
                (put *reported* lang true)
                (def raw (or (get result :stderr "") ""))
                # clamp: slice errors when the end exceeds the length
                (def detail (string/slice raw 0 (min 120 (length raw))))
                (editor/echo
                  (string "Formatter '" (get (get *formatters* lang) :command)
                          "' failed (" code ")" (if (> (length detail) 0)
                                                  (string ": " detail))
                          " — buffer left unchanged")
                  "error"))
              (editor/log-message
                (string "[formatter] " lang " exit " code))
              text)))))))

(defn- format-buffer-if-registered
  "Format the buffer when its language has a formatter and the buffer is
   saved-backing; called from commands and the before-save hook."
  [buf-id &opt force]
  (when buf-id
    (def lang (mode-language (buffer/major-mode buf-id)))
    (when (and lang (get *formatters* lang)
               (or force (= (option/get (string lang "-format-on-save")) "true")))
      (def text (buffer/slice buf-id 0 (buffer/len buf-id)))
      (def formatted (formatter/format lang text))
      (unless (= formatted text)
        (buffer/set-read-only buf-id false)
        (buffer/delete buf-id 0 (buffer/len buf-id))
        (buffer/insert buf-id 0 formatted)
        (buffer/set-read-only buf-id true)
        true))))

(defn formatter/register
  "Declare `lang`'s external formatter: :command (binary name) and :args.
   Also defines `<lang>-format-buffer` for the focused buffer.  Replacing
   an existing registration updates the command in place."
  [lang spec]
  (put *formatters* lang spec)
  (command/define (string lang "-format-buffer")
    (fn [&opt arg]
      # A numeric argument names the buffer (editor/run-command joins
      # positional args onto the command line); otherwise format whatever
      # is focused.  Read-only scratch buffers never take formatting.
      (def target (or (and arg (scan-number arg)) (buffer/current)))
      (when (and target (not (buffer/read-only? target)))
        (format-buffer-if-registered target true)))))

# One subscriber for every language: format-on-save consults the registry
# per buffer instead of each plugin installing its own hook.
(event/on "buffer-before-save"
  (fn [ev]
    (def buf-id (scan-number (get ev :buffer-id)))
    (when buf-id
      (when (not (buffer/read-only? buf-id))
        (format-buffer-if-registered buf-id)))))

# Plugin teardown unregisters the language; its formatter goes with it.
(event/on "language-unregistered"
  (fn [data]
    (put *formatters* (get data :name "?") nil)))
