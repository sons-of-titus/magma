#[macro_use]
mod helpers;
use scripting;

fn root_str(path: &std::path::Path) -> String {
    path.to_str().unwrap().replace('\\', "/")
}

// ── marker matching ──────────────────────────────────────────────────────────

#[test]
fn marker_matching_handles_exact_glob_prefix_and_suffix() {
    janet_test!(_ed, {
        let cases = [
            ("Cargo.toml", "Cargo.toml", "true"),
            ("Cargo.toml", "other.txt", "false"),
            ("*.nimble", "magma.nimble", "true"),
            ("*.nimble", "nimble", "false"),
            ("Dockerfile.*", "Dockerfile.dev", "true"),
            ("Dockerfile.*", "Dockerfile", "false"),
        ];
        for (marker, name, want) in cases {
            let got = scripting::eval_result(&format!(
                r#"(string (lsp-attach/marker-matches? "{marker}" "{name}"))"#
            ))
            .unwrap_or_default();
            assert_eq!(got, format!(r#""{want}""#), "{marker} vs {name}");
        }
    });
}

// ── filename → mode activation ───────────────────────────────────────────────

#[test]
fn declared_filenames_activate_their_mode_on_buffer_created() {
    janet_test!(ed, {
        assert_eq!(
            scripting::eval(r#"(major-mode/define "zmake-mode" {:parent "prog"})"#),
            "ok"
        );
        assert_eq!(
            scripting::eval(
                r#"(language/define {:id "zmake" :extensions ["zmk"] :filenames ["Zmakefile" "BUILD.zmake"]})"#
            ),
            "ok"
        );
        // Registry entries the activation and attach layers both read.
        assert_eq!(
            scripting::eval_result(r#"(string (get *filename-mode-map* "Zmakefile"))"#)
                .unwrap_or_default(),
            r#""zmake-mode""#
        );

        let buf = ed.create_buffer_from_str("Zmakefile", "all:\n\techo hi\n");
        let ok = scripting::eval_result(&format!(
            r#"(do (event/emit "buffer-created" {{"buffer-id" "{buf}" "path" "/proj/Zmakefile"}}) true)"#
        ))
        .unwrap();
        assert_eq!(ok, "true");
        ed.dispatch_events_scoped();
        let mode = ed
            .buffers
            .get(buf)
            .unwrap()
            .lock()
            .unwrap()
            .major_mode
            .name()
            .to_string();
        assert_eq!(mode, "zmake-mode", "basename match activates the mode");
    });
}

// ── launch-policy registration ───────────────────────────────────────────────

#[test]
fn manifest_lsp_declares_launch_policy() {
    janet_test!(_ed, {
        assert_eq!(
            scripting::eval(
                r#"(plugin/define {:name "zz-lsp-pol" :version "0.1.0" :languages ["zzpol"]
                     :extensions {"zzpol" ["zpl"]}
                     :lsp {"zzpol" {:markers ["pol.toml"]}}
                     :capabilities [:process-execution :editor-access]})"#
            ),
            "ok"
        );
        let markers = scripting::eval_result(
            r#"(string/join (get (get *lang-lsp-specs* "zzpol") :markers) ",")"#,
        )
        .unwrap_or_default();
        assert_eq!(markers, r#""pol.toml""#);
    });
}

#[test]
fn manifest_lsp_without_process_execution_capability_is_a_define_error() {
    janet_test!(_ed, {
        let result = scripting::eval_result(
            r#"(try (plugin/define {:name "zz-lsp-bad" :version "0.1.0" :languages ["zzbad"]
                     :lsp {"zzbad" {:markers ["x.txt"]}}
                     :capabilities [:editor-access]})
               ([e] e))"#,
        )
        .unwrap_or_default();
        assert!(
            result.contains("process-execution"),
            "must name the missing capability: {result}"
        );
    });
}

// ── project-opened attach ────────────────────────────────────────────────────

#[test]
fn project_opened_with_unresolvable_server_starts_nothing() {
    janet_test!(_ed, {
        scripting::eval(
            r#"(option/set "zzpol-lsp-command" "definitely-not-a-real-server-binary")"#,
        );
        scripting::eval(
            r#"(lsp/register-language-spec "zzpol2" {:markers ["pol.toml"]})"#,
        );
        let root = std::env::temp_dir().join(format!("magma_lsp_root_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("pol.toml"), "").unwrap();
        let root_s = root.to_str().unwrap().replace('\\', "/");
        let ok = scripting::eval_result(&format!(
            r#"(do (event/emit "project-opened" {{"root" "{root_s}"}}) true)"#
        ))
        .unwrap();
        assert_eq!(ok, "true");
        // The command resolves nowhere, so no spawn was attempted and no
        // server state was recorded — the missing-server report is the
        // only observable.
        let state = scripting::eval_result(r#"(describe (get *lsp-servers* "zzpol2"))"#)
            .unwrap_or_default();
        assert_eq!(state, r#""nil""#);
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn project_opened_without_markers_starts_nothing() {
    janet_test!(_ed, {
        scripting::eval(r#"(lsp/register-language-spec "zzpol3" {:markers ["pol.toml"]})"#);
        let root = std::env::temp_dir().join(format!("magma_lsp_empty_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let root_s = root.to_str().unwrap().replace('\\', "/");
        scripting::eval_result(&format!(
            r#"(do (event/emit "project-opened" {{"root" "{root_s}"}}) true)"#
        ))
        .unwrap();
        let state = scripting::eval_result(r#"(describe (get *lsp-servers* "zzpol3"))"#)
            .unwrap_or_default();
        assert_eq!(state, r#""nil""#);
        std::fs::remove_dir_all(&root).ok();
    });
}

// ── server-key groups ────────────────────────────────────────────────────────

#[test]
fn share_server_nil_without_a_live_server() {
    janet_test!(_ed, {
        let result = scripting::eval_result(
            r#"(describe (lsp/share-server "zz-never-started-a" "zz-never-started-b"))"#,
        )
        .unwrap_or_default();
        assert_eq!(result, r#""nil""#);
    });
}

// ── bundled plugins declare the new surface ──────────────────────────────────

#[test]
fn bundled_plugins_declare_launch_policies_and_filenames() {
    janet_test!(ed, {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let root = repo.join("plugins");
        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));
        scripting::eval(r#"(plugin/enable-many @["rust" "kotlin" "makefile" "dockerfile" "typescript" "javascript"])"#);

        let check = |expr: &str, want: &str, what: &str| {
            let got = scripting::eval_result(expr).unwrap_or_default();
            assert_eq!(got, format!(r#""{want}""#), "{what}");
        };

        // Kotlin projects are detected through either gradle DSL or maven.
        check(
            r#"(string/join (get (get *lang-lsp-specs* "kotlin") :markers) ",")"#,
            "pom.xml,build.gradle,build.gradle.kts,settings.gradle,settings.gradle.kts",
            "kotlin markers cover both build systems",
        );
        // Makefile is reached by basename, not extension.
        check(
            r#"(get *filename-mode-map* "Makefile")"#,
            "makefile-mode",
            "Makefile basename maps to makefile-mode",
        );
        // Dockerfile ships both spellings.
        check(
            r#"(get *filename-mode-map* "Dockerfile")"#,
            "dockerfile-mode",
            "Dockerfile basename maps to dockerfile-mode",
        );
        // js/ts share one tsserver process.
        check(
            r#"(get (get *lang-lsp-specs* "typescript") :server-key)"#,
            "tsserver",
            "typescript declares the shared server key",
        );
        check(
            r#"(get (get *lang-lsp-specs* "javascript") :server-key)"#,
            "tsserver",
            "javascript declares the shared server key",
        );
        let _ = ed;
    });
}
