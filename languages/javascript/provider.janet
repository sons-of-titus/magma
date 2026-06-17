# JavaScript Language Provider

(semantic/register-provider "javascript" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "javascript")
              (os/stat (string root "/package.json")))
      (lsp/start "typescript-language-server" root))))

(semantic/register-provider "javascript" "lsp")

(defn javascript/extensions [] @[".js" ".jsx"])

(defn javascript/debug-adapter [] "node")

(defn javascript/format [text]
  (def result (process/run "npx" @["prettier" "--stdin-filepath" "dummy.js"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
