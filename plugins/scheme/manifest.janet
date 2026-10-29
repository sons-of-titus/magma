# Scheme language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about scheme: highlighting, completion,
# major mode.
(plugin/define
  {:name "scheme"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["scheme"]
   :capabilities [:editor-access]})
