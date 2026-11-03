//! Provider registry — the extension point for AI backends.
//!
//! A provider declares its presentation metadata and authentication methods
//! as data (`ProviderMeta`), and implements three capabilities: building a
//! chat request, parsing a chat reply, and fetching its model catalog.  The
//! UI consumes only `ProviderMeta` and the normalized model records, so
//! adding a provider never touches UI code.

use crate::auth::Credential;
use crate::catalog::ModelInfo;
use crate::llm::ToolCall;
use serde_json::Value;

/// How the provider's chat endpoint authenticates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStyle {
    Bearer,
    XApiKey,
}

/// One way a user can prove their identity to a provider.  The fields are
/// data so the UI can render every method without provider-specific code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthMethodSpec {
    /// Stable method id: "api-key" or "device".
    pub id: &'static str,
    /// Short label for the auth badge ("API key", "GitHub sign-in").
    pub label: &'static str,
    /// One-sentence explanation of what connecting will do.
    pub instructions: &'static str,
    /// Where the user obtains a key (API-key methods).
    pub key_url: &'static str,
    /// OAuth client id (device methods).
    pub client_id: &'static str,
    /// Device-authorization endpoint (device methods).
    pub device_auth_url: &'static str,
    /// Token endpoint polled during a device flow.
    pub device_token_url: &'static str,
    /// Space-separated scopes for the device flow.
    pub scopes: &'static str,
    /// Optional endpoint returning the signed-in account, for a friendly
    /// connection label.
    pub account_url: &'static str,
}

impl AuthMethodSpec {
    pub const fn api_key(
        label: &'static str,
        instructions: &'static str,
        key_url: &'static str,
    ) -> Self {
        AuthMethodSpec {
            id: "api-key",
            label,
            instructions,
            key_url,
            client_id: "",
            device_auth_url: "",
            device_token_url: "",
            scopes: "",
            account_url: "",
        }
    }

    pub const fn device(
        label: &'static str,
        instructions: &'static str,
        client_id: &'static str,
        device_auth_url: &'static str,
        device_token_url: &'static str,
        scopes: &'static str,
        account_url: &'static str,
    ) -> Self {
        AuthMethodSpec {
            id: "device",
            label,
            instructions,
            key_url: "",
            client_id,
            device_auth_url,
            device_token_url,
            scopes,
            account_url,
        }
    }
}

/// Presentation metadata: everything the provider grid and auth dialogs show.
#[derive(Debug, Clone)]
pub struct ProviderMeta {
    pub id: &'static str,
    pub display_name: &'static str,
    /// One line for the provider card.
    pub description: &'static str,
    /// Nerd Font glyph shown on the card.
    pub glyph: &'static str,
    pub auth_methods: &'static [AuthMethodSpec],
    pub auth_style: AuthStyle,
    /// Base URL for chat requests.
    pub chat_base: &'static str,
    /// Path appended to `chat_base` for chat requests.
    pub chat_path: &'static str,
}

/// One model turn to translate into a provider request.
#[derive(Debug, Clone)]
pub struct ChatTurn<'a> {
    pub model: &'a str,
    /// OpenAI-shaped chat transcript (system/user/assistant/tool messages).
    pub messages: &'a [Value],
    /// Selected reasoning level, when the model exposes any.
    pub reasoning: Option<&'a crate::catalog::ReasoningLevel>,
    pub tools: &'a Value,
}

/// A fully translated HTTP request, ready for the transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpReq {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// A provider backend: metadata plus its three capabilities.
pub trait AgentProvider: Send + Sync {
    fn meta(&self) -> &ProviderMeta;

    /// Build the HTTP request for one chat turn with this credential.
    fn chat_request(&self, turn: &ChatTurn, cred: &Credential) -> Result<HttpReq, String>;

    /// Parse a successful reply body into (assistant text, tool calls).
    fn parse_reply(&self, raw: &str) -> Result<(String, Vec<ToolCall>), String>;

    /// Fetch and normalize the models this credential may use.  Doubles as
    /// credential validation: an unauthorized key fails here.
    fn catalog(&self, cred: &Credential) -> Result<Vec<ModelInfo>, String>;
}

// ── Global registry ──────────────────────────────────────────────────────────

static REGISTRY: std::sync::OnceLock<Vec<&'static dyn AgentProvider>> = std::sync::OnceLock::new();

/// Register the built-in providers once.  Called lazily by `providers()`.
fn ensure_builtins() -> &'static Vec<&'static dyn AgentProvider> {
    REGISTRY.get_or_init(|| {
        let list: Vec<&'static dyn AgentProvider> = vec![
            &crate::providers::OPENAI,
            &crate::providers::ANTHROPIC,
            &crate::providers::OPENROUTER,
            &crate::providers::COPILOT,
            &crate::providers::DEEPSEEK,
            &crate::providers::KIMI,
            &crate::providers::ZAI,
        ];
        list
    })
}

/// All registered providers, presentation order.
pub fn providers() -> &'static [&'static dyn AgentProvider] {
    ensure_builtins()
}

pub fn by_id(id: &str) -> Option<&'static dyn AgentProvider> {
    providers().iter().copied().find(|p| p.meta().id == id)
}

/// The first authentication method a provider offers.
pub fn primary_method(id: &str) -> Option<&'static AuthMethodSpec> {
    by_id(id).and_then(|p| p.meta().auth_methods.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_registered_and_unique() {
        let all = providers();
        assert!(all.len() >= 6, "expected the built-in grid, got {}", all.len());
        let mut ids: Vec<_> = all.iter().map(|p| p.meta().id).collect();
        ids.sort_unstable();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n, "provider ids must be unique");
    }

    #[test]
    fn lookup_by_id_resolves_builtins() {
        assert_eq!(by_id("openai").map(|p| p.meta().display_name), Some("OpenAI"));
        assert_eq!(by_id("copilot").map(|p| p.meta().id), Some("copilot"));
        assert!(by_id("nonexistent").is_none());
    }

    #[test]
    fn every_provider_declares_at_least_one_auth_method() {
        for p in providers() {
            assert!(
                !p.meta().auth_methods.is_empty(),
                "{} has no auth method",
                p.meta().id
            );
        }
    }

    #[test]
    fn primary_method_matches_first_declared() {
        let m = primary_method("openai").expect("openai declares a method");
        assert_eq!(m.id, "api-key");
        assert!(by_id("openai").unwrap().meta().auth_methods.first() == Some(m));
    }
}
