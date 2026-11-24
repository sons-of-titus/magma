# Scala language plugin — Scala sources and build files.
(plugin/define
  {:name "scala"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["scala"]
   :extensions {"scala" ["scala" "sc"]}
   :lsp {"scala" {:single-file true}}
   :capabilities [:process-execution :editor-access]})
