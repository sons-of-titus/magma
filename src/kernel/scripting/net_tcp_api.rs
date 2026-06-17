//! Janet C functions for TCP client and server primitives (Sprint 13).

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::extension::Capability;

/// `(net/tcp-connect host port)` → conn-id
///
/// Open an outbound TCP connection.  Emits `tcp-connected` on success,
/// `tcp-data` per received line, `tcp-closed` on EOF, `tcp-error` on failure.
unsafe extern "C-unwind" fn c_tcp_connect(argc: i32, argv: *mut Janet) -> Janet {
    if !super::extension_api::check_capability(&Capability::Network) {
        conv::signal_err("net/tcp-connect requires the Network capability");
    }
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
        ed.io.net_connections.insert(id, crate::kernel::net::NetConnection {
            host: host.clone(), port, connected: false, write_tx: write_tx.clone(),
        });
        debug!("net/tcp-connect id={id} {host}:{port}");
        let sender = bg.sender.clone();
        let addr = format!("{host}:{port}");
        bg.spawn(async move {
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            let stream = match tokio::net::TcpStream::connect(&addr).await {
                Ok(s)  => s,
                Err(e) => {
                    debug!("tcp-error id={id} {e}");
                    let _ = sender.send(crate::kernel::runtime::BackgroundEvent::TcpError { id, error: e.to_string() });
                    return;
                }
            };
            debug!("tcp-connected id={id}");
            let _ = sender.send(crate::kernel::runtime::BackgroundEvent::TcpConnected { id });
            let (reader, mut writer) = stream.into_split();
            let mut lines = BufReader::new(reader).lines();
            let sender_read = sender.clone();
            let read_task = tokio::spawn(async move {
                while let Ok(Some(line)) = lines.next_line().await {
                    debug!("tcp-data id={id} len={}", line.len());
                    let _ = sender_read.send(crate::kernel::runtime::BackgroundEvent::TcpData { id, data: line });
                }
                debug!("tcp-closed id={id}");
                let _ = sender_read.send(crate::kernel::runtime::BackgroundEvent::TcpClosed { id });
            });
            while let Some(msg) = write_rx.recv().await {
                if writer.write_all(format!("{msg}\n").as_bytes()).await.is_err() {
                    break;
                }
            }
            read_task.abort();
        });
        conv::integer(id as i32)
    })
}

/// `(net/tcp-send conn-id data)` → nil
unsafe extern "C-unwind" fn c_tcp_send(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let conn_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        let data = match conv::get_str(argc, argv, 1) {
            Some(s) => s,
            None => conv::signal_err("net/tcp-send: data string required"),
        };
        if let Some(conn) = ed.io.net_connections.get(&conn_id) {
            debug!("net/tcp-send id={conn_id} len={}", data.len());
            let _ = conn.write_tx.send(data);
        }
        conv::nil()
    })
}

/// `(net/tcp-close conn-id)` → nil
///
/// Dropping the connection entry signals the write task to exit, causing EOF on the remote end.
unsafe extern "C-unwind" fn c_tcp_close(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let conn_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        debug!("net/tcp-close id={conn_id}");
        ed.io.net_connections.remove(&conn_id);
        conv::nil()
    })
}

/// `(net/tcp-listen port)` → server-id
///
/// Bind a TCP listener and start accepting clients.  Emits `tcp-client-connected`,
/// `tcp-client-data`, `tcp-client-disconnected` events.
unsafe extern "C-unwind" fn c_tcp_listen(argc: i32, argv: *mut Janet) -> Janet {
    if !super::extension_api::check_capability(&Capability::Network) {
        conv::signal_err("net/tcp-listen requires the Network capability");
    }
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
        ed.io.net_servers.insert(server_id, crate::kernel::net::NetServer {
            port,
            running: running.clone(),
            clients: std::collections::HashMap::new(),
            next_client_id: 1,
        });
        debug!("net/tcp-listen server_id={server_id} port={port}");
        let sender = bg.sender.clone();
        bg.spawn(async move {
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            use std::time::Duration;
            let listener = match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
                Ok(l)  => l,
                Err(e) => {
                    debug!("tcp-listen bind error server_id={server_id} port={port}: {e}");
                    let _ = sender.send(crate::kernel::runtime::BackgroundEvent::TcpError {
                        id: server_id, error: format!("tcp-listen: {e}"),
                    });
                    return;
                }
            };
            let mut next_client_id: u64 = 1;
            while running.load(Ordering::Relaxed) {
                let accepted = tokio::time::timeout(
                    Duration::from_millis(100), listener.accept()
                ).await;
                match accepted {
                    Err(_)          => continue,
                    Ok(Err(_))      => break,
                    Ok(Ok((stream, peer))) => {
                        let client_id = next_client_id;
                        next_client_id += 1;
                        debug!("tcp-client-connected server_id={server_id} client_id={client_id} peer={peer}");
                        let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
                        let _ = sender.send(crate::kernel::runtime::BackgroundEvent::TcpClientConnected {
                            server_id, client_id, write_tx,
                        });
                        let sender_c = sender.clone();
                        let (reader, mut writer) = stream.into_split();
                        let mut lines = BufReader::new(reader).lines();
                        tokio::spawn(async move {
                            while let Ok(Some(line)) = lines.next_line().await {
                                debug!("tcp-client-data server_id={server_id} client_id={client_id} len={}", line.len());
                                let _ = sender_c.send(crate::kernel::runtime::BackgroundEvent::TcpClientData {
                                    server_id, client_id, data: line,
                                });
                            }
                            debug!("tcp-client-disconnected server_id={server_id} client_id={client_id}");
                            let _ = sender_c.send(crate::kernel::runtime::BackgroundEvent::TcpClientDisconnected {
                                server_id, client_id,
                            });
                        });
                        tokio::spawn(async move {
                            while let Some(msg) = write_rx.recv().await {
                                if writer.write_all(format!("{msg}\n").as_bytes()).await.is_err() {
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

/// `(net/tcp-stop server-id)` → nil
unsafe extern "C-unwind" fn c_tcp_stop(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let server_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        debug!("net/tcp-stop server_id={server_id}");
        if let Some(srv) = ed.io.net_servers.remove(&server_id) {
            srv.running.store(false, Ordering::Relaxed);
        }
        conv::nil()
    })
}

/// `(net/tcp-broadcast server-id data)` → nil
unsafe extern "C-unwind" fn c_tcp_broadcast(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let server_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        let data = match conv::get_str(argc, argv, 1) {
            Some(s) => s,
            None => conv::signal_err("net/tcp-broadcast: data string required"),
        };
        if let Some(srv) = ed.io.net_servers.get(&server_id) {
            debug!("net/tcp-broadcast server_id={server_id} clients={} len={}", srv.clients.len(), data.len());
            for tx in srv.clients.values() { let _ = tx.send(data.clone()); }
        }
        conv::nil()
    })
}

/// `(net/tcp-send-to server-id client-id data)` → nil
unsafe extern "C-unwind" fn c_tcp_send_to(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let server_id = conv::get_int(argc, argv, 0).unwrap_or(-1) as u64;
        let client_id = conv::get_int(argc, argv, 1).unwrap_or(-1) as u64;
        let data = match conv::get_str(argc, argv, 2) {
            Some(s) => s,
            None => conv::signal_err("net/tcp-send-to: data string required"),
        };
        if let Some(srv) = ed.io.net_servers.get(&server_id) {
            if let Some(tx) = srv.clients.get(&client_id) {
                debug!("net/tcp-send-to server_id={server_id} client_id={client_id} len={}", data.len());
                let _ = tx.send(data);
            }
        }
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"net/tcp-connect".as_ptr(),
            cfun: Some(c_tcp_connect as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Open an outbound TCP connection; emits tcp-connected, tcp-data, tcp-closed, tcp-error".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-send".as_ptr(),
            cfun: Some(c_tcp_send as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Send a line to an open TCP connection".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-close".as_ptr(),
            cfun: Some(c_tcp_close as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Close an outbound TCP connection".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-listen".as_ptr(),
            cfun: Some(c_tcp_listen as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Bind a TCP listener; emits tcp-client-connected, tcp-client-data, tcp-client-disconnected".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-stop".as_ptr(),
            cfun: Some(c_tcp_stop as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Stop a TCP server".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-broadcast".as_ptr(),
            cfun: Some(c_tcp_broadcast as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Send a line to all clients connected to a server".as_ptr(),
        },
        JanetReg {
            name: c"net/tcp-send-to".as_ptr(),
            cfun: Some(c_tcp_send_to as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Send a line to a specific client on a server".as_ptr(),
        },
    ]
}
