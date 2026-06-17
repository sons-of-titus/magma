(semantic/register-provider "xml" "treesitter")

(defn xml/extensions [] @[".xml"])

(defn xml/debug-adapter [] nil)

(defn xml/format [text] text)
