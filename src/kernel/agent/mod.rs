//! AI/Agent Architecture (Phase 11).
//!
//! Pure data model, planner, and LLM backend live in the `agent` crate and are
//! re-exported here. Editor-coupled pieces (context gathering, executor,
//! turn loop, Janet API) stay in the kernel.

pub mod types;
pub mod context;
pub mod executor;
#[path = "loop.rs"]
pub mod loop_;

// Re-export the editor-independent crate surface so existing paths resolve.
pub use agent::types::*;
pub use agent::planner;
pub use agent::planner::BuiltinPlanner;
pub use agent::llm;
pub use agent::MAX_TURNS;

pub use types::*;
pub use context::{EditorContextProvider, LlmContextProvider};
pub use executor::SafeExecutor;
