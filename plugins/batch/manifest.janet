# Batch language plugin — Windows batch scripts.
(plugin/define
  {:name "batch"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["batch"]
   :extensions {"batch" ["bat" "cmd"]}
   :capabilities [:editor-access]})
