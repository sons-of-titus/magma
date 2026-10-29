# Elixir language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about elixir: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "elixir"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["elixir"]
   :capabilities [:process-execution :editor-access]})
