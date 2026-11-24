# Janet language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about janet: highlighting, completion,
# major mode.
(plugin/define
  {:name "janet"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["janet"]
   :extensions {"janet" ["janet" "jdn"]}
   :grammar-symbols {"janet" "tree_sitter_janet_simple"}
   :capabilities [:editor-access]})
