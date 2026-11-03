//! Authentication flows, declared by provider capabilities.
//!
//! Two mechanisms cover the space: API keys the user pastes (validated
//! against the provider by a catalog fetch before the provider counts as
//! connected), and the OAuth 2.0 device-authorization grant (RFC 8628) for
//! browser sign-in.  All flows run on background threads and report progress
//! through `AuthInstruction` values; the UI never blocks.

use serde_json::{Value, json};

use crate::http;
use crate::registry::AuthMethodSpec;

/// How a provider knows us.  Stored only in the OS keychain, never in files
/// or logs.  `Debug` is redacted so credentials can cross channels and
/// panic messages without leaking.
#[derive(Clone, PartialEq, Eq)]
pub enum Credential {
    ApiKey { key: String },
    OAuth {
        access_token: String,
        expires_at: Option<u64>,
        account: Option<String>,
    },
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Credential::ApiKey { .. } => f.write_str("ApiKey { key: \"***\" }"),
            Credential::OAuth { expires_at, account, .. } => f
                .debug_struct("OAuth")
                .field("access_token", &"***")
                .field("expires_at", expires_at)
                .field("account", account)
                .finish(),
        }
    }
}

impl Credential {
    /// The secret half, for header construction.  Never logged.
    pub fn secret(&self) -> &str {
        match self {
            Credential::ApiKey { key } => key,
            Credential::OAuth { access_token, .. } => access_token,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Credential::ApiKey { .. } => "api-key",
            Credential::OAuth { .. } => "oauth",
        }
    }
}

/// What the UI should show while a flow is in flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthInstruction {
    /// Present the secure key-entry form.
    EnterKey { instructions: String, key_url: String },
    /// Open this URL in the system browser.
    OpenBrowser { url: String },
    /// Show the device code and verification URL, then poll.
    DeviceCode {
        verification_url: String,
        user_code: String,
    },
    /// Plain status line (polling, exchanging tokens, …).
    Waiting { message: String },
}

/// How a flow ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthOutcome {
    Authenticated { credential: Credential },
    Failed(String),
    Cancelled,
}

/// One round of an API-key flow: wrap the pasted key.  Validation against
/// the provider happens in the catalog fetch, not here.
pub fn api_key_credential(key: &str) -> Result<Credential, String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("the key field is empty".to_string());
    }
    Ok(Credential::ApiKey {
        key: trimmed.to_string(),
    })
}

// ── Device-authorization grant (RFC 8628) ────────────────────────────────────

pub struct DeviceGrant {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    pub interval_secs: u64,
    pub expires_at: std::time::Instant,
}

/// Step 1: ask the provider for a device + user code pair.
pub fn request_device_code(spec: &AuthMethodSpec) -> Result<DeviceGrant, String> {
    let body = json!({
        "client_id": spec.client_id,
        "scope": spec.scopes,
    });
    let (_, raw) = http::post_json(
        spec.device_auth_url,
        &[("User-Agent".into(), "magma-agent".into())],
        &body.to_string(),
        std::time::Duration::from_secs(30),
    )?;
    let v: Value =
        serde_json::from_str(&raw).map_err(|e| format!("device-code response bad JSON: {e}"))?;
    if let Some(err) = v.get("error").and_then(Value::as_str) {
        return Err(format!("device-code request failed: {err}"));
    }
    let device_code = v
        .get("device_code")
        .and_then(Value::as_str)
        .ok_or("device-code response missing device_code")?
        .to_string();
    let user_code = v
        .get("user_code")
        .and_then(Value::as_str)
        .ok_or("device-code response missing user_code")?
        .to_string();
    let verification_url = v
        .get("verification_uri")
        .or_else(|| v.get("verification_url"))
        .and_then(Value::as_str)
        .ok_or("device-code response missing verification_uri")?
        .to_string();
    let interval_secs = v.get("interval").and_then(Value::as_u64).unwrap_or(5).max(1);
    let expires_in = v.get("expires_in").and_then(Value::as_u64).unwrap_or(900);
    Ok(DeviceGrant {
        device_code,
        user_code,
        verification_url,
        interval_secs,
        expires_at: std::time::Instant::now()
            + std::time::Duration::from_secs(expires_in),
    })
}

/// Step 2: poll the token endpoint until the user approves, denies, the code
/// expires, or `cancel` flips.  Honors `interval` and `slow_down`.
pub fn poll_device_token(
    spec: &AuthMethodSpec,
    grant: &DeviceGrant,
    cancel: &std::sync::atomic::AtomicBool,
) -> AuthOutcome {
    use std::sync::atomic::Ordering;
    let mut interval = grant.interval_secs;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return AuthOutcome::Cancelled;
        }
        if std::time::Instant::now() >= grant.expires_at {
            return AuthOutcome::Failed("the sign-in code expired — start again".into());
        }
        // The server-mandated interval elapses in small slices so a cancel
        // request lands within a quarter second, not after a 30s sleep.
        let mut remaining = std::time::Duration::from_secs(interval);
        while !remaining.is_zero() {
            if cancel.load(Ordering::Relaxed) {
                return AuthOutcome::Cancelled;
            }
            let step = remaining.min(std::time::Duration::from_millis(250));
            std::thread::sleep(step);
            remaining -= step;
        }
        if cancel.load(Ordering::Relaxed) {
            return AuthOutcome::Cancelled;
        }
        let body = json!({
            "client_id": spec.client_id,
            "device_code": grant.device_code,
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
        });
        let raw = match http::post_json(
            spec.device_token_url,
            &[("User-Agent".into(), "magma-agent".into())],
            &body.to_string(),
            std::time::Duration::from_secs(30),
        ) {
            Ok((_, raw)) => raw,
            Err(e) => return AuthOutcome::Failed(format!("sign-in poll failed: {e}")),
        };
        let v: Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => return AuthOutcome::Failed(format!("sign-in response bad JSON: {e}")),
        };
        if let Some(token) = v.get("access_token").and_then(Value::as_str) {
            let expires_at = v
                .get("expires_in")
                .and_then(Value::as_u64)
                .map(|s| now_epoch() + s);
            return AuthOutcome::Authenticated {
                credential: Credential::OAuth {
                    access_token: token.to_string(),
                    expires_at,
                    account: None,
                },
            };
        }
        match v.get("error").and_then(Value::as_str) {
            Some("authorization_pending") => {}
            Some("slow_down") => interval += 5,
            Some("expired_token") => {
                return AuthOutcome::Failed("the sign-in code expired — start again".into());
            }
            Some("access_denied") => {
                return AuthOutcome::Failed("sign-in was denied in the browser".into());
            }
            Some(other) => return AuthOutcome::Failed(format!("sign-in failed: {other}")),
            None => return AuthOutcome::Failed("sign-in response missing access_token".into()),
        }
    }
}

/// Optional final step for providers that declare an account endpoint:
/// label the connection with the signed-in user.
pub fn fetch_account(account_url: &str, credential: &Credential) -> Option<String> {
    let headers = vec![
        (
            "Authorization".to_string(),
            format!("Bearer {}", credential.secret()),
        ),
        ("User-Agent".to_string(), "magma-agent".to_string()),
    ];
    let (_, raw) = http::get_json(
        account_url,
        &headers,
        std::time::Duration::from_secs(20),
    )
    .ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    ["login", "email", "name", "preferred_username"]
        .iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str).map(String::from))
}

pub fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_api_key_is_rejected() {
        assert!(api_key_credential("  ").is_err());
        assert!(api_key_credential("sk-abc").is_ok());
    }

    #[test]
    fn credential_kind_and_secret_round_trip() {
        let cred = api_key_credential(" sk-live-1 ").unwrap();
        assert_eq!(cred.kind(), "api-key");
        assert_eq!(cred.secret(), "sk-live-1");
        let oauth = Credential::OAuth {
            access_token: "gho_1".into(),
            expires_at: Some(42),
            account: Some("octocat".into()),
        };
        assert_eq!(oauth.kind(), "oauth");
        assert_eq!(oauth.secret(), "gho_1");
    }

    #[test]
    fn debug_representation_never_contains_the_secret() {
        let cred = api_key_credential("sk-super-secret").unwrap();
        let rendered = format!("{cred:?}");
        assert!(!rendered.contains("sk-super-secret"), "got: {rendered}");
        let oauth = Credential::OAuth {
            access_token: "gho-super-secret".into(),
            expires_at: None,
            account: Some("octo".into()),
        };
        let rendered = format!("{oauth:?}");
        assert!(!rendered.contains("gho-super-secret"), "got: {rendered}");
        assert!(rendered.contains("octo"), "account label is not secret");
    }

    // ── Device flow against a local mock server ─────────────────────────

    /// Minimal HTTP/1.1 server answering one JSON body per connection.
    struct MockServer {
        addr: std::net::SocketAddr,
    }

    impl MockServer {
        fn start(responses: Vec<(String, u16, String)>) -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            std::thread::spawn(move || {
                for (expect_path, status, body) in responses {
                    let (stream, _) = listener.accept().unwrap();
                    // Drain the whole request before responding: a short
                    // read followed by close would RST the socket mid-parse
                    // on the client (EINVAL).
                    let mut req = String::new();
                    let mut buf = [0u8; 4096];
                    loop {
                        let n = std::io::Read::read(&mut &stream, &mut buf).unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        req.push_str(&String::from_utf8_lossy(&buf[..n]));
                        if let Some(headers_end) = req.find("\r\n\r\n") {
                            let declared: usize = req
                                .lines()
                                .find_map(|l| {
                                    l.to_lowercase()
                                        .strip_prefix("content-length:")
                                        .and_then(|v| v.trim().parse().ok())
                                })
                                .unwrap_or(0);
                            if req.len() >= headers_end + 4 + declared {
                                break;
                            }
                        }
                    }
                    assert!(
                        req.starts_with(&format!("POST /{expect_path}")),
                        "unexpected request: {req}"
                    );
                    let head = format!(
                        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = std::io::Write::write_all(
                        &mut &stream,
                        format!("{head}{body}").as_bytes(),
                    );
                }
            });
            MockServer { addr }
        }

        fn url(&self, path: &str) -> String {
            format!("http://{}{path}", self.addr)
        }
    }

    fn device_spec(auth_url: String, token_url: String) -> AuthMethodSpec {
        AuthMethodSpec {
            id: "device",
            label: "test",
            instructions: "",
            key_url: "",
            client_id: "test-client",
            device_auth_url: Box::leak(auth_url.into_boxed_str()),
            device_token_url: Box::leak(token_url.into_boxed_str()),
            scopes: "",
            account_url: "",
        }
    }

    #[test]
    fn device_flow_completes_after_pending_rounds() {
        let server = MockServer::start(vec![
            (
                "device/code".into(),
                200,
                r#"{"device_code":"dc1","user_code":"ABCD-1234","verification_uri":"https://example.com/activate","interval":1,"expires_in":60}"#.into(),
            ),
            ("login/oauth/access_token".into(), 200, r#"{"error":"authorization_pending"}"#.into()),
            (
                "login/oauth/access_token".into(),
                200,
                r#"{"access_token":"gho_ok","token_type":"bearer","expires_in":3600}"#.into(),
            ),
        ]);
        let spec = device_spec(server.url("/device/code"), server.url("/login/oauth/access_token"));

        let grant = request_device_code(&spec).unwrap();
        assert_eq!(grant.user_code, "ABCD-1234");
        assert_eq!(grant.verification_url, "https://example.com/activate");

        let cancel = std::sync::atomic::AtomicBool::new(false);
        let outcome = poll_device_token(&spec, &grant, &cancel);
        match outcome {
            AuthOutcome::Authenticated { credential } => {
                assert_eq!(credential.secret(), "gho_ok");
                assert!(matches!(
                    credential,
                    Credential::OAuth { expires_at: Some(_), .. }
                ));
            }
            other => panic!("expected auth, got {other:?}"),
        }
    }

    #[test]
    fn device_flow_reports_denial_and_expiration() {
        let server = MockServer::start(vec![
            (
                "device/code".into(),
                200,
                r#"{"device_code":"dc1","user_code":"X","verification_uri":"https://e.com","interval":1}"#.into(),
            ),
            ("t".into(), 200, r#"{"error":"access_denied"}"#.into()),
        ]);
        let spec = device_spec(server.url("/device/code"), server.url("/t"));
        let grant = request_device_code(&spec).unwrap();
        let outcome = poll_device_token(&spec, &grant, &std::sync::atomic::AtomicBool::new(false));
        assert!(matches!(outcome, AuthOutcome::Failed(e) if e.contains("denied")));

        // Expired grant short-circuits before any network call.
        let mut expired = grant;
        expired.expires_at = std::time::Instant::now();
        let outcome = poll_device_token(&spec, &expired, &std::sync::atomic::AtomicBool::new(false));
        assert!(matches!(outcome, AuthOutcome::Failed(e) if e.contains("expired")));
    }

    #[test]
    fn device_flow_honours_cancellation() {
        let server = MockServer::start(vec![(
            "device/code".into(),
            200,
            r#"{"device_code":"dc1","user_code":"X","verification_uri":"https://e.com","interval":30}"#.into(),
        )]);
        let spec = device_spec(server.url("/device/code"), server.url("/unused"));
        let grant = request_device_code(&spec).unwrap();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let c2 = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            c2.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        let outcome = poll_device_token(&spec, &grant, &cancel);
        assert_eq!(outcome, AuthOutcome::Cancelled);
    }
}
