# Yaml language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about yaml: highlighting, completion,
# major mode.
(plugin/define
  {:name "yaml"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["yaml"]
   :extensions {"yaml" ["yaml" "yml"]}
   :lsp {"yaml" {:single-file true}}
   :capabilities [:process-execution :editor-access]})
