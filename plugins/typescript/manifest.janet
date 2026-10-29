# Typescript language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about typescript: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "typescript"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["typescript"]
   :capabilities [:process-execution :editor-access]})
