# Css language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about css: highlighting, completion,
# major mode.
(plugin/define
  {:name "css"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["css"]
   :capabilities [:editor-access]})
