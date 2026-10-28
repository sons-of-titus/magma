//! Diagnostics carried by a buffer.
//!
//! The buffer owns the two diagnostic lists (language-server and tool
//! owned), so the data model lives with the text engine.  The semantic
//! layer re-exports it for language providers — both sides annotate the
//! same buffer ranges and share one display rendering.

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl DiagnosticSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Info => "info",
            DiagnosticSeverity::Hint => "hint",
        }
    }

    /// Single-char severity code for gutter marks and display strings.
    pub fn code(&self) -> char {
        match self {
            DiagnosticSeverity::Error => 'E',
            DiagnosticSeverity::Warning => 'W',
            DiagnosticSeverity::Info => 'I',
            DiagnosticSeverity::Hint => 'H',
        }
    }
}

/// A typed diagnostic produced by a language provider.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub severity: DiagnosticSeverity,
    /// 0-based line index.
    pub line: usize,
    /// 0-based column index.
    pub column: usize,
    /// End of the range, 0-based; a point diagnostic repeats `line`/`column`.
    pub end_line: usize,
    pub end_column: usize,
}

impl Diagnostic {
    /// The one display rendering of a diagnostic.  Every consumer that
    /// shows diagnostics as text goes through here — buffers, Janet
    /// queries, and event payloads carry this format.
    pub fn display(&self) -> String {
        format!(
            "[{}] line {}:{} {}",
            self.severity.code(),
            self.line + 1,
            self.column + 1,
            self.message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_display_is_the_one_rendering() {
        let d = Diagnostic {
            message: "some error".into(),
            severity: DiagnosticSeverity::Error,
            line: 0,
            column: 4,
            end_line: 0,
            end_column: 9,
        };
        assert_eq!(d.display(), "[E] line 1:5 some error");
    }

    #[test]
    fn severity_codes_round_trip_the_display_prefix() {
        for (sev, code) in [
            (DiagnosticSeverity::Error, 'E'),
            (DiagnosticSeverity::Warning, 'W'),
            (DiagnosticSeverity::Info, 'I'),
            (DiagnosticSeverity::Hint, 'H'),
        ] {
            assert_eq!(sev.code(), code);
        }
    }
}
