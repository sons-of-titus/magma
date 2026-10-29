# Haskell language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about haskell: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "haskell"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["haskell"]
   :capabilities [:process-execution :editor-access]})
