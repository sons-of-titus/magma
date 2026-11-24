# Plugin installation and per-project configuration.
#
# Loaded after plugin_system.janet: install clones a repository into a
# plugin root and enables what lands; a project's .magma/plugins.janet
# adjusts what is loaded while that project is open.  Both ride events
# and verbs the core registers.

(defn plugin/user-root
  "The user plugin directory: the plugins-root option when set, else
   ~/.config/magma/plugins.  Nil when no home directory is known."
  []
  (def configured (option/get "plugins-root"))
  (cond
    (and configured (> (length configured) 0)) configured
    (> (length (os/getenv "HOME" "")) 0)
    (string (os/getenv "HOME" "") "/.config/magma/plugins")
    nil))

(defn plugin/install
  "Clone a plugin repository into a plugin root (the user root unless
   given explicitly), discover it, and enable it.  `source` is anything
   `git clone` accepts; the plugin directory name comes from the URL.
   Returns :ok or a failure description."
  [source &opt root]
  (default root (plugin/user-root))
  (if (or (nil? root) (= (length root) 0))
    "no plugin root (set plugins-root or $HOME)"
    (let [norm (string/replace-all "\\" "/" source)
          stripped (string/replace ".git" "" norm)
          name (last (string/split "/" stripped))
          dest (string root "/" name)]
      (cond
        (or (nil? name) (= (length name) 0))
        "cannot derive a plugin name from the source"
        (dir-exists? dest)
        (string "already installed: " dest)
        (let [result (process/run "git" @["clone" "--depth" "1" source dest])]
          (if (= (get result :exit-code) 0)
            (let [found (plugin/discover root)]
              # The directory name is not the plugin name: enable whatever
              # the cloned manifest declared itself as.
              (def installed
                (find (fn [p] (= (get p :root) dest)) (plugin/list)))
              (if (and installed (= :ok (plugin/enable (get installed :name))))
                (do
                  (editor/log-message
                    (string "[plugin] installed " (get installed :name)
                            " (" found " plugins at " root ")"))
                  :ok)
                "cloned, but no manifest.janet with a valid plugin was found"))
            (let [err (or (get result :stderr "") (get result :stdout ""))]
              (string "git clone failed: " (string/slice err 0 (min 160 (length err)))))))))))

# ── Per-project configuration ────────────────────────────────────────────────
#
# <root>/.magma/plugins.janet holds a table:
#   {:enable  ["rust"]          load these in this project, even if the
#                               global plugins-disabled list keeps them off
#    :disable  ["graphql"]}     unload these while this project is open
#
# Unload is a real teardown now (commands, handlers, and bindings go with
# the plugin), so a project can shed plugins it does not want without a
# restart.  A bad config file reports and is skipped.

# The open project's root — the lazy-enable offer's re-fire aims here.
(var *active-root* "")

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (set *active-root* root)
    (def config (string root "/.magma/plugins.janet"))
    (when (file-exists? config)
      # The file is a Janet file whose last expression is the config table
      # (eval-string returns it across the evaluation boundary; load-file
      # discards values).
      (def spec
        (try
          (eval-string (slurp config))
          ([err]
            (editor/warn (string "[plugin] bad project config " config ": " err))
            nil)))
      (when (dictionary? spec)
        (when (get spec :enable)
          (each name (get spec :enable)
            (when (= :ok (plugin/enable name))
              (editor/log-message (string "[plugin] enabled by project: " name)))))
        (when (get spec :disable)
          (each name (get spec :disable)
            (when (plugin/status name)
              (plugin/unload name)
              (editor/log-message (string "[plugin] disabled by project: " name)))))))))

# ── Lazy enablement ──────────────────────────────────────────────────────────
# Bootstrap discovers plugins but does not load them.  When the detected
# project language has no loaded plugin yet, offer the discovered one
# that declares it — nothing loads until the user accepts.

(defn- plugin-offer-resolve [name accepted?]
  (when accepted?
    (when (= :ok (plugin/enable name))
      (editor/log-message (string "[plugin] enabled by offer: " name))
      # Re-light the project: detection and LSP attach for markers, and
      # open buffers refocus so their modes activate.
      (when (> (length *active-root*) 0)
        (event/emit "project-opened" {"root" *active-root*}))
      (each buf-id (buffer/list)
        (event/emit "buffer-focused" {"buffer-id" (string buf-id)})))))

(install/set-plugin-resolver plugin-offer-resolve)

(defn- supports? [record lang]
  (has-value? (get record :languages @[]) lang))

(event/on "tasks-detected"
  (fn [data]
    (def lang (get data :language ""))
    (when (> (length lang) 0)
      (unless (some |(and (get $ :loaded) (supports? $ lang)) (plugin/list))
        (when-let [candidate
                   (find |(and (not (get $ :loaded)) (supports? $ lang)) (plugin/list))]
          (def disabled (string/split "," (or (option/get "plugins-disabled") "")))
          (unless (has-value? disabled (get candidate :name))
            (install/offer-enable
              (get candidate :name) (get candidate :name)
              (string "Support for " lang " projects — highlighting, language server, build tasks."))))))))
