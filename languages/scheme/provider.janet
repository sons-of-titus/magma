(semantic/register-provider "scheme" "treesitter")

(defn scheme/extensions [] @[".scm"])

(defn scheme/debug-adapter [] nil)

(defn scheme/format [text] text)
