(template/define
  {:id "rust.library"
   :name "Library Crate"
   :description "A Rust library crate with cargo build and test wired up."
   :category "library"
   :language "rust"
   :source "."
   :toolchain "rust.cargo"
   :variables {:project_name {:type "project-name"
                              :required true
                              :description "Crate name"}
               :edition {:type "choice"
                         :default "2021"
                         :choices ["2021" "2024"]
                         :description "Rust edition"}}})
