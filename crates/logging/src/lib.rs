//! Leveled logging for Magma, gated on the `MAGMA_LOG` env var.
//!
//! `MAGMA_LOG` picks the maximum level: `error`, `warn`, `info`, `debug`, or
//! `trace` (`1`/`true`/`on` also mean `debug`).  It defaults to `info`, so
//! errors, warnings, and startup banners stay visible without configuration.
//!
//! `MAGMA_DEBUG` and `MAGMA_DEBUG_HTTP` remain supported: the former raises
//! the level to `debug`, the latter additionally opens the HTTP channel.
//!
//! [`init_file`] adds a persistent sink: every emitted record is also
//! appended to the file, and [`install_panic_hook`] routes panics through
//! `error!` so crashes are recorded too.
//!
//! The `error!`/`warn!`/`info!`/`debug!`/`trace!` and `debug_http!` macros
//! write to stderr through [`emit`]/[`emit_http`].

use std::fmt;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::emit($crate::Level::Error, module_path!(), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::emit($crate::Level::Warn, module_path!(), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::emit($crate::Level::Info, module_path!(), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::emit($crate::Level::Debug, module_path!(), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        $crate::emit($crate::Level::Trace, module_path!(), format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! debug_http {
    ($($arg:tt)*) => {
        $crate::emit_http(module_path!(), format_args!($($arg)*))
    };
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Level {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<u8> {
        match name {
            "error" => Some(Level::Error as u8),
            "warn" | "warning" => Some(Level::Warn as u8),
            "info" => Some(Level::Info as u8),
            "debug" => Some(Level::Debug as u8),
            "trace" => Some(Level::Trace as u8),
            _ => None,
        }
    }
}

pub(crate) const DEFAULT_LEVEL: u8 = Level::Info as u8;

fn configured_level() -> u8 {
    if let Ok(raw) = std::env::var("MAGMA_LOG") {
        let value = raw.trim().to_ascii_lowercase();
        if value == "1" || value == "true" || value == "on" {
            return Level::Debug as u8;
        }
        if let Some(level) = Level::from_name(&value) {
            return level;
        }
    }
    if std::env::var("MAGMA_DEBUG").is_ok() || std::env::var("MAGMA_DEBUG_HTTP").is_ok() {
        return Level::Debug as u8;
    }
    DEFAULT_LEVEL
}

static LEVEL: OnceLock<AtomicU8> = OnceLock::new();

fn level_cell() -> &'static AtomicU8 {
    LEVEL.get_or_init(|| AtomicU8::new(configured_level()))
}

pub fn max_level() -> u8 {
    level_cell().load(Ordering::Relaxed)
}

pub fn set_max_level(level: u8) {
    level_cell().store(level, Ordering::Relaxed);
}

pub fn enabled(level: Level) -> bool {
    max_level() >= level as u8
}

static FILE_SINK: Mutex<Option<std::fs::File>> = Mutex::new(None);

pub fn init_file(path: &Path) -> std::io::Result<()> {
    let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    *FILE_SINK.lock().unwrap_or_else(|e| e.into_inner()) = Some(file);
    Ok(())
}

pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        error!("panic in thread '{}': {info}", thread.name().unwrap_or("<unnamed>"));
        previous(info);
    }));
}

pub fn emit(level: Level, target: &str, args: fmt::Arguments<'_>) {
    if !enabled(level) {
        return;
    }
    eprintln!("[{}] {}: {}", level.label(), target, args);
    if let Some(file) = FILE_SINK.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "{secs} [{}] {}: {}", level.label(), target, args);
    }
}

pub fn is_http() -> bool {
    static HTTP: OnceLock<AtomicBool> = OnceLock::new();
    HTTP.get_or_init(|| AtomicBool::new(std::env::var("MAGMA_DEBUG_HTTP").is_ok()))
        .load(Ordering::Relaxed)
}

pub fn emit_http(target: &str, args: fmt::Arguments<'_>) {
    if is_http() {
        eprintln!("[HTTP] {target}: {args}");
    }
}

#[cfg(test)]
mod tests;
