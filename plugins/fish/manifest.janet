# Fish language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about fish: highlighting, completion,
# major mode.
(plugin/define
  {:name "fish"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["fish"]
   :capabilities [:editor-access]})
