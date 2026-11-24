# Xml language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about xml: highlighting, completion,
# major mode.
(plugin/define
  {:name "xml"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["xml"]
   :extensions {"xml" ["xml"]}
   :capabilities [:editor-access]})
