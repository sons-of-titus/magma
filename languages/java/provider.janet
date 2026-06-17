(semantic/register-provider "java" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "java")
              (os/stat (string root "/pom.xml"))
              (os/stat (string root "/build.gradle")))
      (lsp/start "jdtls" root))))

(semantic/register-provider "java" "lsp")

(defn java/extensions [] @[".java"])

(defn java/debug-adapter [] "java-debug")

(defn java/format [text] text)
