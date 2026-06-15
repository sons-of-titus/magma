# ── Command-mode completion ─────────────────────────────────────────────────
#
# Pressing tab in command mode cycles through all known colon verbs that match
# the current input as a prefix.  State (candidate list, current index) is kept
# in Janet vars and reset whenever the user types a character that no longer
# matches the last selected candidate.

(def *builtin-colon-verbs*
  ["w" "write" "q" "quit" "q!" "wq" "x"
   "set" "s" "e" "edit" "r" "read"
   "make" "cn" "cnext" "cp" "cprev" "cprevious"
   "tab" "tabedit" "tabn" "tabnext" "tabp" "tabprev"
   "find" "find-files"
   "terminal" "term"
   "dired" "Drename" "Dcopy" "Dmkdir"
   "vc" "VCdiff" "VClog" "VCcommit" "VCcommit-save" "VCpush" "VCblame"
   "!" "colorscheme" "scrollpolicy" ])

(defn- all-colon-verbs
  "Return a sorted array of every known colon verb."
  []
  (def out @[])
  (each v *builtin-colon-verbs* (array/push out v))
  (each k (keys *colon-plugins*) (array/push out k))
  (sort out))

(var *cc-list*  @[])   # current candidate list
(var *cc-idx*   -1)    # index of the candidate currently shown (-1 = none)

(command/define "command-complete"
  (fn [&]
    (def cur (or (editor/command-input) ""))
    # Only complete the verb (the part before the first space).
    (when (string/find " " cur) (break))
    # Detect whether we are still cycling (cur matches the last selection).
    (def in-cycle
      (and (> (length *cc-list*) 0)
           (>= *cc-idx* 0)
           (< *cc-idx* (length *cc-list*))
           (= cur (get *cc-list* *cc-idx*))))
    (unless in-cycle
      (set *cc-list*
        (if (< (length cur) 1)
          (all-colon-verbs)
          (filter |(string/has-prefix? cur $) (all-colon-verbs))))
      (set *cc-idx* -1))
    (when (> (length *cc-list*) 0)
      (set *cc-idx* (% (+ *cc-idx* 1) (length *cc-list*)))
      (editor/set-completions *cc-list* *cc-idx*)
      (editor/set-command-input (get *cc-list* *cc-idx*)))))

(command/define "command-complete-prev"
  (fn [&]
    (def cur (or (editor/command-input) ""))
    (when (string/find " " cur) (break))
    # Only cycle backward when already in a completion cycle.
    (when (< (length *cc-list*) 1) (break))
    (def in-cycle
      (and (>= *cc-idx* 0)
           (< *cc-idx* (length *cc-list*))
           (= cur (get *cc-list* *cc-idx*))))
    (unless in-cycle (break))
    (set *cc-idx*
      (if (= *cc-idx* 0)
        (- (length *cc-list*) 1)
        (- *cc-idx* 1)))
    (editor/set-completions *cc-list* *cc-idx*)
    (editor/set-command-input (get *cc-list* *cc-idx*))))
