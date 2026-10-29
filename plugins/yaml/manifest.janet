# Yaml language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about yaml: highlighting, completion,
# major mode.
(plugin/define
  {:name "yaml"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["yaml"]
   :capabilities [:editor-access]})
