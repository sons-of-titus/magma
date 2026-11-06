# Makefile language plugin — make build definitions.
(plugin/define
  {:name "makefile"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["makefile"]
   :grammar-symbols {"makefile" "tree_sitter_make"}
   :capabilities [:editor-access]})
