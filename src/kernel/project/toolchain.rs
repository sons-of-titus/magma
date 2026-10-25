//! Toolchain discovery — resolve a binary name against `$PATH` (or explicit
//! search paths).  Detection and the LSP plugin use this to give a clear
//! "not installed" answer instead of a raw spawn failure.

use std::path::{Path, PathBuf};

/// Resolve `bin` against explicit search paths (the testable core).
pub fn find_in(bin: &str, paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter()
        .map(|dir| dir.join(bin))
        .find(|p| is_executable(p))
}

/// Resolve `bin` against `$PATH`.
pub fn find(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    find_in(bin, &std::env::split_paths(&path).collect::<Vec<_>>())
}

/// Roots that hold toolchain binaries but are routinely missing from `$PATH`:
/// keg-only Homebrew formulas (LLVM ships `lldb-dap`/`clangd` there), MacPorts,
/// and VS Code's CodeLLDB extension.
fn well_known_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = [
        "/opt/homebrew/opt/llvm/bin",
        "/opt/homebrew/bin",
        "/usr/local/opt/llvm/bin",
        "/usr/local/bin",
        "/opt/local/bin",
    ].iter().map(PathBuf::from).collect();

    if let Some(home) = std::env::var_os("HOME") {
        let ext = PathBuf::from(home).join(".vscode").join("extensions");
        if let Ok(entries) = std::fs::read_dir(ext) {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().starts_with("vadimcn.vscode-lldb") {
                    dirs.push(entry.path().join("adapter"));
                }
            }
        }
    }
    dirs
}

/// Resolve `bin` against `$PATH`, then the well-known install roots, then
/// (macOS) `xcrun --find`.  The `xcrun` step is what makes Xcode's `lldb-dap`
/// visible — it ships inside the toolchain and never appears on `$PATH`.
pub fn find_extended(bin: &str) -> Option<PathBuf> {
    find(bin)
        .or_else(|| find_in(bin, &well_known_dirs()))
        .or_else(|| xcrun_find(bin))
}

#[cfg(target_os = "macos")]
fn xcrun_find(bin: &str) -> Option<PathBuf> {
    let out = std::process::Command::new("xcrun")
        .arg("--find").arg(bin)
        .output().ok()?;
    if !out.status.success() {
        return None;
    }
    let path = PathBuf::from(String::from_utf8(out.stdout).ok()?.trim());
    is_executable(&path).then_some(path)
}

#[cfg(not(target_os = "macos"))]
fn xcrun_find(_bin: &str) -> Option<PathBuf> {
    None
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.is_file()
        && std::fs::metadata(p)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_executables_in_explicit_paths() {
        let dir = std::env::temp_dir().join("toolchain_find");
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("fake-tool");
        std::fs::write(&bin, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        assert_eq!(find_in("fake-tool", &[dir.clone()]), Some(bin));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn non_executables_are_skipped() {
        let dir = std::env::temp_dir().join("toolchain_noexec");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("not-a-tool"), "data").unwrap();
        assert_eq!(find_in("not-a-tool", &[dir.clone()]), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn well_known_dirs_include_keg_only_llvm() {
        let dirs = well_known_dirs();
        assert!(dirs.iter().any(|d| d == Path::new("/opt/homebrew/opt/llvm/bin")),
            "Homebrew's keg-only LLVM bin must be searched");
        assert!(dirs.iter().any(|d| d == Path::new("/usr/local/opt/llvm/bin")),
            "Intel Homebrew LLVM bin must be searched");
    }

    #[test]
    fn extended_search_resolves_path_binaries() {
        assert!(find_extended("sh").is_some(), "a $PATH binary still resolves");
    }

    #[test]
    fn extended_search_reports_missing_binaries() {
        assert_eq!(find_extended("magma-no-such-tool-9f3a"), None);
    }
}
