# Lisp language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about lisp: highlighting, completion,
# major mode.
(plugin/define
  {:name "lisp"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["lisp"]
   :grammar-symbols {"lisp" "tree_sitter_commonlisp"}
   :capabilities [:editor-access]})
