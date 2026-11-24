# Graphql language plugin — GraphQL schemas and operations.
(plugin/define
  {:name "graphql"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["graphql"]
   :extensions {"graphql" ["graphql" "gql"]}
   :lsp {"graphql" {:single-file true}}
   :capabilities [:process-execution :editor-access]})
