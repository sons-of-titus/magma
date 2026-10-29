//! Per-platform shared-library compilation for tree-sitter grammars.
//!
//! A grammar is a self-contained C library: `parser.c` plus an optional
//! `scanner.c` / `scanner.cc`.  Mixed C/C++ sources are compiled to
//! objects with the right driver per language, then linked once —
//! compiling `parser.c` through a C++ driver is not an option.

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn lib_extension() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(windows) {
        "dll"
    } else {
        "so"
    }
}

/// Compile `sources` (C and C++ mixed) into a shared library at `out`,
/// exporting `symbol`.  Include paths come from each source's directory
/// (the conventional grammar layout keeps `tree_sitter/parser.h` next to
/// the parser sources).
pub fn build_shared_library(
    sources: &[PathBuf],
    include_dir: &Path,
    symbol: &str,
    out: &Path,
) -> Result<(), String> {
    let objdir = out
        .parent()
        .expect("out parent")
        .join(format!("build-{}", symbol.trim_start_matches("tree_sitter_")));
    std::fs::create_dir_all(&objdir).map_err(|e| e.to_string())?;

    if cfg!(windows) {
        windows_build(sources, include_dir, symbol, out, &objdir)
    } else {
        unix_build(sources, include_dir, out, &objdir)
    }
}

fn is_cxx(source: &Path) -> bool {
    source.extension().is_some_and(|e| e == "cc" || e == "cpp")
}

// ── macOS / Linux ────────────────────────────────────────────────────────────

fn unix_build(
    sources: &[PathBuf],
    include_dir: &Path,
    out: &Path,
    objdir: &Path,
) -> Result<(), String> {
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".into());
    let mut objects = Vec::new();
    for source in sources {
        let driver = if is_cxx(source) { &cxx } else { &cc };
        let obj = objdir.join(
            source
                .file_stem()
                .expect("file stem")
                .to_string_lossy()
                .into_owned()
                + ".o",
        );
        run(
            Command::new(driver)
                .args(["-O2", "-fPIC", "-c", "-I"])
                .arg(include_dir)
                .arg(source)
                .arg("-o")
                .arg(&obj),
        )?;
        objects.push(obj);
    }
    // C++ objects need the C++ runtime at link time, so the C++ driver
    // links whenever one is present.
    let linker = if sources.iter().any(|s| is_cxx(s)) {
        &cxx
    } else {
        &cc
    };
    let mut cmd = Command::new(linker);
    if cfg!(target_os = "macos") {
        cmd.arg("-dynamiclib");
    } else {
        cmd.arg("-shared");
    }
    cmd.arg("-o").arg(out);
    for obj in &objects {
        cmd.arg(obj);
    }
    run(&mut cmd)
}

// ── Windows ──────────────────────────────────────────────────────────────────

fn windows_build(
    sources: &[PathBuf],
    include_dir: &Path,
    symbol: &str,
    out: &Path,
    objdir: &Path,
) -> Result<(), String> {
    if let Some(tool) = cc::windows_registry::find_tool(&msvc_target(), "cl") {
        msvc_build(tool, sources, include_dir, symbol, out, objdir)
    } else {
        gnu_build(sources, include_dir, out)
    }
}

fn msvc_target() -> String {
    format!("{}-pc-windows-msvc", std::env::consts::ARCH)
}

/// MSVC exports nothing from a DLL unless told to: generate a `.def` file
/// listing the language entry point and hand it to the linker via cl.
fn msvc_build(
    tool: cc::Tool,
    sources: &[PathBuf],
    include_dir: &Path,
    symbol: &str,
    out: &Path,
    objdir: &Path,
) -> Result<(), String> {
    let def = objdir.join("exports.def");
    std::fs::write(&def, format!("EXPORTS\n    {symbol}\n")).map_err(|e| e.to_string())?;

    let mut cmd = Command::new(tool.path());
    cmd.envs(tool.get_envs().map(|(k, v)| (k.to_os_string(), v.to_os_string())));
    cmd.current_dir(objdir);
    cmd.args(["/nologo", "/O2", "/LD"]);
    cmd.arg(format!("/I{}", include_dir.display()));
    for s in sources {
        cmd.arg(s);
    }
    cmd.arg(format!("/Fe:{}", out.display()));
    cmd.arg(format!("/DEF:{}", def.display()));
    run(&mut cmd)
}

/// MinGW/MSYS2 toolchain: `-shared` exports every public symbol by
/// default, no .def file needed.
fn gnu_build(sources: &[PathBuf], include_dir: &Path, out: &Path) -> Result<(), String> {
    let gcc = std::env::var("CC").unwrap_or_else(|_| "gcc".into());
    let mut cmd = Command::new(gcc);
    cmd.args(["-O2", "-fPIC", "-shared", "-I"]).arg(include_dir);
    cmd.arg("-o").arg(out);
    for s in sources {
        cmd.arg(s);
    }
    run(&mut cmd)
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let out = cmd
        .output()
        .map_err(|e| format!("{}: {e}", cmd.get_program().to_string_lossy()))?;
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
