//! Child-process hygiene: spawn in a fresh process group so killing a
//! task or terminal takes its whole subtree (`sh -c` children,
//! make/vim grandchildren) instead of orphaning them.

/// Put the child in its own process group (Unix).  A no-op elsewhere.
#[cfg(unix)]
pub fn own_process_group(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
}

#[cfg(not(unix))]
pub fn own_process_group(_cmd: &mut std::process::Command) {}

/// Signal the whole process group of `pid`, falling back to the process
/// itself when the group kill is unavailable or fails.
#[cfg(unix)]
pub fn kill_tree(pid: i32) {
    unsafe {
        let killpg = libc::kill(-pid, libc::SIGTERM);
        if killpg != 0 {
            let _ = libc::kill(pid, libc::SIGTERM);
        }
    }
}

#[cfg(not(unix))]
pub fn kill_tree(pid: i32) {
    let _ = pid; // TerminateProcess via the Child handle remains the path on Windows.
}

/// Take a whole subtree down: group-terminate, brief grace, then
/// group-kill anything that survived.
#[cfg(unix)]
pub fn kill_tree_hard(pid: i32) {
    unsafe {
        let _ = libc::kill(-pid, libc::SIGTERM);
        std::thread::sleep(std::time::Duration::from_millis(50));
        let _ = libc::kill(-pid, libc::SIGKILL);
        let _ = libc::kill(pid, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
pub fn kill_tree_hard(pid: i32) {
    let _ = pid;
}
