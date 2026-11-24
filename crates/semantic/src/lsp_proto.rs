//! LSP wire protocol — Content-Length framing and JSON-RPC message shaping.
//!
//! All JSON is built and parsed with serde_json: hand-rolled string escaping
//! is how control characters and quotes corrupt server streams.

use serde_json::{Value, json};

/// One parsed inbound LSP message.
#[derive(Debug, Clone)]
pub enum Inbound {
    Request {
        id: u64,
        method: String,
        params: Value,
    },
    Response {
        id: u64,
        result: Option<Value>,
        error: Option<Value>,
    },
    Notification {
        method: String,
        params: Value,
    },
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
    let len =
        content_length.ok_or_else(|| FrameError::Malformed("missing Content-Length".into()))?;
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

/// Convert a `file://` URI to a filesystem path, decoding every `%XX`
/// escape.  Drive-letter forms (`file:///C:/…`, `file://C:/…`) come back
/// as `C:/…`; non-URI input passes through unchanged.
pub fn uri_to_path(uri: &str) -> String {
    let Some(path) = uri.strip_prefix("file://") else {
        return uri.to_string();
    };
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&path[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let decoded = String::from_utf8_lossy(&out).into_owned();
    // "/C:/Users/x" (and the sloppy "C:/Users/x" some servers send) both
    // mean drive C; strip the slash a real URI carries before the letter.
    if decoded.starts_with('/') && decoded.len() >= 3 && decoded.as_bytes()[2] == b':' {
        return decoded[1..].to_string();
    }
    decoded
}

/// Match a server-sent document URI against a buffer path.  didOpen sends
/// the percent-encoded URI, and servers echo it back — decode that side
/// before comparing; raw-path and undecoded comparisons keep sloppy
/// servers working.  Separator style never matters: Windows buffers hold
/// backslash paths while URIs carry slashes.
pub fn uri_matches_path(uri: &str, buf_path: &str) -> bool {
    let normalized = |s: &str| s.replace('\\', "/");
    let decoded = uri_to_path(uri);
    decoded == buf_path
        || normalized(&decoded) == normalized(buf_path)
        || uri.strip_prefix("file://") == Some(buf_path)
        || uri == buf_path
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
                "completion": {
                    "completionItem": {
                        "snippetSupport": true,
                        "labelDetailsSupport": true,
                        "documentationFormat": ["markdown", "plaintext"],
                    }
                },
                "signatureHelp": {
                    "signatureInformation": {
                        "documentationFormat": ["markdown", "plaintext"]
                    }
                },
                "semanticTokens": {
                    "requests": { "full": true },
                    "formats": ["relative"],
                    // rust-analyzer deserializes these strictly and aborts
                    // the handshake when either is missing — the legend
                    // names the client is prepared to see.
                    "tokenTypes": [
                        "comment", "decorator", "enum", "enumMember", "function",
                        "interface", "keyword", "macro", "method", "namespace",
                        "number", "operator", "parameter", "property", "string",
                        "struct", "type", "typeParameter", "variable"
                    ],
                    "tokenModifiers": [
                        "declaration", "definition", "static", "deprecated",
                        "abstract", "documentation", "defaultLibrary",
                        "unreferenced", "constant", "mutable", "reference",
                        "async", "callable", "public"
                    ],
                    "multilineTokenSupport": false,
                },
            }
        },
        "rootUri": root.map(path_to_uri),
    })
}

pub fn path_to_uri(path: &str) -> String {
    let slashes = path.replace('\\', "/");
    // A drive path ("C:/x") needs the empty-authority slash: file:///C:/x.
    let prefix = if slashes.len() >= 2 && slashes.as_bytes()[1] == b':' {
        "/"
    } else {
        ""
    };
    format!(
        "file://{prefix}{}",
        slashes
            .replace('%', "%25")
            .replace(' ', "%20")
            .replace('#', "%23")
            .replace('?', "%3F")
    )
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
        let wire = format!(
            "Content-Type: application/vscode-jsonrpc\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
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

    #[test]
    fn uri_to_path_decodes_every_escape() {
        assert_eq!(uri_to_path("file:///a%23b%3Fc%25d.rs"), "/a#b?c%d.rs");
        // A literal %-sequence that is not valid hex stays put.
        assert_eq!(uri_to_path("file:///a%zz.rs"), "/a%zz.rs");
        assert_eq!(uri_to_path("file:///a%.rs"), "/a%.rs");
    }

    #[test]
    fn path_to_uri_round_trips_through_the_decoder() {
        for path in [
            "/home/u/file.rs",
            "/home/u/my notes.rs",
            "/home/u/50% off/x#y?z.rs",
        ] {
            assert_eq!(uri_to_path(&path_to_uri(path)), path);
        }
    }

    #[test]
    fn uri_matches_path_handles_encoded_and_raw() {
        assert!(uri_matches_path("file:///a/b.rs", "/a/b.rs"));
        assert!(uri_matches_path(
            "file:///a/my%20notes.rs",
            "/a/my notes.rs"
        ));
        assert!(!uri_matches_path("file:///a/b.rs", "/a/other.rs"));
    }

    #[test]
    fn drive_paths_round_trip_through_uris() {
        assert_eq!(
            path_to_uri("C:\\Users\\u\\my notes.rs"),
            "file:///C:/Users/u/my%20notes.rs"
        );
        assert_eq!(uri_to_path("file:///C:/Users/u/x.rs"), "C:/Users/u/x.rs");
        // Some servers drop the empty authority: file://C:/…
        assert_eq!(uri_to_path("file://C:/Users/u/x.rs"), "C:/Users/u/x.rs");
        assert!(uri_matches_path(
            "file:///C:/Users/u/my%20notes.rs",
            "C:\\Users\\u\\my notes.rs"
        ));
        for path in ["C:/Users/u/file.rs", "C:/Users/u/50% off/x#y.rs"] {
            assert_eq!(uri_to_path(&path_to_uri(path)), path);
        }
    }
}
