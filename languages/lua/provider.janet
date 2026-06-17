# Lua Language Provider

(semantic/register-provider "lua" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "lua")
              (os/stat (string root "/.luarc.json")))
      (lsp/start "lua-language-server" root))))

(semantic/register-provider "lua" "lsp")

(defn lua/extensions [] @[".lua"])

(defn lua/debug-adapter [] nil)

(defn lua/format [text] text)
