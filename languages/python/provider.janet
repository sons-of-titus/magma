# Python Language Provider
#
# Registers the Python language with the Semantic Engine using both an LSP
# provider (pylsp / pyright) and tree-sitter for offline symbol extraction.

(semantic/register-provider "python" "treesitter")

# Start pylsp when a Python project is opened.
(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "python")
              (os/stat (string root "/pyproject.toml"))
              (os/stat (string root "/setup.py"))
              (os/stat (string root "/requirements.txt")))
      (lsp/start "pylsp" root))))

(semantic/register-provider "python" "lsp")

(defn python/extensions [] @[".py" ".pyi" ".pyw"])

(defn python/debug-adapter [] "debugpy")

(defn python/format [text]
  (def result (process/run "black" @["-q" "-"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
