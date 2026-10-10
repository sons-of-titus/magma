//! Detection-seeded default run configurations per recognized language.
//! Split from `mod.rs` at the language-table boundary.

use std::collections::HashMap;

use super::RunConfig;

fn mk(name: &str, command: &str, args: &[&str], debuggable: bool) -> RunConfig {
    RunConfig {
        name: name.to_string(),
        command: command.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd: None,
        env: HashMap::new(),
        build_before_run: None,
        debuggable,
    }
}

/// Default build/test/run configs for a detected language.  Commands that
/// build themselves (`cargo run`, `npm start`, `make run`) do not chain a
/// build-first step.
pub fn defaults_for(language: &str) -> Vec<RunConfig> {
    match language {
        "rust" => vec![
            mk("build", "cargo", &["build"], false),
            mk("test", "cargo", &["test"], false),
            mk("run", "cargo", &["run"], true),
        ],
        "go" => vec![
            mk("build", "go", &["build", "./..."], false),
            mk("test", "go", &["test", "./..."], false),
            mk("run", "go", &["run", "."], false),
        ],
        "node" => vec![
            mk("build", "npm", &["run", "build"], false),
            mk("test", "npm", &["test"], false),
            mk("run", "npm", &["start"], false),
        ],
        "make" => vec![
            mk("build", "make", &[], false),
            mk("test", "make", &["test"], false),
            mk("run", "make", &["run"], false),
        ],
        "python" => vec![
            mk("test", "python", &["-m", "pytest"], false),
            mk("run", "python", &["-m", "main"], false),
        ],
        "cmake" => vec![
            mk("build", "cmake", &["--build", "build"], false),
            mk("test", "ctest", &["--test-dir", "build"], false),
        ],
        "maven" => vec![
            mk("build", "mvn", &["compile"], false),
            mk("test", "mvn", &["test"], false),
            mk("run", "mvn", &["exec:java"], false),
        ],
        "gradle" => vec![
            mk("build", "gradle", &["build"], false),
            mk("test", "gradle", &["test"], false),
            mk("run", "gradle", &["run"], false),
        ],
        _ => Vec::new(),
    }
}
