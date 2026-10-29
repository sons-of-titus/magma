#[macro_use]
mod helpers;
use scripting;

use std::io::Write as _;

// Unique per-test plugin roots so tests never collide with each other or
// with the user's ~/.config/magma/plugins.
fn plugin_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "magma_plugin_{}_{}_{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(path: &std::path::Path, content: &str) {
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
}

fn root_str(path: &std::path::Path) -> String {
    path.to_str().unwrap().replace('\\', "/")
}

// ── plugin/register ───────────────────────────────────────────────────────────

#[test]
fn plugin_register_returns_ok() {
    janet_test!(ed, {
        let result = scripting::eval_result(
            r#"(plugin/register "zz-test-a" "1.0" "/tmp/a" "main.janet" @[] @[] @[])"#,
        )
        .unwrap_or_default();
        assert_eq!(result, ":ok");
    });
}

#[test]
fn plugin_register_is_listed_with_fields() {
    janet_test!(ed, {
        scripting::eval(
            r#"(plugin/register "zz-test-listed" "2.1" "/tmp/l" "entry.janet" @["dep"] @["rust"] @[:editor-access])"#,
        );
        let result = scripting::eval_result(
            r#"(let [p (plugin/status "zz-test-listed")]
                 (string (get p :version) "|"
                         (get p :entry) "|"
                         (length (get p :deps)) "|"
                         (length (get p :languages)) "|"
                         (length (get p :capabilities))))"#,
        )
        .unwrap_or_default();
        assert_eq!(result, r#""2.1|entry.janet|1|1|1""#);
    });
}

#[test]
fn plugin_status_nil_for_unknown() {
    janet_test!(ed, {
        let result =
            scripting::eval_result(r#"(type (plugin/status "zz-test-never-registered"))"#)
                .unwrap_or_default();
        assert_eq!(result, ":nil");
    });
}

// ── plugin/load ───────────────────────────────────────────────────────────────

#[test]
fn plugin_load_evaluates_entry_and_marks_loaded() {
    janet_test!(ed, {
        let root = plugin_root("load");
        write_file(&root.join("main.janet"), "(plugin-state/set \"zz-load-hit\" \"yes\")");
        scripting::eval(&format!(
            r#"(plugin/register "zz-test-load" "1.0" "{}" "main.janet" @[] @[] @[])"#,
            root_str(&root)
        ));
        let result =
            scripting::eval_result(r#"(plugin/load "zz-test-load")"#).unwrap_or_default();
        assert_eq!(result, ":ok");
        let status = scripting::eval_result(
            r#"(string (get (plugin/status "zz-test-load") :loaded))"#,
        )
        .unwrap_or_default();
        assert_eq!(status, r#""true""#);
        let side_effect = scripting::eval_result(
            r#"(plugin-state/get "zz-load-hit")"#,
        )
        .unwrap_or_default();
        assert!(side_effect.contains("yes"), "entry did not run: {side_effect}");
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn plugin_load_error_for_missing_entry() {
    janet_test!(ed, {
        scripting::eval(
            r#"(plugin/register "zz-test-missing" "1.0" "/tmp/definitely-not-here" "main.janet" @[] @[] @[])"#,
        );
        let result =
            scripting::eval_result(r#"(plugin/load "zz-test-missing")"#).unwrap_or_default();
        assert_eq!(result, ":error");
    });
}

// ── dependency resolution ─────────────────────────────────────────────────────

#[test]
fn enable_loads_dependencies_first() {
    janet_test!(ed, {
        let root = plugin_root("deps");
        let dep = root.join("dep-plugin");
        let main = root.join("main-plugin");
        std::fs::create_dir_all(&dep).unwrap();
        std::fs::create_dir_all(&main).unwrap();
        write_file(&dep.join("main.janet"), "(plugin-state/set \"zz-dep-loaded\" \"1\")");
        write_file(&main.join("main.janet"), "(plugin-state/set \"zz-main-loaded\" \"1\")");
        scripting::eval(&format!(
            r#"(do (plugin/register "zz-test-dep" "1.0" "{}" "main.janet" @[] @[] @[])
                  (plugin/register "zz-test-main" "1.0" "{}" "main.janet" @["zz-test-dep"] @[] @[]))"#,
            root_str(&dep),
            root_str(&main)
        ));
        let result =
            scripting::eval_result(r#"(plugin/enable "zz-test-main")"#).unwrap_or_default();
        assert_eq!(result, ":ok");
        let both = scripting::eval_result(
            r#"(string (get (plugin/status "zz-test-dep") :loaded) "|"
                    (get (plugin/status "zz-test-main") :loaded))"#,
        )
        .unwrap_or_default();
        assert_eq!(both, r#""true|true""#);
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn enable_reports_unknown_dependency() {
    janet_test!(ed, {
        scripting::eval(
            r#"(plugin/register "zz-test-orphan" "1.0" "/tmp/o" "main.janet" @["zz-test-ghost"] @[] @[])"#,
        );
        let result =
            scripting::eval_result(r#"(plugin/enable "zz-test-orphan")"#).unwrap_or_default();
        assert!(
            result.contains("not found"),
            "expected not-found error, got: {result}"
        );
    });
}

#[test]
fn enable_detects_dependency_cycles() {
    janet_test!(ed, {
        scripting::eval(
            r#"(do (plugin/register "zz-test-cyc-a" "1.0" "/tmp/ca" "main.janet" @["zz-test-cyc-b"] @[] @[])
                  (plugin/register "zz-test-cyc-b" "1.0" "/tmp/cb" "main.janet" @["zz-test-cyc-a"] @[] @[]))"#,
        );
        let result =
            scripting::eval_result(r#"(plugin/enable "zz-test-cyc-a")"#).unwrap_or_default();
        assert!(
            result.contains("cycle"),
            "expected cycle error, got: {result}"
        );
    });
}

// ── capability enforcement ────────────────────────────────────────────────────

#[test]
fn undeclared_capability_denied_during_load() {
    janet_test!(ed, {
        let root = plugin_root("caps");
        write_file(&root.join("main.janet"), "(fs/cwd)");
        scripting::eval(&format!(
            r#"(plugin/register "zz-test-nocap" "1.0" "{}" "main.janet" @[] @[] @[])"#,
            root_str(&root)
        ));
        let result =
            scripting::eval_result(r#"(plugin/load "zz-test-nocap")"#).unwrap_or_default();
        assert_eq!(result, ":error", "fs/cwd should fail without :filesystem");
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn declared_capability_allowed_during_load() {
    janet_test!(ed, {
        let root = plugin_root("caps-ok");
        write_file(&root.join("main.janet"), "(fs/cwd)");
        scripting::eval(&format!(
            r#"(plugin/register "zz-test-cap" "1.0" "{}" "main.janet" @[] @[] @[:filesystem])"#,
            root_str(&root)
        ));
        let result =
            scripting::eval_result(r#"(plugin/load "zz-test-cap")"#).unwrap_or_default();
        assert_eq!(result, ":ok", "fs/cwd should pass with :filesystem declared");
        std::fs::remove_dir_all(&root).ok();
    });
}

// ── discovery ─────────────────────────────────────────────────────────────────

#[test]
fn discover_registers_manifest_plugins_only() {
    janet_test!(ed, {
        let root = plugin_root("disc");
        let full = root.join("full-plugin");
        let stray = root.join("stray-dir");
        std::fs::create_dir_all(&full).unwrap();
        std::fs::create_dir_all(&stray).unwrap();
        write_file(
            &full.join("manifest.janet"),
            r#"(plugin/define {:name "zz-test-full" :version "0.2" :entry "main.janet"
                               :languages ["zz-full-lang"] :capabilities [:editor-access]})"#,
        );
        write_file(&full.join("main.janet"), "# no-op entry");
        write_file(&full.join("syntax.scm"), "; query");
        write_file(&stray.join("provider.janet"), "# not a plugin");

        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));

        let plugin_ok = scripting::eval_result(
            r#"(string (if (plugin/status "zz-test-full") "yes" "no"))"#,
        )
        .unwrap_or_default();
        assert_eq!(plugin_ok, r#""yes""#, "manifest plugin should be registered");

        let stray_ok = scripting::eval_result(
            r#"(let [l (find (fn [i] (= (get i :name) "stray-dir")) (language/list))]
                 (if l "registered" "missing"))"#,
        )
        .unwrap_or_default();
        assert_eq!(
            stray_ok, r#""missing""#,
            "a directory without a manifest is not a plugin"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn discover_registers_plugin_languages_for_buffer_mapping() {
    janet_test!(ed, {
        let root = plugin_root("langreg");
        let full = root.join("rusty");
        std::fs::create_dir_all(&full).unwrap();
        write_file(
            &full.join("manifest.janet"),
            r#"(plugin/define {:name "zz-test-langreg" :languages ["zz-lang"]})"#,
        );
        write_file(&full.join("main.janet"), "# no-op entry");

        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));

        let registered = scripting::eval_result(
            r#"(let [l (find (fn [i] (= (get i :name) "zz-lang")) (language/list))]
                 (if l "registered" "missing"))"#,
        )
        .unwrap_or_default();
        assert_eq!(registered, r#""registered""#);
        std::fs::remove_dir_all(&root).ok();
    });
}

// ── run-config contribution ───────────────────────────────────────────────────

#[test]
fn run_config_register_upserts_into_project_manager() {
    janet_test!(ed, {
        let result = scripting::eval_result(
            r#"(run-config/register "zz-test-build" "cargo" @["build"] nil false)"#,
        )
        .unwrap_or_default();
        assert_eq!(result, ":ok");
        let cfg = ed.project_manager.run_configs.named("zz-test-build").cloned();
        let cfg = cfg.expect("run config should be registered");
        assert_eq!(cfg.command, "cargo");
        assert_eq!(cfg.args, vec!["build".to_string()]);
        assert!(!cfg.debuggable);
    });
}

// ── bundled rust plugin (end to end) ──────────────────────────────────────────

#[test]
fn bundled_rust_plugin_discovers_enables_and_contributes() {
    janet_test!(ed, {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let root = repo.join("plugins");
        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));
        let result =
            scripting::eval_result(r#"(plugin/enable "rust")"#).unwrap_or_default();
        let detail = ed
            .status_message
            .as_ref()
            .map(|m| m.text.clone())
            .unwrap_or_default();
        assert_eq!(result, ":ok", "bundled rust plugin must enable cleanly: {detail}");

        let status = scripting::eval_result(
            r#"(string (get (plugin/status "rust") :loaded))"#,
        )
        .unwrap_or_default();
        assert_eq!(status, r#""true""#);

        // The plugin serves the rust language through its own entry script.
        let lang = scripting::eval_result(
            r#"(let [l (find (fn [i] (= (get i :name) "rust")) (language/list))]
                 (if l (string (get l :loaded)) "missing"))"#,
        )
        .unwrap_or_default();
        assert_eq!(lang, r#""true""#, "rust language registered and marked loaded");

        // Mode command, formatter command, and the toolchain declaring
        // build/test/run all contributed.
        assert!(
            ed.commands.exists("rust-mode"),
            "rust-mode command should be registered"
        );
        assert!(
            ed.commands.exists("rust-format-buffer"),
            "formatter command should be registered"
        );
        let cargo = ed
            .toolchains
            .get("rust.cargo")
            .expect("cargo toolchain contributed");
        assert_eq!(cargo.detect_markers, vec!["Cargo.toml".to_string()]);
        assert!(cargo.actions.contains_key("build"), "build action declared");
        assert!(
            cargo.actions.get("run").is_some_and(|a| a.debuggable),
            "run action must be debuggable"
        );
        let major_modes = scripting::eval_result(
            r#"(string (if (get *major-modes* "rust-mode") "yes" "no"))"#,
        )
        .unwrap_or_default();
        assert_eq!(major_modes, r#""yes""#, "rust-mode should be a major mode");

        // Build→reload cycle: compile the entry to an image (from a temp
        // copy — images are build artifacts, the repo never holds one),
        // flip :entry, and load again.  The image must contribute the same
        // surface as the source entry.
        let copy = plugin_root("rust-image");
        copy_dir(&repo.join("plugins/rust"), &copy.join("rust"));
        let copied = root_str(&copy.join("rust"));
        scripting::eval(&format!(
            r#"(plugin/register "rust" "0.1.0" "{copied}" "main.janet" @[] @[] @[:process-execution :editor-access])"#,
        ));
        let build = scripting::eval_result(
            r#"(try (plugin/build "rust") ([e] (string "build-error: " e)))"#,
        )
        .unwrap_or_default();
        assert!(
            build.contains("main.jimage"),
            "bundled rust plugin should build: {build}"
        );
        let image_len = std::fs::metadata(copy.join("rust/main.jimage"))
            .expect("image should exist")
            .len();
        assert!(
            image_len < 1024 * 1024,
            "image bloat — env embedding grew: {image_len} bytes"
        );
        scripting::eval(&format!(
            r#"(plugin/register "rust" "0.1.0" "{copied}" "main.jimage" @[] @[] @[:process-execution :editor-access])"#,
        ));
        let reload = scripting::eval_result(r#"(plugin/enable "rust")"#).unwrap_or_default();
        assert_eq!(reload, ":ok", "image-loaded rust plugin must enable cleanly");
        let reloaded_status = scripting::eval_result(
            r#"(string (get (plugin/status "rust") :entry))"#,
        )
        .unwrap_or_default();
        assert_eq!(reloaded_status, r#""main.jimage""#);
        assert!(
            ed.commands.exists("rust-mode"),
            "image-loaded entry should register rust-mode"
        );
        std::fs::remove_dir_all(&copy).ok();
    });
}

// ── bundled c plugin (end to end) ─────────────────────────────────────────────

#[test]
fn bundled_c_plugin_discovers_enables_and_contributes() {
    janet_test!(ed, {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let root = repo.join("plugins");
        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));
        let result = scripting::eval_result(r#"(plugin/enable "c")"#).unwrap_or_default();
        assert_eq!(result, ":ok", "bundled c plugin must enable cleanly");

        let status = scripting::eval_result(
            r#"(string (get (plugin/status "c") :loaded))"#,
        )
        .unwrap_or_default();
        assert_eq!(status, r#""true""#);

        // The plugin serves the c language through its own entry script.
        let lang = scripting::eval_result(
            r#"(let [l (find (fn [i] (= (get i :name) "c")) (language/list))]
                 (if l (string (get l :loaded)) "missing"))"#,
        )
        .unwrap_or_default();
        assert_eq!(lang, r#""true""#, "c language registered and marked loaded");

        // Mode command, formatter command, and major mode all contributed.
        assert!(
            ed.commands.exists("c-mode"),
            "c-mode command should be registered"
        );
        assert!(
            ed.commands.exists("c-format-buffer"),
            "formatter command should be registered"
        );
        let major_modes = scripting::eval_result(
            r#"(string (if (get *major-modes* "c-mode") "yes" "no"))"#,
        )
        .unwrap_or_default();
        assert_eq!(major_modes, r#""yes""#, "c-mode should be a major mode");

        // The plugin's syntax.scm became the c-mode tree-sitter highlight query.
        let query_len = scripting::eval_result(
            r#"(length (get *ts-mode-queries* "c-mode"))"#,
        )
        .unwrap_or_default();
        assert_ne!(query_len, "0", "c-mode must have a tree-sitter highlight query");
    });
}

#[test]
fn bundled_c_plugin_contributes_run_configs_per_project_type() {
    janet_test!(ed, {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        scripting::eval(&format!(
            r#"(plugin/discover "{}")"#,
            root_str(&repo.join("plugins"))
        ));
        scripting::eval(r#"(plugin/enable "c")"#);

        // CMake project: configure → build → test, chained by build-before.
        let cmake_root = plugin_root("c-cmake");
        std::fs::write(cmake_root.join("CMakeLists.txt"), "project(x)\n").unwrap();
        scripting::eval(&format!(
            r#"(event/emit "project-opened" {{"root" "{}" "name" "cmake"}})"#,
            root_str(&cmake_root)
        ));
        ed.events.drain_and_dispatch();
        let build = ed
            .project_manager
            .run_configs
            .named("build")
            .cloned()
            .expect("cmake build config should be registered");
        assert_eq!(build.command, "cmake");
        assert_eq!(build.args, vec!["--build".to_string(), "build".to_string()]);
        assert_eq!(build.build_before_run.as_deref(), Some("cmake-configure"));
        let test = ed
            .project_manager
            .run_configs
            .named("test")
            .cloned()
            .expect("cmake test config should be registered");
        assert_eq!(test.command, "ctest");
        assert_eq!(test.build_before_run.as_deref(), Some("build"));
        std::fs::remove_dir_all(&cmake_root).ok();

        // Bare Makefile project: build and test through make.
        let make_root = plugin_root("c-make");
        std::fs::write(make_root.join("Makefile"), "all:\n").unwrap();
        scripting::eval(&format!(
            r#"(event/emit "project-opened" {{"root" "{}" "name" "make"}})"#,
            root_str(&make_root)
        ));
        ed.events.drain_and_dispatch();
        let build = ed
            .project_manager
            .run_configs
            .named("build")
            .cloned()
            .expect("make build config should be registered");
        assert_eq!(build.command, "make");
        std::fs::remove_dir_all(&make_root).ok();
    });
}

// ── plugin images ─────────────────────────────────────────────────────────────

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), to).unwrap();
        }
    }
}

#[test]
fn plugin_build_creates_loadable_image() {
    janet_test!(ed, {
        let root = plugin_root("image");
        write_file(
            &root.join("main.janet"),
            "(plugin-state/set \"zz-image-hit\" \"via-image\")",
        );
        scripting::eval(&format!(
            r#"(plugin/register "zz-test-image" "1.0" "{}" "main.janet" @[] @[] @[])"#,
            root_str(&root)
        ));
        let build = scripting::eval_result(
            r#"(try (plugin/build "zz-test-image") ([e] (string "build-error: " e)))"#,
        )
        .unwrap_or_default();
        assert!(
            build.contains("main.jimage"),
            "plugin/build should return the image path: {build}"
        );
        let image = root.join("main.jimage");
        assert!(image.exists(), "plugin/build should write the image");

        // The image must stand alone: remove the source, flip :entry, load.
        std::fs::remove_file(&root.join("main.janet")).unwrap();
        scripting::eval(&format!(
            r#"(plugin/register "zz-test-image" "1.0" "{}" "main.jimage" @[] @[] @[])"#,
            root_str(&root)
        ));
        let load =
            scripting::eval_result(r#"(plugin/load "zz-test-image")"#).unwrap_or_default();
        assert_eq!(load, ":ok");
        let status = scripting::eval_result(
            r#"(string (get (plugin/status "zz-test-image") :loaded))"#,
        )
        .unwrap_or_default();
        assert_eq!(status, r#""true""#);
        let side_effect =
            scripting::eval_result(r#"(plugin-state/get "zz-image-hit")"#).unwrap_or_default();
        assert!(
            side_effect.contains("via-image"),
            "image did not run: {side_effect}"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn image_rejects_wrong_vm_version() {
    janet_test!(ed, {
        let root = plugin_root("image-ver");
        write_file(&root.join("main.jimage"), "MGIMG1:0.0.0\nnot-really-bytecode");
        scripting::eval(&format!(
            r#"(plugin/register "zz-test-image-ver" "1.0" "{}" "main.jimage" @[] @[] @[])"#,
            root_str(&root)
        ));
        let result =
            scripting::eval_result(r#"(plugin/load "zz-test-image-ver")"#).unwrap_or_default();
        assert_eq!(result, ":error");
        let detail = ed
            .status_message
            .as_ref()
            .map(|m| m.text.clone())
            .unwrap_or_default();
        assert!(
            detail.contains("rebuild"),
            "version mismatch should hint at rebuild: {detail}"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn image_caps_still_enforced() {
    janet_test!(ed, {
        let root = plugin_root("image-caps");
        write_file(&root.join("main.janet"), "(fs/cwd)");
        scripting::eval(&format!(
            r#"(plugin/register "zz-test-image-cap" "1.0" "{}" "main.janet" @[] @[] @[])"#,
            root_str(&root)
        ));
        scripting::eval(r#"(plugin/build "zz-test-image-cap")"#);
        scripting::eval(&format!(
            r#"(plugin/register "zz-test-image-cap" "1.0" "{}" "main.jimage" @[] @[] @[])"#,
            root_str(&root)
        ));
        let result =
            scripting::eval_result(r#"(plugin/load "zz-test-image-cap")"#).unwrap_or_default();
        assert_eq!(
            result, ":error",
            "fs/cwd must fail through the image path without :filesystem"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn plugin_build_rejects_non_janet_entry() {
    janet_test!(ed, {
        let root = plugin_root("image-built");
        write_file(&root.join("main.jimage"), "MGIMG1:x\n");
        scripting::eval(&format!(
            r#"(plugin/register "zz-test-image-built" "1.0" "{}" "main.jimage" @[] @[] @[])"#,
            root_str(&root)
        ));
        let result = scripting::eval_result(
            r#"(try (plugin/build "zz-test-image-built") ([e] (string "build-error: " e)))"#,
        )
        .unwrap_or_default();
        assert!(
            result.contains("not Janet source"),
            "plugin/build must refuse a non-source entry: {result}"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn every_bundled_plugin_compiles_to_an_image() {
    janet_test!(ed, {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        // Images are build artifacts: compile a temp copy so the repo tree
        // never gains a .jimage.
        let copy = plugin_root("build-all");
        copy_dir(&repo.join("plugins"), &copy.join("plugins"));
        let root = root_str(&copy.join("plugins"));
        scripting::eval(&format!(r#"(plugin/discover "{root}")"#));
        let failures = scripting::eval_result(&format!(
            r#"(string/join (plugin/build-all "{root}") "\n")"#
        ))
        .unwrap_or_default();
        assert!(
            failures.is_empty() || failures == r#""""#,
            "every bundled plugin must compile:\n{failures}"
        );
        std::fs::remove_dir_all(&copy).ok();
    });
}

// ── plugin loading settings ───────────────────────────────────────────────────

#[test]
fn plugins_root_setting_loads_plugins_from_disk() {
    janet_test!(ed, {
        let root = plugin_root("setting");
        let full = root.join("my-plugin");
        std::fs::create_dir_all(&full).unwrap();
        write_file(
            &full.join("manifest.janet"),
            r#"(plugin/define {:name "zz-test-setting" :entry "main.janet"})"#,
        );
        write_file(&full.join("main.janet"), "# no-op entry");

        scripting::eval(&format!(
            r#"(do (option/set "plugins-root" "{}") (plugin/bootstrap "" ""))"#,
            root_str(&root)
        ));

        let status = scripting::eval_result(
            r#"(string (get (plugin/status "zz-test-setting") :loaded))"#,
        )
        .unwrap_or_default();
        assert_eq!(status, r#""true""#, "plugins-root dir should be discovered and enabled");
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn plugins_enabled_false_disables_disk_loading() {
    janet_test!(ed, {
        let root = plugin_root("disabled");
        let full = root.join("off-plugin");
        std::fs::create_dir_all(&full).unwrap();
        write_file(
            &full.join("manifest.janet"),
            r#"(plugin/define {:name "zz-test-off" :entry "main.janet"})"#,
        );
        write_file(&full.join("main.janet"), "# no-op entry");

        scripting::eval(&format!(
            r#"(do (option/set "plugins-enabled" "false")
                  (option/set "plugins-root" "{}")
                  (plugin/bootstrap "" ""))"#,
            root_str(&root)
        ));

        let status = scripting::eval_result(r#"(plugin/status "zz-test-off")"#)
            .unwrap_or_default();
        assert_eq!(status, "nil", "no plugin should load when plugins-enabled=false");
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn plugins_disabled_option_skips_plugin_at_startup() {
    janet_test!(ed, {
        let root = plugin_root("disabled-opt");
        let full = root.join("off-plugin");
        std::fs::create_dir_all(&full).unwrap();
        write_file(
            &full.join("manifest.janet"),
            r#"(plugin/define {:name "zz-test-dis" :entry "main.janet"})"#,
        );
        write_file(&full.join("main.janet"), "# no-op entry");

        scripting::eval(&format!(
            r#"(do (option/set "plugins-root" "{}")
                  (option/set "plugins-disabled" "zz-test-dis")
                  (plugin/bootstrap "" ""))"#,
            root_str(&root)
        ));

        let status = scripting::eval_result(
            r#"(string (get (plugin/status "zz-test-dis") :loaded))"#,
        )
        .unwrap_or_default();
        assert_eq!(
            status, r#""false""#,
            "a plugins-disabled name must stay unloaded"
        );

        // Re-enabling through the same setting loads it on the next pass.
        scripting::eval(r#"(option/set "plugins-disabled" "")"#);
        scripting::eval(r#"(plugin/bootstrap "" "")"#);
        let status = scripting::eval_result(
            r#"(string (get (plugin/status "zz-test-dis") :loaded))"#,
        )
        .unwrap_or_default();
        assert_eq!(status, r#""true""#, "clearing plugins-disabled should load it");
        std::fs::remove_dir_all(&root).ok();
    });
}

// ── new bundled language plugins ──────────────────────────────────────────────

#[test]
fn bundled_new_language_plugins_enable_cleanly() {
    janet_test!(ed, {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        scripting::eval(&format!(
            r#"(plugin/discover "{}")"#,
            root_str(&repo.join("plugins"))
        ));
        for name in [
            "batch", "cmake", "dart", "dockerfile", "graphql", "groovy",
            "ini", "latex", "makefile", "ocaml", "perl", "php", "powershell",
            "r", "scala",
        ] {
            let result = scripting::eval_result(&format!(r#"(plugin/enable "{name}")"#))
                .unwrap_or_default();
            assert_eq!(result, ":ok", "bundled {name} plugin must enable cleanly");
            let mode = format!("(string (if (get *major-modes* \"{name}-mode\") \"yes\" \"no\"))");
            let registered = scripting::eval_result(&mode).unwrap_or_default();
            assert_eq!(registered, r#""yes""#, "{name}-mode should be a major mode");
        }
    });
}

// ── http plugin (end to end) ──────────────────────────────────────────────────

const ECHO_SERVER: &str = r#"#!/usr/bin/env python3
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

class H(BaseHTTPRequestHandler):
    def do_GET(self):
        v = self.headers.get('X-Magma-Test', 'missing')
        b = v.encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(b)))
        self.end_headers()
        self.wfile.write(b)
    def log_message(self, *a): pass

HTTPServer(('127.0.0.1', int(sys.argv[1])), H).serve_forever()
"#;

#[test]
fn http_plugin_sends_request_with_variables_and_headers() {
    if !helpers::python3_available() {
        return;
    }
    let _lock = helpers::acquire_janet_lock();
    let mut ed = kernel::state::Editor::new(Box::new(kernel::storage::disk::DiskFileSystem::new()));
    kernel::command::builtin::register_builtin_commands(&mut ed);
    // The response arrives through the background event pump, exactly as
    // the GUI loop drives it.
    let (bg, mut rx) = kernel::runtime::spawn_background();
    ed.background = Some(bg);
    let key = ed.create_buffer("test");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);

    // Echo server: answers 200 with the request's X-Magma-Test header as
    // the body — the round trip proves the header left the plugin.
    let dir = std::env::temp_dir().join(format!("magma_http_e2e_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Reserve a port, then hand it to the server.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let server_path = dir.join("echo_server.py");
    std::fs::write(&server_path, ECHO_SERVER.replace("__PORT__", &port.to_string())).unwrap();
    let mut child = std::process::Command::new("python3")
        .arg(&server_path)
        .arg(port.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn echo server");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        if std::time::Instant::now() > deadline {
            panic!("echo server never came up");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // Enable the bundled http plugin.
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    scripting::eval(&format!(
        r#"(plugin/discover "{}")"#,
        root_str(&repo.join("plugins"))
    ));
    let enable =
        scripting::eval_result(r#"(plugin/enable "http")"#).unwrap_or_default();
    assert_eq!(enable, ":ok", "http plugin must enable cleanly");

    // A request file with a variable feeding a header; the cursor goes
    // inside the second request segment.
    let content = format!(
        "@token = hdr-flow-42\n\n### send\nGET http://127.0.0.1:{port}/\nX-Magma-Test: {{{{token}}}}\n"
    );
    let bkey = ed.create_buffer_from_str("api.http", &content);
    let cursor = content.find("\nGET").map(|i| i + 1).unwrap_or(0);
    scripting::eval(&format!("(buffer/set-cursor {bkey} {cursor})"));
    // http/send operates on the focused buffer; make the request file it.
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(bkey);
    }

    scripting::eval("(http/send)");

    // Pump background events and dispatch until the response lands.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut output = String::new();
    while std::time::Instant::now() < deadline {
        while let Ok(event) = rx.try_recv() {
            kernel::runtime::process_background_event(&mut ed, event);
        }
        ed.events.drain_and_dispatch();
        output = (|| {
            for (_, arc) in &ed.buffers {
                let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
                if buf.name == "*http-response*" {
                    return buf.slice(0, buf.len());
                }
            }
            String::new()
        })();
        if output.contains("hdr-flow-42") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    child.kill().ok();
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        output.contains("GET http://127.0.0.1:"),
        "request should be logged: {output}"
    );
    assert!(output.contains("← 200"), "response should carry the status: {output}");
    assert!(
        output.contains("hdr-flow-42"),
        "the {{token}} variable must reach the server as a header: {output}"
    );
}
