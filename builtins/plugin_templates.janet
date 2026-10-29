# Plugin template contributions.
#
# plugin/define calls plugin/register-templates to pick up every
# project-templates/<name>/template.janet under the plugin's directory.
# Bad template manifests warn and are skipped — one broken template must
# not take the plugin or its siblings down.  The :plugin-unload /
# :plugin-reload verbs live in builtins/plugin_verbs.janet.

(defn- stat-mode
  "The :mode of a filesystem entry, :missing if it does not exist."
  [path]
  (if-let [st (try (os/stat path) ([_] nil))]
    (get st :mode :missing)
    :missing))

(defn- dir-exists? [path] (= :directory (stat-mode path)))

(defn- file-exists? [path] (let [m (stat-mode path)] (or (= :file m) (= :link m))))

(defn plugin/register-templates
  "Evaluate every template manifest under <root>/project-templates/."
  [root name]
  (def templates-dir (string root "/project-templates"))
  (each entry (try (os/dir templates-dir) ([_] @[]))
    (def dir (string templates-dir "/" entry))
    (when (dir-exists? dir)
      (def manifest (string dir "/template.janet"))
      (when (file-exists? manifest)
        (setdyn :magma/template-dir dir)
        (setdyn :magma/plugin-name name)
        (try
          (editor/load-file manifest)
          ([err]
           (editor/warn
             (string "[plugin] bad template manifest " manifest ": " err)))))))
  nil)
