//! Connection flows: authentication lifecycles, catalog fetches, and the
//! `BackgroundEvent` handlers that land their results.
//!
//! Every network step runs on the background pool; these functions only
//! stage state and spawn work.  The UI observes progress through the
//! connection state this module mutates.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use agent::registry;
use agent::AuthInstruction;

use crate::state::Editor;

use super::{
    credential_of, selected_reasoning, secret_option, ConnStatus, Credential,
    ModelInfo, PendingAuth, OAUTH_ACCOUNT_PREFIX,
};

// ── Flows (foreground orchestration + background work) ───────────────────────

/// Open the auth experience for a provider.  API-key methods show the entry
/// form immediately; device methods start polling on the pool.
pub fn begin_auth(ed: &mut Editor, provider_id: &str) -> Result<(), String> {
    let provider =
        registry::by_id(provider_id).ok_or_else(|| format!("unknown provider {provider_id}"))?;
    let method = provider
        .meta()
        .auth_methods
        .first()
        .ok_or_else(|| format!("{} declares no authentication method", provider_id))?
        .clone();
    let instruction = match method.id {
        "api-key" => AuthInstruction::EnterKey {
            instructions: method.instructions.to_string(),
            key_url: method.key_url.to_string(),
        },
        "device" => AuthInstruction::Waiting {
            message: "Contacting the provider…".into(),
        },
        other => return Err(format!("unsupported authentication method {other}")),
    };
    if method.id == "device" {
        let cancel = Arc::new(AtomicBool::new(false));
        start_device_flow(ed, provider_id, &method, cancel.clone())?;
        ed.agent_connections.pending = Some(PendingAuth {
            provider: provider_id.to_string(),
            method: method.id.to_string(),
            instruction,
            busy: true,
            cancel,
            error: None,
        });
    } else {
        ed.agent_connections.pending = Some(PendingAuth {
            provider: provider_id.to_string(),
            method: method.id.to_string(),
            instruction,
            busy: false,
            cancel: Arc::new(AtomicBool::new(false)),
            error: None,
        });
    }
    Ok(())
}

fn start_device_flow(
    ed: &mut Editor,
    provider_id: &str,
    method: &agent::AuthMethodSpec,
    cancel: Arc<AtomicBool>,
) -> Result<(), String> {
    let Some(bg) = ed.background.clone() else {
        return Err("agent: no background runtime available".to_string());
    };
    let method = method.clone();
    let provider_id = provider_id.to_string();
    let sender = bg.sender.clone();
    bg.spawn_blocking(move || {
        let grant = match agent::auth::request_device_code(&method) {
            Ok(g) => g,
            Err(e) => {
                logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::AgentAuthResult {
                    provider: provider_id,
                    ok: false,
                    credential: None,
                    error: Some(e),
                }));
                return;
            }
        };
        logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::AgentAuthInstruction {
            provider: provider_id.clone(),
            instruction: AuthInstruction::DeviceCode {
                verification_url: grant.verification_url.clone(),
                user_code: grant.user_code.clone(),
            },
        }));
        match agent::auth::poll_device_token(&method, &grant, &cancel) {
            agent::AuthOutcome::Authenticated { credential } => {
                let credential = match &credential {
                    agent::Credential::OAuth {
                        access_token,
                        expires_at,
                        ..
                    } => {
                        let account = agent::providers::account_label(&method, &credential);
                        agent::Credential::OAuth {
                            access_token: access_token.clone(),
                            expires_at: *expires_at,
                            account,
                        }
                    }
                    other => other.clone(),
                };
                logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::AgentAuthResult {
                    provider: provider_id,
                    ok: true,
                    credential: Some(credential),
                    error: None,
                }));
            }
            agent::AuthOutcome::Cancelled => {}
            agent::AuthOutcome::Failed(e) => {
                logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::AgentAuthResult {
                    provider: provider_id,
                    ok: false,
                    credential: None,
                    error: Some(e),
                }));
            }
        }
    });
    Ok(())
}

/// The user submitted an API key: store it, then let the catalog fetch
/// validate it against the real account.
pub fn submit_api_key(ed: &mut Editor, key: &str) -> Result<(), String> {
    let credential = agent::auth::api_key_credential(key)?;
    let Some(pending) = ed.agent_connections.pending.take() else {
        return Err("no provider is waiting for a key".to_string());
    };
    let option_key = secret_option("api-key", &pending.provider);
    ed.options.insert(option_key.clone(), credential.secret().to_string());
    ed.agent_connections.sync_from_options(&ed.options);
    if let Some(conn) = ed.agent_connections.conns.get_mut(&pending.provider) {
        conn.status = ConnStatus::Connecting;
    }
    crate::state::secrets::save_key(&option_key, credential.secret());
    persist_active_provider(ed, &pending.provider);
    // The catalog fetch validates the key against the real account; if the
    // pool is unavailable now, `ensure_catalogs` retries when the panel opens.
    logging::LogErr::log_err(fetch_models(ed, &pending.provider));
    Ok(())
}

/// Cancel an in-flight auth flow.
pub fn cancel_auth(ed: &mut Editor) {
    if let Some(pending) = ed.agent_connections.pending.take() {
        pending.cancel.store(true, Ordering::Relaxed);
    }
}

/// Drop a provider: credential options, stored secret, connection state.
pub fn disconnect(ed: &mut Editor, provider_id: &str) {
    for method in ["api-key", "device"] {
        let option_key = secret_option(method, provider_id);
        if ed.options.remove(&option_key).is_some() {
            crate::state::secrets::delete_key(&option_key);
        }
    }
    ed.options.remove(&format!("{OAUTH_ACCOUNT_PREFIX}{provider_id}"));
    ed.agent_connections.conns.remove(provider_id);
    ed.agent_connections.sync_from_options(&ed.options);
    if ed.options.get("agent.provider").map(String::as_str) == Some(provider_id) {
        ed.options.remove("agent.provider");
        ed.options.remove("agent.model");
        ed.options.remove("agent.reasoning");
    }
}

/// Fetch (or refresh) a provider's model catalog on the pool.
pub fn fetch_models(ed: &mut Editor, provider_id: &str) -> Result<(), String> {
    let Some(bg) = ed.background.clone() else {
        return Err("agent: no background runtime available".to_string());
    };
    let Some((provider, credential)) = registry::by_id(provider_id)
        .zip(credential_of(&ed.options, provider_id).map(|(_, c)| c))
    else {
        return Err(format!("agent: no credential for {provider_id}"));
    };
    if let Some(conn) = ed.agent_connections.conns.get_mut(provider_id) {
        conn.fetching = true;
        conn.models_error = None;
    }
    let sender = bg.sender.clone();
    let pid = provider_id.to_string();
    bg.spawn_blocking(move || match provider.catalog(&credential) {
        Ok(models) => {
            logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::AgentModels {
                provider: pid,
                models,
                error: None,
            }));
        }
        Err(e) => {
            logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::AgentModels {
                provider: pid,
                models: Vec::new(),
                error: Some(e),
            }));
        }
    });
    Ok(())
}

/// Kick catalog fetches for connections that lack models (boot restore,
/// first dock open).  Idempotent: the `fetching` flag guards duplicates.
pub fn ensure_catalogs(ed: &mut Editor) {
    let ids: Vec<String> = ed
        .agent_connections
        .conns
        .iter()
        .filter(|(_, c)| c.models.is_empty() && !c.fetching && c.status != ConnStatus::Connecting)
        .map(|(id, _)| id.clone())
        .collect();
    for id in ids {
        logging::LogErr::log_err(fetch_models(ed, &id));
    }
}

// ── BackgroundEvent handlers (called from runtime/handlers/agent.rs) ────────

pub fn note_instruction(ed: &mut Editor, provider: &str, instruction: AuthInstruction) {
    if let Some(pending) = &mut ed.agent_connections.pending
        && pending.provider == provider
    {
        pending.instruction = instruction;
    }
}

pub fn note_auth_result(
    ed: &mut Editor,
    provider: &str,
    ok: bool,
    credential: Option<Credential>,
    error: Option<String>,
) {
    let Some(pending) = ed.agent_connections.pending.take() else {
        return;
    };
    if pending.provider != provider {
        ed.agent_connections.pending = Some(pending);
        return;
    }
    if ok {
        let Some(credential) = credential else {
            return;
        };
        let option_key = secret_option(&pending.method, provider);
        ed.options.insert(option_key.clone(), credential.secret().to_string());
        if let agent::Credential::OAuth { account: Some(account), .. } = &credential {
            ed.options
                .insert(format!("{OAUTH_ACCOUNT_PREFIX}{provider}"), account.clone());
        }
        crate::state::secrets::save_key(&option_key, credential.secret());
        ed.agent_connections.sync_from_options(&ed.options);
        if let Some(conn) = ed.agent_connections.conns.get_mut(provider) {
            conn.status = ConnStatus::Connected;
            if let agent::Credential::OAuth { account, .. } = &credential {
                conn.account = account.clone();
            }
        }
        persist_active_provider(ed, provider);
        logging::LogErr::log_err(fetch_models(ed, provider));
    } else if let Some(err) = error {
        if let Some(conn) = ed.agent_connections.conns.get_mut(provider) {
            conn.status = ConnStatus::Failed(err.clone());
        }
        // Keep the flow on screen with the failure, one back-click away.
        ed.agent_connections.pending = Some(PendingAuth {
            error: Some(err),
            busy: false,
            ..pending
        });
    }
}

pub fn note_models(
    ed: &mut Editor,
    provider: &str,
    models: Vec<ModelInfo>,
    error: Option<String>,
) {
    let Some(conn) = ed.agent_connections.conns.get_mut(provider) else {
        return;
    };
    conn.fetching = false;
    match error {
        None => {
            let fresh = conn.models.is_empty();
            conn.models = models;
            conn.models_error = None;
            if conn.status != ConnStatus::Connected {
                conn.status = ConnStatus::Connected;
            }
            if fresh {
                // First catalog: give the user a sensible default model.
                if let Some(first) = conn.models.first() {
                    let current = ed.options.get("agent.model");
                    if current.map(|m| !conn.models.iter().any(|m2| m2.id == *m)).unwrap_or(true) {
                        ed.options.insert("agent.model".into(), first.id.clone());
                        if let Some(level) = selected_reasoning(first, &ed.options) {
                            ed.options
                                .insert("agent.reasoning".into(), level.id);
                        }
                    }
                }
            }
        }
        Some(err) => {
            let status = agent::http::status_of_error(&err);
            if status == Some(401) || status == Some(403) {
                // The credential is bad: drop it and return the user to the
                // key-entry form with the rejection on screen.
                for method in ["api-key", "device"] {
                    let option_key = secret_option(method, provider);
                    ed.options.remove(&option_key);
                    crate::state::secrets::delete_key(&option_key);
                }
                ed.agent_connections.sync_from_options(&ed.options);
                if let Some(p) = registry::by_id(provider)
                    && let Some(method) = p.meta().auth_methods.first()
                {
                    ed.agent_connections.pending = Some(PendingAuth {
                        provider: provider.to_string(),
                        method: method.id.to_string(),
                        instruction: AuthInstruction::EnterKey {
                            instructions: method.instructions.to_string(),
                            key_url: method.key_url.to_string(),
                        },
                        busy: false,
                        cancel: Arc::new(AtomicBool::new(false)),
                        error: Some(format!(
                            "{} rejected this credential: {err}",
                            p.meta().display_name
                        )),
                    });
                }
            } else {
                conn.models_error = Some(err.clone());
                conn.status = ConnStatus::Failed(format!("model discovery failed: {err}"));
            }
        }
    }
}

fn persist_active_provider(ed: &mut Editor, provider: &str) {
    let unset = ed
        .options
        .get("agent.provider")
        .map(|p| !ed.agent_connections.conns.contains_key(p))
        .unwrap_or(true);
    if unset {
        ed.options.insert("agent.provider".into(), provider.to_string());
    }
}

/// User-facing summary for the message line after state changes.
pub fn describe(ed: &Editor) -> String {
    match ed.agent_connections.active(&ed.options) {
        Some((provider, conn)) => {
            let who = conn.account.as_deref().unwrap_or(provider.meta().display_name);
            format!("connected to {who}")
        }
        None => "no provider connected".to_string(),
    }
}
