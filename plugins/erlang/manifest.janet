# Erlang language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about erlang: highlighting, completion,
# major mode.
(plugin/define
  {:name "erlang"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["erlang"]
   :capabilities [:editor-access]})
