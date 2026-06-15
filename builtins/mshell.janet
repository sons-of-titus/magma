# builtins/mshell.janet
# MShell — a Janet-powered buffer shell, Sprint 12.
# Requires fs/cwd, fs/chdir, process/spawn, editor/eval from Rust.

# ── State ─────────────────────────────────────────────────────────────────────

(var *mshell-cwd* (fs/cwd))
(var *mshell-history* @[])
(var *mshell-history-idx* -1)
(var *mshell-input-start* 0)

# ── Prompt ────────────────────────────────────────────────────────────────────

(var mshell-prompt
  (fn [] (string *mshell-cwd* " λ ")))

# ── Built-in commands ─────────────────────────────────────────────────────────

(def *mshell-builtins* @{})

(put *mshell-builtins* "cd"
  (fn [args]
    (def dir (if (or (nil? args) (empty? args))
               (or (os/getenv "HOME") "/")
               (first args)))
    (try
      (do (fs/chdir dir)
          (set *mshell-cwd* (fs/cwd))
          nil)
      ([err] (string "cd: " err)))))

(put *mshell-builtins* "pwd"
  (fn [_] *mshell-cwd*))

(put *mshell-builtins* "echo"
  (fn [args] (string/join args " ")))

(put *mshell-builtins* "export"
  (fn [_] nil))

(put *mshell-builtins* "history"
  (fn [_]
    (def lines @[])
    (var i 0)
    (each h *mshell-history*
      (array/push lines (string (++ i) "  " h)))
    (string/join lines "\n")))

(put *mshell-builtins* "clear"
  (fn [_]
    (def buf (buffer/find-or-create "*mshell*"))
    (buffer/set-read-only buf false)
    (def len (buffer/len buf))
    (when (> len 0) (buffer/delete buf 0 len))
    (buffer/set-read-only buf true)
    nil))

# ── Output helpers ────────────────────────────────────────────────────────────

(defn mshell/append
  "Append text to the *mshell* buffer (bypasses read-only)."
  [text]
  (def buf (buffer/find-or-create "*mshell*"))
  (buffer/set-read-only buf false)
  (buffer/insert buf (buffer/len buf) text)
  (buffer/set-read-only buf true))

(defn mshell/write-prompt []
  (mshell/append (mshell-prompt))
  (def buf (buffer/find-or-create "*mshell*"))
  (set *mshell-input-start* (buffer/len buf))
  (buffer/set-read-only buf false))

# ── Input parsing ─────────────────────────────────────────────────────────────

(defn mshell/parse-line
  "Parse an input line into [:janet expr], [:builtin name args], or [:shell cmd args]."
  [input]
  (def trimmed (string/trim input))
  (cond
    (string/has-prefix? "(" trimmed)
    [:janet trimmed]

    (do
      (def space (string/find " " trimmed))
      (def cmd (if space (string/slice trimmed 0 space) trimmed))
      (def args-str (if space (string/trim (string/slice trimmed (+ space 1))) ""))
      (def args (if (> (length args-str) 0) (string/split " " args-str) []))
      (if (get *mshell-builtins* cmd)
        [:builtin cmd args]
        [:shell cmd args]))))

# ── Command dispatch ──────────────────────────────────────────────────────────

(defn mshell/execute
  "Execute a parsed mshell command."
  [parsed]
  (case (get parsed 0)
    :janet
    (do
      (def result (editor/eval (get parsed 1)))
      (mshell/append (string result "\n"))
      (mshell/write-prompt))

    :builtin
    (do
      (def result ((get *mshell-builtins* (get parsed 1)) (get parsed 2)))
      (when result (mshell/append (string result "\n")))
      (mshell/write-prompt))

    :shell
    (do
      (def cmd (get parsed 1))
      (def args (get parsed 2 []))
      (def args-str (string/join args " "))
      (def full-cmd (if (> (length args-str) 0) (string cmd " " args-str) cmd))
      (def pid (process/spawn full-cmd "" *mshell-cwd*))
      (event/on "process-output"
        (fn [data]
          (when (= (get data :id "0") (string pid))
            (mshell/append (string (get data :line "") "\n")))))
      (event/once "process-exit"
        (fn [data]
          (when (= (get data :id "0") (string pid))
            (mshell/write-prompt)))))))

# ── Return handler ────────────────────────────────────────────────────────────

(defn mshell/submit
  "Submit the current input line for execution."
  []
  (def buf (buffer/find-or-create "*mshell*"))
  (def total-len (buffer/len buf))
  (def input
    (string/trim
      (if (>= *mshell-input-start* total-len)
        ""
        (buffer/slice buf *mshell-input-start* total-len))))
  (buffer/set-read-only buf true)
  (mshell/append "\n")
  (when (> (length input) 0)
    (array/push *mshell-history* input)
    (set *mshell-history-idx* (length *mshell-history*))
    (mshell/execute (mshell/parse-line input)))
  (when (= (mshell/parse-line (or input " ")) [:shell "" []])
    (mshell/write-prompt)))

(command/define "mshell-submit" (fn [&] (mshell/submit)))

# ── Mode ──────────────────────────────────────────────────────────────────────

(major-mode/define "mshell-mode"
  {:parent "text"
   :setup (fn []
     (option/set-local "read-only" "false"))})

# ── Open / colon verbs ────────────────────────────────────────────────────────

(defn mshell/open []
  (def buf (buffer/find-or-create "*mshell*"))
  (buffer/set-ephemeral buf true)
  (when-let [win (window/current)]
    (window/set-buffer win buf))
  (editor/run-command "mshell-mode")
  (when (= (buffer/len buf) 0)
    (mshell/append "MShell — Janet-powered buffer shell\n")
    (mshell/append "Type shell commands or Janet expressions (starting with '(')\n\n")
    (mshell/write-prompt)))

(put *colon-plugins* "mshell" (fn [_] (mshell/open)))
(put *colon-plugins* "ms"    (fn [_] (mshell/open)))
