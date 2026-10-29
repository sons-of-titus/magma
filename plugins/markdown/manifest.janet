# Markdown language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about markdown: highlighting, completion,
# major mode.
(plugin/define
  {:name "markdown"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["markdown"]
   :capabilities [:editor-access]})
