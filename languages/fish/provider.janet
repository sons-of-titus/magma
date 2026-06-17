(semantic/register-provider "fish" "treesitter")

(defn fish/extensions [] @[".fish"])

(defn fish/debug-adapter [] nil)

(defn fish/format [text] text)
