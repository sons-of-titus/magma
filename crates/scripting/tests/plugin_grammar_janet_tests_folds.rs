#[macro_use]
mod helpers;
use scripting;

// Every shipped folds.scm must compile as a query against its grammar —
// one wrong node name would kill the whole file's folds at runtime (the
// recompute clears the ranges), silently, which is exactly the kind of
// failure a compile check exists to catch.
#[test]
fn every_fold_query_compiles_against_its_grammar() {
    janet_test!(_ed, {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let plugins = repo.join("plugins");
        scripting::eval(&format!(
            r#"(plugin/discover "{}")"#,
            plugins.to_str().unwrap().replace('\\', "/")
        ));

        let mut checked = 0;
        for entry in std::fs::read_dir(&plugins).unwrap() {
            let dir = entry.unwrap().path();
            let folds = dir.join("folds.scm");
            if !folds.exists() {
                continue;
            }
            let lang = dir.file_name().unwrap().to_str().unwrap().to_string();
            let query = std::fs::read_to_string(&folds).unwrap();
            let escaped = query.replace('\\', "\\\\").replace('"', "\\\"");
            let ok = scripting::eval_result(&format!(
                r#"(string (ts/validate-query "{lang}" "{escaped}"))"#
            ))
            .unwrap_or_default();
            assert_eq!(
                ok,
                r#""true""#,
                "{lang}: folds.scm does not compile against its grammar"
            );
            checked += 1;
        }
        assert!(
            checked >= 19,
            "the fold-query packs should all be checked, got {checked}"
        );
    });
}
