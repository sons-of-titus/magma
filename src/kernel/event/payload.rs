use std::collections::HashMap;
use crate::kernel::event::EventData;

macro_rules! event_payload {
    ($name:ident { $($field:ident $(: $key:expr)?),+ $(,)? }) => {
        pub struct $name {
            $(pub $field: String),+
        }

        impl From<$name> for EventData {
            fn from(e: $name) -> Self {
                let mut m = HashMap::new();
                $(
                    m.insert(
                        stringify!($field).replace('_', "-"),
                        e.$field,
                    );
                )+
                m
            }
        }
    };
    (empty) => {
        pub struct EmptyPayload;
        impl From<EmptyPayload> for EventData {
            fn from(_: EmptyPayload) -> Self {
                HashMap::new()
            }
        }
    };
}

event_payload!(empty);
event_payload!(BufferAfterSavePayload { path, buffer_id });
event_payload!(BufferBeforeSavePayload { path, buffer_id });
event_payload!(BufferChangedPayload { buffer_id });
event_payload!(BufferClosedPayload { buffer_id });
event_payload!(BufferCreatedPayload { buffer_id, name, path });
event_payload!(BufferFocusedPayload { buffer_id });
event_payload!(BufferMessageAppendedPayload { buffer_id, text });
event_payload!(BufferReadOnlyPayload { buffer_id });
event_payload!(CursorMovedPayload { buffer_id, cursor });
event_payload!(DecorationChangedPayload { buffer, layer });
event_payload!(EvalResultPayload { value, error });
event_payload!(FaceChangedPayload { face });
event_payload!(FileChangedPayload { path, kind });
event_payload!(FileErrorPayload { path, error });
event_payload!(FileIndexedPayload { project_name, count });
event_payload!(FileLoadedPayload { path, content });
event_payload!(FileSavedPayload { path });
event_payload!(FinderResultsPayload { buffer_id, count, pattern });
event_payload!(GutterClickedPayload { column, line, buf });
event_payload!(GutterSignChangedPayload { column, buffer, line });
event_payload!(HelpShownPayload { buffer_id });
event_payload!(HttpErrorPayload { id, error });
event_payload!(HttpResponsePayload { id, status, body });
event_payload!(LspCodeActionsPayload { path, actions });
event_payload!(LspCompletionItemsPayload { path, items });
event_payload!(LspCompletionPayload { path, items });
event_payload!(LspDefinitionPayload { path, uri, start_line, end_line, start_col, end_col });
event_payload!(LspDiagnosticsPayload { path, count, items });
event_payload!(LspHoverPayload { path, contents });
event_payload!(LspProgressPayload { token, message, percentage });
event_payload!(LspRenameResultPayload { path, edit });
event_payload!(LspResponsePayload { path, method, result });
event_payload!(MajorModeChangedPayload { mode, buffer_id });
event_payload!(MinibufferClosedPayload { input, kind });
event_payload!(MinibufferOpenedPayload { prompt, kind });
event_payload!(ModeChangedPayload { from, to });
event_payload!(ProcessExitPayload { id, exit_code, cmd });
event_payload!(ProcessOutputPayload { id, line, stream });
event_payload!(ProjectMemberFocusedPayload { name, root, project_name });
event_payload!(ProjectOpenedPayload { root, name });
event_payload!(TaskResultPayload { id, value, error });
event_payload!(TaskStartedPayload { id, name });
event_payload!(TaskCompletedPayload { id, name, exit_code, duration_ms });
event_payload!(TaskFailedPayload { id, name, error });
event_payload!(TcpClientConnectedPayload { server_id, client_id });
event_payload!(TcpClientDataPayload { server_id, client_id, data });
event_payload!(TcpClientDisconnectedPayload { server_id, client_id });
event_payload!(TcpClosedPayload { id });
event_payload!(TcpConnectedPayload { id });
event_payload!(TcpDataPayload { id, data });
event_payload!(TcpErrorPayload { id, error });
event_payload!(WarningEmittedPayload { buffer_id, text });
event_payload!(WindowFocusedPayload { id, buffer });
