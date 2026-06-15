//! Debug logging gated on `MAGMA_DEBUG` / `MAGMA_DEBUG_HTTP` env vars.
//! The `debug!` and `debug_http!` macros in `lib.rs` call into this module.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

pub fn enabled() -> bool {
    
    std::env::var("MAGMA_DEBUG").is_ok()
        || std::env::var("MAGMA_DEBUG_HTTP").is_ok()
}

pub fn is_http() -> bool {
    static HTTP: OnceLock<AtomicBool> = OnceLock::new();
    HTTP.get_or_init(|| AtomicBool::new(
        std::env::var("MAGMA_DEBUG_HTTP").is_ok()
    )).load(Ordering::Relaxed)
}
