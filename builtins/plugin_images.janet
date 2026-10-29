# Plugin Images — plugin entries as precompiled, single-file artifacts.
#
# A plugin can ship its entry as a marshaled Janet image (main.jimage)
# instead of source.  plugin/build compiles the entry against the live
# environment and writes <stem>.jimage next to it; plugin/load sniffs the
# magic and routes to plugin/load-image.  manifest.janet stays source —
# it is needed before any code runs (discovery, deps, capabilities).
#
# Image file format:
#
#   MGIMG1:<janet-version>\n<marshaled bytes>
#
# Images pin the bytecode format to the building VM: the version header
# turns a VM upgrade into a clean "rebuild this plugin" error instead of
# a cryptic unmarshal panic.  An image is code with the same trust level
# as source — the capability system, not the header, is the gate.

(def- image-magic "MGIMG1")

(defn- env-lookup-tables
  "The marshal/unmarshal registry pair for the live environment: fwd maps
   symbol → value for unmarshal, rreg value → symbol for marshal.  C calls
   cannot marshal raw; through the registry they become symbolic references
   resolved live at load time.  Var bindings ride along as their ref arrays
   so an image reads and writes the live var, not a frozen copy."
  []
  (def fwd @{})
  (def rreg @{})
  (eachp [sym binding] (curenv)
    (cond
      (get binding :value)
      (do (put fwd sym (get binding :value))
          (put rreg (get binding :value) sym))
      (get binding :ref)
      (do (put fwd sym (get binding :ref))
          (put rreg (get binding :ref) sym))))
  [fwd rreg])

(defn- forward-table []
  (def fwd @{})
  (eachp [sym binding] (curenv)
    (cond
      (get binding :value) (put fwd sym (get binding :value))
      (get binding :ref)   (put fwd sym (get binding :ref))))
  fwd)

# ── Build ────────────────────────────────────────────────────────────────────

(defn plugin/build
  "Compile the plugin's entry source into <stem>.jimage next to it and
   return the image path.  The entry must be Janet source — an image
   cannot be recompiled.  Errors carry the plugin name."
  [name]
  (def status (plugin/status name))
  (unless status
    (errorf "plugin/build: plugin not registered: %s" name))
  (def root (get status :root ""))
  (def entry (get status :entry ""))
  (unless (string/has-suffix? ".janet" entry)
    (errorf "plugin/build: %s entry is %s, not Janet source — point :entry at the .janet file to rebuild" name entry))
  (def source
    (try (slurp (string root "/" entry))
      ([e] (errorf "plugin/build: %s: cannot read entry: %v" name e))))
  (def forms
    (try (parse-all source)
      ([e] (errorf "plugin/build: %s failed to parse: %v" name e))))
  (def [fwd rreg] (env-lookup-tables))
  (def f (compile ~(do ,;forms) (curenv)))
  (when (get f :error)
    (errorf "plugin/build: %s failed to compile: %v" name (get f :error)))
  (def image (string root "/" (string/slice entry 0 -6) "jimage"))
  (spit image (string image-magic ":" janet/version "\n" (marshal f rreg)))
  image)

(defn plugin/build-all
  "Build images for every registered plugin discovered under `root` (all
   registered plugins when root is nil) whose entry is still source.
   Returns an array of failure strings — empty means every plugin compiled."
  [&opt root]
  (def failures @[])
  (each p (plugin/list)
    (when (and (or (nil? root) (string/has-prefix? root (get p :root "")))
               (string/has-suffix? ".janet" (get p :entry "")))
      (try (plugin/build (get p :name))
        ([e] (array/push failures (string (get p :name) ": " e))))))
  failures)

# ── Load ─────────────────────────────────────────────────────────────────────

(defn plugin/load-image
  "Load a plugin image built by plugin/build: validate the header, resolve
   the bytecode's symbolic references against this VM, and run the entry.
   Called by plugin/load with the plugin's capability context already
   active, so handlers the image registers are gated exactly like source."
  [path]
  (def raw (slurp path))
  (unless (string/has-prefix? image-magic raw)
    (errorf "plugin/load-image: %s is not a plugin image (bad magic)" path))
  (def header-end (string/find "\n" raw))
  (unless header-end
    (errorf "plugin/load-image: %s has a truncated header" path))
  (def built-for (string/slice raw (inc (length image-magic)) header-end))
  (unless (= built-for janet/version)
    (errorf "plugin/load-image: %s was built for Janet %s, this VM is %s — rebuild the plugin with :plugin-build" path built-for janet/version))
  (def entry-fn (unmarshal (string/slice raw (inc header-end)) (forward-table)))
  (entry-fn))

# ── Colon verbs ──────────────────────────────────────────────────────────────

(defn colon-plugin-build
  "Compile a plugin's entry into a loadable image: :plugin-build <name>"
  [arg]
  (def name (string/trim (string arg)))
  (if (= (length name) 0)
    (print "plugin-build: provide a plugin name")
    (do
      (var image nil)
      (var failure nil)
      (try
        (set image (plugin/build name))
        ([err] (set failure (string err))))
      (if image
        (do
          (print "plugin image built: " image)
          (print "to ship it, set :entry in the manifest to this .jimage file"))
        (print "plugin build failed: " failure)))))

(defn colon-plugin-build-all
  "Build images for every plugin under a root: :plugin-build-all [root]"
  [arg]
  (def root (string/trim (string arg)))
  (def failures (plugin/build-all (if (> (length root) 0) root nil)))
  (if (= (length failures) 0)
    (print "all plugins compiled")
    (do
      (print "plugin builds failed:")
      (each f failures (print "  " f)))))
