(semantic/register-provider "markdown" "treesitter")

(defn markdown/extensions [] @[".md" ".markdown"])

(defn markdown/debug-adapter [] nil)

(defn markdown/format [text] text)
