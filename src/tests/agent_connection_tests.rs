use kernel::agent::connections::{
begin_auth, cancel_auth, disconnect, note_auth_result, note_instruction, note_models,
resolve_chat, submit_api_key,
};
use kernel::agent::connections::{AgentConnections, PendingAuth};
use kernel::agent::{AuthInstruction, Credential};
use kernel::state::options::OptionKind;
use kernel::state::Editor;

fn editor() -> Editor {
let mut ed = Editor::new(Box::new(kernel::storage::disk::DiskFileSystem::new()));
kernel::command::builtin::register_builtin_commands(&mut ed);
ed
}

fn gpt5() -> crate::agent::ModelInfo {
    crate::agent::ModelInfo {
        id: "gpt-5".into(),
        display: "GPT-5".into(),
        context_window: Some(400_000),
        max_output: None,
        reasoning: ReasoningSupport::Levels(agent::catalog::effort_levels(true)),
        tools: true,
        image_input: true,
        note: String::new(),
    }
}

#[test]
fn begin_auth_opens_the_key_form_for_api_key_providers() {
    let mut ed = editor();
    begin_auth(&mut ed, "openai").unwrap();
    let pending = ed.agent_connections.pending.as_ref().expect("pending auth");
    assert_eq!(pending.provider, "openai");
    assert_eq!(pending.method, "api-key");
    assert!(!pending.busy, "api-key needs no background work yet");
    match &pending.instruction {
        AuthInstruction::EnterKey {
            instructions,
            key_url,
        } => {
            assert!(!instructions.is_empty());
            assert!(key_url.starts_with("https://"));
        }
        other => panic!("expected the key form, got {other:?}"),
    }
    cancel_auth(&mut ed);
    assert!(ed.agent_connections.pending.is_none());
}

#[test]
fn begin_auth_requires_a_background_runtime_for_device_flows() {
    let mut ed = editor();
    assert!(begin_auth(&mut ed, "copilot").is_err());
    assert!(ed.agent_connections.pending.is_none(), "no half-open flow");
}

#[test]
fn begin_auth_rejects_unknown_providers() {
    let mut ed = editor();
    assert!(begin_auth(&mut ed, "not-a-provider").is_err());
}

#[test]
fn submit_api_key_stores_the_credential_and_starts_validation() {
    let mut ed = editor();
    begin_auth(&mut ed, "openai").unwrap();
    submit_api_key(&mut ed, "  sk-test-123  ").unwrap();
    assert_eq!(
        ed.options.get("agent.api-key.openai").map(String::as_str),
        Some("sk-test-123"),
        "the trimmed key lands in the secret option"
    );
    assert_eq!(
        ed.options.get("agent.provider").map(String::as_str),
        Some("openai"),
        "the first connection becomes active"
    );
    let conn = ed.agent_connections.conns.get("openai").unwrap();
    assert_eq!(conn.status, ConnStatus::Connecting,
        "the catalog fetch decides validity; the state waits for it");
    assert!(ed.agent_connections.pending.is_none(), "the form closed");
}

#[test]
fn submit_api_key_rejects_empty_keys_and_keeps_the_flow_open() {
    let mut ed = editor();
    begin_auth(&mut ed, "openai").unwrap();
    assert!(submit_api_key(&mut ed, "   ").is_err());
    assert!(
        ed.agent_connections.pending.is_some(),
        "the user can retry from the same form"
    );
    assert!(!ed.options.contains_key("agent.api-key.openai"));
}

#[test]
fn a_successful_catalog_connects_and_selects_defaults() {
    let mut ed = editor();
    begin_auth(&mut ed, "openai").unwrap();
    submit_api_key(&mut ed, "sk-test").unwrap();

    note_models(&mut ed, "openai", vec![gpt5()], None);
    let conn = ed.agent_connections.conns.get("openai").unwrap();
    assert_eq!(conn.status, ConnStatus::Connected);
    assert!(!conn.fetching);
    assert_eq!(conn.models.len(), 1);
    assert_eq!(
        ed.options.get("agent.model").map(String::as_str),
        Some("gpt-5"),
        "the first model becomes the default selection"
    );
    assert_eq!(
        ed.options.get("agent.reasoning").map(String::as_str),
        Some("minimal"),
        "the model's first reasoning level is selected"
    );
}

#[test]
fn a_rejected_credential_is_removed_and_the_form_returns_with_the_error() {
    let mut ed = editor();
    begin_auth(&mut ed, "openai").unwrap();
    submit_api_key(&mut ed, "sk-bad").unwrap();

    note_models(
        &mut ed,
        "openai",
        Vec::new(),
        Some("HTTP 401: {\"error\":\"invalid key\"}".into()),
    );
    assert!(
        !ed.options.contains_key("agent.api-key.openai"),
        "the invalid key is dropped"
    );
    assert!(!ed.agent_connections.conns.contains_key("openai"));
    let pending = ed.agent_connections.pending.as_ref().expect("form reopened");
    assert_eq!(pending.provider, "openai");
    let error = pending.error.as_deref().expect("error surfaced");
    assert!(error.contains("rejected"), "got: {error}");
}

#[test]
fn a_network_failure_keeps_the_credential_and_marks_the_connection_failed() {
    let mut ed = editor();
    begin_auth(&mut ed, "openai").unwrap();
    submit_api_key(&mut ed, "sk-test").unwrap();

    note_models(
        &mut ed,
        "openai",
        Vec::new(),
        Some("HTTP 500: internal".into()),
    );
    assert_eq!(
        ed.options.get("agent.api-key.openai").map(String::as_str),
        Some("sk-test"),
        "provider downtime must not delete the user's key"
    );
    let conn = ed.agent_connections.conns.get("openai").unwrap();
    assert!(matches!(conn.status, ConnStatus::Failed(_)));
    assert_eq!(
        conn.models_error.as_deref(),
        Some("HTTP 500: internal"),
        "the error is visible in the composer"
    );
}

#[test]
fn device_auth_result_stores_the_token_and_the_account() {
    let mut ed = editor();
    begin_auth(&mut ed, "openai").unwrap();
    // Pretend the user picked a device-flow provider by rebuilding the
    // pending state the device task would leave behind.
    ed.agent_connections.pending = Some(PendingAuth {
        provider: "copilot".into(),
        method: "device".into(),
        instruction: AuthInstruction::DeviceCode {
            verification_url: "https://github.com/login/device".into(),
            user_code: "ABCD-1234".into(),
        },
        busy: true,
        cancel: Arc::new(AtomicBool::new(false)),
        error: None,
    });

    note_auth_result(
        &mut ed,
        "copilot",
        true,
        Some(Credential::OAuth {
            access_token: "gho_token".into(),
            expires_at: Some(1_000),
            account: Some("octocat".into()),
        }),
        None,
    );
    assert_eq!(
        ed.options.get("agent.oauth.copilot").map(String::as_str),
        Some("gho_token"),
        "the token lands under the oauth option"
    );
    assert_eq!(
        ed.options
            .get("agent.oauth-account.copilot")
            .map(String::as_str),
        Some("octocat")
    );
    let conn = ed.agent_connections.conns.get("copilot").unwrap();
    assert_eq!(conn.status, ConnStatus::Connected);
    assert_eq!(conn.account.as_deref(), Some("octocat"));
}

#[test]
fn a_failed_device_flow_keeps_the_panel_with_the_error() {
    let mut ed = editor();
    ed.agent_connections.pending = Some(PendingAuth {
        provider: "copilot".into(),
        method: "device".into(),
        instruction: AuthInstruction::Waiting {
            message: "polling".into(),
        },
        busy: true,
        cancel: Arc::new(AtomicBool::new(false)),
        error: None,
    });
    note_auth_result(
        &mut ed,
        "copilot",
        false,
        None,
        Some("sign-in was denied in the browser".into()),
    );
    let pending = ed.agent_connections.pending.as_ref().expect("panel kept");
    assert_eq!(
        pending.error.as_deref(),
        Some("sign-in was denied in the browser")
    );
    assert!(!pending.busy, "the flow is no longer running");
    assert!(!ed.agent_connections.conns.contains_key("copilot"));
}

#[test]
fn disconnect_removes_credentials_state_and_selection() {
    let mut ed = editor();
    begin_auth(&mut ed, "openai").unwrap();
    submit_api_key(&mut ed, "sk-test").unwrap();
    note_models(&mut ed, "openai", vec![gpt5()], None);
    assert!(ed.agent_connections.any_connected());

    disconnect(&mut ed, "openai");
    assert!(!ed.options.contains_key("agent.api-key.openai"));
    assert!(!ed.agent_connections.conns.contains_key("openai"));
    assert!(
        ed.options.get("agent.provider").map(String::as_str) != Some("openai"),
        "the selection does not point at a ghost"
    );
    assert!(!ed.agent_connections.any_connected());
}

#[test]
fn resolve_chat_demands_a_connected_provider_with_models() {
    let mut ed = editor();
    assert!(
        resolve_chat(&ed).is_err(),
        "no connection → no chat config (the loop falls back to the planner)"
    );

    begin_auth(&mut ed, "openai").unwrap();
    submit_api_key(&mut ed, "sk-test").unwrap();
    assert!(
        resolve_chat(&ed).is_err(),
        "connecting without models is not usable yet"
    );

    note_models(&mut ed, "openai", vec![gpt5()], None);
    let chat = resolve_chat(&ed).expect("usable now");
    assert_eq!(chat.provider.meta().id, "openai");
    assert_eq!(chat.model.id, "gpt-5");
    assert_eq!(chat.reasoning.expect("gpt-5 reasons").id, "minimal");
    assert_eq!(chat.credential.secret(), "sk-test");
}

#[test]
fn note_instruction_only_touches_the_matching_flow() {
    let mut ed = editor();
    ed.agent_connections.pending = Some(PendingAuth {
        provider: "copilot".into(),
        method: "device".into(),
        instruction: AuthInstruction::Waiting {
            message: "contacting".into(),
        },
        busy: true,
        cancel: Arc::new(AtomicBool::new(false)),
        error: None,
    });
    note_instruction(
        &mut ed,
        "openai",
        AuthInstruction::EnterKey {
            instructions: "x".into(),
            key_url: "y".into(),
        },
    );
    assert!(
        matches!(
            &ed.agent_connections.pending.as_ref().unwrap().instruction,
            AuthInstruction::Waiting { .. }
        ),
        "a foreign provider's instruction is ignored"
    );
    note_instruction(
        &mut ed,
        "copilot",
        AuthInstruction::DeviceCode {
            verification_url: "https://github.com/login/device".into(),
            user_code: "ABCD-1234".into(),
        },
    );
    assert!(matches!(
        &ed.agent_connections.pending.as_ref().unwrap().instruction,
        AuthInstruction::DeviceCode { .. }
    ));
}

#[test]
fn agent_provider_option_is_a_registry_backed_str() {
    let spec = crate::state::options::OPTION_SPECS
        .iter()
        .find(|s| s.name == "agent.provider")
        .expect("spec exists");
    assert_eq!(spec.kind, OptionKind::Str, "choices come from the registry");
}
