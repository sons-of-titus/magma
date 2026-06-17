(semantic/register-provider "json" "treesitter")

(defn json/extensions [] @[".json"])

(defn json/debug-adapter [] nil)

(defn json/format [text] text)
