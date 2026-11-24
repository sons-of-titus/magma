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
        let result = scripting::eval_result(r#"(type (plugin/status "zz-test-never-registered"))"#)
            .unwrap_or_default();
        assert_eq!(result, ":nil");
    });
}

// ── plugin/load ───────────────────────────────────────────────────────────────

#[test]
fn plugin_load_evaluates_entry_and_marks_loaded() {
    janet_test!(ed, {
        let root = plugin_root("load");
        write_file(
            &root.join("main.janet"),
            "(plugin-state/set \"zz-load-hit\" \"yes\")",
        );
        scripting::eval(&format!(
            r#"(plugin/register "zz-test-load" "1.0" "{}" "main.janet" @[] @[] @[])"#,
            root_str(&root)
        ));
        let result = scripting::eval_result(r#"(plugin/load "zz-test-load")"#).unwrap_or_default();
        assert_eq!(result, ":ok");
        let status =
            scripting::eval_result(r#"(string (get (plugin/status "zz-test-load") :loaded))"#)
                .unwrap_or_default();
        assert_eq!(status, r#""true""#);
        let side_effect =
            scripting::eval_result(r#"(plugin-state/get "zz-load-hit")"#).unwrap_or_default();
        assert!(
            side_effect.contains("yes"),
            "entry did not run: {side_effect}"
        );
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
        write_file(
            &dep.join("main.janet"),
            "(plugin-state/set \"zz-dep-loaded\" \"1\")",
        );
        write_file(
            &main.join("main.janet"),
            "(plugin-state/set \"zz-main-loaded\" \"1\")",
        );
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
        let result = scripting::eval_result(r#"(plugin/load "zz-test-nocap")"#).unwrap_or_default();
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
        let result = scripting::eval_result(r#"(plugin/load "zz-test-cap")"#).unwrap_or_default();
        assert_eq!(
            result, ":ok",
            "fs/cwd should pass with :filesystem declared"
        );
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

        let plugin_ok =
            scripting::eval_result(r#"(string (if (plugin/status "zz-test-full") "yes" "no"))"#)
                .unwrap_or_default();
        assert_eq!(
            plugin_ok, r#""yes""#,
            "manifest plugin should be registered"
        );

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
        let cfg = ed
            .project_manager
            .run_configs
            .named("zz-test-build")
            .cloned();
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
        let result = scripting::eval_result(r#"(plugin/enable "rust")"#).unwrap_or_default();
        let detail = ed
            .status_message
            .as_ref()
            .map(|m| m.text.clone())
            .unwrap_or_default();
        assert_eq!(
            result, ":ok",
            "bundled rust plugin must enable cleanly: {detail}"
        );

        let status = scripting::eval_result(r#"(string (get (plugin/status "rust") :loaded))"#)
            .unwrap_or_default();
        assert_eq!(status, r#""true""#);

        // The plugin serves the rust language through its own entry script.
        let lang = scripting::eval_result(
            r#"(let [l (find (fn [i] (= (get i :name) "rust")) (language/list))]
                 (if l (string (get l :loaded)) "missing"))"#,
        )
        .unwrap_or_default();
        assert_eq!(
            lang, r#""true""#,
            "rust language registered and marked loaded"
        );

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
        let major_modes =
            scripting::eval_result(r#"(string (if (get *major-modes* "rust-mode") "yes" "no"))"#)
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
        assert_eq!(
            reload, ":ok",
            "image-loaded rust plugin must enable cleanly"
        );
        let reloaded_status =
            scripting::eval_result(r#"(string (get (plugin/status "rust") :entry))"#)
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

        let status = scripting::eval_result(r#"(string (get (plugin/status "c") :loaded))"#)
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
        let major_modes =
            scripting::eval_result(r#"(string (if (get *major-modes* "c-mode") "yes" "no"))"#)
                .unwrap_or_default();
        assert_eq!(major_modes, r#""yes""#, "c-mode should be a major mode");

        // The plugin's syntax.scm became the c-mode tree-sitter highlight query.
        let query_len = scripting::eval_result(r#"(length (get *ts-mode-queries* "c-mode"))"#)
            .unwrap_or_default();
        assert_ne!(
            query_len, "0",
            "c-mode must have a tree-sitter highlight query"
        );
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
        let load = scripting::eval_result(r#"(plugin/load "zz-test-image")"#).unwrap_or_default();
        assert_eq!(load, ":ok");
        let status =
            scripting::eval_result(r#"(string (get (plugin/status "zz-test-image") :loaded))"#)
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
        write_file(
            &root.join("main.jimage"),
            "MGIMG1:0.0.0\nnot-really-bytecode",
        );
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

        let status =
            scripting::eval_result(r#"(string (get (plugin/status "zz-test-setting") :loaded))"#)
                .unwrap_or_default();
        assert_eq!(
            status, r#""true""#,
            "plugins-root dir should be discovered and enabled"
        );
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

        let status = scripting::eval_result(r#"(plugin/status "zz-test-off")"#).unwrap_or_default();
        assert_eq!(
            status, "nil",
            "no plugin should load when plugins-enabled=false"
        );
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

        let status =
            scripting::eval_result(r#"(string (get (plugin/status "zz-test-dis") :loaded))"#)
                .unwrap_or_default();
        assert_eq!(
            status, r#""false""#,
            "a plugins-disabled name must stay unloaded"
        );

        // Re-enabling through the same setting loads it on the next pass.
        scripting::eval(r#"(option/set "plugins-disabled" "")"#);
        scripting::eval(r#"(plugin/bootstrap "" "")"#);
        let status =
            scripting::eval_result(r#"(string (get (plugin/status "zz-test-dis") :loaded))"#)
                .unwrap_or_default();
        assert_eq!(
            status, r#""true""#,
            "clearing plugins-disabled should load it"
        );
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
            "batch",
            "cmake",
            "dart",
            "dockerfile",
            "graphql",
            "groovy",
            "ini",
            "latex",
            "makefile",
            "ocaml",
            "perl",
            "php",
            "powershell",
            "r",
            "scala",
        ] {
            let result =
                scripting::eval_result(&format!(r#"(plugin/enable "{name}")"#)).unwrap_or_default();
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
    std::fs::write(
        &server_path,
        ECHO_SERVER.replace("__PORT__", &port.to_string()),
    )
    .unwrap();
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
    let enable = scripting::eval_result(r#"(plugin/enable "http")"#).unwrap_or_default();
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
    assert!(
        output.contains("← 200"),
        "response should carry the status: {output}"
    );
    assert!(
        output.contains("hdr-flow-42"),
        "the {{token}} variable must reach the server as a header: {output}"
    );
}

// ── unload tears down script-level registrations ─────────────────────────────

// A plugin's entry script registers commands, event handlers, and keymap
// bindings that no registry attributes to it.  The contribution journal
// records them at registration time (the entry evaluates under the
// plugin's Extension context) so unload removes exactly those — the old
// behavior left zombie handlers running until the session ended.
#[test]
fn unload_removes_script_level_registrations() {
    janet_test!(ed, {
        let root = plugin_root("teardown");
        let dir = root.join("zz-tdown");
        std::fs::create_dir_all(&dir).unwrap();
        write_file(
            &dir.join("main.janet"),
            r#"(language/define {:id "zztd" :extensions ["zztd"] :filenames ["Zztdfile"] :plugin "zz-tdown"})
(major-mode/define "zztd-mode" {:parent "prog"})
(formatter/register "zztd" {:command "cat" :args @[]})
(command/define "zztd-cmd" (fn [&] nil))
(event/on "zztd-tick" (fn [_] nil))
(keymap/set "ctrl-shift-zzz" "zztd-cmd")"#,
        );
        scripting::eval(&format!(
            r#"(plugin/register "zz-tdown" "1.0" "{}" "main.janet" @[] @[] @[:editor-access])"#,
            root_str(&dir)
        ));
        let result = scripting::eval_result(r#"(plugin/enable "zz-tdown")"#).unwrap_or_default();
        assert_eq!(result, ":ok");

        // Everything the entry registered is live.
        assert!(ed.commands.exists("zztd-cmd"), "command registered");
        assert_eq!(
            ed.events.subscriber_count("zztd-tick"),
            1,
            "event subscription registered"
        );
        assert_eq!(
            ed.keymaps.describe("ctrl-shift-zzz"),
            Some("zztd-cmd".to_string()),
            "keymap binding registered"
        );
        assert_eq!(
            scripting::eval_result(r#"(get *extension-mode-map* "zztd")"#).unwrap_or_default(),
            r#""zztd-mode""#
        );
        assert!(
            ed.language_registry.get("zztd").is_some(),
            "language registered"
        );

        let result = scripting::eval_result(r#"(plugin/unload "zz-tdown")"#).unwrap_or_default();
        assert_eq!(result, ":ok");
        ed.dispatch_events_scoped();

        // And nothing survives it.
        assert!(
            !ed.commands.exists("zztd-cmd"),
            "the plugin's command must be gone"
        );
        assert_eq!(
            ed.events.subscriber_count("zztd-tick"),
            0,
            "the plugin's subscription must be gone"
        );
        assert!(
            ed.keymaps.describe("ctrl-shift-zzz").is_none(),
            "the plugin's keybinding must be gone"
        );
        assert_eq!(
            scripting::eval_result(r#"(get *extension-mode-map* "zztd")"#).unwrap_or_default(),
            "nil",
            "the extension map entry must be gone"
        );
        assert_eq!(
            scripting::eval_result(r#"(get *formatters* "zztd")"#).unwrap_or_default(),
            "nil",
            "the formatter registration must be gone"
        );
        assert!(
            ed.language_registry.get("zztd").is_none(),
            "the language must be unregistered"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

// ── manifest metadata ────────────────────────────────────────────────────────

// plugin/define hands the whole manifest to plugin/register, so
// description/author/homepage/tags land in the record the Plugins page
// and :plugin-info read.
#[test]
fn plugin_register_spec_table_carries_metadata() {
    janet_test!(_ed, {
        scripting::eval(
            r#"(plugin/register {:name "zz-meta" :version "1.2" :root "/tmp/m" :entry "main.janet"
                   :description "A test plugin" :author "Ada" :homepage "https://example.com"
                   :tags @["tools" "test"] :languages @[] :capabilities @[:editor-access]})"#,
        );
        let fields = scripting::eval_result(
            r#"(let [p (plugin/status "zz-meta")]
                 (string (get p :description) "|" (get p :author) "|"
                         (get p :homepage) "|" (string/join (get p :tags) ",") "|"
                         (get p :version)))"#,
        )
        .unwrap_or_default();
        assert_eq!(
            fields,
            r#""A test plugin|Ada|https://example.com|tools,test|1.2""#
        );
    });
}

// ── plugin/install ───────────────────────────────────────────────────────────

// Install is a git clone into a plugin root, then discover + enable.
// The test clones a local repository — same code path as a URL, no
// network.
#[test]
fn plugin_install_clones_enables_and_registers() {
    janet_test!(ed, {
        // The clone directory comes from the source name, so name the fixture
        // repo what its manifest declares — the normal layout.
        let src = std::env::temp_dir().join(format!("zz-installed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&src);
        std::fs::create_dir_all(&src).unwrap();
        write_file(
            &src.join("manifest.janet"),
            r#"(plugin/define {:name "zz-installed" :version "0.3.0"
                 :languages ["zzinst"] :extensions {"zzinst" ["zzinst"]}
                 :capabilities [:editor-access]})"#,
        );
        write_file(&src.join("main.janet"), "(plugin-state/set \"installed\" \"yes\")");
        let git = |args: &[&str]| {
            let mut cmd = std::process::Command::new("git");
            cmd.args(args)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t");
            assert!(cmd.current_dir(&src).status().unwrap().success(), "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "init"]);

        let dst_root = std::env::temp_dir().join(format!("magma_install_root_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dst_root);
        std::fs::create_dir_all(&dst_root).unwrap();

        let result = scripting::eval_result(&format!(
            r#"(describe (plugin/install "{}" "{}"))"#,
            src.to_str().unwrap().replace('\\', "/"),
            dst_root.to_str().unwrap().replace('\\', "/")
        ))
        .unwrap_or_default();
        assert_eq!(result, r#"":ok""#, "install must clone, discover, enable");

        let loaded = scripting::eval_result(
            r#"(string (get (plugin/status "zz-installed") :loaded))"#,
        )
        .unwrap_or_default();
        assert_eq!(loaded, r#""true""#, "the installed plugin must be loaded");
        assert!(
            ed.language_registry.get("zzinst").is_some(),
            "the installed plugin's language is registered"
        );
        assert!(
            dst_root.join(format!("zz-installed-{}", std::process::id())).join("manifest.janet").exists(),
            "the clone lands in the plugin root"
        );

        // Re-installing the same name is refused, not clobbered.
        let again = scripting::eval_result(&format!(
            r#"(describe (plugin/install "{}" "{}"))"#,
            src.to_str().unwrap().replace('\\', "/"),
            dst_root.to_str().unwrap().replace('\\', "/")
        ))
        .unwrap_or_default();
        assert!(
            again.contains("already installed"),
            "second install must be refused: {again}"
        );

        std::fs::remove_dir_all(&src).ok();
        std::fs::remove_dir_all(&dst_root).ok();
    });
}

// ── per-project plugin configuration ─────────────────────────────────────────

// <root>/.magma/plugins.janet can unload plugins the project does not
// want and load ones it needs — unload is a real teardown, so this is
// not a zombie-handlers game anymore.
#[test]
fn project_config_disables_and_enables_plugins() {
    janet_test!(ed, {
        let root = plugin_root("projcfg");
        let victim = root.join("zz-victim");
        let wanted = root.join("zz-wanted");
        std::fs::create_dir_all(&victim).unwrap();
        std::fs::create_dir_all(&wanted).unwrap();
        write_file(&victim.join("main.janet"), "(command/define \"zz-victim-cmd\" (fn [&] nil))");
        write_file(&wanted.join("main.janet"), "(command/define \"zz-wanted-cmd\" (fn [&] nil))");
        scripting::eval(&format!(
            r#"(do (plugin/register "zz-victim" "1.0" "{}" "main.janet" @[] @[] @[:editor-access])
                  (plugin/register "zz-wanted" "1.0" "{}" "main.janet" @[] @[] @[:editor-access]))"#,
            root_str(&victim),
            root_str(&wanted)
        ));
        scripting::eval(r#"(plugin/enable "zz-victim")"#);
        assert!(ed.commands.exists("zz-victim-cmd"));
        assert!(!ed.commands.exists("zz-wanted-cmd"));

        let proj = root.join("proj");
        std::fs::create_dir_all(proj.join(".magma")).unwrap();
        write_file(
            &proj.join(".magma/plugins.janet"),
            r#"{:disable ["zz-victim"] :enable ["zz-wanted"]}"#,
        );

        let ok = scripting::eval_result(&format!(
            r#"(do (event/emit "project-opened" {{"root" "{}"}}) true)"#,
            proj.to_str().unwrap().replace('\\', "/")
        ))
        .unwrap();
        assert_eq!(ok, "true");
        ed.dispatch_events_scoped();

        assert!(
            !ed.commands.exists("zz-victim-cmd"),
            "project-disable must tear the plugin down"
        );
        assert_eq!(
            scripting::eval_result(r#"(string (get (plugin/status "zz-victim") :loaded))"#)
                .unwrap_or_default(),
            r#""false""#
        );
        assert!(
            ed.commands.exists("zz-wanted-cmd"),
            "project-enable must load the plugin"
        );
        assert_eq!(
            scripting::eval_result(r#"(string (get (plugin/status "zz-wanted") :loaded))"#)
                .unwrap_or_default(),
            r#""true""#
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

// ── mainstream toolchains ────────────────────────────────────────────────────

// The deep half of the manifest (build/test/run/debug actions) reaches
// every mainstream language: their actions seed run configurations at
// project detection, and the debug adapter chain consults :debug.
#[test]
fn mainstream_plugins_declare_toolchains_with_actions() {
    janet_test!(ed, {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        scripting::eval(&format!(
            r#"(plugin/discover "{}")"#,
            repo.join("plugins").to_str().unwrap().replace('\\', "/")
        ));
        scripting::eval(
            r#"(plugin/enable-many @["go" "python" "javascript" "java" "kotlin" "zig" "swift" "dart"])"#,
        );

        let actions = |id: &str| {
            ed.toolchains.get(id).map(|t| t.actions.len()).unwrap_or(0)
        };
        for id in [
            "go.toolchain",
            "python.toolchain",
            "javascript.node",
            "java.maven",
            "java.gradle",
            "kotlin.gradle",
            "zig.toolchain",
            "swift.package",
            "dart.pub",
        ] {
            assert!(
                actions(id) >= 2,
                "{id} must declare build/test/run actions"
            );
        }
        // Toolchain actions seed run configs at detection: a go project
        // gets its build/test/run even though the core table already has
        // go defaults (toolchain actions fill the gaps, names upsert).
        let root = std::env::temp_dir().join(format!("magma_go_seed_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("go.mod"), "module t\n").unwrap();
        let detected = kernel::toolchains::detect_and_seed(
            &root,
            ed.toolchains.list(),
            &mut ed.project_manager.run_configs,
        );
        assert_eq!(detected.as_deref(), Some("go"));
        assert!(
            ed.project_manager.run_configs.named("test").is_some(),
            "detection seeds the test config"
        );
        assert!(
            ed.project_manager.run_configs.named("run").unwrap().debuggable,
            "toolchain run action carries debuggable"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}
