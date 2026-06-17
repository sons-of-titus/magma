//! Magma kernel — all core subsystems.

pub mod event;
pub mod debug;
pub mod project;
pub mod state;
pub mod text_engine;
pub mod semantic;
pub mod task;
pub mod render;
pub mod storage;
pub mod keymap;
pub mod scripting;
pub mod vc;
pub mod command;
pub mod clipboard;
pub mod net;
pub mod snippet;
pub mod terminal;
pub mod util;
pub mod input;
pub mod server;
pub mod runtime;

#[cfg(feature = "janet")]
pub use scripting::JanetRuntime;
