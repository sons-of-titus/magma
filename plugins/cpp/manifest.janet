# Cpp language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about cpp: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "cpp"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["cpp"]
   :capabilities [:process-execution :editor-access]})
