//! Blocking HTTP helpers for provider traffic.  Every call runs on a
//! background thread; the foreground only sees the parsed result.

pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// One model round-trip may take a while on reasoning models.
pub const CHAT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

fn client(timeout: std::time::Duration) -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(timeout).build()
}

pub fn post_json(
    url: &str,
    headers: &[(String, String)],
    body: &str,
    timeout: std::time::Duration,
) -> Result<(u16, String), String> {
    let mut req = client(timeout)
        .post(url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json");
    for (k, v) in headers {
        req = req.set(k, v);
    }
    let resp = req
        .send_string(body)
        .map_err(|e| match e {
            ureq::Error::Status(code, resp) => {
                let mut text = String::new();
                use std::io::Read as _;
                let _ = std::io::Read::take(resp.into_reader(), 4096)
                    .read_to_string(&mut text);
                format!("HTTP {code}: {}", text.trim())
            }
            other => other.to_string(),
        })?;
    read_body(resp.status(), resp)
}

pub fn get_json(
    url: &str,
    headers: &[(String, String)],
    timeout: std::time::Duration,
) -> Result<(u16, String), String> {
    let mut req = client(timeout).get(url).set("Accept", "application/json");
    for (k, v) in headers {
        req = req.set(k, v);
    }
    let resp = req.call().map_err(|e| match e {
        ureq::Error::Status(code, resp) => {
            let mut text = String::new();
            use std::io::Read as _;
            let _ = std::io::Read::take(resp.into_reader(), 4096).read_to_string(&mut text);
            format!("HTTP {code}: {}", text.trim())
        }
        other => other.to_string(),
    })?;
    read_body(resp.status(), resp)
}

fn read_body(status: u16, resp: ureq::Response) -> Result<(u16, String), String> {
    let mut raw = String::new();
    use std::io::Read as _;
    resp.into_reader()
        .take(MAX_RESPONSE_BYTES as u64)
        .read_to_string(&mut raw)
        .map_err(|e| format!("provider response read failed: {e}"))?;
    Ok((status, raw))
}

/// Best-effort status extraction from an error string produced above, used
/// to tell "bad credential" (delete it) from "provider unreachable" (retry).
pub fn status_of_error(err: &str) -> Option<u16> {
    let rest = err.strip_prefix("HTTP ")?;
    let code: u16 = rest.split(':').next()?.trim().parse().ok()?;
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_status_is_extractable() {
        assert_eq!(status_of_error("HTTP 401: {\"error\":\"bad\"}"), Some(401));
        assert_eq!(status_of_error("agent LLM request failed: boom"), None);
    }
}
