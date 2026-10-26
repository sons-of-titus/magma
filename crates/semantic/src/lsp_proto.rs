//! LSP wire protocol — Content-Length framing and JSON-RPC message shaping.
//!
//! All JSON is built and parsed with serde_json: hand-rolled string escaping
//! is how control characters and quotes corrupt server streams.

use serde_json::{json, Value};

/// One parsed inbound LSP message.
#[derive(Debug, Clone)]
pub enum Inbound {
    Request { id: u64, method: String, params: Value },
    Response { id: u64, result: Option<Value>, error: Option<Value> },
    Notification { method: String, params: Value },
}

/// Errors while reading the byte stream — malformed input is skipped where
/// possible; only a dead stream is fatal.
#[derive(Debug)]
pub enum FrameError {
    Eof,
    Malformed(String),
}

/// Read one `Content-Length`-framed message from `read`.
pub fn read_message(read: &mut impl std::io::Read) -> Result<Value, FrameError> {
    let mut content_length: Option<usize> = None;
    let mut line = Vec::new();
    loop {
        line.clear();
        let mut byte = [0u8; 1];
        loop {
            match read.read_exact(&mut byte) {
                Ok(()) if byte[0] == b'\n' => break,
                Ok(()) => line.push(byte[0]),
                Err(_) => return Err(FrameError::Eof),
            }
        }
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end_matches('\r');
        if text.is_empty() {
            break;
        }
        if let Some(rest) = text.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse().ok();
        }
        // Other headers (Content-Type) are legal and ignored.
    }
    let len = content_length.ok_or_else(|| FrameError::Malformed("missing Content-Length".into()))?;
    let mut body = vec![0u8; len];
    read.read_exact(&mut body).map_err(|_| FrameError::Eof)?;
    serde_json::from_slice(&body).map_err(|e| FrameError::Malformed(format!("bad JSON body: {e}")))
}

/// Encode a message for the wire.
pub fn encode(value: &Value) -> Vec<u8> {
    let body = serde_json::to_string(value).unwrap_or_else(|_| "{}".into());
    format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

pub fn request(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

pub fn notification(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

/// Convert a `file://` URI to a filesystem path (best effort, no percent
/// decoding beyond the common space).
pub fn uri_to_path(uri: &str) -> String {
    uri.strip_prefix("file://")
        .map(|p| p.replace("%20", " "))
        .unwrap_or_else(|| uri.to_string())
}

/// Build `initialize` params.  A real rootUri matters: rust-analyzer refuses
/// to index without one.
pub fn initialize_params(root: Option<&str>) -> Value {
    json!({
        "processId": std::process::id(),
        "capabilities": {
            "textDocument": {
                "synchronization": { "didSave": true },
                "hover": { "contentFormat": ["plaintext", "markdown"] },
            }
        },
        "rootUri": root.map(path_to_uri),
    })
}

pub fn path_to_uri(path: &str) -> String {
    format!("file://{}", path
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace('#', "%23")
        .replace('?', "%3F"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_round_trip() {
        let msg = request(7, "textDocument/hover", json!({"a": 1}));
        let bytes = encode(&msg);
        let parsed = read_message(&mut &bytes[..]).unwrap();
        assert_eq!(parsed["id"], 7);
        assert_eq!(parsed["method"], "textDocument/hover");
    }

    #[test]
    fn framing_multiple_headers_then_body() {
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"x"}"#;
        let wire = format!("Content-Type: application/vscode-jsonrpc\r\nContent-Length: {}\r\n\r\n", body.len());
        let mut stream = wire.into_bytes();
        stream.extend_from_slice(body);
        let parsed = read_message(&mut &stream[..]).unwrap();
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn empty_stream_is_eof() {
        assert!(matches!(read_message(&mut &b""[..]), Err(FrameError::Eof)));
    }

    #[test]
    fn truncated_body_is_eof() {
        let stream = b"Content-Length: 100\r\n\r\n{\"a\":1}".to_vec();
        let mut slice: &[u8] = &stream;
        assert!(matches!(read_message(&mut slice), Err(FrameError::Eof)));
    }

    #[test]
    fn uri_to_path_strips_scheme() {
        assert_eq!(uri_to_path("file:///a/b%20c.rs"), "/a/b c.rs");
        assert_eq!(uri_to_path("untitled:x"), "untitled:x");
    }
}
