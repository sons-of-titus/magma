//! Per-provider connection state: authentication lifecycle, model catalogs,
//! and selection preferences.
//!
//! Durable facts live in the options map — `agent.api-key.<provider>` and
//! `agent.oauth.<provider>` (keychain-backed), `agent.oauth-account.<provider>`,
//! and the selection options `agent.provider` / `agent.model` /
//! `agent.reasoning` — so settings persistence needs no new store.  This
//! file layers runtime state on top: connection status, the in-flight auth
//! flow, and cached model catalogs.  `flows` owns the state transitions.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use agent::registry;
use agent::{AuthInstruction, Credential, ModelInfo, ReasoningLevel, ReasoningSupport};

use crate::state::Editor;

pub mod flows;

pub use flows::{
    begin_auth, cancel_auth, disconnect, ensure_catalogs, fetch_models, note_auth_result,
    note_instruction, note_models, submit_api_key,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnStatus {
    Disconnected,
    /// Credential accepted locally; catalog fetch decides validity.
    Connecting,
    Connected,
    Failed(String),
}

impl ConnStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConnStatus::Disconnected => "disconnected",
            ConnStatus::Connecting => "connecting",
            ConnStatus::Connected => "connected",
            ConnStatus::Failed(_) => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Connection {
    pub provider: String,
    /// Auth method id ("api-key" / "device").
    pub method: String,
    pub status: ConnStatus,
    pub account: Option<String>,
    pub models: Vec<ModelInfo>,
    pub models_error: Option<String>,
    pub fetching: bool,
}

impl Connection {
    pub fn usable(&self) -> bool {
        self.status == ConnStatus::Connected && !self.models.is_empty()
    }
}

/// The auth flow currently on screen, if any.
pub struct PendingAuth {
    pub provider: String,
    pub method: String,
    pub instruction: AuthInstruction,
    /// Set while a background task owns the flow (device polling).
    pub busy: bool,
    pub cancel: Arc<AtomicBool>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct AgentConnections {
    pub conns: HashMap<String, Connection>,
    pub pending: Option<PendingAuth>,
}

// ── Option keys ──────────────────────────────────────────────────────────────

pub fn secret_option(method: &str, provider: &str) -> String {
    match method {
        "device" => format!("agent.oauth.{provider}"),
        _ => format!("agent.api-key.{provider}"),
    }
}

const OAUTH_ACCOUNT_PREFIX: &str = "agent.oauth-account.";

fn credential_of(options: &HashMap<String, String>, provider: &str) -> Option<(String, Credential)> {
    let key = options
        .get(&format!("agent.api-key.{provider}"))
        .filter(|k| !k.trim().is_empty())
        .map(|k| ("api-key".to_string(), Credential::ApiKey { key: k.clone() }));
    key.or_else(|| {
        options
            .get(&format!("agent.oauth.{provider}"))
            .filter(|t| !t.trim().is_empty())
            .map(|t| {
                (
                    "device".to_string(),
                    Credential::OAuth {
                        access_token: t.clone(),
                        expires_at: None,
                        account: options
                            .get(&format!("{OAUTH_ACCOUNT_PREFIX}{provider}"))
                            .cloned(),
                    },
                )
            })
    })
}

// ── Reconciliation with the options map ──────────────────────────────────────

impl AgentConnections {
    /// Add/remove connections so they mirror the credential options.  Called
    /// at boot and after any credential change.
    pub fn sync_from_options(&mut self, ed_options: &HashMap<String, String>) {
        for p in registry::providers() {
            let id = p.meta().id;
            let has_credential = credential_of(ed_options, id).is_some();
            match (self.conns.get(id), has_credential) {
                (None, true) => {
                    let (method, _) = credential_of(ed_options, id).unwrap();
                    self.conns.insert(
                        id.to_string(),
                        Connection {
                            provider: id.to_string(),
                            method,
                            status: ConnStatus::Connected,
                            account: ed_options
                                .get(&format!("{OAUTH_ACCOUNT_PREFIX}{id}"))
                                .cloned(),
                            models: Vec::new(),
                            models_error: None,
                            fetching: false,
                        },
                    );
                }
                (Some(conn), false) => {
                    // Keep the row if it is mid-auth so errors stay visible;
                    // otherwise a revoked credential drops the connection.
                    if conn.status != ConnStatus::Connecting || self.pending.is_none() {
                        self.conns.remove(id);
                    }
                }
                _ => {
                    if let Some(conn) = self.conns.get_mut(id) {
                        conn.account = ed_options
                            .get(&format!("{OAUTH_ACCOUNT_PREFIX}{id}"))
                            .cloned();
                    }
                }
            }
        }
    }

    pub fn connected_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .conns
            .iter()
            .filter(|(_, c)| c.status != ConnStatus::Disconnected)
            .map(|(id, _)| id.clone())
            .collect();
        ids.sort();
        ids
    }

    pub fn any_connected(&self) -> bool {
        !self.connected_ids().is_empty()
    }

    /// The connection backing new sessions: the active selection, else the
    /// first connected provider.
    pub fn active<'a>(
        &'a self,
        ed_options: &HashMap<String, String>,
    ) -> Option<(&'static dyn registry::AgentProvider, &'a Connection)> {
        let ids = self.connected_ids();
        let active_id = ed_options
            .get("agent.provider")
            .filter(|p| ids.iter().any(|id| id == *p))
            .or_else(|| ids.first())?;
        let provider = registry::by_id(active_id)?;
        self.conns.get(active_id).map(|c| (provider, c))
    }
}

// ── Chat resolution for the turn loop ────────────────────────────────────────

pub struct ResolvedChat {
    pub provider: &'static dyn registry::AgentProvider,
    pub credential: Credential,
    pub model: ModelInfo,
    pub reasoning: Option<ReasoningLevel>,
}

/// Pick the model for new turns: the persisted choice when the current
/// catalog still lists it, else the first model.
pub fn selected_model<'a>(
    conn: &'a Connection,
    ed_options: &HashMap<String, String>,
) -> Option<&'a ModelInfo> {
    let choice = ed_options.get("agent.model");
    conn.models
        .iter()
        .find(|m| Some(&m.id) == choice)
        .or_else(|| conn.models.first())
}

/// The reasoning level for `model`: the persisted choice when that model
/// still supports it, else the model's first level.
pub fn selected_reasoning(
    model: &ModelInfo,
    ed_options: &HashMap<String, String>,
) -> Option<ReasoningLevel> {
    match &model.reasoning {
        ReasoningSupport::None => None,
        support => {
            let choice = ed_options.get("agent.reasoning");
            support
                .levels()
                .iter()
                .find(|l| Some(&l.id) == choice)
                .or_else(|| support.default_level())
                .cloned()
        }
    }
}

/// The model new turns will use, resolved against the editor's options.
pub fn selected_model_of(ed: &Editor) -> Option<&ModelInfo> {
    let (_, conn) = ed.agent_connections.active(&ed.options)?;
    selected_model(conn, &ed.options)
}

/// The reasoning level new turns will use, resolved against the options.
pub fn selected_reasoning_of(ed: &Editor) -> Option<ReasoningLevel> {
    let model = selected_model_of(ed)?;
    selected_reasoning(model, &ed.options)
}

/// Assemble everything one chat turn needs from the active connection.
pub fn resolve_chat(ed: &Editor) -> Result<ResolvedChat, String> {
    let (provider, conn) = ed
        .agent_connections
        .active(&ed.options)
        .ok_or_else(|| "agent: no connected provider".to_string())?;
    if !conn.usable() {
        return Err(format!(
            "agent: {} is still connecting — no models available yet",
            conn.provider
        ));
    }
    let (_, credential) = credential_of(&ed.options, conn.provider.as_str())
        .ok_or_else(|| "agent: credential vanished".to_string())?;
    let model = selected_model(conn, &ed.options)
        .ok_or_else(|| "agent: provider exposes no models".to_string())?
        .clone();
    let reasoning = selected_reasoning(&model, &ed.options);
    Ok(ResolvedChat {
        provider,
        credential,
        model,
        reasoning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn sync_creates_connections_from_credential_options() {
        let mut conns = AgentConnections::default();
        conns.sync_from_options(&opts(&[("agent.api-key.openai", "sk-1")]));
        assert_eq!(conns.connected_ids(), vec!["openai".to_string()]);
        assert_eq!(conns.conns["openai"].method, "api-key");

        conns.sync_from_options(&opts(&[]));
        assert!(conns.conns.is_empty(), "revoked credentials drop out");
    }

    #[test]
    fn oauth_credentials_map_to_the_device_method() {
        let mut conns = AgentConnections::default();
        conns.sync_from_options(&opts(&[
            ("agent.oauth.copilot", "gho_1"),
            ("agent.oauth-account.copilot", "octocat"),
        ]));
        let conn = &conns.conns["copilot"];
        assert_eq!(conn.method, "device");
        assert_eq!(conn.account.as_deref(), Some("octocat"));
        assert_eq!(conn.status, ConnStatus::Connected);
    }

    #[test]
    fn active_prefers_the_selection_then_the_first_connection() {
        let mut conns = AgentConnections::default();
        conns.sync_from_options(&opts(&[
            ("agent.api-key.openai", "sk"),
            ("agent.api-key.zai", "zk"),
        ]));
        let (provider, conn) = conns
            .active(&opts(&[("agent.provider", "zai")]))
            .expect("active resolves");
        assert_eq!(provider.meta().id, "zai");
        assert_eq!(conn.provider, "zai");

        let (provider, _) = conns.active(&opts(&[])).unwrap();
        assert_eq!(provider.meta().id, "openai", "falls back to the first sorted");

        let missing = opts(&[("agent.provider", "anthropic-nope")]);
        assert!(conns.active(&missing).is_some(), "unknown selection falls back");
    }

    #[test]
    fn model_selection_validates_against_the_catalog() {
        use agent::catalog::{effort_levels, ReasoningSupport};
        let conn = Connection {
            provider: "openai".into(),
            method: "api-key".into(),
            status: ConnStatus::Connected,
            account: None,
            models: vec![
                ModelInfo {
                    id: "gpt-4o".into(),
                    display: "GPT-4o".into(),
                    context_window: Some(128_000),
                    max_output: None,
                    reasoning: ReasoningSupport::None,
                    tools: true,
                    image_input: false,
                    note: String::new(),
                },
                ModelInfo {
                    id: "gpt-5".into(),
                    display: "GPT-5".into(),
                    context_window: Some(400_000),
                    max_output: None,
                    reasoning: ReasoningSupport::Levels(effort_levels(true)),
                    tools: true,
                    image_input: false,
                    note: String::new(),
                },
            ],
            models_error: None,
            fetching: false,
        };

        let picked = selected_model(&conn, &opts(&[("agent.model", "gpt-5")])).unwrap();
        assert_eq!(picked.id, "gpt-5");
        let picked = selected_model(&conn, &opts(&[("agent.model", "retired-model")])).unwrap();
        assert_eq!(picked.id, "gpt-4o", "stale choice falls back to the first");
        let picked = selected_model(&conn, &opts(&[])).unwrap();
        assert_eq!(picked.id, "gpt-4o");

        let reasoning = selected_reasoning(picked, &opts(&[]));
        assert!(reasoning.is_none(), "no reasoning controls without support");
        let gpt5 = conn.models.last().unwrap();
        let reasoning = selected_reasoning(gpt5, &opts(&[])).unwrap();
        assert_eq!(reasoning.id, "minimal", "default is the first level");
        let reasoning =
            selected_reasoning(gpt5, &opts(&[("agent.reasoning", "high")])).unwrap();
        assert_eq!(reasoning.id, "high");
        let reasoning = selected_reasoning(
            gpt5,
            &opts(&[("agent.reasoning", "turbo-infinite")]),
        )
        .unwrap();
        assert_eq!(reasoning.id, "minimal", "unknown level falls back to default");
    }

    #[test]
    fn secret_option_key_follows_the_method() {
        assert_eq!(secret_option("api-key", "openai"), "agent.api-key.openai");
        assert_eq!(secret_option("device", "copilot"), "agent.oauth.copilot");
    }
}
