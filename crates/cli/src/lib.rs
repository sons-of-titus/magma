//! Command-line interface: argument parsing and startup-target resolution.
//!
//! Pure functions only — nothing here touches the editor or the window
//! system, so every rule is unit-testable.

use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

pub const HELP: &str = "\
Magma — a programmable text editor

USAGE:
    magma [OPTIONS] [PATH[:LINE]]...

With no PATH the welcome window opens.  A directory opens as a project; a
file opens inside its parent directory as the project.  Several paths may be
given: the first directory is the project, files open as tabs.

OPTIONS:
    -h, --help               Print this help and exit
    -V, --version            Print the version and exit
    -d, --debug              Verbose logging (same as --log-level debug)
        --log-level <LEVEL>  error | warn | info | debug | trace
        --reopen             Reopen the most recent project instead of
                             showing the welcome window
        --server [ADDR]      Run headless, serving clients (default 127.0.0.1:7890)
        --connect [ADDR]     Connect to a running server (token in MAGMA_SERVER_TOKEN)
        --                   Treat every following argument as a path

ENVIRONMENT:
    MAGMA_LOG                Log level; --debug / --log-level override it
";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Run(Options),
    Help,
    Version,
    Server { addr: String },
    Connect { addr: String },
}

pub const DEFAULT_ADDR: &str = "127.0.0.1:7890";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub paths: Vec<String>,
    pub reopen_last: bool,
    pub log_level: Option<u8>,
}

pub fn version_line() -> String {
    format!("magma {}", env!("CARGO_PKG_VERSION"))
}

fn parse_level(name: &str) -> Result<u8, String> {
    match name.to_ascii_lowercase().as_str() {
        "error" => Ok(logging::Level::Error as u8),
        "warn" | "warning" => Ok(logging::Level::Warn as u8),
        "info" => Ok(logging::Level::Info as u8),
        "debug" => Ok(logging::Level::Debug as u8),
        "trace" => Ok(logging::Level::Trace as u8),
        other => Err(format!(
            "invalid log level '{other}' (expected error, warn, info, debug, or trace)"
        )),
    }
}

pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Command, String> {
    let mut opts = Options::default();
    let mut args = args.into_iter().peekable();
    let mut only_paths = false;
    let mut remote: Option<Command> = None;
    while let Some(arg) = args.next() {
        if only_paths || !arg.starts_with('-') || arg == "-" {
            opts.paths.push(arg);
            continue;
        }
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "-d" | "--debug" => opts.log_level = Some(logging::Level::Debug as u8),
            "--reopen" => opts.reopen_last = true,
            "--" => only_paths = true,
            "--server" | "--connect" => {
                // An address is recognizable by its port separator; anything
                // else stays a path argument.
                let addr = match args.peek() {
                    Some(next) if next.contains(':') && !next.starts_with('-') => {
                        args.next().unwrap_or_default()
                    }
                    _ => DEFAULT_ADDR.to_string(),
                };
                remote = Some(if arg == "--server" {
                    Command::Server { addr }
                } else {
                    Command::Connect { addr }
                });
            }
            "--log-level" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--log-level requires a value".to_string())?;
                opts.log_level = Some(parse_level(&value)?);
            }
            other => {
                if let Some(value) = other.strip_prefix("--log-level=") {
                    opts.log_level = Some(parse_level(value)?);
                } else if let Some(addr) = other.strip_prefix("--server=") {
                    remote = Some(Command::Server { addr: addr.to_string() });
                } else if let Some(addr) = other.strip_prefix("--connect=") {
                    remote = Some(Command::Connect { addr: addr.to_string() });
                } else {
                    return Err(format!("unknown option '{other}'"));
                }
            }
        }
    }
    Ok(remote.unwrap_or(Command::Run(opts)))
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Launch {
    pub project: Option<PathBuf>,
    pub files: Vec<(PathBuf, Option<usize>)>,
}

/// Split a trailing `:LINE` (or `:LINE:COL`) unless the literal path exists.
fn split_location(raw: &str, cwd: &Path) -> (PathBuf, Option<usize>) {
    let literal = cwd.join(raw);
    if literal.exists() {
        return (literal, None);
    }
    let mut head = raw;
    let mut line = None;
    for _ in 0..2 {
        match head.rsplit_once(':') {
            Some((rest, tail)) if !rest.is_empty() => match tail.parse::<usize>() {
                Ok(n) => {
                    line = Some(n);
                    head = rest;
                }
                Err(_) => break,
            },
            _ => break,
        }
    }
    // With `:LINE:COL` the loop leaves the last-parsed number, which is the
    // line; the column is parsed first from the right and then overwritten.
    (cwd.join(head), line)
}

fn expand_home(raw: &str) -> String {
    if (raw == "~" || raw.starts_with("~/"))
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}{}", &raw[1..]);
    }
    raw.to_string()
}

pub fn resolve(opts: &Options, cwd: &Path) -> Result<Launch, String> {
    let mut launch = Launch::default();
    for raw in &opts.paths {
        let (path, line) = split_location(&expand_home(raw), cwd);
        let path = std::fs::canonicalize(&path)
            .map_err(|_| format!("no such file or directory: {}", path.display()))?;
        if path.is_dir() {
            launch.project.get_or_insert(path);
        } else {
            launch.files.push((path, line));
        }
    }
    if launch.project.is_none()
        && let Some((first, _)) = launch.files.first()
    {
        launch.project = first.parent().map(Path::to_path_buf);
    }
    Ok(launch)
}
