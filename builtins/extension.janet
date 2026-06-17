# Magma Extension Capability System — Phase 10
#
# Event handlers for extension lifecycle events and colon-mode verbs for
# inspecting registered extensions and their capabilities.

# --- Event handlers ----------------------------------------------------------

(event/on "extension-declared"
  (fn [data]
    (def name    (get data :name "?"))
    (def version (get data :version "?"))
    (print "Extension declared: " name " v" version)))

(event/on "extension-undeclared"
  (fn [data]
    (def name (get data :name "?"))
    (print "Extension context cleared: " name)))

# --- Helpers -----------------------------------------------------------------

(defn- extension-summary [ext]
  (def name  (get ext :name "?"))
  (def ver   (get ext :version "?"))
  (def caps  (get ext :capabilities @[]))
  (string name " v" ver " [" (string/join (map string caps) ", ") "]"))

# --- Colon verbs -------------------------------------------------------------

(defn colon-ext-list
  "List all registered extensions with their capabilities."
  [_]
  (def exts (extension/list))
  (if (= (length exts) 0)
    (print "No extensions registered")
    (each ext exts
      (print (extension-summary ext)))))

(put *colon-plugins* "ext-list" colon-ext-list)

(defn colon-ext-capabilities
  "Show capabilities for a named extension: :ext-capabilities <name>"
  [arg]
  (def name (string/trim (string arg)))
  (if (= (length name) 0)
    (print "ext-capabilities: provide an extension name")
    (do
      (def caps (extension/capabilities name))
      (if (nil? caps)
        (print "Extension not found: " name)
        (do
          (print "Capabilities for " name ":")
          (each cap caps
            (print "  " cap)))))))

(put *colon-plugins* "ext-capabilities" colon-ext-capabilities)
