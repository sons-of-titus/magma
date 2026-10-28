#[macro_use]
mod helpers;
// Every builtins/*.janet file must at least parse.
//
// A malformed builtin fails silently at editor startup (the loader logs and
// moves on), and nothing in the behavior suites notices — so this gate runs
// the Janet parser over each file, including plugins/.

use scripting;

const PARSE_HELPER: &str = r#"
(defn parse-ok? [path]
  (def p (parser/new))
  (var ok true)
  (try
    (do
      (parser/consume p (slurp path))
      (parser/eof p))
    ([e] (set ok false) (put module/cache :parse-error e)))
  ok)
"#;

fn list_janet_files() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../builtins");
    let mut files = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("builtins directory exists") {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().map(|e| e == "janet").unwrap_or(false) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn every_builtin_file_parses() {
    janet_test!(ed, {
        let r = scripting::eval(PARSE_HELPER);
        assert_eq!(r, "ok", "parse helper itself must load");

        let mut failures = Vec::new();
        for path in list_janet_files() {
            let display = path.display().to_string();
            // eval() collapses every successful result to "ok" — eval_result
            // returns the value's description, which is what we need.
            let quoted = format!("(if (parse-ok? {display:?}) :yes :no)");
            match scripting::eval_result(&quoted) {
                Ok(v) if v == ":yes" => {}
                Ok(other) => {
                    let err = scripting::eval("(get module/cache :parse-error)");
                    failures.push(format!("{display}: {other} ({err})"));
                }
                Err(e) => failures.push(format!("{display}: eval error: {e}")),
            }
        }
        assert!(
            failures.is_empty(),
            "builtin files that fail to parse:\n{}",
            failures.join("\n")
        );
    });
}
