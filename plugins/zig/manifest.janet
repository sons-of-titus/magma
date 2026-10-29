# Zig language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about zig: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "zig"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["zig"]
   :capabilities [:process-execution :editor-access]})
