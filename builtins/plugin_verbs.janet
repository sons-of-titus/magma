# Plugin unload / reload verbs.
#
# plugin/unload (the C function) removes a plugin's registry-level
# contributions — languages, project templates, toolchains — and marks it
# not loaded.  Scripts the plugin already registered (commands, event
# handlers, keymaps) keep running until the session ends: the scripting
# runtime has no per-plugin teardown, so unload is an unregistration, not
# a sandboxed suspension.

(defn colon-plugin-unload
  "Unload a plugin's registry-level contributions: :plugin-unload <name>"
  [arg]
  (def name (string/trim (string arg)))
  (if (= (length name) 0)
    (print "plugin-unload: provide a plugin name")
    (do
      (def result (plugin/unload name))
      (if (= result :ok)
        (print "plugin unloaded: " name " (scripts it registered stay active until restart)")
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
      (print "root:          " (get p :root))
      (print "entry:         " (get p :entry))
      (print "deps:          " (string/join (get p :deps @[]) " "))
      (print "languages:     " (string/join (get p :languages @[]) " "))
      (print "capabilities:  " (string/join (map string (get p :capabilities @[])) " "))
      (print "loaded:        " (get p :loaded)))))
