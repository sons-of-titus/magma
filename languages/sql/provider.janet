(semantic/register-provider "sql" "treesitter")

(semantic/register-provider "sql" "lsp")

(defn sql/extensions [] @[".sql"])

(defn sql/debug-adapter [] nil)

(defn sql/format [text] text)
