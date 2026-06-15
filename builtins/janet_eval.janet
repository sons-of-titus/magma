# builtins/janet_eval.janet
# Janet Eval and interactive REPL support (Sprint 12).
# Requires *colon-plugins* and buffer/* API from init.janet.

# ── *janet-output* buffer helper ─────────────────────────────────────────────

(defn janet-output/write
  "Append text to the *janet-output* buffer, creating it on first use."
  [text]
  (def buf (buffer/find-or-create "*janet-output*"))
  (buffer/set-ephemeral buf true)
  (buffer/set-read-only buf false)
  (buffer/insert buf (buffer/len buf) text)
  (buffer/set-read-only buf true))

# ── eval-result event handler ────────────────────────────────────────────────
# Routes eval-region / eval-buffer results to *janet-output*.

(event/on "eval-result"
  (fn [data]
    (def val (get data :value ""))
    (def err (get data :error ""))
    (if (> (length err) 0)
      (janet-output/write (string "Error: " err "\n"))
      (janet-output/write (string val "\n")))))

# ── :janet colon verb ────────────────────────────────────────────────────────
# `:janet expr` evaluates a Janet expression and shows the result.

(put *colon-plugins* "janet"
  (fn [rest]
    (def trimmed (string/trim rest))
    (when (> (length trimmed) 0)
      (def result (editor/eval trimmed))
      (janet-output/write (string "> " trimmed "\n" result "\n"))
      (def buf (buffer/find-or-create "*janet-output*"))
      (when-let [win (window/current)]
        (window/set-buffer win buf)))))

# ── *janet-repl* interactive buffer ─────────────────────────────────────────

(var *janet-repl-input-start* 0)
(var *janet-repl-history* @[])
(var *janet-repl-history-idx* -1)

(defn janet-repl/write
  "Append text to the *janet-repl* buffer."
  [text]
  (def buf (buffer/find-or-create "*janet-repl*"))
  (buffer/set-read-only buf false)
  (buffer/insert buf (buffer/len buf) text)
  (buffer/set-read-only buf true))

(defn janet-repl/write-prompt []
  (janet-repl/write "janet> ")
  (def buf (buffer/find-or-create "*janet-repl*"))
  (set *janet-repl-input-start* (buffer/len buf))
  (buffer/set-read-only buf false))

(defn janet-repl/submit
  "Evaluate the current input line in *janet-repl* and show the result."
  []
  (def buf (buffer/find-or-create "*janet-repl*"))
  (def total-len (buffer/len buf))
  (def input
    (if (>= *janet-repl-input-start* total-len)
      ""
      (string/trim (buffer/slice buf *janet-repl-input-start* total-len))))
  (buffer/set-read-only buf true)
  (janet-repl/write "\n")
  (when (> (length input) 0)
    (array/push *janet-repl-history* input)
    (set *janet-repl-history-idx* (length *janet-repl-history*))
    (def result (editor/eval input))
    (janet-repl/write (string result "\n")))
  (janet-repl/write-prompt))

(command/define "janet-repl-submit"
  (fn [&] (janet-repl/submit)))

(defn janet-repl/open []
  (def buf (buffer/find-or-create "*janet-repl*"))
  (buffer/set-ephemeral buf true)
  (when-let [win (window/current)]
    (window/set-buffer win buf))
  (when (= (buffer/len buf) 0)
    (janet-repl/write "Janet REPL — press return to evaluate\n\n")
    (janet-repl/write-prompt)))

(put *colon-plugins* "janet-repl"
  (fn [_] (janet-repl/open)))
