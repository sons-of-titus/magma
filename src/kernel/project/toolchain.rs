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
}
