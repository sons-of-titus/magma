(semantic/register-provider "yaml" "treesitter")

(defn yaml/extensions [] @[".yaml" ".yml"])

(defn yaml/debug-adapter [] nil)

(defn yaml/format [text] text)
