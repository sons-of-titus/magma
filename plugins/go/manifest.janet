# Go language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about go: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "go"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["go"]
   :capabilities [:process-execution :editor-access]})
