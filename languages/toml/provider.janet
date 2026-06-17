(semantic/register-provider "toml" "treesitter")

(defn toml/extensions [] @[".toml"])

(defn toml/debug-adapter [] nil)

(defn toml/format [text] text)
