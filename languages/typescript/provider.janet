# TypeScript Language Provider

(semantic/register-provider "typescript" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "typescript")
              (os/stat (string root "/tsconfig.json")))
      (lsp/start "typescript-language-server" root))))

(semantic/register-provider "typescript" "lsp")

(defn typescript/extensions [] @[".ts" ".tsx"])

(defn typescript/debug-adapter [] "node")

(defn typescript/format [text]
  (def result (process/run "npx" @["prettier" "--stdin-filepath" "dummy.ts"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
