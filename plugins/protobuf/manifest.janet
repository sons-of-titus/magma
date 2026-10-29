# Protobuf language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about protobuf: highlighting, completion,
# major mode.
(plugin/define
  {:name "protobuf"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["protobuf"]
   :capabilities [:editor-access]})
