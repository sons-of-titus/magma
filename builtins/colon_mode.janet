# --- Colon-mode / :set moved from Rust builtin.rs ------------------

# Colon-mode commands — replaces the Rust command-execute, command-backspace, etc.

(command/redefine "exit-command-mode"
  (fn [&]
    (editor/clear-completions)
    (set-vim-mode "normal" false)))

(command/redefine "command-backspace"
  (fn [&]
    (editor/clear-completions)
    (def detail (editor/mode-detail))
    (def input (get detail :input ""))
    (var new-input "")
    (if (> (length input) 0)
      (set new-input (string/slice input 0 (- (length input) 1)))
      (set new-input ""))
    (if (= (length new-input) 0)
      (set-vim-mode "normal" false)
      (minibuffer/set-command-input new-input))))

# Helper functions for colon mode — defined before colon-execute so they are in scope

(defn colon-set
  [args]
  (def parts (filter (fn [s] (not= s "")) (string/split " " args)))
  (each part parts
    (def parts (string/split "=" part))
    (if (> (length parts) 1)
      (option/set (first parts) (last parts))
      (option/set (first parts) ""))))

(defn colon-substitute
  [rest]
  (unless (> (length rest) 2) (print ":s usage: s/old/new/") (break))
  (def sep (string/slice rest 0 1))
  (def sub-parts (string/split sep (string/slice rest 1)))
  (unless (> (length sub-parts) 1) (print "Need at least /pattern/replacement/") (break))
  (def pattern (first sub-parts))
  (def replacement (get sub-parts 1))
  (def flags (if (> (length sub-parts) 2) (get sub-parts 2) ""))
  (def global? (string/find "g" flags))
  (def buf (buffer/current))
  (when buf
    (def text (buffer/slice buf 0 (buffer/len buf)))
    (def result (if global? (string/replace-all pattern replacement text)
                         (string/replace pattern replacement text)))
    (buffer/delete buf 0 (buffer/len buf))
    (buffer/insert buf 0 result)))



# Plugin hook — plugins register colon verbs here.
(def *colon-plugins* @{})

# Register a named colon-mode verb.
# (colon/define "verb" (fn [arg] ...))
(defn colon/define [verb handler]
  (put *colon-plugins* verb handler))

(defn colon-edit
  [path]
  (unless (> (length path) 0) (print ":e requires a path") (break))
  # If path is a directory, open it in the built-in dired browser
  (def is-dir (= (string/trim (editor/shell (string "test -d -- " path " 2>/dev/null && echo YES"))) "YES"))
  (when is-dir (editor/run-command "dired" path) (break))
  (def content (try (editor/fs-read path) ([err] (print err) (break))))
  (def name (last (string/split "/" path)))
  (def buf-key (buffer/create name content))
  (buffer/set-path buf-key path)
  (when-let [win (window/current)]
    (window/set-buffer (get win :id) buf-key)))

(defn colon-read
  [rest]
  (unless (> (length rest) 0) (print ":r requires a path") (break))
  (def content (editor/fs-read rest))
  (def buf (buffer/current))
  (when buf
    (def pos (buffer/cursor buf))
    (buffer/insert buf pos content)))

(defn colon-tabe
  [rest]
  (unless (> (length rest) 0) (print ":tabe requires a path") (break))
  (def content (try (editor/fs-read rest) ([err] (print err) (break))))
  (def name (last (string/split "/" rest)))
  (buffer/create name content))

# Dispatch a colon command string — e.g. "w", "set number", "q!"
(defn colon-execute
  [cmd]
  (def verb
    (let [space-pos (string/find " " cmd)
          slash-pos (string/find "/" cmd)
          end-pos   (min (or space-pos (length cmd)) (or slash-pos (length cmd)))]
      (string/slice cmd 0 end-pos)))
  (def rest (string/trim (string/slice cmd (length verb))))
  (case verb
    "w" (do
          (def buf (buffer/current))
          (unless buf (break))
          (if (> (length rest) 0)
            (buffer/set-path buf rest))
          (def write-path (buffer/path buf))
          (unless write-path (break))
          (editor/fs-write write-path (buffer/slice buf 0 (buffer/len buf)))
          (buffer/mark-saved buf))
    "write" (colon-execute (string "w " rest))
    "q" (editor/set-running false)
    "quit" (editor/set-running false)
    "q!" (editor/set-running false)
    "wq" (do (colon-execute (string "w " rest)) (editor/set-running false))
    "x" (do (colon-execute "w") (editor/set-running false))
    "set" (colon-set rest)
    "s" (colon-substitute rest)
    "!" (do
          # Route :! through the new async process/spawn API (Sprint 3).
          # Spawn the command, collect output lines into a *shell* buffer,
          # and show it when the process exits.
          (def proc-id (process/spawn rest))
          (def buf-key (buffer/find-or-create "*shell*"))
          (buffer/set-read-only buf-key false)
          # Replace previous content
          (def old-len (buffer/len buf-key))
          (when (> old-len 0) (buffer/delete buf-key 0 old-len))
          (buffer/insert buf-key 0 (string "$ " rest "\n"))
          (buffer/set-read-only buf-key true)
          # Subscribe to process-output for this process
          (event/once "process-output"
            (fn [data]
              (when (= (get data :id "0") (string proc-id))
                (buffer/set-read-only buf-key false)
                (buffer/insert buf-key (buffer/len buf-key) (get data :line ""))
                (buffer/insert buf-key (buffer/len buf-key) "\n")
                (buffer/set-read-only buf-key true))))
          # On exit, show the buffer
          (event/once "process-exit"
            (fn [data]
              (when (= (get data :id "0") (string proc-id))
                (buffer/set-read-only buf-key false)
                (buffer/insert buf-key (buffer/len buf-key)
                  (string "\n[Exit code: " (get data :exit-code "?") "]\n"))
                (buffer/set-read-only buf-key true)
                (when-let [win (window/current)]
                  (window/set-buffer win buf-key))))))
    "e" (colon-edit rest)
    "edit" (colon-edit rest)
    "r" (colon-read rest)
    "read" (colon-read rest)
    "make" (editor/run-command "make" rest)
    "tab" (colon-tabe rest)
    "tabedit" (colon-tabe rest)
    "tabn" (editor/run-command "tabnext")
    "tabnext" (editor/run-command "tabnext")
    "tabp" (editor/run-command "tabprev")
    "tabprev" (editor/run-command "tabprev")
    "find" (editor/run-command "find-files" (string/trim rest))
    "find-files" (editor/run-command "find-files" (string/trim rest))
    "cn" (editor/run-command "quickfix-next")
    "cnext" (editor/run-command "quickfix-next")
    "cp" (editor/run-command "quickfix-prev")
    "cprev" (editor/run-command "quickfix-prev")
    "cprevious" (editor/run-command "quickfix-prev")
    "terminal"      (editor/run-command "terminal-start" (string/trim rest))
    "term"          (editor/run-command "terminal-start" (string/trim rest))
    "dired"         (editor/run-command "dired" (string/trim rest))
    "Drename"       (editor/run-command "dired-rename"  (string/trim rest))
    "Dcopy"         (editor/run-command "dired-copy"    (string/trim rest))
    "Dmkdir"        (editor/run-command "dired-mkdir"   (string/trim rest))
    "vc"            (editor/run-command "vc")
    "VCdiff"        (editor/run-command "vc-diff"       (string/trim rest))
    "VClog"         (editor/run-command "vc-log")
    "VCcommit"      (editor/run-command "vc-commit"     (string/trim rest))
    "VCcommit-save" (editor/run-command "vc-commit-save")
    "VCpush"        (editor/run-command "vc-push")
    "VCblame"       (editor/run-command "vc-blame")
    # Check plugin-registered verbs before giving up
    (do
      (def handler (get *colon-plugins* verb))
      (if handler
        (handler rest)
        (print "Unknown command: " verb)))))

(command/redefine "command-execute"
  (fn [&]
    (def detail (editor/mode-detail))
    (def raw-input (get detail :input ""))
    (def input (string/trim raw-input))
    (set-vim-mode "normal" false)
    (if (> (length input) 0)
      (colon-execute input)
      nil)))
