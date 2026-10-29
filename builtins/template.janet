# Project templates — plugin-facing registration.
#
# A template is a directory of files copied into every generated project;
# the template's own template.janet declares it:
#
#   (template/define
#     {:id "rust.binary" :name "Binary Application"
#      :description "Create a Rust executable project."
#      :category "application" :language "rust" :source "."
#      :variables {:project_name {:type "project-name" :required true}
#                  :edition {:type "choice" :default "2024"
#                            :choices ["2021" "2024"]}}
#      :toolchain "rust.cargo"
#      :debug {:adapter "codelldb"}})
#
# :source resolves against the template's directory; {{variable}}
# placeholders in file contents and path segments substitute at generation
# time.  `project_name` is always provided — declaring it is only needed to
# control requiredness in the wizard, never for substitution.

(defn template/define
  "Declare a project template.  Called from a template.janet inside the
   plugin's project-templates/ directory; the loader binds
   :magma/template-dir to the template directory while it evaluates."
  [spec]
  (def plugin-dir (dyn :magma/plugin-dir ""))
  (def template-dir (dyn :magma/template-dir plugin-dir))
  (def source (get spec :source "."))
  (def abs-source
    (if (or (string/has-prefix? "/" source) (string/has-prefix? ":" source))
      source
      (string template-dir "/" source)))
  (template/register
    (merge spec
           {:source abs-source
            :plugin (get spec :plugin (dyn :magma/plugin-name ""))})))
