//! Agent panel projections for the connection layer: provider cards, auth
//! flow state, and the composer's capability-driven controls.

use kernel::agent::AuthInstruction;
use kernel::state::Editor;

use crate::{AuthPanelView, PickerChoice, ProviderCard};

// ── Onboarding / connections ─────────────────────────────────────────────────

/// Provider cards from the registry, annotated with connection state.
pub fn provider_cards(ed: &Editor) -> Vec<ProviderCard> {
    kernel::agent::registry::providers()
        .iter()
        .map(|p| {
            let meta = p.meta();
            let conn = ed.agent_connections.conns.get(meta.id);
            let (status, account) = match conn {
                Some(c) => (
                    c.status.as_str().to_string(),
                    c.account.clone().unwrap_or_default(),
                ),
                None => (String::new(), String::new()),
            };
            ProviderCard {
                id: meta.id.into(),
                name: meta.display_name.into(),
                description: meta.description.into(),
                glyph: meta.glyph.into(),
                logo: provider_logo(meta.id),
                auth_label: meta
                    .auth_methods
                    .first()
                    .map(|m| m.label)
                    .unwrap_or("")
                    .into(),
                status: status.into(),
                account: account.into(),
            }
        })
        .collect()
}

/// Embedded provider logo, loaded from the bundled SVG assets.  Falls back
/// to an empty image when a provider has no asset, so the UI keeps showing
/// its Nerd Font glyph.
fn provider_logo(id: &str) -> slint::Image {
    let bytes: &[u8] = match id {
        "openai" => include_bytes!("../../ui/assets/providers/openai.svg"),
        "anthropic" => include_bytes!("../../ui/assets/providers/anthropic.svg"),
        "openrouter" => include_bytes!("../../ui/assets/providers/openrouter.svg"),
        "copilot" => include_bytes!("../../ui/assets/providers/githubcopilot.svg"),
        "deepseek" => include_bytes!("../../ui/assets/providers/deepseek.svg"),
        "kimi" => include_bytes!("../../ui/assets/providers/kimi.svg"),
        "zai" => include_bytes!("../../ui/assets/providers/zai.svg"),
        _ => return slint::Image::default(),
    };
    slint::Image::load_from_svg_data(bytes).unwrap_or_default()
}

/// The auth flow on screen, if any.
pub fn auth_panel(ed: &Editor) -> (bool, AuthPanelView) {
    let Some(pending) = &ed.agent_connections.pending else {
        return (
            false,
            AuthPanelView {
                provider_id: "".into(),
                provider_name: "".into(),
                provider_glyph: "".into(),
                provider_logo: slint::Image::default(),
                method_label: "".into(),
                instruction: "".into(),
                instructions: "".into(),
                key_url: "".into(),
                verification_url: "".into(),
                user_code: "".into(),
                message: "".into(),
                error: "".into(),
                busy: false,
            },
        );
    };
    let meta = kernel::agent::registry::by_id(&pending.provider)
        .map(|p| p.meta())
        .unwrap_or_else(|| unreachable_default_meta());
    let method = meta
        .auth_methods
        .iter()
        .find(|m| m.id == pending.method);
    let (instruction, instructions, key_url, verification_url, user_code, message) =
        match &pending.instruction {
            AuthInstruction::EnterKey {
                instructions,
                key_url,
            } => (
                "enter-key",
                instructions.clone(),
                key_url.clone(),
                String::new(),
                String::new(),
                String::new(),
            ),
            AuthInstruction::DeviceCode {
                verification_url,
                user_code,
            } => (
                "device-code",
                method.map(|m| m.instructions.to_string()).unwrap_or_default(),
                String::new(),
                verification_url.clone(),
                user_code.clone(),
                String::new(),
            ),
            AuthInstruction::OpenBrowser { url } => (
                "waiting",
                String::new(),
                String::new(),
                url.clone(),
                String::new(),
                "Finish in the browser".to_string(),
            ),
            AuthInstruction::Waiting { message } => (
                "waiting",
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                message.clone(),
            ),
        };
    let panel = AuthPanelView {
        provider_id: pending.provider.clone().into(),
        provider_name: meta.display_name.into(),
        provider_glyph: meta.glyph.into(),
        provider_logo: provider_logo(&pending.provider),
        method_label: method.map(|m| m.label).unwrap_or("").into(),
        instruction: instruction.into(),
        instructions: instructions.into(),
        key_url: key_url.into(),
        verification_url: verification_url.into(),
        user_code: user_code.into(),
        message: message.into(),
        error: pending.error.clone().unwrap_or_default().into(),
        busy: pending.busy,
    };
    (true, panel)
}

fn unreachable_default_meta() -> &'static kernel::agent::registry::ProviderMeta {
    // The pending auth always references a registry provider; this only
    // silences the type checker if a provider was unregistered mid-flow.
    kernel::agent::registry::by_id("openai")
        .map(|p| p.meta())
        .expect("builtin provider present")
}

// ── Composer controls ────────────────────────────────────────────────────────

/// Connected providers as toolbar choices.
pub fn provider_choices(ed: &Editor) -> Vec<PickerChoice> {
    let mut choices: Vec<PickerChoice> = ed
        .agent_connections
        .connected_ids()
        .iter()
        .filter_map(|id| {
            kernel::agent::registry::by_id(id).map(|p| PickerChoice {
                id: p.meta().id.into(),
                label: p.meta().display_name.into(),
                hint: ed
                    .agent_connections
                    .conns
                    .get(id.as_str())
                    .and_then(|c| c.account.clone())
                    .unwrap_or_default()
                    .into(),
            })
        })
        .collect();
    // Keep the active selection first.
    if let Some(active) = ed.options.get("agent.provider") {
        if let Some(pos) = choices.iter().position(|c| c.id.as_str() == active) {
            let chosen = choices.remove(pos);
            choices.insert(0, chosen);
        }
    }
    choices
}

/// The active provider chip.
pub fn active_provider_choice(ed: &Editor) -> PickerChoice {
    provider_choices(ed).first().cloned().unwrap_or(PickerChoice {
        id: "".into(),
        label: "No provider".into(),
        hint: "".into(),
    })
}

/// Models of the active connection as toolbar choices with capability hints.
pub fn model_choices(ed: &Editor, search: &str) -> Vec<PickerChoice> {
    let Some((_, conn)) = ed.agent_connections.active(&ed.options) else {
        return Vec::new();
    };
    let needle = search.trim().to_lowercase();
    conn.models
        .iter()
        .filter(|m| {
            if needle.is_empty() {
                return true;
            }
            m.label().to_lowercase().contains(&needle)
                || m.id.to_lowercase().contains(&needle)
        })
        .map(|m| {
            let mut hints: Vec<String> = Vec::new();
            if !m.reasoning.levels().is_empty() {
                let names: Vec<&str> = m.reasoning.levels().iter().map(|l| l.label.as_str()).collect();
                hints.push(format!("reasoning: {}", names.join("/").to_lowercase()));
            }
            if m.tools {
                hints.push("tools".to_string());
            }
            if m.image_input {
                hints.push("images".to_string());
            }
            PickerChoice {
                id: m.id.clone().into(),
                label: m.label().into(),
                hint: hints.join(" · ").into(),
            }
        })
        .collect()
}

/// The active model chip.
pub fn active_model_choice(ed: &Editor) -> PickerChoice {
    match kernel::agent::connections::selected_model_of(ed) {
        Some(m) => PickerChoice {
            id: m.id.clone().into(),
            label: m.label().into(),
            hint: "".into(),
        },
        None => PickerChoice {
            id: "".into(),
            label: "No models".into(),
            hint: "".into(),
        },
    }
}

/// Reasoning levels of the active model.
pub fn reasoning_choices(ed: &Editor) -> Vec<PickerChoice> {
    match kernel::agent::connections::selected_model_of(ed) {
        Some(m) => m
            .reasoning
            .levels()
            .iter()
            .map(|l| PickerChoice {
                id: l.label.clone().into(),
                label: l.label.clone().into(),
                hint: "".into(),
            })
            .collect(),
        None => Vec::new(),
    }
}

pub fn reasoning_supported(ed: &Editor) -> bool {
    kernel::agent::connections::selected_model_of(ed)
        .map(|m| !m.reasoning.levels().is_empty())
        .unwrap_or(false)
}

/// The selected reasoning level's display label.
pub fn reasoning_label(ed: &Editor) -> String {
    kernel::agent::connections::selected_reasoning_of(ed)
        .map(|l| l.label.clone())
        .unwrap_or_default()
}

/// Why the composer cannot send, shown in the placeholder.
pub fn send_blocked_reason(ed: &Editor) -> String {
    match ed.agent_connections.active(&ed.options) {
        None => "Connect a provider to start".to_string(),
        Some((_, conn)) if !conn.usable() => {
            "Models are still loading for this provider…".to_string()
        }
        _ => String::new(),
    }
}

/// Attachment labels for the composer's context strip.
pub fn attachment_labels(state: &crate::AppState) -> Vec<String> {
    state
        .agent_attachments
        .iter()
        .map(|a| a.label.clone())
        .collect()
}
