(semantic/register-provider "nix" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "nix")
              (os/stat (string root "/flake.nix")))
      (lsp/start "nil" root))))

(semantic/register-provider "nix" "lsp")

(defn nix/extensions [] @[".nix"])

(defn nix/debug-adapter [] nil)

(defn nix/format [text]
  (def result (process/run "nixpkgs-fmt" @[] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
