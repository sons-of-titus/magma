# VCS diff gutter extension — Phase 7.
#
# Subscribes to `buffer-after-save` and runs `git diff --unified=0` to mark
# added/changed lines in the `:git-signs` gutter provider.

(def *vcs-pids* @{})   # buf-key -> active process id

(defn- vcs-parse-hunk-header [line]
  # Parse @@ -old +new,count @@ lines.
  # Returns [start-line count] for the new-file side, or nil on no match.
  (when (string/has-prefix? "@@" line)
    (when-let [plus-pos (string/find " +" line)]
      (let [rest (string/slice line (+ plus-pos 2))
            comma-pos (or (string/find "," rest) (string/find " " rest) (length rest))
            start-str (string/slice rest 0 comma-pos)
            count-str (if (string/find "," rest)
                        (string/slice rest (+ comma-pos 1) (or (string/find " " rest (+ comma-pos 1)) (length rest)))
                        "1")]
        (let [start (scan-number start-str)
              cnt   (scan-number count-str)]
          (when (and start cnt)
            [start cnt]))))))

(event/on "buffer-after-save"
  (fn [data]
    (let [buf  (get data :buffer-id)
          path (buffer/path buf)]
      (when (and path (not (string/has-prefix? "*" (buffer/name buf))))
        # Kill any existing VCS diff process for this buffer.
        (when-let [old-pid (get *vcs-pids* buf)]
          (process/kill old-pid))
        # Accumulate cells for this buffer.
        (def cells @[])
        (let [pid (process/spawn "git" ["diff" "--unified=0" path])]
          (put *vcs-pids* buf pid)
          (var current-start 0)
          (var current-count 0)
          (event/on "process-output"
            (fn [d]
              (when (= (get d :id) pid)
                (let [line (get d :line)]
                  (when-let [hunk (vcs-parse-hunk-header line)]
                    (set current-start (first hunk))
                    (set current-count (get hunk 1)))
                  (when (string/has-prefix? "+" line)
                    (array/push cells
                      {:line (- current-start 1) :text "▎" :face "gutter-vcs-changed"}))))))
          (event/once "process-exit"
            (fn [d]
              (when (= (get d :id) pid)
                (put *vcs-pids* buf nil)
                # Push accumulated cells to the :git-signs provider.
                (gutter/provider-update ":git-signs" buf cells)))))))))
