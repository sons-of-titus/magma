# Dockerfile language plugin — container build definitions.
(plugin/define
  {:name "dockerfile"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["dockerfile"]
   :extensions {"dockerfile" ["dockerfile"]}
   :filenames {"dockerfile" ["Dockerfile" "dockerfile"]}
   :lsp {"dockerfile" {:single-file true}}
   :capabilities [:process-execution :editor-access]})
