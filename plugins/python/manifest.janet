# Python language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about python: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "python"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["python"]
   :capabilities [:process-execution :editor-access]})
