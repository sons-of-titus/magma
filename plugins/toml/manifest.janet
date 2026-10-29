# Toml language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about toml: highlighting, completion,
# major mode.
(plugin/define
  {:name "toml"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["toml"]
   :capabilities [:editor-access]})
