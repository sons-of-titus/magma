# C++ Language Provider

(semantic/register-provider "cpp" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "cpp")
              (os/stat (string root "/compile_commands.json")))
      (lsp/start "clangd" root))))

(semantic/register-provider "cpp" "lsp")

(defn cpp/extensions [] @[".cpp" ".cc" ".cxx" ".hpp"])

(defn cpp/debug-adapter [] "lldb")

(defn cpp/format [text]
  (def result (process/run "clang-format" @["-style=file"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
