# Marker and path helpers for the LSP attach layer.
#
# Pure functions: buffer-path → language, directory walks, and glob
# markers ("*.nimble", "Dockerfile.*") matching.

(defn lsp-attach/marker-matches?
  "Does a directory entry match a marker?  Exact names match exactly;
   a leading * matches by suffix (*.nimble), a trailing * by prefix
   (Dockerfile.*)."
  [marker name]
  (cond
    (string/has-suffix? "*" marker)
    (string/has-prefix? (string/slice marker 0 -2) name)
    (string/has-prefix? "*" marker)
    (string/has-suffix? (string/slice marker 1) name)
    (= marker name)))

(defn- path-extension [path]
  (when-let [i (last (string/find-all "." path))]
    (string/slice path (+ i 1))))

(defn- file-basename [path]
  (def norm (string/replace-all "\\" "/" path))
  (if-let [i (last (string/find-all "/" norm))]
    (string/slice norm (+ i 1))
    norm))

(defn- parent-dir [path]
  (def norm (string/replace-all "\\" "/" path))
  (when-let [i (last (string/find-all "/" norm))]
    (if (zero? i) "/" (string/slice norm 0 i))))

(defn- language-for-path
  "The language id of `path` via the mode registry (rust-mode → rust)."
  [path]
  (def mode (or (get *extension-mode-map* (path-extension path))
                (get *filename-mode-map* (file-basename path))))
  (when (and mode (string/has-suffix? "-mode" mode))
    (string/slice mode 0 (- (length mode) 5))))

(defn- resolve-command
  "Command override for a language: the `<lang>-lsp-command` editor option
  wins (the per-language config packs set these).  Resolution goes through
  `toolchain/find-extended` and returns the resolved *path*, so a server
  installed outside `$PATH` (keg-only Homebrew, Xcode) is both found and
  spawnable.  A command that is missing everywhere reports once and returns
  nil — a raw spawn failure is the wrong way for a user to learn a server
  is missing."
  [lang]
  (def cmd (option/get (string lang "-lsp-command")))
  (if (and cmd (> (length cmd) 0))
    (if-let [path (toolchain/find-extended cmd)]
      path
      (do
        (unless (get *missing-reported* cmd)
          (put *missing-reported* cmd true)
          (notify/post "error"
                       (string "Language server not found: " cmd)
                       (string "LSP features disabled for " lang
                               " — install it or set " lang "-lsp-command")))
        nil))
    nil))

(defn- resolve-args
  "Extra argv: the `<lang>-lsp-args` option wins over the manifest spec."
  [lang]
  (def opt (option/get (string lang "-lsp-args")))
  (if (and opt (> (length (string/trim opt)) 0))
    (filter |(> (length $) 0) (string/split " " (string/trim opt)))
    (get (spec-for lang) :args @[])))

(defn- group-started?
  "True when a sibling language of this :server-key group already has a
   live or starting server — the group shares it once ready."
  [lang]
  (def key (server-key lang))
  (when key
    (some (fn [l] (and (not (= l lang))
                       (= key (server-key l))
                       (get *lsp-servers* l)))
          (keys *lang-lsp-specs*))))

(defn- ensure-server
  "Start `lang`'s server unless it or its server-key group is live."
  [lang]
  (when (and (spec-for lang)
             (not (get *lsp-servers* lang))
             (not (group-started? lang)))
    (def cmd (resolve-command lang))
    (when cmd
      (put *lsp-servers* lang :starting)
      (lsp/start lang cmd ;(resolve-args lang)))))

(defn- marker-present?
  "Does `root` contain a file matching `marker`?  Glob markers scan the
   directory; exact names are a single existence check."
  [root marker]
  (if (or (string/has-prefix? "*" marker) (string/has-suffix? "*" marker))
    (let [entries (try (os/dir root) ([_] @[]))]
      (some |(lsp-attach/marker-matches? marker $) entries))
    (let [st (try (os/stat (string root "/" marker)) ([_] nil))]
      (and st (not (= :missing (get st :mode)))))))

(defn- marker-in-root?
  "Does the project root contain any of the spec's markers?"
  [root markers]
  (some |(marker-present? root $) markers))

(defn- marker-upstream?
  "Does any directory from the file's parent up to the filesystem root
   contain a marker?  Bounded so an unexpected path cannot spin."
  [path markers]
  (var dir (parent-dir path))
  (var found false)
  (var depth 0)
  (while (and dir (not found) (< depth 15))
    (when (marker-in-root? dir markers) (set found true))
    (set dir (parent-dir dir))
    (++ depth))
  found)
