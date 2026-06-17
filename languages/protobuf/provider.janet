(semantic/register-provider "protobuf" "treesitter")

(defn protobuf/extensions [] @[".proto"])

(defn protobuf/debug-adapter [] nil)

(defn protobuf/format [text] text)
