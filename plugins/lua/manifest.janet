# Lua language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about lua: highlighting, completion,
# major mode.
(plugin/define
  {:name "lua"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["lua"]
   :extensions {"lua" ["lua"]}
   :lsp {"lua" {:markers [".luarc.json" ".luarc.jsonc"]}}
   :capabilities [:process-execution :editor-access]})
