# --- Plugin loader (require) ----------------------------------------------
#
# Plugins live in ~/.config/magma/plugins/<name>.janet.
# (require "name") evaluates the plugin file once and caches the result.

(def *loaded-modules* @{})

(defn require
  "Load a plugin by name from ~/.config/magma/plugins/<name>.janet.
   Each plugin is evaluated at most once per session."
  [name]
  (if (get *loaded-modules* name)
    (get *loaded-modules* name)
    (do
      (def home (or (os/getenv "HOME") ""))
      (def path (string home "/.config/magma/plugins/" name ".janet"))
      (def result
        (try
          (do (editor/load-file path) true)
          ([err]
            (print "require: could not load " name ": " err)
            false)))
      (put *loaded-modules* name result)
      result)))
