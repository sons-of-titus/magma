# Html language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about html: highlighting, completion,
# major mode.
(plugin/define
  {:name "html"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["html"]
   :capabilities [:editor-access]})
