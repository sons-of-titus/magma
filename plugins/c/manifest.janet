# C language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about C: highlighting, completion, LSP,
# major mode, formatting, and build run configurations.
(plugin/define
  {:name "c"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["c"]
   :capabilities [:process-execution :editor-access]})
