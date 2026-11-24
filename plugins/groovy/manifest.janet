# Groovy language plugin — Groovy sources, Gradle builds, Jenkinsfiles.
(plugin/define
  {:name "groovy"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["groovy"]
   :extensions {"groovy" ["groovy" "gradle" "jenkinsfile"]}
   :filenames {"groovy" ["Jenkinsfile"]}
   :lsp {"groovy" {:single-file true}}
   :capabilities [:process-execution :editor-access]})
