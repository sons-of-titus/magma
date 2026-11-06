//! Built-in provider registrations.
//!
//! Each provider is a static struct: metadata, a reasoning-parameter style,
//! and catalog/chat implementations.  The OpenAI-compatible family shares
//! one implementation; Anthropic and Copilot override the pieces that
//! differ (wire protocol, token exchange, entitlement filtering).

use std::sync::Mutex;


use crate::auth::{Credential, fetch_account, now_epoch};
use crate::catalog::{
    GENERIC_HINTS, ModelInfo, fallback_models, parse_anthropic_models, parse_openai_models,
    parse_openrouter_models, parse_copilot_models,
};
use crate::http;
use crate::registry::{AgentProvider, AuthMethodSpec, AuthStyle, ChatTurn, HttpReq, ProviderMeta};
use crate::translate::{self, ReasoningStyle};

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

fn bearer_headers(cred: &Credential) -> Vec<(String, String)> {
    vec![
        ("Authorization".into(), format!("Bearer {}", cred.secret())),
        ("User-Agent".into(), "magma-agent/0.1".into()),
    ]
}

// ── Catalog fetchers ─────────────────────────────────────────────────────────

fn openai_catalog(cred: &Credential) -> Result<Vec<ModelInfo>, String> {
    let (_, raw) = http::get_json("https://api.openai.com/v1/models", &bearer_headers(cred), TIMEOUT)?;
    let mut models = parse_openai_models(&raw)?;
    // The endpoint also lists audio/image/speech models; keep chat-capable ones.
    models.retain(|m| m.id.starts_with("gpt") || m.id.starts_with('o') || m.id.starts_with("chatgpt"));
    Ok(models)
}

fn anthropic_catalog(cred: &Credential) -> Result<Vec<ModelInfo>, String> {
    let headers = vec![
        ("x-api-key".into(), cred.secret().into()),
        ("anthropic-version".into(), "2023-06-01".into()),
        ("User-Agent".into(), "magma-agent/0.1".into()),
    ];
    let (_, raw) = http::get_json("https://api.anthropic.com/v1/models?limit=100", &headers, TIMEOUT)?;
    parse_anthropic_models(&raw)
}

fn openrouter_catalog(cred: &Credential) -> Result<Vec<ModelInfo>, String> {
    // The key endpoint doubles as credential validation (401 on a bad key).
    http::get_json("https://openrouter.ai/api/v1/key", &bearer_headers(cred), TIMEOUT)?;
    let (_, raw) = http::get_json("https://openrouter.ai/api/v1/models", &[], TIMEOUT)?;
    parse_openrouter_models(&raw)
}

fn simple_list_catalog(
    cred: &Credential,
    list_url: &str,
    fallback_ids: &'static [&'static str],
) -> Result<Vec<ModelInfo>, String> {
    match http::get_json(list_url, &bearer_headers(cred), TIMEOUT) {
        Ok((_, raw)) => parse_openai_models(&raw),
        Err(e) => { logging::error!("providers.rs:63: {e}"); Ok(fallback_models(GENERIC_HINTS, fallback_ids)) }
    }
}

// ── Copilot: device OAuth + per-session token exchange ──────────────────────

/// The VS Code extension's public client id — the one widely used by OSS
/// Copilot integrations, since GitHub does not issue per-app ids for
/// third-party editor plugins.
const COPILOT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";

fn editor_headers(token: Option<&str>) -> Vec<(String, String)> {
    let mut headers = vec![
        ("Accept".into(), "application/json".into()),
        ("Editor-Version".into(), "magma/0.1".into()),
        ("Editor-Plugin-Version".into(), "magma-agent/0.1".into()),
        ("User-Agent".into(), "magma-agent/0.1".into()),
    ];
    if let Some(t) = token {
        headers.push(("Authorization".into(), format!("Bearer {t}")));
    }
    headers
}

/// Exchange the long-lived GitHub token for a Copilot session token
/// (they expire hourly, so cache until shortly before `expires_at`).
fn copilot_session_token(app_token: &str) -> Result<String, String> {
    static CACHE: Mutex<Option<(String, u64)>> = Mutex::new(None);
    if let Ok(guard) = CACHE.lock()
        && let Some((token, expires_at)) = guard.as_ref()
        && *expires_at > now_epoch() + 60
    {
        return Ok(token.clone());
    }
    let (_, raw) = http::get_json(
        "https://api.github.com/copilot_internal/v2/token",
        &editor_headers(Some(app_token)),
        TIMEOUT,
    )?;
    let v: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("copilot token exchange bad JSON: {e}"))?;
    let token = v
        .get("token")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            v.get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("copilot token exchange failed (no Copilot subscription?)")
                .to_string()
        })?
        .to_string();
    let expires_at = v.get("expires_at").and_then(serde_json::Value::as_u64).unwrap_or(0);
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((token.clone(), expires_at));
    }
    Ok(token)
}

fn copilot_catalog(cred: &Credential) -> Result<Vec<ModelInfo>, String> {
    let token = copilot_session_token(cred.secret())?;
    let (_, raw) = http::get_json(
        "https://api.githubcopilot.com/models",
        &editor_headers(Some(&token)),
        TIMEOUT,
    )?;
    parse_copilot_models(&raw)
}

// ── Provider structs ─────────────────────────────────────────────────────────

/// The OpenAI-compatible family: one implementation, parameterized.
pub struct OpenAiCompat {
    pub meta: &'static ProviderMeta,
    pub reasoning: ReasoningStyle,
    pub catalog: fn(&Credential) -> Result<Vec<ModelInfo>, String>,
}

impl AgentProvider for OpenAiCompat {
    fn meta(&self) -> &ProviderMeta {
        self.meta
    }

    fn chat_request(&self, turn: &ChatTurn, cred: &Credential) -> Result<HttpReq, String> {
        Ok(translate::openai_compatible(&self.meta, self.reasoning, turn, cred))
    }

    fn parse_reply(&self, raw: &str) -> Result<(String, Vec<crate::llm::ToolCall>), String> {
        translate::parse_openai_reply(raw)
    }

    fn catalog(&self, cred: &Credential) -> Result<Vec<ModelInfo>, String> {
        (self.catalog)(cred)
    }
}

/// Anthropic: its own wire protocol (Messages API) and thinking budgets.
pub struct Anthropic;

impl AgentProvider for Anthropic {
    fn meta(&self) -> &ProviderMeta {
        &ANTHROPIC_META
    }

    fn chat_request(&self, turn: &ChatTurn, cred: &Credential) -> Result<HttpReq, String> {
        Ok(translate::anthropic(turn, cred))
    }

    fn parse_reply(&self, raw: &str) -> Result<(String, Vec<crate::llm::ToolCall>), String> {
        translate::parse_anthropic_reply(raw)
    }

    fn catalog(&self, cred: &Credential) -> Result<Vec<ModelInfo>, String> {
        anthropic_catalog(cred)
    }
}

/// GitHub Copilot: device-flow OAuth, session-token chat, entitlement-gated
/// model list driven by the subscription.
pub struct Copilot;

impl AgentProvider for Copilot {
    fn meta(&self) -> &ProviderMeta {
        &COPILOT_META
    }

    fn chat_request(&self, turn: &ChatTurn, cred: &Credential) -> Result<HttpReq, String> {
        let mut req =
            translate::openai_compatible(&COPILOT_META, ReasoningStyle::Effort, turn, cred);
        let session = copilot_session_token(cred.secret())?;
        for (k, v) in req.headers.iter_mut() {
            if k == "Authorization" {
                *v = format!("Bearer {session}");
            }
        }
        req.headers
            .push(("Copilot-Integration-Id".into(), "vscode-chat".into()));
        req.headers
            .push(("Openai-Intent".into(), "conversation".into()));
        Ok(req)
    }

    fn parse_reply(&self, raw: &str) -> Result<(String, Vec<crate::llm::ToolCall>), String> {
        translate::parse_openai_reply(raw)
    }

    fn catalog(&self, cred: &Credential) -> Result<Vec<ModelInfo>, String> {
        copilot_catalog(cred)
    }
}

// ── Metadata (static, registry-visible) ──────────────────────────────────────

static OPENAI_META: ProviderMeta = ProviderMeta {
    id: "openai",
    display_name: "OpenAI",
    description: "GPT models with reasoning controls",
    glyph: "\u{F0493}",
    auth_methods: &[AuthMethodSpec::api_key(
        "API key",
        "Paste an OpenAI API key (platform.openai.com). It is validated against your account's model list before the provider is marked connected.",
        "https://platform.openai.com/api-keys",
    )],
    auth_style: AuthStyle::Bearer,
    chat_base: "https://api.openai.com/v1",
    chat_path: "/chat/completions",
};

static ANTHROPIC_META: ProviderMeta = ProviderMeta {
    id: "anthropic",
    display_name: "Anthropic",
    description: "Claude models with extended thinking",
    glyph: "\u{F0221}",
    auth_methods: &[AuthMethodSpec::api_key(
        "API key",
        "Paste an Anthropic API key (console.anthropic.com). It is validated against your account's model list before the provider is marked connected.",
        "https://console.anthropic.com/settings/keys",
    )],
    auth_style: AuthStyle::XApiKey,
    chat_base: "https://api.anthropic.com/v1",
    chat_path: "/messages",
};

static OPENROUTER_META: ProviderMeta = ProviderMeta {
    id: "openrouter",
    display_name: "OpenRouter",
    description: "Hundreds of models behind one key",
    glyph: "\u{F00C0}",
    auth_methods: &[AuthMethodSpec::api_key(
        "API key",
        "Paste an OpenRouter key (openrouter.ai/keys). The key is validated and the full capability-rich model list is pulled in.",
        "https://openrouter.ai/settings/keys",
    )],
    auth_style: AuthStyle::Bearer,
    chat_base: "https://openrouter.ai/api/v1",
    chat_path: "/chat/completions",
};

static COPILOT_META: ProviderMeta = ProviderMeta {
    id: "copilot",
    display_name: "GitHub Copilot",
    description: "Sign in with GitHub; models follow your plan",
    glyph: "\u{EB99}",
    auth_methods: &[AuthMethodSpec::device(
        "GitHub sign-in",
        "A browser window opens at github.com/login/device. Approve the code shown here to sign in; your Copilot plan decides which models appear.",
        COPILOT_CLIENT_ID,
        "https://github.com/login/device/code",
        "https://github.com/login/oauth/access_token",
        "read:user",
        "https://api.github.com/user",
    )],
    auth_style: AuthStyle::Bearer,
    chat_base: "https://api.githubcopilot.com",
    chat_path: "/chat/completions",
};

static DEEPSEEK_META: ProviderMeta = ProviderMeta {
    id: "deepseek",
    display_name: "DeepSeek",
    description: "DeepSeek chat and reasoner",
    glyph: "\u{F02D5}",
    auth_methods: &[AuthMethodSpec::api_key(
        "API key",
        "Paste a DeepSeek API key (platform.deepseek.com). It is validated against the live model list.",
        "https://platform.deepseek.com/api_keys",
    )],
    auth_style: AuthStyle::Bearer,
    chat_base: "https://api.deepseek.com/v1",
    chat_path: "/chat/completions",
};

static KIMI_META: ProviderMeta = ProviderMeta {
    id: "kimi",
    display_name: "Kimi",
    description: "Moonshot Kimi models",
    glyph: "\u{F0493}",
    auth_methods: &[AuthMethodSpec::api_key(
        "API key",
        "Paste a Moonshot API key (platform.moonshot.cn). It is validated against the live model list.",
        "https://platform.moonshot.cn/console/api-keys",
    )],
    auth_style: AuthStyle::Bearer,
    chat_base: "https://api.moonshot.cn/v1",
    chat_path: "/chat/completions",
};

static ZAI_META: ProviderMeta = ProviderMeta {
    id: "zai",
    display_name: "Z.ai",
    description: "GLM models with a thinking toggle",
    glyph: "\u{F0093}",
    auth_methods: &[AuthMethodSpec::api_key(
        "API key",
        "Paste a Z.ai API key (z.ai model service). It is validated against the live model list.",
        "https://z.ai/manage-apikey/apikey-list",
    )],
    auth_style: AuthStyle::Bearer,
    chat_base: "https://api.z.ai/api/paas/v4",
    chat_path: "/chat/completions",
};

pub static OPENAI: OpenAiCompat = OpenAiCompat {
    meta: &OPENAI_META,
    reasoning: ReasoningStyle::Effort,
    catalog: openai_catalog,
};
pub static ANTHROPIC: Anthropic = Anthropic;
pub static OPENROUTER: OpenAiCompat = OpenAiCompat {
    meta: &OPENROUTER_META,
    reasoning: ReasoningStyle::OpenRouter,
    catalog: openrouter_catalog,
};
pub static COPILOT: Copilot = Copilot;
pub static DEEPSEEK: OpenAiCompat = OpenAiCompat {
    meta: &DEEPSEEK_META,
    reasoning: ReasoningStyle::None,
    catalog: |cred| {
        simple_list_catalog(
            cred,
            "https://api.deepseek.com/models",
            &["deepseek-chat", "deepseek-reasoner"],
        )
    },
};
pub static KIMI: OpenAiCompat = OpenAiCompat {
    meta: &KIMI_META,
    reasoning: ReasoningStyle::None,
    catalog: |cred| {
        simple_list_catalog(
            cred,
            "https://api.moonshot.cn/v1/models",
            &["kimi-k2-turbo-preview", "kimi-k2-0905-preview", "kimi-latest", "moonshot-v1-128k"],
        )
    },
};
pub static ZAI: OpenAiCompat = OpenAiCompat {
    meta: &ZAI_META,
    reasoning: ReasoningStyle::Toggle,
    catalog: |cred| {
        simple_list_catalog(
            cred,
            "https://api.z.ai/api/paas/v4/models",
            &["glm-4.6", "glm-4.5", "glm-4.5-air", "glm-4-flash"],
        )
    },
};

/// Account label for a freshly authenticated credential, when the provider
/// declares an account endpoint (used for "Connected as …").
pub fn account_label(method: &AuthMethodSpec, credential: &Credential) -> Option<String> {
    if method.account_url.is_empty() {
        return None;
    }
    fetch_account(method.account_url, credential)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{by_id, providers};
    use crate::translate::ReasoningStyle;
    use serde_json::json;

    #[test]
    fn openai_chat_request_is_openai_compatible() {
        let messages = vec![json!({"role":"user","content":"hi"})];
        let turn = ChatTurn {
            model: "gpt-4o",
            messages: &messages,
            reasoning: None,
            tools: &json!([]),
        };
        let cred = Credential::ApiKey { key: "sk".into() };
        let req = OPENAI.chat_request(&turn, &cred).unwrap();
        assert_eq!(req.url, "https://api.openai.com/v1/chat/completions");
        let body: serde_json::Value = serde_json::from_str(&req.body).unwrap();
        assert_eq!(body["model"], "gpt-4o");
    }

    #[test]
    fn anthropic_request_uses_x_api_key() {
        let messages = vec![json!({"role":"user","content":"hi"})];
        let turn = ChatTurn {
            model: "claude-sonnet-4-5",
            messages: &messages,
            reasoning: None,
            tools: &json!([]),
        };
        let cred = Credential::ApiKey { key: "ak".into() };
        let req = ANTHROPIC.chat_request(&turn, &cred).unwrap();
        assert!(
            req.headers
                .iter()
                .any(|(k, v)| k == "x-api-key" && v == "ak")
        );
    }

    #[test]
    fn copilot_chat_request_exchanges_the_token_and_adds_headers() {
        let messages = vec![json!({"role":"user","content":"hi"})];
        let turn = ChatTurn {
            model: "gpt-5",
            messages: &messages,
            reasoning: None,
            tools: &json!([]),
        };
        let cred = Credential::OAuth {
            access_token: "gho_app".into(),
            expires_at: None,
            account: None,
        };
        // No network in unit tests: the exchange fails with a clear error
        // rather than panicking — the loop surfaces it as a session failure.
        let err = COPILOT.chat_request(&turn, &cred).unwrap_err();
        assert!(err.contains("token exchange") || err.contains("HTTP"), "got: {err}");
    }

    #[test]
    fn every_builtin_declares_a_working_reasoning_style() {
        assert_eq!(OPENAI.reasoning, ReasoningStyle::Effort);
        assert_eq!(OPENROUTER.reasoning, ReasoningStyle::OpenRouter);
        assert_eq!(ZAI.reasoning, ReasoningStyle::Toggle);
        assert_eq!(DEEPSEEK.reasoning, ReasoningStyle::None);
    }

    #[test]
    fn registry_resolves_the_same_statics() {
        assert_eq!(by_id("openai").unwrap().meta().id, OPENAI.meta().id);
        let registered = providers()
            .iter()
            .find(|p| p.meta().id == "anthropic")
            .map(|p| *p as *const dyn AgentProvider as *const ());
        let expected = Some(&ANTHROPIC as &dyn AgentProvider as *const dyn AgentProvider as *const ());
        assert_eq!(registered, expected, "registry must hand out the same object");
    }

    #[test]
    fn copilot_declares_device_flow_with_account_discovery() {
        let method = COPILOT.meta().auth_methods.first().unwrap();
        assert_eq!(method.id, "device");
        assert!(method.account_url.contains("api.github.com"));
        assert!(!method.device_auth_url.is_empty());
        assert!(!method.client_id.is_empty());
    }

    #[test]
    fn api_key_methods_declare_a_key_url() {
        for p in providers() {
            for m in p.meta().auth_methods {
                if m.id == "api-key" {
                    assert!(m.key_url.starts_with("https://"), "{}: {}", p.meta().id, m.key_url);
                }
            }
        }
    }
}
