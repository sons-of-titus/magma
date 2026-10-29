# Plugin System — discovery, manifests, dependency resolution.
#
# A plugin is a directory with a manifest and an entry script:
#
#   plugins/<name>/
#     manifest.janet   (plugin/define {:name .. :version .. :deps [..]
#                                      :languages [..] :capabilities [..]
#                                      :entry "main.janet"})
#     main.janet       entry — wires major modes, hooks, commands
#     syntax.scm       tree-sitter highlight query for the plugin's major
#                      modes (registered automatically, see plugin/define)
#     config.janet     default options (optional)
#
# Dependencies are resolved depth-first before a plugin loads; unknown or
# cyclic dependencies are reported, never silently skipped.

# ── Helpers ──────────────────────────────────────────────────────────────────

(defn- stat-mode
  "The :mode of a filesystem entry, :missing if it does not exist.
   os/stat returns nil for absent paths (no error), so the default of
   the get must be :missing — never a plausible mode."
  [path]
  (if-let [st (try (os/stat path) ([_] nil))]
    (get st :mode :missing)
    :missing))

(defn- dir-exists? [path] (= :directory (stat-mode path)))

(defn- file-exists? [path] (let [m (stat-mode path)] (or (= :file m) (= :link m))))

# ── Manifests ────────────────────────────────────────────────────────────────

(defn plugin/define
  "Declare a plugin.  Called from a plugin's manifest.janet; the loader
   binds the plugin's directory as :magma/plugin-dir while the manifest
   evaluates.  Languages the plugin serves are registered with the
   language registry using the conventional sibling file names."
  [spec]
  (def root (dyn :magma/plugin-dir ""))
  (def name (get spec :name))
  (unless name (error "plugin/define: :name required"))
  (def entry (get spec :entry "main.janet"))
  (def langs @[])
  (each lang (get spec :languages @[])
    (def syntax (when (file-exists? (string root "/syntax.scm"))
                  (string root "/syntax.scm")))
    (def config (when (file-exists? (string root "/config.janet"))
                  (string root "/config.janet")))
    (language/register lang syntax config)
    # The plugin's syntax.scm drives tree-sitter highlighting for its
    # conventional `<lang>-mode` major mode.
    (when syntax
      (ts/load-mode-query (string lang "-mode") syntax))
    (array/push langs lang))
  (plugin/register
    name
    (get spec :version "0.0.0")
    root
    entry
    (get spec :deps @[])
    langs
    (get spec :capabilities @[])))

# ── Discovery ────────────────────────────────────────────────────────────────

(defn plugin/discover
  "Register every plugin found under `root`.  A sub-directory is a plugin
   when it holds a manifest.janet; directories without one are not plugins.
   Returns the number of plugins found."
  [root]
  (var count 0)
  (each entry (try (os/dir root) ([_] @[]))
    (def dir (string root "/" entry))
    (when (dir-exists? dir)
      (def manifest (string dir "/manifest.janet"))
      (when (file-exists? manifest)
        (setdyn :magma/plugin-dir dir)
        (try
          (editor/load-file manifest)
          ([err] (editor/warn (string "[plugin] bad manifest " manifest ": " err))))
        (++ count))))
  count)

# ── Dependency resolution ────────────────────────────────────────────────────

(defn plugin/enable-many
  "Load each plugin named in `names` together with its dependencies,
   depth-first.  Returns :ok, or a string describing the first unknown
   plugin, cycle, or load failure."
  [names]
  (def visiting @{})
  (def done @{})
  (var result :ok)
  (defn visit [n]
    (unless (get done n)
      (when (get visiting n)
        (error (string "dependency cycle at plugin " n)))
      (def status (plugin/status n))
      (unless status
        (error (string "plugin not found: " n)))
      (put visiting n true)
      (each dep (get status :deps @[])
        (visit dep))
      (put visiting n nil)
      # The entry script resolves sibling files against its own directory.
      (setdyn :magma/plugin-dir (get status :root ""))
      (when (and (= (get status :loaded) false) (= (plugin/load n) :error))
        (error (string "plugin failed to load: " n)))
      (put done n true)))
  (try
    (each n names (visit n))
    ([err] (set result err)))
  result)

(defn plugin/enable
  "Load a plugin and its dependencies.  See plugin/enable-many."
  [name]
  (plugin/enable-many [name]))

# ── Startup ──────────────────────────────────────────────────────────────────

(defn plugin/bootstrap
  "Discover plugins from disk, then enable the bundled language plugins.
   Honors the user settings: plugins-enabled=false disables loading
   entirely; plugins-root overrides the user plugin directory (the
   fallback is ~/.config/magma/plugins); plugins-disabled names
   (comma-separated) stay off until the next startup.  The bundled
   directory is <cwd>/plugins."
  [cwd home]
  (unless (= (option/get "plugins-enabled") "false")
    (plugin/discover (string cwd "/plugins"))
    (def configured (option/get "plugins-root"))
    (def user-root
      (cond
        (and configured (> (length configured) 0)) configured
        (> (length home) 0) (string home "/.config/magma/plugins")
        nil))
    (when (and user-root (> (length user-root) 0))
      (plugin/discover user-root))
    # Installed means enabled: every manifest plugin found is loaded with
    # its dependencies, in dependency order — except plugins-disabled
    # names (comma-separated), which stay off until the next startup.
    (def disabled (string/split "," (or (option/get "plugins-disabled") "")))
    (plugin/enable-many
      (filter (fn [n] (not (has-value? disabled n)))
              (map (fn [p] (get p :name)) (plugin/list))))))

# ── Events ───────────────────────────────────────────────────────────────────

(event/on "plugin-registered"
  (fn [data]
    (editor/log-message
      (string "[plugin] registered: " (get data "name" "?") " v" (get data "version" "?")))))

(event/on "plugin-loaded"
  (fn [data]
    (editor/log-message (string "[plugin] loaded: " (get data "name" "?")))))

# ── Colon verbs ──────────────────────────────────────────────────────────────

(defn colon-plugin-list
  "List all registered plugins with load status: :plugin-list"
  [_]
  (def plugins (plugin/list))
  (if (= (length plugins) 0)
    (print "No plugins registered")
    (each p plugins
      (print (get p :name "?") " v" (get p :version "?")
             (if (get p :loaded) " [loaded]" "")
             " [" (string/join (map string (get p :capabilities @[])) ", ") "]"))))

(defn colon-plugin-enable
  "Enable a plugin and its dependencies: :plugin-enable <name>"
  [arg]
  (def name (string/trim (string arg)))
  (if (= (length name) 0)
    (print "plugin-enable: provide a plugin name")
    (do
      (def result (plugin/enable name))
      (if (= result :ok)
        (print "plugin enabled: " name)
        (print "plugin enable failed: " result)))))

(defn colon-plugin-info
  "Show one plugin's manifest details: :plugin-info <name>"
  [arg]
  (def name (string/trim (string arg)))
  (def p (plugin/status name))
  (if (nil? p)
    (print "plugin not registered: " name)
    (do
      (print "name:          " (get p :name))
      (print "version:       " (get p :version))
      (print "root:          " (get p :root))
      (print "entry:         " (get p :entry))
      (print "deps:          " (string/join (get p :deps @[]) " "))
      (print "languages:     " (string/join (get p :languages @[]) " "))
      (print "capabilities:  " (string/join (map string (get p :capabilities @[])) " "))
      (print "loaded:        " (get p :loaded)))))
