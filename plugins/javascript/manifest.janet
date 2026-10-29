# Javascript language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about javascript: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "javascript"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["javascript"]
   :capabilities [:process-execution :editor-access]})
