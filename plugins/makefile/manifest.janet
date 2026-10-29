# Makefile language plugin — make build definitions.
(plugin/define
  {:name "makefile"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["makefile"]
   :capabilities [:editor-access]})
