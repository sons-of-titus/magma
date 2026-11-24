# Haskell language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about haskell: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "haskell"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["haskell"]
   :extensions {"haskell" ["hs" "lhs"]}
   :lsp {"haskell" {:markers ["*.cabal" "stack.yaml" "cabal.project" "hie.yaml"]}}
   :capabilities [:process-execution :editor-access]})
