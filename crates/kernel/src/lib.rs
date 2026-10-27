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
pub mod scheduler;
pub mod debug;
pub mod project;
pub mod run_config;
pub mod session;
pub mod state;
pub use text as text_engine;
pub mod index;
pub mod semantic;
pub mod task;
pub mod render;
pub mod storage;
pub use keymap;
pub mod script_runtime;
pub mod vc;
pub mod command;
pub use clipboard;
pub use net;
pub mod snippet;
pub mod completion;
pub mod terminal;
pub use util;
pub mod input;
pub mod server;
pub mod runtime;
pub mod runtime_dap;
pub mod workspace;

pub use script_runtime::ScriptRuntime;
