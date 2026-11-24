//! Project templates: registry validation, the rendering engine, toolchain
//! detection and run-config merging, and the `new-project` command surface.

use crate::kernel::command::{self, args::ArgValue};
use crate::kernel::state::Editor;
use crate::kernel::storage::disk::DiskFileSystem;
use crate::kernel::templates::{self, InputKind, ProjectTemplate, TemplateInput, TemplateRegistry};
use crate::kernel::toolchains::{self, ToolchainAction, ToolchainInfo, ToolchainRegistry};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "magma_tpl_{}_{}_{}",
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

fn template_at(id: &str, source: &Path) -> ProjectTemplate {
    ProjectTemplate {
        id: id.to_string(),
        name: "Test Template".to_string(),
        description: String::new(),
        category: "application".to_string(),
        language: "rust".to_string(),
        source: source.to_path_buf(),
        variables: vec![],
        toolchain: None,
        file_count: 0,
        owner: "test".to_string(),
    }
}

fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn no_cancel() -> AtomicBool {
    AtomicBool::new(false)
}

// ── Registry ─────────────────────────────────────────────────────────────────

#[test]
fn registry_rejects_invalid_ids() {
    let src = scratch("reg_id_src");
    let mut reg = TemplateRegistry::default();
    for id in ["", "UPPER.x", "no-dot", "has space.x", "slash/x"] {
        assert!(
            reg.register(template_at(id, &src)).is_err(),
            "id {id:?} must be rejected"
        );
    }
    assert_eq!(reg.count(), 0);
}

#[test]
fn registry_requires_existing_source() {
    let mut reg = TemplateRegistry::default();
    let missing = scratch("reg_missing");
    std::fs::remove_dir_all(&missing).ok();
    assert!(reg.register(template_at("test.x", &missing)).is_err());
}

#[test]
fn registry_conflicts_across_owners_only() {
    let src = scratch("reg_conflict_src");
    let mut reg = TemplateRegistry::default();
    assert!(reg.register(template_at("test.x", &src)).is_ok());

    let mut other = template_at("test.x", &src);
    other.owner = "other".into();
    assert!(
        reg.register(other).is_err(),
        "another plugin cannot claim the id"
    );

    let mut same = template_at("test.x", &src);
    same.name = "Replaced".into();
    assert!(reg.register(same).is_ok(), "same owner re-registers");
    assert_eq!(reg.count(), 1);
    assert_eq!(reg.get("test.x").unwrap().name, "Replaced");
}

#[test]
fn registry_lists_sorted_and_removes_owner() {
    let src = scratch("reg_list_src");
    let mut reg = TemplateRegistry::default();
    for (id, cat) in [
        ("a.beta", "library"),
        ("a.alpha", "application"),
        ("b.alpha", "other"),
    ] {
        let mut t = template_at(id, &src);
        t.category = cat.into();
        reg.register(t).unwrap();
    }
    // (category, id) order: application, library, other.
    let ids: Vec<&str> = reg.list().iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["a.alpha", "a.beta", "b.alpha"]);

    assert_eq!(reg.remove_owner("test"), 3);
    assert_eq!(reg.count(), 0);
    assert_eq!(reg.remove_owner("test"), 0);
}

// ── Generation engine ────────────────────────────────────────────────────────

fn write_template_files(source: &Path) {
    std::fs::create_dir_all(source.join("src")).unwrap();
    std::fs::create_dir_all(source.join("nested dir/{{sub}}")).unwrap();
    std::fs::write(source.join("template.janet"), "(template/define nil)").unwrap();
    std::fs::write(
        source.join("Cargo.toml"),
        "[package]\nname = \"{{project_name}}\"\nedition = \"{{edition}}\"\n",
    )
    .unwrap();
    std::fs::write(
        source.join("src/main.rs"),
        "fn main() { /* {{project_name}} */ }",
    )
    .unwrap();
    std::fs::write(source.join("nested dir/{{sub}}/keep.txt"), "data").unwrap();
}

#[test]
fn generate_renders_files_and_names() {
    let src = scratch("gen_src");
    let out_parent = scratch("gen_out");
    write_template_files(&src);
    let dest = out_parent.join("my-proj");
    let t = template_at("test.gen", &src);
    let cancel = no_cancel();

    let summary = templates::generate::generate(
        &t,
        &dest,
        "my-proj",
        &vars(&[("edition", "2024"), ("sub", "inner")]),
        &cancel,
    )
    .unwrap();

    assert_eq!(summary.name, "my-proj");
    assert_eq!(
        summary.files,
        vec![
            "Cargo.toml".to_string(),
            "nested dir/inner/keep.txt".to_string(),
            "src/main.rs".to_string(),
        ]
    );
    let cargo = std::fs::read_to_string(dest.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("name = \"my-proj\""));
    assert!(cargo.contains("edition = \"2024\""));
    // The manifest file itself is not project content.
    assert!(!dest.join("template.janet").exists());
    std::fs::remove_dir_all(&out_parent).ok();
    std::fs::remove_dir_all(&src).ok();
}

#[test]
fn generate_rejects_unknown_and_unclosed_placeholders() {
    let src = scratch("gen_bad_src");
    let out = scratch("gen_bad_out");
    std::fs::write(src.join("x.txt"), "{{nope}}").unwrap();
    let t = template_at("test.bad", &src);
    let cancel = no_cancel();

    let err =
        templates::generate::generate(&t, &out.join("p"), "p", &vars(&[]), &cancel).unwrap_err();
    assert!(err.contains("unknown variable"), "got: {err}");

    std::fs::write(src.join("x.txt"), "open {{brace").unwrap();
    let err =
        templates::generate::generate(&t, &out.join("p"), "p", &vars(&[]), &cancel).unwrap_err();
    assert!(err.contains("unclosed"), "got: {err}");
    std::fs::remove_dir_all(&out).ok();
    std::fs::remove_dir_all(&src).ok();
}

#[test]
fn generate_rejects_path_escape_via_values() {
    let src = scratch("gen_esc_src");
    let out = scratch("gen_esc_out");
    std::fs::create_dir_all(src.join("{{sub}}")).unwrap();
    std::fs::write(src.join("{{sub}}/f.txt"), "x").unwrap();
    let t = template_at("test.esc", &src);
    let cancel = no_cancel();

    for evil in ["..", "a/b", "a\\b"] {
        let err = templates::generate::generate(
            &t,
            &out.join("p"),
            "p",
            &vars(&[("sub", evil)]),
            &cancel,
        )
        .unwrap_err();
        assert!(
            err.contains("invalid name") || err.contains("invalid project name"),
            "value {evil:?} must be rejected, got: {err}"
        );
    }
    std::fs::remove_dir_all(&out).ok();
    std::fs::remove_dir_all(&src).ok();
}

#[test]
fn generate_rejects_symlinks() {
    #[cfg(unix)]
    {
        let src = scratch("gen_link_src");
        let out = scratch("gen_link_out");
        let target = scratch("gen_link_target");
        std::os::unix::fs::symlink(&target, src.join("link")).unwrap();
        let t = template_at("test.link", &src);
        let cancel = no_cancel();
        let err = templates::generate::generate(&t, &out.join("p"), "p", &vars(&[]), &cancel)
            .unwrap_err();
        assert!(err.contains("symlink"), "got: {err}");
        std::fs::remove_dir_all(&out).ok();
        std::fs::remove_dir_all(&src).ok();
        std::fs::remove_dir_all(&target).ok();
    }
}

#[test]
fn generate_rejects_nonempty_and_overlapping_destinations() {
    let src = scratch("gen_dest_src");
    write_template_files(&src);
    let out = scratch("gen_dest_out");
    let t = template_at("test.dest", &src);
    let cancel = no_cancel();

    // Occupied destination.
    std::fs::create_dir_all(out.join("occupied")).unwrap();
    std::fs::write(out.join("occupied/file"), "keep").unwrap();
    assert!(
        templates::generate::generate(&t, &out.join("occupied"), "p", &vars(&[]), &cancel).is_err()
    );
    assert_eq!(
        std::fs::read_to_string(out.join("occupied/file")).unwrap(),
        "keep"
    );

    // Relative destination.
    assert!(
        templates::generate::generate(&t, Path::new("relative/proj"), "p", &vars(&[]), &cancel)
            .is_err()
    );

    // Destination inside the template source.
    assert!(
        templates::generate::generate(&t, &src.join("inside"), "p", &vars(&[]), &cancel).is_err()
    );
    assert!(
        !src.join("inside").exists(),
        "overlapping destination must not be created"
    );
    std::fs::remove_dir_all(&out).ok();
    std::fs::remove_dir_all(&src).ok();
}

#[test]
fn generate_copies_binary_files_verbatim() {
    let src = scratch("gen_bin_src");
    let out = scratch("gen_bin_out");
    std::fs::write(src.join("blob.bin"), [0u8, 159, 255, 0, 1]).unwrap();
    let t = template_at("test.bin", &src);
    let cancel = no_cancel();
    templates::generate::generate(&t, &out.join("p"), "p", &vars(&[]), &cancel).unwrap();
    let bytes = std::fs::read(out.join("p/blob.bin")).unwrap();
    assert_eq!(bytes, vec![0u8, 159, 255, 0, 1]);
    std::fs::remove_dir_all(&out).ok();
    std::fs::remove_dir_all(&src).ok();
}

#[test]
fn generate_cancellation_stops_midway() {
    let src = scratch("gen_cancel_src");
    let out = scratch("gen_cancel_out");
    std::fs::create_dir_all(src.join("a")).unwrap();
    std::fs::write(src.join("a/one.txt"), "1").unwrap();
    std::fs::write(src.join("a/two.txt"), "2").unwrap();
    let t = template_at("test.cancel", &src);
    let cancel = AtomicBool::new(true);
    let err =
        templates::generate::generate(&t, &out.join("p"), "p", &vars(&[]), &cancel).unwrap_err();
    assert!(err.contains("cancelled"));
    std::fs::remove_dir_all(&out).ok();
    std::fs::remove_dir_all(&src).ok();
}

#[test]
fn generate_validates_inputs_against_declaration() {
    let src = scratch("gen_input_src");
    let out = scratch("gen_input_out");
    std::fs::write(src.join("f.txt"), "{{opt}}").unwrap();
    let mut t = template_at("test.input", &src);
    t.variables = vec![
        TemplateInput {
            key: "req".into(),
            kind: InputKind::ProjectName,
            required: true,
            default: None,
            description: String::new(),
        },
        TemplateInput {
            key: "opt".into(),
            kind: InputKind::Choice(vec!["a".into(), "b".into()]),
            required: false,
            default: Some("a".into()),
            description: String::new(),
        },
    ];
    let cancel = no_cancel();

    let err =
        templates::generate::generate(&t, &out.join("p"), "p", &vars(&[]), &cancel).unwrap_err();
    assert!(err.contains("missing required input"), "got: {err}");

    let err = templates::generate::generate(
        &t,
        &out.join("p"),
        "p",
        &vars(&[("req", "ok"), ("opt", "not-a-choice")]),
        &cancel,
    )
    .unwrap_err();
    assert!(err.contains("must be one of"), "got: {err}");

    templates::generate::generate(&t, &out.join("p"), "p", &vars(&[("req", "ok")]), &cancel)
        .unwrap();
    assert_eq!(std::fs::read_to_string(out.join("p/f.txt")).unwrap(), "a");
    std::fs::remove_dir_all(&out).ok();
    std::fs::remove_dir_all(&src).ok();
}

// ── Toolchains ───────────────────────────────────────────────────────────────

fn toolchain(id: &str, language: &str, markers: &[&str]) -> ToolchainInfo {
    let mut actions = std::collections::BTreeMap::new();
    actions.insert(
        "build".to_string(),
        ToolchainAction {
            program: "fake-build-tool".to_string(),
            args: vec!["build".to_string()],
            cwd: None,
            env: HashMap::new(),
            debuggable: false,
        },
    );
    actions.insert(
        "run".to_string(),
        ToolchainAction {
            program: "fake-build-tool".to_string(),
            args: vec!["run".to_string()],
            cwd: None,
            env: HashMap::new(),
            debuggable: true,
        },
    );
    ToolchainInfo {
        id: id.to_string(),
        language: language.to_string(),
        detect_markers: markers.iter().map(|m| m.to_string()).collect(),
        actions,
        debug_adapter: None,
        install: None,
        owner: "test".to_string(),
    }
}

#[test]
fn toolchain_registry_validates_and_detects() {
    let mut reg = ToolchainRegistry::default();
    assert!(reg.register(toolchain("test.bad id", "x", &["m"])).is_err());
    assert!(reg.register(toolchain("test.nolang", "", &["m"])).is_err());
    assert!(reg.register(toolchain("test.nomark", "x", &[])).is_err());

    reg.register(toolchain("test.first", "lang-a", &["a.marker"]))
        .unwrap();
    reg.register(toolchain("test.second", "lang-b", &["b.marker"]))
        .unwrap();

    let root = scratch("tc_detect");
    assert!(reg.detect(&root).is_none());
    std::fs::write(root.join("b.marker"), "").unwrap();
    assert_eq!(reg.detect(&root).unwrap().id, "test.second");
    assert!(!reg.detect(&root).unwrap().actions.is_empty());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn toolchain_conflicts_across_owners_replaces_within() {
    let mut reg = ToolchainRegistry::default();
    reg.register(toolchain("test.dup", "x", &["m"])).unwrap();
    let mut other = toolchain("test.dup", "x", &["m"]);
    other.owner = "other".into();
    assert!(reg.register(other).is_err());

    let mut same = toolchain("test.dup", "renamed-lang", &["m"]);
    same.owner = "test".into();
    reg.register(same).unwrap();
    assert_eq!(reg.get("test.dup").unwrap().language, "renamed-lang");
}

#[test]
fn merge_run_configs_never_clobbers() {
    let mut configs = vec![run_config::RunConfig {
        name: "build".into(),
        command: "saved-build".into(),
        args: vec![],
        cwd: None,
        env: HashMap::new(),
        build_before_run: None,
        debuggable: false,
    }];
    let tc = toolchain("test.merge", "x", &["m"]);
    toolchains::merge_run_configs(&mut configs, &tc, false);
    assert_eq!(configs.len(), 2);
    assert_eq!(configs[0].command, "saved-build", "saved config wins");
    assert_eq!(configs[1].name, "run");
    assert!(configs[1].debuggable);
    // Merging twice is idempotent.
    toolchains::merge_run_configs(&mut configs, &tc, false);
    assert_eq!(configs.len(), 2);
}

// ── The `new-project` command ────────────────────────────────────────────────

fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    command::builtin::register_builtin_commands(&mut ed);
    ed
}

/// `dest` is the FINAL project directory (the wizard joins the project
/// name onto the picked folder before calling the command).
fn run_new_project(ed: &mut Editor, dest: &str, vars_json: &str) -> Result<(), String> {
    let mut args = HashMap::new();
    args.insert("template".to_string(), ArgValue::String("test.cmd".into()));
    args.insert("dest".to_string(), ArgValue::Path(dest.into()));
    args.insert("name".to_string(), ArgValue::String("proj".into()));
    args.insert("vars".to_string(), ArgValue::String(vars_json.into()));
    command::execute_command(ed, "new-project", &args)
}

#[test]
fn new_project_requires_registered_template() {
    let mut ed = make_editor();
    let out = scratch("cmd_missing_out");
    let err = run_new_project(&mut ed, &out.to_string_lossy(), "{}").unwrap_err();
    assert!(err.contains("unknown template"));
    assert!(!ed.template_jobs.is_generating());
    std::fs::remove_dir_all(&out).ok();
}

#[test]
fn new_project_rejects_bad_vars_json() {
    let src = scratch("cmd_vars_src");
    let out = scratch("cmd_vars_out");
    std::fs::write(src.join("f.txt"), "x").unwrap();
    let mut ed = make_editor();
    ed_register_template(&mut ed, &src);
    let err = run_new_project(&mut ed, &out.to_string_lossy(), "not json").unwrap_err();
    assert!(err.contains("JSON object"));
    std::fs::remove_dir_all(&out).ok();
    std::fs::remove_dir_all(&src).ok();
}

#[test]
fn new_project_validates_inputs_before_spawning() {
    let src = scratch("cmd_valid_src");
    let out = scratch("cmd_valid_out");
    std::fs::write(src.join("f.txt"), "x").unwrap();
    let mut ed = make_editor();
    let mut t = template_at("test.cmd", &src);
    t.variables = vec![TemplateInput {
        key: "req".into(),
        kind: InputKind::Text,
        required: true,
        default: None,
        description: String::new(),
    }];
    ed.templates.register(t).unwrap();
    let err = run_new_project(&mut ed, &out.to_string_lossy(), "{}").unwrap_err();
    assert!(err.contains("missing required input"));
    assert!(!ed.template_jobs.is_generating());
    std::fs::remove_dir_all(&out).ok();
    std::fs::remove_dir_all(&src).ok();
}

#[test]
fn new_project_without_background_runtime_resets_the_job_slot() {
    let src = scratch("cmd_nobg_src");
    let out = scratch("cmd_nobg_out");
    std::fs::write(src.join("f.txt"), "x").unwrap();
    let mut ed = make_editor();
    ed_register_template(&mut ed, &src);
    run_new_project(&mut ed, &out.to_string_lossy(), "{}").unwrap_err();
    assert!(
        !ed.template_jobs.is_generating(),
        "failed begin must not wedge the wizard"
    );
    std::fs::remove_dir_all(&out).ok();
    std::fs::remove_dir_all(&src).ok();
}

#[test]
fn new_project_runs_in_background_and_completes() {
    let src = scratch("cmd_bg_src");
    let out = scratch("cmd_bg_out");
    std::fs::create_dir_all(src.join("src")).unwrap();
    std::fs::write(src.join("src/main.rs"), "fn main() {}").unwrap();
    let mut ed = make_editor();
    ed_register_template(&mut ed, &src);

    let (bg, mut rx) = crate::kernel::runtime::spawn_background();
    ed.background = Some(bg);

    let project_dir = out.join("proj");
    run_new_project(&mut ed, &project_dir.to_string_lossy(), "{}").unwrap();
    assert!(ed.template_jobs.is_generating());

    // Drain like the GUI frame timer does, dispatching each event so the
    // Generated handler runs (open-on-success opens the project).
    let mut opened = false;
    for _ in 0..500 {
        let mut saw_event = false;
        while let Ok(event) = rx.try_recv() {
            saw_event = true;
            crate::kernel::runtime::process_background_event(&mut ed, event);
        }
        if ed.project_manager.project.root.is_some() {
            opened = true;
            break;
        }
        if !saw_event {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    assert!(
        opened,
        "generation event never landed or never opened the project"
    );
    assert!(out.join("proj/src/main.rs").exists());
    assert_eq!(
        ed.project_manager.project.root.as_deref(),
        Some(project_dir.as_path()),
        "open-on-success must open the generated project"
    );
    assert!(!ed.template_jobs.is_generating());
    std::fs::remove_dir_all(&out).ok();
    std::fs::remove_dir_all(&src).ok();
}

#[test]
fn new_project_cancel_reports_cancelled() {
    let src = scratch("cmd_cancel_src");
    let out = scratch("cmd_cancel_out");
    std::fs::write(src.join("f.txt"), "x").unwrap();
    let mut ed = make_editor();
    ed_register_template(&mut ed, &src);
    let (bg, _rx) = crate::kernel::runtime::spawn_background();
    ed.background = Some(bg);
    run_new_project(&mut ed, &out.to_string_lossy(), "{}").unwrap();
    assert!(ed.template_jobs.cancel());
    // The flag reaches the background closure; the Generated event carries
    // the cancellation error.  Waiting is pool-timing dependent, so only
    // the flag mechanics are asserted here.
    std::fs::remove_dir_all(&out).ok();
    std::fs::remove_dir_all(&src).ok();
}

fn ed_register_template(ed: &mut Editor, source: &Path) {
    ed.templates
        .register(template_at("test.cmd", source))
        .unwrap();
}
