# Java language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about java: highlighting, completion,
# major mode.
(plugin/define
  {:name "java"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["java"]
   :capabilities [:editor-access]})
