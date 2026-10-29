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
    (def name (get data "name" "?"))
    (editor/log-message (string "[language] registered: " name))))

(event/on "language-unregistered"
  (fn [data]
    (def name (get data "name" "?"))
    (editor/log-message (string "[language] unregistered: " name))))

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
