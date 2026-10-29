/// One overload in a `textDocument/signatureHelp` response.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SignatureInfo {
    /// The full signature text, e.g. `fn push(&mut self, value: T)`.
    pub label: String,
    /// (start, end) byte offsets of each parameter within `label`, in
    /// parameter order.
    pub params: Vec<(usize, usize)>,
    /// Server documentation for this overload, flattened to text.
    pub docs: String,
}

/// Signature-help popup state — the active call signature shown while
/// typing a call's arguments.  Refreshed on every `(`/`,` keystroke; the
/// server picks the active overload and parameter.
#[derive(Debug, Clone, Default)]
pub struct SignatureState {
    pub signatures: Vec<SignatureInfo>,
    pub active_signature: usize,
    pub active_parameter: usize,
    pub visible: bool,
}

impl SignatureState {
    /// The active overload, if any.
    pub fn active(&self) -> Option<&SignatureInfo> {
        self.signatures.get(self.active_signature)
    }
}
