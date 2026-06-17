# C Language Provider

(semantic/register-provider "c" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "c")
              (os/stat (string root "/compile_commands.json")))
      (lsp/start "clangd" root))))

(semantic/register-provider "c" "lsp")

(defn c/extensions [] @[".c" ".h"])

(defn c/debug-adapter [] "lldb")

(defn c/format [text]
  (def result (process/run "clang-format" @["-style=file"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
