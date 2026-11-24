# Nim language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about nim: highlighting, completion,
# major mode.
(plugin/define
  {:name "nim"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["nim"]
   :extensions {"nim" ["nim"]}
   :lsp {"nim" {:markers ["*.nimble" "nim.cfg"]}}
   :capabilities [:process-execution :editor-access]})
