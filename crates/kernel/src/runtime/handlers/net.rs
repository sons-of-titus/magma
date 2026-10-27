//! Network background-event handlers — HTTP results and the TCP
//! connection/server state machines.

use crate::state::Editor;
use crate::event::keys;
use crate::event::payload::*;

use super::super::BackgroundEvent;

pub(super) fn handle(ed: &mut Editor, event: BackgroundEvent) {
    match event {
        BackgroundEvent::HttpResponse { id, status, body } => {
            ed.events.emit_typed(keys::events::HTTP_RESPONSE, HttpResponsePayload {
                id: id.to_string(),
                status: status.to_string(),
                body,
            });
        }
        BackgroundEvent::HttpError { id, error } => {
            ed.events.emit_typed(keys::events::HTTP_ERROR, HttpErrorPayload {
                id: id.to_string(),
                error,
            });
        }
        BackgroundEvent::TcpConnected { id } => {
            if let Some(conn) = ed.io.net_connections.get_mut(&id) {
                conn.connected = true;
            }
            ed.events.emit_typed(keys::events::TCP_CONNECTED, TcpConnectedPayload {
                id: id.to_string(),
            });
        }
        BackgroundEvent::TcpData { id, data: line } => {
            ed.events.emit_typed(keys::events::TCP_DATA, TcpDataPayload {
                id: id.to_string(),
                data: line,
            });
        }
        BackgroundEvent::TcpClosed { id } => {
            ed.io.net_connections.remove(&id);
            ed.events.emit_typed(keys::events::TCP_CLOSED, TcpClosedPayload {
                id: id.to_string(),
            });
        }
        BackgroundEvent::TcpError { id, error } => {
            ed.io.net_connections.remove(&id);
            ed.events.emit_typed(keys::events::TCP_ERROR, TcpErrorPayload {
                id: id.to_string(),
                error,
            });
        }
        BackgroundEvent::TcpClientConnected { server_id, client_id, write_tx } => {
            if let Some(srv) = ed.io.net_servers.get_mut(&server_id) {
                srv.clients.insert(client_id, write_tx);
            }
            ed.events.emit_typed(keys::events::TCP_CLIENT_CONNECTED, TcpClientConnectedPayload {
                server_id: server_id.to_string(),
                client_id: client_id.to_string(),
            });
        }
        BackgroundEvent::TcpClientData { server_id, client_id, data: line } => {
            ed.events.emit_typed(keys::events::TCP_CLIENT_DATA, TcpClientDataPayload {
                server_id: server_id.to_string(),
                client_id: client_id.to_string(),
                data: line,
            });
        }
        BackgroundEvent::TcpClientDisconnected { server_id, client_id } => {
            if let Some(srv) = ed.io.net_servers.get_mut(&server_id) {
                srv.clients.remove(&client_id);
            }
            ed.events.emit_typed(keys::events::TCP_CLIENT_DISCONNECTED, TcpClientDisconnectedPayload {
                server_id: server_id.to_string(),
                client_id: client_id.to_string(),
            });
        }
        _ => {}
    }
}
