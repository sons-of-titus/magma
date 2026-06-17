# Go Language Provider

(semantic/register-provider "go" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "go")
              (os/stat (string root "/go.mod")))
      (lsp/start "gopls" root))))

(semantic/register-provider "go" "lsp")

(defn go/extensions [] @[".go"])

(defn go/debug-adapter [] "delve")

(defn go/format [text]
  (def result (process/run "gofmt" @[] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
