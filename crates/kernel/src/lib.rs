//! Magma kernel — all core subsystems.

#![allow(unsafe_op_in_unsafe_fn)]

// Logging macros come from the `logging` crate.
#[macro_use]
extern crate logging;

pub mod agent;
pub mod boot;
pub mod icons;
pub use event;
pub use extension;
pub mod debug;
pub mod project;
pub mod run_config;
pub mod scheduler;
pub mod session;
pub mod state;
pub use text as text_engine;
pub mod index;
pub mod render;
pub mod semantic;
pub mod storage;
pub mod task;
pub use keymap;
pub mod command;
pub mod script_runtime;
pub mod vc;
pub use clipboard;
pub use net;
pub mod completion;
pub mod snippet;
pub mod terminal;
pub use util;
pub mod input;
pub mod runtime;
pub mod runtime_dap;
pub mod server;
pub mod workspace;

pub use script_runtime::ScriptRuntime;
