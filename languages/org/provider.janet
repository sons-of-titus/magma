(semantic/register-provider "org" "treesitter")

(defn org/extensions [] @[".org"])

(defn org/debug-adapter [] nil)

(defn org/format [text] text)
