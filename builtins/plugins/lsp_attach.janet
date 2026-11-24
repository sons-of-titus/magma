# LSP attach — launch policies declared by plugins.
#
# Marker and path helpers live in lsp_markers.janet.  A plugin's manifest
# declares how its language server starts:
#   :lsp {"rust" {:markers ["Cargo.toml"]}}
# Languages with :markers start when the marker is found — at the open
# project's root, or walking up from an opened file's directory.  Languages
# without markers are single-file: the server starts with the first buffer.
# A :server-key groups languages onto one process (tsserver for js/ts,
# clangd for c/cpp); the group shares the first server that becomes ready.

(var *lang-lsp-specs* @{})

(var *missing-reported* @{})

(defn lsp/register-language-spec
  "Record `lang`'s launch policy.  Spec keys: :markers (project-root file
   names, \"*.suffix\" and \"prefix.*\" globs allowed), :single-file (start
   with the first buffer even when markers are set), :server-key (share one
   process across languages), :args (extra argv)."
  [lang spec]
  (put *lang-lsp-specs* lang spec))

(defn- spec-for [lang] (get *lang-lsp-specs* lang))

(defn- server-key [lang] (get (spec-for lang) :server-key nil))

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

(defn- attach-buffer
  "Track the buffer's language and start its server when the buffer's
   location calls for one."
  [buf-id]
  (def path (or (buffer/path buf-id) ""))
  (def lang (language-for-path path))
  (when lang
    (put *buffer-languages* buf-id lang)
    (def spec (spec-for lang))
    (when spec
      (def markers (get spec :markers @[]))
      (cond
        (or (empty? markers) (get spec :single-file false)) (ensure-server lang)
        (marker-upstream? path markers) (ensure-server lang))))
  (send-did-open buf-id))

(event/on "buffer-created" (fn [ev] (when-let [id (scan-number (get ev :buffer-id))] (attach-buffer id))))
(event/on "buffer-focused" (fn [ev] (when-let [id (scan-number (get ev :buffer-id))] (attach-buffer id))))

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (when (> (length root) 0)
      (each lang (keys *lang-lsp-specs*)
        (def markers (get (spec-for lang) :markers @[]))
        (when (and (not (empty? markers)) (marker-in-root? root markers))
          (ensure-server lang))))))

# One process, several languages: when a server-key group's first server
# becomes ready, alias it under the sibling language ids so their buffers
# attach instead of spawning a second process.
(event/on "lsp-initialized"
  (fn [ev]
    (def lang (get ev :language))
    (def key (server-key lang))
    (when key
      (each other (keys *lang-lsp-specs*)
        (when (and (not (= other lang)) (= key (server-key other)))
          (when (lsp/share-server lang other)
            (put *lsp-servers* other :ready)
            (each buf-id (keys *buffer-languages*)
              (when (= other (get *buffer-languages* buf-id))
                (put *attached-buffers* buf-id nil)
                (send-did-open buf-id)))))))))

# A shared server crash clears the whole group's state, not just the
# language the event names.  Restart stays passive — the kernel owns the
# restart budget, and the next buffer attach or project-open retries.
(event/on "lsp-server-exited"
  (fn [ev]
    (def exited (get ev :language))
    # No live server of any language left: the pill goes.
    (when (empty? *lsp-servers*)
      (status/set-segment "lsp" ""))
    (def key (server-key exited))
    (each buf-id (keys *buffer-languages*)
      (def lang (get *buffer-languages* buf-id))
      (when (and lang
                 (or (= lang exited) (and key (= key (server-key lang)))))
        (put *lsp-servers* lang nil)
        (put *attached-buffers* buf-id nil)))))

# Plugin teardown unregisters the language; its launch policy goes with it.
(event/on "language-unregistered"
  (fn [data]
    (put *lang-lsp-specs* (get data :name "?") nil)))
