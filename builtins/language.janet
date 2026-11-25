# Language registry — Phase 12
#
# Tracks the languages plugins serve: name, asset paths, load state.
# Plugin manifests register their languages via plugin/define (see
# builtins/plugin_system.janet); this file holds the event handlers and
# colon verbs for inspecting the registry.
#
# Events emitted by the Rust layer:
#   language-registered  {:name}        — language/register succeeded
#   language-unregistered {:name}       — language/unregister removed a language
#
# Colon verbs:
#   :lang-list            — print all registered languages
#   :lang-info <name>     — show paths for a language

# ── Event handlers ────────────────────────────────────────────────────────────

(event/on "language-registered"
  (fn [data]
    (def name (get data :name "?"))
    (editor/log-message (string "[language] registered: " name))))

(event/on "language-unregistered"
  (fn [data]
    (def name (get data :name "?"))
    # Drop the registration's file-type entries and the mode itself —
    # plugin teardown emits this per language so its activation surface
    # disappears with it.
    (def mode (string name "-mode"))
    (eachp [ext m] *extension-mode-map*
      (when (= m mode) (put *extension-mode-map* ext nil)))
    (eachp [fname m] *filename-mode-map*
      (when (= m mode) (put *filename-mode-map* fname nil)))
    (put *major-modes* mode nil)
    (editor/log-message (string "[language] unregistered: " name))))

# ── Registration ─────────────────────────────────────────────────────────────
#
# language/define is the plugin-facing entry point: it registers the
# language with the Rust registry (loading and ABI-validating a plugin
# grammar when one is declared) and wires the user-facing behavior that
# stays in Janet — the extension→mode map, the tree-sitter highlight
# query, and the comment/indentation option defaults.

(defn- set-option-default
  "Set an option only when no user or config value exists."
  [key value]
  (when value
    (def current (option/get key))
    (when (or (nil? current) (= current ""))
      (option/set key value))))

(defn language/define
  "Register a language and wire its behavior.  Spec keys:
     :id         language name (required)
     :extensions file extensions mapped to `<id>-mode` (array)
     :filenames  exact file basenames mapped to `<id>-mode` (array) —
                 Makefile, Dockerfile, Jenkinsfile …
     :grammar    absolute path to a plugin grammar shared library
     :syntax     absolute path to the highlight query (conventionally
                 syntax.scm; drives tree-sitter highlighting for the mode)
     :config     absolute path to config.janet, evaluated at registration
                 so its option defaults exist before any mode activates
     :comments   {:line \"//\" :block [\"/*\" \"*/\"]}
     :indentation {:size 4 :style \"spaces\"|\"tabs\"}
     :plugin     owning plugin name (set by plugin/define)
   Extension conflicts never override: the first registration wins and the
   later one is reported."
  [spec]
  (def id (get spec :id))
  (unless id (error "language/define: :id required"))
  (def mode (string id "-mode"))
  (def result (language/register spec))
  # The config's option defaults must exist before the mode can ever
  # activate: registration (a manifest) always precedes mode definition
  # and activation, so a setup never reads a missing option.
  (when-let [cfg (get spec :config)]
    (try
      (editor/load-file cfg)
      ([err]
        (editor/warn (string "[language] bad config file " cfg ": " err)))))
  # Grammar/query failures are reported by the registration; the wiring
  # here applies either way.
  (each ext (get spec :extensions @[])
    (def existing (get *extension-mode-map* ext))
    (if (and existing (not (= existing mode)))
      (editor/warn
        (string "[language] extension ." ext " already maps to " existing
                "; refusing to override for " id))
      (put *extension-mode-map* ext mode)))
  (each fname (get spec :filenames @[])
    (def existing (get *filename-mode-map* fname))
    (if (and existing (not (= existing mode)))
      (editor/warn
        (string "[language] file " fname " already maps to " existing
                "; refusing to override for " id))
      (put *filename-mode-map* fname mode)))
  (when-let [syntax (get spec :syntax)]
    (ts/load-mode-query mode syntax))
  (when-let [folds (get spec :folds)]
    (ts/load-mode-fold-query mode folds))
  (when-let [comments (get spec :comments)]
    (set-option-default (string id "-comment-prefix") (get comments :line))
    (when-let [block (get comments :block)]
      (set-option-default (string id "-comment-open") (get block 0))
      (set-option-default (string id "-comment-close") (get block 1))))
  (when-let [indent (get spec :indentation)]
    (set-option-default (string id "-indent-width")
                        (if-let [size (get indent :size)] (string size)))
    (set-option-default (string id "-indent-style") (get indent :style)))
  result)

# ── Helpers ───────────────────────────────────────────────────────────────────

(defn- language-summary [info]
  (def name   (get info :name "?"))
  (def loaded (get info :loaded false))
  (def sp     (get info :syntax-path nil))
  (string name (if loaded " [loaded]" "") (if sp (string " " sp) "")))

# ── Colon verbs ───────────────────────────────────────────────────────────────

(defn colon-lang-list
  "List all registered languages with their load status."
  [_]
  (def langs (language/list))
  (if (= (length langs) 0)
    (print "No languages registered")
    (each l langs
      (print (language-summary l)))))

(defn colon-lang-info
  "Show paths for a registered language: :lang-info <name>"
  [arg]
  (def name (string/trim (string arg)))
  (if (= (length name) 0)
    (print "lang-info: provide a language name")
    (do
      (def langs (language/list))
      (def info (find (fn [l] (= (get l :name) name)) langs))
      (if (nil? info)
        (print "language not registered: " name)
        (do
          (print "name:          " (get info :name))
          (print "loaded:        " (get info :loaded))
          (print "syntax-path:   " (or (get info :syntax-path)   "(none)"))
          (print "config-path:   " (or (get info :config-path)   "(none)")))))))
