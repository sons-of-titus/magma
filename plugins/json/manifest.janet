# Json language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about json: highlighting, completion,
# major mode.
(plugin/define
  {:name "json"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["json"]
   :capabilities [:editor-access]})
