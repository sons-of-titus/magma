# Org language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about org: highlighting, completion,
# major mode.
(plugin/define
  {:name "org"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["org"]
   :extensions {"org" ["org"]}
   :capabilities [:editor-access]})
