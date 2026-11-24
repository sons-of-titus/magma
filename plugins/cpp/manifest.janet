# Cpp language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about cpp: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "cpp"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["cpp"]
   :extensions {"cpp" ["cpp" "cc" "cxx" "hpp" "hh" "hxx"]}
   :lsp {"cpp" {:markers ["compile_commands.json" "compile_flags.txt"] :server-key "clangd"}}
   :capabilities [:process-execution :editor-access]})
