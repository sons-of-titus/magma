//! Language plugins and project templates, end to end: fixture plugins
//! register languages, toolchains, and templates through the manifest
//! pipeline; templates generate projects; toolchains seed run configs on
//! detection; unloading cleans registry-level contributions up.

#[macro_use]
mod helpers;
use scripting;

use std::io::Write as _;

fn plugin_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "magma_tpljanet_{}_{}_{}",
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

/// A fixture plugin with a language (extensions), a toolchain, and one
/// project template — the full manifest surface this feature adds.
fn write_full_plugin(root: &std::path::Path) {
    let plugin = root.join("fixture");
    let template = plugin.join("project-templates/binary");
    std::fs::create_dir_all(template.join("src")).unwrap();
    write_file(
        &plugin.join("manifest.janet"),
        r#"(plugin/define
  {:name "zz-fixture" :version "1.0" :entry "main.janet"
   :languages ["zz-fixture-lang"]
   :extensions {"zz-fixture-lang" ["zfx" "zfx2"]}
   :toolchain {:id "zz-fixture.build"
               :language "zz-fixture-lang"
               :detect ["zzbuild.marker"]
               :actions {"build" {:program "zz-fake-build" :args ["all"]}
                         "run" {:program "zz-fake-build" :args ["run"] :debuggable true}}}
   :capabilities [:editor-access]})"#,
    );
    write_file(&plugin.join("main.janet"), "# no-op entry");
    write_file(&plugin.join("syntax.scm"), "; fixture query");
    write_file(
        &template.join("template.janet"),
        r#"(template/define
  {:id "zz-fixture.binary"
   :name "Fixture Binary"
   :description "Creates a fixture project."
   :category "application"
   :language "zz-fixture-lang"
   :source "."
   :toolchain "zz-fixture.build"
   :variables {:project_name {:type "project-name" :required true}
               :greeting {:type "choice" :default "hi" :choices ["hi" "hello"]}}})"#,
    );
    write_file(&template.join("hello.txt"), "{{greeting}} {{project_name}}");
    write_file(&template.join("zzbuild.marker"), "# fixture marker");
    write_file(
        &template.join("src/{{project_name}}.txt"),
        "generated for {{project_name}}",
    );
}

// ── Manifest pipeline: language, toolchain, templates ────────────────────────

#[test]
fn manifest_registers_language_extensions_toolchain_and_templates() {
    janet_test!(ed, {
        let root = plugin_root("full");
        write_full_plugin(&root);
        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));
        assert_eq!(
            scripting::eval_result(r#"(plugin/enable "zz-fixture")"#).unwrap_or_default(),
            ":ok"
        );

        // Language registered with its extensions.
        let lang = scripting::eval_result(
            r#"(let [l (find (fn [i] (= (get i :name) "zz-fixture-lang")) (language/list))]
                 (string (length (get l :extensions)) "|" (get l :plugin)))"#,
        )
        .unwrap_or_default();
        assert_eq!(lang, r#""2|zz-fixture""#);

        // Extensions mapped to the conventional mode name, first-wins.
        let ext = scripting::eval_result(r#"(string (get *extension-mode-map* "zfx"))"#)
            .unwrap_or_default();
        assert_eq!(ext, r#""zz-fixture-lang-mode""#);

        // The highlight query rides the conventional syntax.scm.
        let dbg = scripting::eval_result(
            r#"(string (try
                          (do (def f (file/open "/etc/hosts" :r))
                              (def content (file/read f :all))
                              (file/close f)
                              (string "opened " (length content)))
                          ([e] (string "err: " e))))"#,
        )
        .unwrap_or_default();
        eprintln!("DBG-FILE: {dbg}");
        let dbg2 = scripting::eval_result(
            r#"(string/format
                 "syntax=%q queries=%q"
                 (get (find (fn [i] (= (get i :name) "zz-fixture-lang")) (language/list)) :syntax-path)
                 (get *ts-mode-queries* "zz-fixture-lang-mode"))"#,
        )
        .unwrap_or_default();
        eprintln!("DBG: {dbg2}");
        let query = scripting::eval_result(
            r#"(string (if (get *ts-mode-queries* "zz-fixture-lang-mode") "yes" "no"))"#,
        )
        .unwrap_or_default();
        assert_eq!(query, r#""yes""#);

        // Toolchain registered with structured actions.
        let tc = ed
            .toolchains
            .get("zz-fixture.build")
            .expect("toolchain registered");
        assert_eq!(tc.language, "zz-fixture-lang");
        assert_eq!(tc.detect_markers, vec!["zzbuild.marker".to_string()]);
        assert_eq!(tc.actions["build"].program, "zz-fake-build");
        assert!(tc.actions["run"].debuggable);

        // Template registered from project-templates/, source resolved
        // against the template directory.
        let t = ed
            .templates
            .get("zz-fixture.binary")
            .expect("template registered");
        assert_eq!(t.owner, "zz-fixture");
        assert!(t.source.join("hello.txt").exists());
        assert_eq!(t.file_count, 3, "template.janet itself is not content");
        assert_eq!(
            t.variables
                .iter()
                .find(|v| v.key == "greeting")
                .unwrap()
                .default
                .as_deref(),
            Some("hi")
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn extension_conflicts_keep_the_first_registration() {
    janet_test!(ed, {
        let root = plugin_root("extconf");
        let a = root.join("plugin-a");
        let b = root.join("plugin-b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        write_file(
            &a.join("manifest.janet"),
            r#"(plugin/define {:name "zz-ext-a" :languages ["zz-ext-a-lang"]
                                :extensions {"zz-ext-a-lang" ["zzshared"]}})"#,
        );
        write_file(&a.join("main.janet"), "");
        write_file(
            &b.join("manifest.janet"),
            r#"(plugin/define {:name "zz-ext-b" :languages ["zz-ext-b-lang"]
                                :extensions {"zz-ext-b-lang" ["zzshared"]}})"#,
        );
        write_file(&b.join("main.janet"), "");

        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));
        let mapping = scripting::eval_result(r#"(string (get *extension-mode-map* "zzshared"))"#)
            .unwrap_or_default();
        assert_eq!(
            mapping, r#""zz-ext-a-lang-mode""#,
            "the second plugin must not silently steal the extension"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

// ── Template generation through the new-project command ─────────────────────

#[test]
fn generate_project_from_plugin_template_end_to_end() {
    janet_test!(ed, {
        let root = plugin_root("gen");
        write_full_plugin(&root);
        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));
        scripting::eval(r#"(plugin/enable "zz-fixture")"#);

        let out = root.join("generated");
        let (bg, mut rx) = kernel::runtime::spawn_background();
        ed.background = Some(bg);
        let dest = out.join("my-proj");
        let mut args = std::collections::HashMap::new();
        args.insert(
            "template".to_string(),
            kernel::command::args::ArgValue::String("zz-fixture.binary".into()),
        );
        args.insert(
            "dest".to_string(),
            kernel::command::args::ArgValue::Path(dest.to_string_lossy().into_owned()),
        );
        args.insert(
            "name".to_string(),
            kernel::command::args::ArgValue::String("my-proj".into()),
        );
        args.insert(
            "vars".to_string(),
            kernel::command::args::ArgValue::String(r#"{"greeting":"hello"}"#.into()),
        );
        kernel::command::execute_command(&mut ed, "new-project", &args)
            .expect("new-project command");
        assert!(
            ed.template_jobs.is_generating(),
            "generation should be in flight"
        );

        // Drain like the GUI frame timer; the Generated handler opens the
        // project (open defaults true) and its detection seeds the run
        // configs asynchronously — wait for both.
        let mut opened = false;
        let mut event_names: Vec<String> = Vec::new();
        for _ in 0..500 {
            while let Ok(event) = rx.try_recv() {
                kernel::runtime::process_background_event(&mut ed, event);
            }
            // Commands queue editor events (project-opened drives task
            // detection); the GUI's refresh drains them every frame.
            ed.dispatch_events_scoped();
            if ed.project_manager.project.root.is_some()
                && !ed.project_manager.project.detecting_tasks
            {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !opened {
            let result = ed
                .template_jobs
                .last_result
                .as_ref()
                .map(|r| match r {
                    Ok(s) => format!("ok: {} files", s.files.len()),
                    Err(e) => format!("error: {e}"),
                })
                .unwrap_or_else(|| "none".to_string());
            panic!("generated project was never opened; last_result: {result}");
        }
        let listing: Vec<String> = std::fs::read_dir(&dest)
            .map(|d| {
                d.filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
                    .collect()
            })
            .unwrap_or_default();
        eprintln!(
            "DBG lang={:?} detecting={} toolchains={} dest-files={:?}",
            ed.project_manager.project.language,
            ed.project_manager.project.detecting_tasks,
            ed.toolchains.count(),
            listing
        );
        assert_eq!(
            ed.project_manager.project.language, "zz-fixture-lang",
            "the generated project's marker file must detect the plugin language"
        );
        let hello = std::fs::read_to_string(dest.join("hello.txt")).unwrap();
        assert_eq!(
            hello, "hello my-proj",
            "choice default overridden, name substituted"
        );
        let named = std::fs::read_to_string(dest.join("src/my-proj.txt")).unwrap();
        assert_eq!(named, "generated for my-proj");
        assert!(
            ed.project_manager.run_configs.named("build").is_some(),
            "detection seeds the toolchain's build action as a run config"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn template_registration_errors_are_actionable() {
    janet_test!(ed, {
        let err = scripting::eval_result(
            r#"(try (template/register {:id "badid" :name "X" :source "/tmp"})
                 ([e] (string "error: " e)))"#,
        )
        .unwrap_or_default();
        assert!(err.contains("invalid"), "bad id rejected: {err}");

        let err = scripting::eval_result(
            r#"(try (template/register {:id "ok.but" :name "X" :source "/definitely/not/here"})
                 ([e] (string "error: " e)))"#,
        )
        .unwrap_or_default();
        assert!(
            err.contains("does not exist"),
            "missing source rejected: {err}"
        );
    });
}

#[test]
fn new_project_on_occupied_destination_fails_cleanly() {
    janet_test!(ed, {
        let root = plugin_root("occupied");
        write_full_plugin(&root);
        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));
        scripting::eval(r#"(plugin/enable "zz-fixture")"#);

        let (bg, _rx) = kernel::runtime::spawn_background();
        ed.background = Some(bg);
        let dest = root.join("occupied");
        std::fs::create_dir_all(&dest).unwrap();
        write_file(&dest.join("keep.txt"), "precious");

        let mut args = std::collections::HashMap::new();
        args.insert(
            "template".to_string(),
            kernel::command::args::ArgValue::String("zz-fixture.binary".into()),
        );
        args.insert(
            "dest".to_string(),
            kernel::command::args::ArgValue::Path(dest.to_string_lossy().into_owned()),
        );
        args.insert(
            "name".to_string(),
            kernel::command::args::ArgValue::String("p".into()),
        );
        let err = kernel::command::execute_command(&mut ed, "new-project", &args)
            .expect_err("occupied destination must fail");
        assert!(
            err.contains("not empty"),
            "occupied destination rejected: {err}"
        );
        assert!(
            dest.join("keep.txt").exists(),
            "the occupied destination is untouched"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

// ── Toolchain-driven project detection ───────────────────────────────────────

#[test]
fn toolchain_markers_detect_project_and_seed_configs() {
    janet_test!(ed, {
        let root = plugin_root("detect");
        write_full_plugin(&root);
        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));
        scripting::eval(r#"(plugin/enable "zz-fixture")"#);

        let project = root.join("proj");
        std::fs::create_dir_all(&project).unwrap();
        write_file(&project.join("zzbuild.marker"), "");
        // No background runtime: detection runs inline and deterministically.
        scripting::eval(&format!(
            r#"(task/detect-project "{}")"#,
            root_str(&project)
        ));

        assert_eq!(ed.project_manager.project.language, "zz-fixture-lang");
        assert!(
            ed.project_manager.run_configs.named("build").is_some(),
            "toolchain build action seeded"
        );
        assert!(
            ed.project_manager
                .run_configs
                .named("run")
                .is_some_and(|c| c.debuggable),
            "toolchain run action seeded debuggable"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

#[test]
fn toolchain_actions_fill_gaps_without_clobbering_saved_configs() {
    janet_test!(ed, {
        let root = plugin_root("merge");
        write_full_plugin(&root);
        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));
        scripting::eval(r#"(plugin/enable "zz-fixture")"#);

        let project = root.join("proj");
        std::fs::create_dir_all(project.join(".magma")).unwrap();
        write_file(&project.join("zzbuild.marker"), "");
        write_file(
            &project.join(".magma/run-configs.json"),
            r#"{"configs":[{"name":"build","command":"saved-build","args":[]}],
                "active":"build"}"#,
        );
        scripting::eval(&format!(
            r#"(task/detect-project "{}")"#,
            root_str(&project)
        ));

        assert_eq!(
            ed.project_manager
                .run_configs
                .named("build")
                .unwrap()
                .command,
            "saved-build",
            "saved config wins over the toolchain action"
        );
        assert!(
            ed.project_manager.run_configs.named("run").is_some(),
            "toolchain fills the missing run action"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

// ── Plugin unload / reload ───────────────────────────────────────────────────

#[test]
fn unload_removes_contributions_and_reload_restores_them() {
    janet_test!(ed, {
        let root = plugin_root("unload");
        write_full_plugin(&root);
        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));
        scripting::eval(r#"(plugin/enable "zz-fixture")"#);
        assert!(ed.templates.get("zz-fixture.binary").is_some());
        assert!(ed.toolchains.get("zz-fixture.build").is_some());

        let result = scripting::eval_result(r#"(plugin/unload "zz-fixture")"#).unwrap_or_default();
        assert_eq!(result, ":ok");
        assert!(
            ed.templates.get("zz-fixture.binary").is_none(),
            "templates of an unloaded plugin leave the wizard"
        );
        assert!(ed.toolchains.get("zz-fixture.build").is_none());
        assert!(
            ed.language_registry.get("zz-fixture-lang").is_none(),
            "languages of an unloaded plugin are removed"
        );
        assert!(!ed.plugin_registry.get("zz-fixture").unwrap().loaded);

        // Reload re-reads the manifest, restoring everything.
        let reload = scripting::eval_result(
            r#"(do (def root (get (plugin/status "zz-fixture") :root))
                   (setdyn :magma/plugin-dir root)
                   (editor/load-file (string root "/manifest.janet"))
                   (plugin/enable "zz-fixture"))"#,
        )
        .unwrap_or_default();
        assert_eq!(reload, ":ok");
        assert!(ed.templates.get("zz-fixture.binary").is_some());
        assert!(ed.toolchains.get("zz-fixture.build").is_some());
        std::fs::remove_dir_all(&root).ok();
    });
}

// ── Malformed plugins stay isolated ──────────────────────────────────────────

#[test]
fn malformed_template_manifest_does_not_break_the_plugin() {
    janet_test!(ed, {
        let root = plugin_root("malformed");
        let plugin = root.join("half-broken");
        let good_tpl = plugin.join("project-templates/good");
        let bad_tpl = plugin.join("project-templates/bad");
        std::fs::create_dir_all(&good_tpl).unwrap();
        std::fs::create_dir_all(&bad_tpl).unwrap();
        write_file(
            &plugin.join("manifest.janet"),
            r#"(plugin/define {:name "zz-broken" :languages ["zz-broken-lang"]})"#,
        );
        write_file(&plugin.join("main.janet"), "");
        write_file(
            &good_tpl.join("template.janet"),
            r#"(template/define {:id "zz-broken.good" :name "Good" :source "."})"#,
        );
        write_file(&good_tpl.join("f.txt"), "x");
        write_file(&bad_tpl.join("template.janet"), r#"(error "boom")"#);

        scripting::eval(&format!(r#"(plugin/discover "{}")"#, root_str(&root)));
        let enabled = scripting::eval_result(r#"(plugin/enable "zz-broken")"#).unwrap_or_default();
        assert_eq!(
            enabled, ":ok",
            "a broken template manifest must not fail the plugin"
        );
        assert!(
            ed.templates.get("zz-broken.good").is_some(),
            "the healthy template registered"
        );
        assert!(
            ed.templates.get("zz-broken.bad").is_none(),
            "the broken template registered nothing"
        );
        std::fs::remove_dir_all(&root).ok();
    });
}

// ── Wizard-visible surface ───────────────────────────────────────────────────

#[test]
fn template_list_is_sorted_and_shaped_for_the_wizard() {
    janet_test!(ed, {
        let src = std::env::temp_dir().join(format!("magma_wizsrc_{}", std::process::id()));
        std::fs::create_dir_all(&src).unwrap();
        write_file(&src.join("a.txt"), "A");
        write_file(&src.join("b.txt"), "B");

        scripting::eval(&format!(
            r#"(template/register {{:id "zz-wiz.library" :name "Zed Library" :description "lib"
                                    :category "library" :language "zz" :source "{}"
                                    :toolchain "zz-wiz.tc"}})"#,
            root_str(&src)
        ));
        scripting::eval(&format!(
            r#"(template/register {{:id "zz-wiz.app" :name "Zed App" :description "app"
                                    :category "application" :language "zz" :source "{}"}})"#,
            root_str(&src)
        ));

        let listed = scripting::eval_result(
            r#"(string/join (map (fn [t] (get t :id)) (template/list)) ",")"#,
        )
        .unwrap_or_default();
        assert_eq!(
            listed, r#""zz-wiz.app,zz-wiz.library""#,
            "sorted by (category, id)"
        );

        let one =
            scripting::eval_result(r#"(string (get (template/get "zz-wiz.app") :file-count))"#)
                .unwrap_or_default();
        assert_eq!(one, r#""2""#);
        std::fs::remove_dir_all(&src).ok();
    });
}
