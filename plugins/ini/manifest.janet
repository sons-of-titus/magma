# Ini language plugin — INI-style configuration files.
(plugin/define
  {:name "ini"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["ini"]
   :extensions {"ini" ["ini" "cfg" "conf" "properties" "env"]}
   :capabilities [:editor-access]})
