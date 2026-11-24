# Latex language plugin — LaTeX documents.
(plugin/define
  {:name "latex"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["latex"]
   :extensions {"latex" ["tex" "sty" "cls"]}
   :lsp {"latex" {:single-file true}}
   :capabilities [:process-execution :editor-access]})
