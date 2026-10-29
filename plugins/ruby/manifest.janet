# Ruby language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about ruby: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "ruby"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["ruby"]
   :capabilities [:process-execution :editor-access]})
