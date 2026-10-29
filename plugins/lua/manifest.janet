# Lua language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about lua: highlighting, completion,
# major mode.
(plugin/define
  {:name "lua"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["lua"]
   :capabilities [:editor-access]})
