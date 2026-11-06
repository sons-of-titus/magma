# Shell language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about sh: highlighting, completion,
# major mode.
(plugin/define
  {:name "sh"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["sh"]
   :grammar-symbols {"sh" "tree_sitter_bash"}
   :capabilities [:editor-access]})
