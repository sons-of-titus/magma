//! Agent state model, provider registry, authentication, model catalog, and
//! request translation (editor-independent).

pub mod acp;
pub mod auth;
pub mod catalog;
pub mod http;
pub mod llm;
pub mod planner;
pub mod providers;
pub mod registry;
pub mod translate;
pub mod types;

pub use auth::{AuthInstruction, AuthOutcome, Credential};
pub use catalog::{ModelInfo, ReasoningLevel, ReasoningSupport};
pub use planner::BuiltinPlanner;
pub use registry::{
    AgentProvider, AuthMethodSpec, AuthStyle, ChatTurn, HttpReq, ProviderMeta, by_id, providers,
};
pub use types::Planner;
pub use types::*;

pub use types::MAX_TURNS;
