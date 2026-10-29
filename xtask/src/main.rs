//! Grammar build tool: `cargo xtask grammars [--all | --lang <names>…] [--verify] [--list]`.
//!
//! Reads `grammars.toml` at the repository root, clones each pinned
//! grammar repository into `target/grammars/`, and compiles its parser
//! into `plugins/<lang>/grammars/` as a shared library for the running
//! platform (`.dylib` on macOS, `.dll` on Windows, `.so` elsewhere).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

mod compile;

use compile::{lib_extension, build_shared_library};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

#[derive(Deserialize)]
struct GrammarsToml {
    grammars: std::collections::BTreeMap<String, GrammarSpec>,
}

#[derive(Deserialize)]
struct GrammarSpec {
    repo: String,
    rev: String,
    #[serde(default)]
    src: Option<String>,
    #[serde(default)]
    extra: Vec<String>,
    #[serde(default)]
    symbol: Option<String>,
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // The alias is subcommand-shaped (`cargo xtask grammars …`) so future
    // tasks can join; only the grammars task exists today.
    if args.first().map(String::as_str) == Some("grammars") {
        args.remove(0);
    }
    let mut langs = Vec::new();
    let mut all = false;
    let mut verify = false;
    let mut list = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--all" => all = true,
            "--verify" => verify = true,
            "--list" => list = true,
            "--lang" => {
                i += 1;
                while i < args.len() && !args[i].starts_with("--") {
                    langs.push(args[i].clone());
                    i += 1;
                }
                continue;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let toml_path = Path::new(ROOT).join("grammars.toml");
    let parsed: GrammarsToml = toml::from_str(
        &std::fs::read_to_string(&toml_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", toml_path.display())),
    )
    .unwrap_or_else(|e| panic!("cannot parse grammars.toml: {e}"));

    if list {
        for (lang, spec) in &parsed.grammars {
            println!("{lang}  {} @ {}", spec.repo, &spec.rev[..10.min(spec.rev.len())]);
        }
        return;
    }

    if !all && langs.is_empty() {
        eprintln!("nothing to do: pass --all or --lang <names>… (see grammars.toml)");
        std::process::exit(2);
    }

    let selected: Vec<(&String, &GrammarSpec)> = if all {
        parsed.grammars.iter().collect()
    } else {
        langs
            .iter()
            .map(|l| {
                let spec = parsed
                    .grammars
                    .get(l)
                    .unwrap_or_else(|| panic!("unknown language {l} (not in grammars.toml)"));
                (l, spec)
            })
            .collect()
    };

    let root = Path::new(ROOT).canonicalize().expect("repo root");
    let cache = root.join("target/grammars");
    std::fs::create_dir_all(&cache).expect("create grammar cache dir");

    let mut failures = Vec::new();
    for (lang, spec) in &selected {
        print!("grammars: {lang} … ");
        std::io::Write::flush(&mut std::io::stdout()).ok();
        match build_one(&root, &cache, lang, spec) {
            Ok(out) => println!("{}", out.display()),
            Err(e) => {
                println!("FAILED");
                failures.push(format!("{lang}: {e}"));
            }
        }
    }

    if verify {
        verify_built(&root, &selected, &mut failures);
    }

    if failures.is_empty() {
        println!("grammars: all done");
    } else {
        eprintln!("grammars: {} failed", failures.len());
        for f in &failures {
            eprintln!("  {f}");
        }
        std::process::exit(1);
    }
}

fn build_one(
    root: &Path,
    cache: &Path,
    lang: &str,
    spec: &GrammarSpec,
) -> Result<PathBuf, String> {
    let src_dir = fetch(repo_cache(cache, lang), &spec.repo, &spec.rev)?;

    let parser_dir = src_dir.join(spec.src.as_deref().unwrap_or("src"));
    let parser_c = parser_dir.join("parser.c");
    // Newer grammar repositories no longer vendor the generated parser:
    // regenerate it with the tree-sitter CLI when only the grammar source
    // is present.
    if !parser_c.exists() {
        run(
            Command::new("tree-sitter")
                .arg("generate")
                .current_dir(&src_dir),
        )
        .map_err(|e| format!("tree-sitter generate failed: {e} (is the tree-sitter CLI installed?)"))?;
    }
    let mut sources = vec![parser_c];
    for scanner in ["scanner.c", "scanner.cc"] {
        let s = parser_dir.join(scanner);
        if s.exists() {
            sources.push(s);
        }
    }
    for extra in &spec.extra {
        sources.push(src_dir.join(extra));
    }
    for s in &sources {
        if !s.exists() {
            return Err(format!("missing source {}", s.display()));
        }
    }

    let symbol = spec
        .symbol
        .clone()
        .unwrap_or_else(|| format!("tree_sitter_{}", lang.replace('-', "_")));
    let out = root
        .join("plugins")
        .join(lang)
        .join("grammars")
        .join(format!("tree-sitter-{lang}.{}", lib_extension()));
    std::fs::create_dir_all(out.parent().expect("grammar dir")).map_err(|e| e.to_string())?;

    build_shared_library(&sources, &parser_dir, &symbol, &out)
        .map_err(|e| format!("compile failed: {e}"))?;
    Ok(out)
}

fn repo_cache(cache: &Path, lang: &str) -> PathBuf {
    cache.join(lang)
}

/// Clone (once) and check out the pinned revision of a grammar repository.
fn fetch(dir: PathBuf, repo: &str, rev: &str) -> Result<PathBuf, String> {
    if !dir.join(".git").exists() {
        run(Command::new("git").args(["clone", "--quiet", repo]).arg(&dir))?;
    }
    let head = String::from_utf8_lossy(
        &Command::new("git")
            .args(["-C"])
            .arg(&dir)
            .args(["rev-parse", "HEAD"])
            .output()
            .map_err(|e| e.to_string())?
            .stdout,
    )
    .trim()
    .to_string();
    if head != rev {
        run(Command::new("git").args(["-C"]).arg(&dir).arg("fetch").args(["--quiet", "origin"]))?;
        run(Command::new("git").args(["-C"]).arg(&dir).args(["checkout", "--quiet", rev]))?;
    }
    Ok(dir)
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let out = cmd.output().map_err(|e| format!("{}: {e}", cmd.get_program().to_string_lossy()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} failed: {}",
            cmd.get_program().to_string_lossy(),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Open every built library and probe its expected export — the
/// cross-platform acceptance check (Windows DLLs without the .def export
/// fail here, as do grammars built for the wrong platform).  The ABI
/// version is the first field of the TSLanguage struct; it must sit in
/// the runtime's window (keep in sync with the workspace tree-sitter
/// crate: 0.27 accepts 13–15).  The editor's own loader re-validates.
fn verify_built(root: &Path, selected: &[(&String, &GrammarSpec)], failures: &mut Vec<String>) {
    const MIN_ABI: u32 = 13;
    const MAX_ABI: u32 = 15;
    for (lang, spec) in selected {
        let path = root
            .join("plugins")
            .join(lang)
            .join("grammars")
            .join(format!("tree-sitter-{lang}.{}", lib_extension()));
        print!("verify:   {lang} … ");
        std::io::Write::flush(&mut std::io::stdout()).ok();
        let symbol = spec
            .symbol
            .clone()
            .unwrap_or_else(|| format!("tree_sitter_{}", lang.replace('-', "_")));
        match unsafe { libloading::Library::new(&path) } {
            Ok(lib) => {
                let entry: libloading::Symbol<
                    unsafe extern "C" fn() -> *const std::ffi::c_void,
                > = match unsafe { lib.get(symbol.as_bytes()) } {
                    Ok(f) => f,
                    Err(_) => {
                        println!("FAILED");
                        failures.push(format!("verify {lang}: missing export {symbol}"));
                        continue;
                    }
                };
                let version = unsafe { *entry().cast::<u32>() };
                if (MIN_ABI..=MAX_ABI).contains(&version) {
                    println!("ok ({symbol}, ABI {version})");
                } else {
                    println!("FAILED");
                    failures.push(format!(
                        "verify {lang}: ABI {version} outside the runtime window {MIN_ABI}–{MAX_ABI}"
                    ));
                }
            }
            Err(e) => {
                println!("FAILED");
                failures.push(format!("verify {lang}: cannot open {}: {e}", path.display()));
            }
        }
    }
}
