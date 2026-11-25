//! Process-spawning primitives: one shell wrapper for command strings and
//! PATH resolution that knows Windows `.cmd` shims.  Spawn sites for
//! user- or tool-supplied names go through these — bare `Command::new`
//! cannot run a `.cmd` file and has no shell.

use std::path::{Path, PathBuf};

/// Run `cmd` through the platform shell: `sh -c` on Unix, `%COMSPEC% /C`
/// on Windows.  The string carries whatever syntax that platform's shell
/// understands; shell templates written for POSIX quoting stay Unix-only
/// (see vcs shell templates).
pub fn shell_command(cmd: &str) -> std::process::Command {
    #[cfg(target_os = "windows")]
    {
        let mut c = std::process::Command::new(default_shell());
        c.arg("/C").arg(cmd);
        c
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(cmd);
        c
    }
}

/// The default interactive shell: `$SHELL` on Unix, `%COMSPEC%` on Windows.
pub fn default_shell() -> String {
    #[cfg(target_os = "windows")]
    {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into())
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
    }
}

/// Resolve a command name against PATH.  Anything containing a path
/// separator is already a path and passes through untouched.  On Windows
/// the `.exe` / `.cmd` / `.bat` shims resolve the way a shell would.
/// Unresolved names come back unchanged so the spawn error still names
/// what the user asked for.
pub fn resolve_command(name: &str) -> PathBuf {
    if name.contains('/') || name.contains('\\') {
        return PathBuf::from(name);
    }
    for dir in path_dirs() {
        for ext in candidate_extensions() {
            let candidate = dir.join(format!("{name}{ext}"));
            if is_executable(&candidate) {
                return candidate;
            }
        }
    }
    PathBuf::from(name)
}

fn path_dirs() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn candidate_extensions() -> Vec<String> {
    let mut exts = vec![String::new()];
    match std::env::var("PATHEXT") {
        Ok(list) => exts.extend(list.split(';').map(str::to_string)),
        Err(_) => exts.extend([".exe", ".cmd", ".bat"].map(String::from)),
    }
    exts
}

#[cfg(not(target_os = "windows"))]
fn candidate_extensions() -> Vec<String> {
    vec![String::new()]
}

#[cfg(target_os = "windows")]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

#[cfg(not(target_os = "windows"))]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.is_file()
        && std::fs::metadata(p)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_pass_through_untouched() {
        let p = resolve_command("/bin/sh");
        assert_eq!(p, PathBuf::from("/bin/sh"));
        assert_eq!(resolve_command("./tool"), PathBuf::from("./tool"));
        assert_eq!(resolve_command("a\\b.exe"), PathBuf::from("a\\b.exe"));
    }

    #[cfg(unix)]
    #[test]
    fn bare_names_resolve_on_unix() {
        // The absolute location is distribution-specific — /bin/sh on
        // classic layouts, /usr/bin/sh on usr-merged ones — so assert the
        // contract: a bare name becomes an executable found on PATH.
        let resolved = resolve_command("sh");
        assert!(resolved.is_absolute(), "resolved: {resolved:?}");
        assert_eq!(resolved.file_name(), Some(std::ffi::OsStr::new("sh")));
        assert!(resolved.exists(), "resolved: {resolved:?}");
        assert!(is_executable(&resolved), "resolved: {resolved:?}");
    }

    #[cfg(windows)]
    #[test]
    fn bare_names_resolve_on_windows() {
        let resolved = resolve_command("cmd");
        assert!(resolved.extension().is_some(), "resolved: {resolved:?}");
    }

    #[cfg(unix)]
    #[test]
    fn shell_wrapper_is_sh_dash_c() {
        let c = shell_command("true");
        assert_eq!(c.get_program(), std::ffi::OsStr::new("sh"));
        assert!(c.get_args().eq(["-c", "true"]));
    }

    #[test]
    fn default_shell_is_never_empty() {
        assert!(!default_shell().is_empty());
    }
}
