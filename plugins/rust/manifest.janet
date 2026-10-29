# Rust language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about Rust: highlighting, completion, LSP,
# major mode, formatting, debugging, and run configurations.
(plugin/define
  {:name "rust"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["rust"]
   :capabilities [:process-execution :editor-access]})
