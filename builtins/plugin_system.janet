# Plugin System — discovery, manifests, dependency resolution.
#
# A plugin is a directory with a manifest.janet calling plugin/define and
# an entry script (conventionally main.janet); the spec keys are documented
# on plugin/define below.  Dependencies resolve depth-first before a plugin
# loads; unknown or cyclic dependencies are reported, never silently
# skipped.  Template/toolchain contributions are picked up here too — see
# builtins/plugin_templates.janet.

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
   language registry (wiring modes, queries, and option defaults — see
   language/define); a grammar the plugin ships at
   grammars/tree-sitter-<lang>.<platform-ext> is picked up
   conventionally; an optional :toolchain spec and every
   project-templates/<name>/ template register too.

   Spec keys beyond the loader's own:
     :description    one-line summary, shown on the Plugins page
     :author         author name or handle
     :homepage       project URL
     :tags           discovery keywords (array of strings)
     :extensions     {\"rust\" [\"rs\"]} — file extensions per language
     :filenames      {\"makefile\" [\"Makefile\"]} — exact basenames per
                     language, for extensionless build files
     :lsp            {\"rust\" {:markers [\"Cargo.toml\"]}} — language-server
                     launch policy per language.  :markers are project-root
                     file names (\"*.suffix\" globs allowed) that trigger a
                     server on project open or buffer attach; without them
                     the server starts per buffer (:single-file true is
                     implied).  :server-key names a shared process for
                     several languages (\"tsserver\" for js/ts, \"clangd\"
                     for c/cpp); :args are extra argv.
     :grammar-symbols {\"terraform\" \"tree_sitter_hcl\"} — export name
                     override for a grammar whose C entry point deviates
                     from the conventional tree_sitter_<lang>
     :toolchain      {\"id\" ...} — a toolchain/register spec table"
  [spec]
  (def root (dyn :magma/plugin-dir ""))
  (def name (get spec :name))
  (unless name (error "plugin/define: :name required"))
  (def entry (get spec :entry "main.janet"))
  (def ext-map (get spec :extensions @{}))
  (def filename-map (get spec :filenames @{}))
  (def lsp-map (get spec :lsp @{}))
  (def caps (get spec :capabilities @[]))
  (def symbol-map (get spec :grammar-symbols @{}))
  (def langs @[])
  (each lang (get spec :languages @[])
    (def syntax (when (file-exists? (string root "/syntax.scm"))
                  (string root "/syntax.scm")))
    (def folds (when (file-exists? (string root "/folds.scm"))
                 (string root "/folds.scm")))
    (def config (when (file-exists? (string root "/config.janet"))
                  (string root "/config.janet")))
    (def spec {:id lang
               :syntax syntax
               :folds folds
               :config config
               :extensions (get ext-map lang @[])
               :filenames (get filename-map lang @[])
               :plugin name})
    (def grammar (ts/grammar-lib-path root lang))
    # The spec literal is a struct; optional grammar keys merge into a
    # fresh table rather than mutating it.
    (def overrides @{})
    (when grammar (put overrides :grammar grammar))
    (when-let [sym (get symbol-map lang)]
      (put overrides :grammar-symbol sym))
    (language/define (merge spec overrides))
    # Spawning a language server is process execution: a manifest that
    # launches one without declaring the capability is a bug, not a
    # sandbox escape — fail loudly so the author fixes the manifest.
    (when-let [lsp (get lsp-map lang)]
      (unless (has-value? caps :process-execution)
        (error (string "plugin/define: " name " declares :lsp for " lang
                       " without the :process-execution capability")))
      (when (and (get lsp :install) (not (has-value? caps :network)))
        (error (string "plugin/define: " name " declares :lsp :install for "
                       lang " without the :network capability")))
      # The root rides along so a bundled :command resolves plugin-relative.
      (lsp/register-language-spec lang (merge lsp {:plugin-root root})))
    (array/push langs lang))
  (when-let [toolchain (get spec :toolchain)]
    (toolchain/register (merge toolchain {:plugin name})))
  # Multi-flavor languages (maven + gradle, make + cmake) declare a list.
  (each tc (get spec :toolchains @[])
    (toolchain/register (merge tc {:plugin name})))
  (plugin/register-templates root name)
  # Snippets are language-scoped in the registry, so plugin teardown
  # removes them with the language — no plugin attribution needed here.
  (def snippets (string root "/snippets.janet"))
  (when (file-exists? snippets)
    (try
      (editor/load-file snippets)
      ([err]
        (editor/warn (string "[plugin] bad snippets file " snippets ": " err)))))
  (plugin/register
    (merge spec
           {:root root
            :entry entry
            :languages langs
            :capabilities caps})))

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
  "Discover plugins from disk — nothing loads here.  Manifests evaluate
   (languages, launch policies, toolchains register) but entries stay
   unloaded until enabled: explicitly by a verb or the settings page, or
   on request when an opened project matches a plugin's language
   (plugin_install.janet offers the enable).  plugins-enabled=false
   disables discovery entirely; plugins-root overrides the user plugin
   directory (the fallback is ~/.config/magma/plugins); plugins-disabled
   names are never offered.  The bundled directory is <cwd>/plugins."
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
      (plugin/discover user-root))))

# ── Events ───────────────────────────────────────────────────────────────────

(event/on "plugin-registered"
  (fn [data]
    (editor/log-message
      (string "[plugin] registered: " (get data :name "?") " v" (get data :version "?")))))

(event/on "plugin-loaded"
  (fn [data]
    (editor/log-message (string "[plugin] loaded: " (get data :name "?")))))
