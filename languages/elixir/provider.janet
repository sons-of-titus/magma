(semantic/register-provider "elixir" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "elixir")
              (os/stat (string root "/mix.exs")))
      (lsp/start "elixir-ls" root))))

(semantic/register-provider "elixir" "lsp")

(defn elixir/extensions [] @[".ex" ".exs"])

(defn elixir/debug-adapter [] nil)

(defn elixir/format [text]
  (def result (process/run "mix" @["format" "-"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
