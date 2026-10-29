# Sql language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about sql: highlighting, completion,
# major mode.
(plugin/define
  {:name "sql"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["sql"]
   :capabilities [:editor-access]})
