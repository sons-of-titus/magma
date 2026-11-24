# Ocaml language plugin — OCaml sources and interfaces.
(plugin/define
  {:name "ocaml"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["ocaml"]
   :extensions {"ocaml" ["ml" "mli"]}
   :lsp {"ocaml" {:single-file true}}
   :capabilities [:process-execution :editor-access]})
