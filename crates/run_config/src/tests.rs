use super::*;

fn cfg(name: &str) -> RunConfig {
    RunConfig {
        name: name.into(),
        command: "cargo".into(),
        args: vec!["run".into()],
        cwd: None,
        env: HashMap::new(),
        build_before_run: None,
        debuggable: false,
    }
}

// ── Defaults ─────────────────────────────────────────────────────────────────

#[test]
fn unknown_languages_have_no_defaults() {
    assert!(defaults_for("cobol").is_empty());
    assert!(defaults_for("").is_empty());
}

#[test]
fn every_known_language_seeds_named_configs_with_commands() {
    for language in ["rust", "go", "node", "make", "python", "cmake", "maven", "gradle"] {
        let configs = defaults_for(language);
        assert!(!configs.is_empty(), "{language} must seed at least one config");
        for config in &configs {
            assert!(!config.name.trim().is_empty(), "{language} config needs a name");
            assert!(!config.command.trim().is_empty(), "{language} config needs a command");
        }
    }
}

#[test]
fn rust_defaults_are_build_test_and_run_with_a_debuggable_run() {
    let configs = defaults_for("rust");
    let names: Vec<&str> = configs.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["build", "test", "run"]);
    assert_eq!(configs[0].args, vec!["build"]);
    assert!(configs[2].debuggable, "only the run target is debuggable");
    assert!(!configs[0].debuggable && !configs[1].debuggable);
    assert!(configs.iter().all(|c| c.build_before_run.is_none()));
}

#[test]
fn python_has_only_test_and_run() {
    let configs = defaults_for("python");
    let names: Vec<&str> = configs.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["test", "run"]);
}

#[test]
fn self_building_commands_do_not_chain_a_build_step() {
    for language in ["rust", "node", "make"] {
        let run = defaults_for(language)
            .into_iter()
            .find(|c| c.name == "run")
            .expect("a run config");
        assert!(run.build_before_run.is_none(), "{language} run builds itself");
    }
}

// ── Working directory ────────────────────────────────────────────────────────

#[test]
fn a_missing_cwd_resolves_to_the_project_root() {
    assert_eq!(resolve_cwd(&cfg("run"), Path::new("/project")), Path::new("/project"));
}

#[test]
fn a_blank_cwd_resolves_to_the_project_root() {
    let mut c = cfg("run");
    c.cwd = Some("   ".into());
    assert_eq!(resolve_cwd(&c, Path::new("/project")), Path::new("/project"));
}

#[test]
fn a_relative_cwd_resolves_against_the_root() {
    let mut c = cfg("run");
    c.cwd = Some("crates/app".into());
    assert_eq!(resolve_cwd(&c, Path::new("/project")), Path::new("/project/crates/app"));
}

#[test]
fn an_absolute_cwd_is_used_as_is() {
    let mut c = cfg("run");
    c.cwd = Some("/elsewhere".into());
    assert_eq!(resolve_cwd(&c, Path::new("/project")), Path::new("/elsewhere"));
}

// ── DAP launch arguments ─────────────────────────────────────────────────────

#[test]
fn launch_args_carry_request_program_arguments_cwd_and_name() {
    let mut c = cfg("serve");
    c.args = vec!["--port".into(), "8080".into()];
    c.cwd = Some("server".into());

    let args = c.to_launch_args(Path::new("/project"));
    assert_eq!(args["request"], "launch");
    assert_eq!(args["program"], "cargo");
    assert_eq!(args["args"], serde_json::json!(["--port", "8080"]));
    assert_eq!(args["cwd"], "/project/server");
    assert_eq!(args["name"], "serve");
}

#[test]
fn launch_args_without_a_cwd_use_the_root() {
    let args = cfg("run").to_launch_args(Path::new("/project"));
    assert_eq!(args["cwd"], "/project");
}

// ── Serialization ────────────────────────────────────────────────────────────

#[test]
fn optional_fields_default_when_absent() {
    let parsed: RunConfig =
        serde_json::from_str(r#"{"name":"run","command":"cargo"}"#).unwrap();
    assert!(parsed.args.is_empty());
    assert!(parsed.cwd.is_none());
    assert!(parsed.env.is_empty());
    assert!(parsed.build_before_run.is_none());
    assert!(!parsed.debuggable);
}

#[test]
fn a_config_round_trips_through_json() {
    let mut original = cfg("serve");
    original.env.insert("RUST_LOG".into(), "debug".into());
    original.build_before_run = Some("build".into());
    original.debuggable = true;

    let text = serde_json::to_string(&original).unwrap();
    assert_eq!(serde_json::from_str::<RunConfig>(&text).unwrap(), original);
}
