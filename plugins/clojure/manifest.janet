# Clojure language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about clojure: highlighting, completion,
# major mode.
(plugin/define
  {:name "clojure"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["clojure"]
   :capabilities [:editor-access]})
