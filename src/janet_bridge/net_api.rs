//! Janet API — network primitives for HTTP and TCP (Sprint 13).
//!
//! Provides:
//!   net/http-request method url &opt body  → request-id
//!   net/http-get     url                   → request-id
//!   net/http-post    url body              → request-id
//!   net/tcp-connect  host port             → conn-id
//!   net/tcp-send     conn-id data          → nil
//!   net/tcp-close    conn-id               → nil
//!   net/tcp-listen   port                  → server-id
//!   net/tcp-stop     server-id             → nil
//!   net/tcp-broadcast server-id data       → nil
//!   net/tcp-send-to  server-id client-id data → nil

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use evil_janet::*;
use super::conv;
use super::with_editor;

// ── net/http-request ─────────────────────────────────────────────────────────

/// `(net/http-request method url &opt body)` → request-id
///
/// Fire an HTTP request in the background.  Emits `http-response` (with
/// `:id`, `:status`, `:body`) or `http-error` (with `:id`, `:error`) when done.
unsafe extern "C-unwind" fn c_http_request(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let method = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("net/http-request: method string required"),
        };
        let url = match conv::get_str(argc, argv, 1) {
            Some(s) => s,
            None => conv::signal_err("net/http-request: url string required"),
        };
        let body = conv::get_str(argc, argv, 2).unwrap_or_default();

        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("net/http-request: no background runtime"),
        };

        let id = ed.io.next_net_id;
        ed.io.next_net_id += 1;

        let sender = bg.sender.clone();
        bg.spawn_blocking(move || {
            let result = (|| -> Result<(u16, String), String> {
                let req = ureq::request(&method, &url);
                let resp = if body.is_empty() {
                    req.call().map_err(|e| e.to_string())?
                } else {
                    req.send_string(&body).map_err(|e| e.to_string())?
                };
                let status = resp.status();
                let text = resp.into_string().map_err(|e| e.to_string())?;
                Ok((status, text))
            })();
            match result {
                Ok((status, body)) => {
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpResponse {
                        id, status, body,
                    });
                }
                Err(error) => {
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpError {
                        id, error,
                    });
                }
            }
        });

        conv::integer(id as i32)
    })
}

// ── net/http-get ─────────────────────────────────────────────────────────────

/// `(net/http-get url)` → request-id
///
/// Convenience wrapper around `net/http-request` using the GET method.
unsafe extern "C-unwind" fn c_http_get(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let url = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("net/http-get: url string required"),
        };

        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("net/http-get: no background runtime"),
        };

        let id = ed.io.next_net_id;
        ed.io.next_net_id += 1;

        let sender = bg.sender.clone();
        bg.spawn_blocking(move || {
            let result = (|| -> Result<(u16, String), String> {
                let resp = ureq::get(&url).call().map_err(|e| e.to_string())?;
                let status = resp.status();
                let text = resp.into_string().map_err(|e| e.to_string())?;
                Ok((status, text))
            })();
            match result {
                Ok((status, body)) => {
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpResponse {
                        id, status, body,
                    });
                }
                Err(error) => {
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpError {
                        id, error,
                    });
                }
            }
        });

        conv::integer(id as i32)
    })
}

// ── net/http-post ────────────────────────────────────────────────────────────

/// `(net/http-post url body)` → request-id
///
/// Convenience wrapper around `net/http-request` using the POST method.
unsafe extern "C-unwind" fn c_http_post(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let url = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("net/http-post: url string required"),
        };
        let body = conv::get_str(argc, argv, 1).unwrap_or_default();

        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("net/http-post: no background runtime"),
        };

        let id = ed.io.next_net_id;
        ed.io.next_net_id += 1;

        let sender = bg.sender.clone();
        bg.spawn_blocking(move || {
            let result = (|| -> Result<(u16, String), String> {
                let resp = ureq::post(&url)
                    .send_string(&body)
                    .map_err(|e| e.to_string())?;
                let status = resp.status();
                let text = resp.into_string().map_err(|e| e.to_string())?;
                Ok((status, text))
            })();
            match result {
                Ok((status, body)) => {
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpResponse {
                        id, status, body,
                    });
                }
                Err(error) => {
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpError {
                        id, error,
                    });
                }
            }
        });

        conv::integer(id as i32)
    })
}

// ── net/tcp-connect ───────────────────────────────────────────────────────────

/// `(net/tcp-connect host port)` → conn-id
///
/// Establish a TCP connection asynchronously.  Emits `tcp-connected` when the
/// connection is up, `tcp-data` for each line received, `tcp-closed` on EOF,
/// and `tcp-error` on failure.  The returned integer is the handle for
/// `net/tcp-send` and `net/tcp-close`.
unsafe extern "C-unwind" fn c_tcp_connect(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let host = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => conv::signal_err("net/tcp-connect: host string required"),
        };
        let port = match conv::get_int(argc, argv, 1) {
            Some(p) => p as u16,
            None => conv::signal_err("net/tcp-connect: port integer required"),
        };

        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("net/tcp-connect: no background runtime"),
        };

        let id = ed.io.next_net_id;
        ed.io.next_net_id += 1;

        let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        ed.io.net_connections.insert(id, crate::net::NetConnection {
            host: host.clone(),
            port,
            connected: false,
            write_tx: write_tx.clone(),
        });

        let sender = bg.sender.clone();
        let addr = format!("{host}:{port}");

        bg.spawn(async move {
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

            let stream = match tokio::net::TcpStream::connect(&addr).await {
                Ok(s) => s,
                Err(e) => {
                    let _ = sender.send(crate::runtime::BackgroundEvent::TcpError {
                        id, error: e.to_string(),
                    });
                    return;
                }
            };

            let _ = sender.send(crate::runtime::BackgroundEvent::TcpConnected { id });

            let (reader, mut writer) = stream.into_split();
            let mut lines = BufReader::new(reader).lines();

            let sender_read = sender.clone();

            // Spawn a task to drive the read half
            let read_task = tokio::spawn(async move {
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = sender_read.send(crate::runtime::BackgroundEvent::TcpData {
                        id, data: line,
                    });
                }
                let _ = sender_read.send(crate::runtime::BackgroundEvent::TcpClosed { id });
            });

            // Drive the write half from the channel
            while let Some(msg) = write_rx.recv().await {
                let line = format!("{msg}\n");
                if writer.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
            }

            read_task.abort();
        });

        conv::integer(id as i32)
    })
}

// ── net/tcp-send ─────────────────────────────────────────────────────────────

/// `(net/tcp-send conn-id data)` → nil
///
/// Send a line of text to an open TCP connection.
unsafe extern "C-unwind" fn c_tcp_send(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let conn_id = match conv::get_int(argc, argv, 0) {
            Some(i) => i as u64,
            None => conv::signal_err("net/tcp-send: conn-id integer required"),
        };
        let data = match conv::get_str(argc, argv, 1) {
            Some(s) => s,
            None => conv::signal_err("net/tcp-send: data string required"),
        };

        if let Some(conn) = ed.io.net_connections.get(&conn_id) {
            let _ = conn.write_tx.send(data);
        }
        conv::nil()
    })
}

// ── net/tcp-close ────────────────────────────────────────────────────────────

/// `(net/tcp-close conn-id)` → nil
///
/// Close an outbound TCP connection.  Dropping the connection entry signals
/// the write task to exit, which causes the remote end to see EOF.
unsafe extern "C-unwind" fn c_tcp_close(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let conn_id = match conv::get_int(argc, argv, 0) {
            Some(i) => i as u64,
            None => conv::signal_err("net/tcp-close: conn-id integer required"),
        };
        ed.io.net_connections.remove(&conn_id);
        conv::nil()
    })
}

// ── net/tcp-listen ───────────────────────────────────────────────────────────

/// `(net/tcp-listen port)` → server-id
///
/// Bind a TCP listener on the given port and start accepting connections.
/// Emits `tcp-client-connected` (`:server-id`, `:client-id`) when a client
/// connects, `tcp-client-data` (`:server-id`, `:client-id`, `:data`) per line,
/// and `tcp-client-disconnected` on EOF.
unsafe extern "C-unwind" fn c_tcp_listen(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let port = match conv::get_int(argc, argv, 0) {
            Some(p) => p as u16,
            None => conv::signal_err("net/tcp-listen: port integer required"),
        };

        let bg = match ed.background.as_ref() {
            Some(bg) => bg.clone(),
            None => conv::signal_err("net/tcp-listen: no background runtime"),
        };

        let server_id = ed.io.next_net_id;
        ed.io.next_net_id += 1;

        let running = Arc::new(AtomicBool::new(true));
        ed.io.net_servers.insert(server_id, crate::net::NetServer {
            port,
            running: running.clone(),
            clients: std::collections::HashMap::new(),
            next_client_id: 1,
        });

        let sender = bg.sender.clone();

        bg.spawn(async move {
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            use std::time::Duration;

            let listener = match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
                Ok(l) => l,
                Err(e) => {
                    let _ = sender.send(crate::runtime::BackgroundEvent::HttpError {
                        id: server_id, error: format!("tcp-listen bind error: {e}"),
                    });
                    return;
                }
            };

            let mut next_client_id: u64 = 1;

            while running.load(Ordering::Relaxed) {
                let accepted = tokio::time::timeout(Duration::from_millis(100), listener.accept()).await;
                match accepted {
                    Err(_) => {
                        // Timeout — re-check running flag
                        continue;
                    }
                    Ok(Err(_)) => break,
                    Ok(Ok((stream, _peer))) => {
                        let client_id = next_client_id;
                        next_client_id += 1;

                        let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

                        let _ = sender.send(crate::runtime::BackgroundEvent::TcpClientConnected {
                            server_id, client_id, write_tx,
                        });

                        let sender_c = sender.clone();
                        let (reader, mut writer) = stream.into_split();
                        let mut lines = BufReader::new(reader).lines();

                        // Read task
                        tokio::spawn(async move {
                            while let Ok(Some(line)) = lines.next_line().await {
                                let _ = sender_c.send(crate::runtime::BackgroundEvent::TcpClientData {
                                    server_id, client_id, data: line,
                                });
                            }
                            let _ = sender_c.send(crate::runtime::BackgroundEvent::TcpClientDisconnected {
                                server_id, client_id,
                            });
                        });

                        // Write task
                        tokio::spawn(async move {
                            while let Some(msg) = write_rx.recv().await {
                                let line = format!("{msg}\n");
                                if writer.write_all(line.as_bytes()).await.is_err() {
                                    break;
                                }
                            }
                        });
                    }
                }
            }
        });

        conv::integer(server_id as i32)
    })
}

// ── net/tcp-stop ─────────────────────────────────────────────────────────────

/// `(net/tcp-stop server-id)` → nil
///
/// Stop a listening TCP server.  The accept loop exits on the next 100 ms tick.
unsafe extern "C-unwind" fn c_tcp_stop(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let server_id = match conv::get_int(argc, argv, 0) {
            Some(i) => i as u64,
            None => conv::signal_err("net/tcp-stop: server-id integer required"),
        };
        if let Some(srv) = ed.io.net_servers.remove(&server_id) {
            srv.running.store(false, Ordering::Relaxed);
        }
        conv::nil()
    })
}

// ── net/tcp-broadcast ────────────────────────────────────────────────────────

/// `(net/tcp-broadcast server-id data)` → nil
///
/// Send a line of text to every client connected to a server.
unsafe extern "C-unwind" fn c_tcp_broadcast(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let server_id = match conv::get_int(argc, argv, 0) {
            Some(i) => i as u64,
            None => conv::signal_err("net/tcp-broadcast: server-id integer required"),
        };
        let data = match conv::get_str(argc, argv, 1) {
            Some(s) => s,
            None => conv::signal_err("net/tcp-broadcast: data string required"),
        };

        if let Some(srv) = ed.io.net_servers.get(&server_id) {
            for tx in srv.clients.values() {
                let _ = tx.send(data.clone());
            }
        }
        conv::nil()
    })
}

// ── net/tcp-send-to ──────────────────────────────────────────────────────────

/// `(net/tcp-send-to server-id client-id data)` → nil
///
/// Send a line of text to a specific client connected to a server.
unsafe extern "C-unwind" fn c_tcp_send_to(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let server_id = match conv::get_int(argc, argv, 0) {
            Some(i) => i as u64,
            None => conv::signal_err("net/tcp-send-to: server-id integer required"),
        };
        let client_id = match conv::get_int(argc, argv, 1) {
            Some(i) => i as u64,
            None => conv::signal_err("net/tcp-send-to: client-id integer required"),
        };
        let data = match conv::get_str(argc, argv, 2) {
            Some(s) => s,
            None => conv::signal_err("net/tcp-send-to: data string required"),
        };

        if let Some(srv) = ed.io.net_servers.get(&server_id) {
            if let Some(tx) = srv.clients.get(&client_id) {
                let _ = tx.send(data);
            }
        }
        conv::nil()
    })
}

// ── Registration ─────────────────────────────────────────────────────────────

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"net/http-request".as_ptr(),
            cfun: Some(c_http_request),
            documentation: c"(net/http-request method url &opt body)\n\nFire an HTTP request in the background. Returns a request-id integer. Emits http-response or http-error event when done.".as_ptr(),
        },
        JanetReg {
            name: c"net/http-get".as_ptr(),
            cfun: Some(c_http_get),
            documentation: c"(net/http-get url)\n\nGET the given URL in the background. Returns a request-id integer.".as_ptr(),
        },
        JanetReg {
            name: c"net/http-post".as_ptr(),
            cfun: Some(c_http_post),
            documentation: c"(net/http-post url body)\n\nPOST body to the given URL in the background. Returns a request-id integer.".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-connect".as_ptr(),
            cfun: Some(c_tcp_connect),
            documentation: c"(net/tcp-connect host port)\n\nOpen an outbound TCP connection. Returns a conn-id integer. Emits tcp-connected, tcp-data, tcp-closed, tcp-error events.".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-send".as_ptr(),
            cfun: Some(c_tcp_send),
            documentation: c"(net/tcp-send conn-id data)\n\nSend a line of text to an open TCP connection.".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-close".as_ptr(),
            cfun: Some(c_tcp_close),
            documentation: c"(net/tcp-close conn-id)\n\nClose an outbound TCP connection.".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-listen".as_ptr(),
            cfun: Some(c_tcp_listen),
            documentation: c"(net/tcp-listen port)\n\nBind a TCP listener on the given port. Returns a server-id. Emits tcp-client-connected, tcp-client-data, tcp-client-disconnected events.".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-stop".as_ptr(),
            cfun: Some(c_tcp_stop),
            documentation: c"(net/tcp-stop server-id)\n\nStop a listening TCP server.".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-broadcast".as_ptr(),
            cfun: Some(c_tcp_broadcast),
            documentation: c"(net/tcp-broadcast server-id data)\n\nSend a line to every client connected to a server.".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-send-to".as_ptr(),
            cfun: Some(c_tcp_send_to),
            documentation: c"(net/tcp-send-to server-id client-id data)\n\nSend a line to a specific client connected to a server.".as_ptr(),
        },
    ]
}
