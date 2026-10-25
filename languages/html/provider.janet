(semantic/register-provider "html" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (= name "html")
      (lsp/start "html" "vscode-html-languageserver"))))

(semantic/register-provider "html" "lsp")

(defn html/extensions [] @[".html" ".htm"])

(defn html/debug-adapter [] nil)

(defn html/format [text] text)
