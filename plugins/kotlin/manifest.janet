# Kotlin language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about kotlin: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "kotlin"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["kotlin"]
   :capabilities [:process-execution :editor-access]})
