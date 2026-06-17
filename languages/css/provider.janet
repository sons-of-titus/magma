(semantic/register-provider "css" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (= name "css")
      (lsp/start "vscode-css-languageserver" root))))

(semantic/register-provider "css" "lsp")

(defn css/extensions [] @[".css" ".scss"])

(defn css/debug-adapter [] nil)

(defn css/format [text] text)
