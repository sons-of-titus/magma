//! AI/Agent Architecture (Phase 11).
//!
//! Pure data model, planner, and LLM backend live in the `agent` crate and are
//! re-exported here. Editor-coupled pieces (context gathering, executor,
//! turn loop, Janet API) stay in the kernel.

pub mod connections;
pub mod context;
pub mod executor;
#[path = "loop.rs"]
pub mod loop_;
pub mod tools;
pub mod types;

// Re-export the editor-independent crate surface so existing paths resolve.
pub use agent::MAX_TURNS;
pub use agent::acp;
pub use agent::auth;
pub use agent::catalog;
pub use agent::http;
pub use agent::llm;
pub use agent::planner;
pub use agent::planner::BuiltinPlanner;
pub use agent::registry;
pub use agent::types::*;

pub use agent::{AuthInstruction, AuthOutcome, Credential, ModelInfo, ReasoningLevel};
pub use context::{EditorContextProvider, LlmContextProvider};
pub use executor::SafeExecutor;
pub use types::*;
