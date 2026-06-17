(semantic/register-provider "zig" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "zig")
              (os/stat (string root "/build.zig")))
      (lsp/start "zls" root))))

(semantic/register-provider "zig" "lsp")

(defn zig/extensions [] @[".zig"])

(defn zig/debug-adapter [] nil)

(defn zig/format [text]
  (def result (process/run "zig" @["fmt" "--stdin"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
