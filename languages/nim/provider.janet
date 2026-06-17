(semantic/register-provider "nim" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "nim")
              (os/stat (string root "/nimble")))
      (lsp/start "nimlangserver" root))))

(semantic/register-provider "nim" "lsp")

(defn nim/extensions [] @[".nim"])

(defn nim/debug-adapter [] nil)

(defn nim/format [text] text)
