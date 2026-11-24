# Shell language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about sh: highlighting, completion,
# major mode.
(plugin/define
  {:name "sh"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["sh"]
   :extensions {"sh" ["sh" "bash" "zsh"]}
   :lsp {"sh" {:single-file true}}
   :grammar-symbols {"sh" "tree_sitter_bash"}
   :capabilities [:process-execution :editor-access]})
