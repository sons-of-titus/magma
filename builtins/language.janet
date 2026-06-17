# Language Provider Ecosystem — Phase 12
#
# Manages the `languages/<lang>/` directory convention.  On startup,
# `language/discover` scans a root directory and registers each sub-directory
# as a language.  When a buffer is focused, the matching provider is
# auto-loaded on demand.
#
# Events emitted by the Rust layer:
#   language-registered  {:name}        — language/register succeeded
#   language-loaded      {:name}        — language/load succeeded
#   language-unregistered {:name}       — language/unregister removed a language
#
# Colon verbs:
#   :lang-list            — print all registered languages
#   :lang-load <name>     — load a language's provider.janet
#   :lang-info <name>     — show paths for a language

# ── Auto-discovery ────────────────────────────────────────────────────────────

(defn language/discover
  "Scan `root` for language directories and register each one found.

  Each sub-directory of `root` is treated as a language whose name is the
  directory name.  The function looks for `provider.janet`, `syntax.scm`,
  and `config.janet` inside each directory and registers whichever exist.

  Returns the number of languages discovered."
  [root]
  (var count 0)
  (each entry (try (os/dir root) ([_] @[]))
    (def lang-dir (string root "/" entry))
    (when (= :directory (get (os/stat lang-dir) :mode :file))
      (def provider (string lang-dir "/provider.janet"))
      (def syntax   (string lang-dir "/syntax.scm"))
      (def config   (string lang-dir "/config.janet"))
      (def pp (when (os/stat provider) provider))
      (def sp (when (os/stat syntax)   syntax))
      (def cp (when (os/stat config)   config))
      (language/register entry pp sp cp)
      (++ count)))
  count)

# ── Event handlers ────────────────────────────────────────────────────────────

(event/on "language-registered"
  (fn [data]
    (def name (get data "name" "?"))
    (editor/log-message (string "[language] registered: " name))))

(event/on "language-loaded"
  (fn [data]
    (def name (get data "name" "?"))
    (editor/log-message (string "[language] loaded: " name))))

(event/on "language-unregistered"
  (fn [data]
    (def name (get data "name" "?"))
    (editor/log-message (string "[language] unregistered: " name))))

# Auto-load a provider when a buffer is focused and a matching language is
# registered but not yet loaded.
(event/on "buffer-focused"
  (fn [data]
    (def buf-id (get data "buffer-id"))
    (when buf-id
      (def lang (language/for-buffer (scan-number (string buf-id))))
      (when (and lang (not (nil? lang)))
        (def langs (language/list))
        (def info (find (fn [l] (= (get l :name) lang)) langs))
        (when (and info (not (get info :loaded)))
          (language/load lang))))))

# ── Helpers ───────────────────────────────────────────────────────────────────

(defn- language-summary [info]
  (def name   (get info :name "?"))
  (def loaded (get info :loaded false))
  (def pp     (get info :provider-path nil))
  (string name (if loaded " [loaded]" "") (if pp (string " " pp) "")))

# ── Colon verbs ───────────────────────────────────────────────────────────────

(defn colon-lang-list
  "List all registered languages with their load status."
  [_]
  (def langs (language/list))
  (if (= (length langs) 0)
    (print "No languages registered")
    (each l langs
      (print (language-summary l)))))

(put *colon-plugins* "lang-list" colon-lang-list)

(defn colon-lang-load
  "Load a language's provider.janet: :lang-load <name>"
  [arg]
  (def name (string/trim (string arg)))
  (if (= (length name) 0)
    (print "lang-load: provide a language name")
    (do
      (def result (language/load name))
      (if (= result :ok)
        (print "language loaded: " name)
        (print "language load failed: " name)))))

(put *colon-plugins* "lang-load" colon-lang-load)

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
          (print "provider-path: " (or (get info :provider-path) "(none)"))
          (print "syntax-path:   " (or (get info :syntax-path)   "(none)"))
          (print "config-path:   " (or (get info :config-path)   "(none)")))))))

(put *colon-plugins* "lang-info" colon-lang-info)
