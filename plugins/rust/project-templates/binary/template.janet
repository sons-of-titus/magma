(template/define
  {:id "rust.binary"
   :name "Binary Application"
   :description "A Rust executable with cargo build, test, and run wired up."
   :category "application"
   :language "rust"
   :source "."
   :toolchain "rust.cargo"
   :variables {:project_name {:type "project-name"
                              :required true
                              :description "Crate and binary name"}
               :edition {:type "choice"
                         :default "2021"
                         :choices ["2021" "2024"]
                         :description "Rust edition"}}})
