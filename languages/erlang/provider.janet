(semantic/register-provider "erlang" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "erlang")
              (os/stat (string root "/rebar.config")))
      (lsp/start "erlang_ls" root))))

(semantic/register-provider "erlang" "lsp")

(defn erlang/extensions [] @[".erl"])

(defn erlang/debug-adapter [] nil)

(defn erlang/format [text] text)
