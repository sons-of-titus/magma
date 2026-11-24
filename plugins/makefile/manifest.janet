# Makefile language plugin — make build definitions.
(plugin/define
  {:name "makefile"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["makefile"]
   :extensions {"makefile" ["mk" "mak"]}
   :filenames {"makefile" ["Makefile" "makefile" "GNUmakefile" "BSDmakefile"]}
   :grammar-symbols {"makefile" "tree_sitter_make"}
   :capabilities [:editor-access]})
