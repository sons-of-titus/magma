# Php language plugin — PHP scripts and templates.
(plugin/define
  {:name "php"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["php"]
   :extensions {"php" ["php" "phtml"]}
   :lsp {"php" {:single-file true}}
   :capabilities [:process-execution :editor-access]})
