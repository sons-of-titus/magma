# Plugin unload / reload verbs.
#
# plugin/unload (the C function) removes everything a plugin contributed:
# registry-level state (languages, templates, toolchains) and the
# script-level registrations its entry made — commands, event handlers,
# keymap bindings — which the contribution journal attributes at
# registration time.  What survives is state the plugin wrote elsewhere
# (buffers, options it set); unload is an unregistration, not a sandboxed
# suspension of arbitrary written state.

(defn colon-plugin-unload
  "Unload a plugin's registry-level and script-level contributions: :plugin-unload <name>"
  [arg]
  (def name (string/trim (string arg)))
  (if (= (length name) 0)
    (print "plugin-unload: provide a plugin name")
    (do
      (def result (plugin/unload name))
      (if (= result :ok)
        (print "plugin unloaded: " name)
        (print "plugin unload failed")))))

(defn colon-plugin-reload
  "Re-read a plugin's manifest and load it: :plugin-reload <name>"
  [arg]
  (def name (string/trim (string arg)))
  (def p (if (= (length name) 0) nil (plugin/status name)))
  (if (nil? p)
    (print "plugin not registered: " name)
    (do
      (plugin/unload name)
      # Re-evaluating the manifest re-registers languages, templates, and
      # toolchains before the entry script loads.  plugin/discover scans
      # for subdirectories, so it cannot rediscover the plugin itself.
      (def root (get p :root))
      (setdyn :magma/plugin-dir root)
      (try
        (editor/load-file (string root "/manifest.janet"))
        ([err] (editor/warn (string "[plugin] bad manifest " root ": " err))))
      (def result (plugin/enable name))
      (if (= result :ok)
        (print "plugin reloaded: " name)
        (print "plugin reload failed: " result)))))

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
      (when-let [d (get p :description)] (print "description:   " d))
      (when-let [a (get p :author)] (print "author:        " a))
      (when-let [h (get p :homepage)] (print "homepage:      " h))
      (print "root:          " (get p :root))
      (print "entry:         " (get p :entry))
      (print "deps:          " (string/join (get p :deps @[]) " "))
      (print "languages:     " (string/join (get p :languages @[]) " "))
      (print "tags:          " (string/join (get p :tags @[]) " "))
      (print "capabilities:  " (string/join (map string (get p :capabilities @[])) " "))
      (print "loaded:        " (get p :loaded)))))

(defn colon-plugin-install
  "Clone a plugin repository into a plugin root and enable it: :plugin-install <git-url> [root]"
  [arg]
  (def parts (string/split " " (string/trim (string arg))))
  (def source (string/trim (or (first parts) "")))
  (if (= (length source) 0)
    (print "plugin-install: provide a git URL or path")
    (do
      (def explicit-root (string/trim (or (get parts 1) "")))
      (def result (plugin/install source (if (> (length explicit-root) 0) explicit-root nil)))
      (if (= result :ok)
        nil
        (print "plugin-install failed: " result)))))
