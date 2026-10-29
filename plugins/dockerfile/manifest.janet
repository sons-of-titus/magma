# Dockerfile language plugin — container build definitions.
(plugin/define
  {:name "dockerfile"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["dockerfile"]
   :capabilities [:editor-access]})
