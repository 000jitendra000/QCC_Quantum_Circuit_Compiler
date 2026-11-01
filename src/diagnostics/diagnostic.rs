use std::fmt;

use crate::diagnostics::span::SourceSpan;

/// Severity level of a compiler diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagnosticLevel {
    /// Informational note providing additional context or tips.
    Note,
    /// Warning indicating potential issues that do not block compilation.
    Warning,
    /// Error indicating a semantic or syntactic violation that halts compilation.
    Error,
}

impl DiagnosticLevel {
    /// Returns the lowercase string identifier for the diagnostic level.
    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for DiagnosticLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A structured compiler diagnostic with optional source span, error code, and notes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// The severity level of this diagnostic.
    pub level: DiagnosticLevel,
    /// An optional standardized diagnostic code (e.g. `"E0001"`).
    pub code: Option<String>,
    /// The primary diagnostic explanation.
    pub message: String,
    /// The optional source location or span associated with the diagnostic.
    pub span: Option<SourceSpan>,
    /// The optional file path where the issue originated.
    pub file_path: Option<String>,
    /// Additional contextual notes or hints.
    pub notes: Vec<String>,
}

impl Diagnostic {
    /// Constructs a new [`Diagnostic`] with the given severity level and message.
    pub fn new(level: DiagnosticLevel, message: impl Into<String>) -> Self {
        Self {
            level,
            code: None,
            message: message.into(),
            span: None,
            file_path: None,
            notes: Vec::new(),
        }
    }

    /// Convenience constructor for an error diagnostic.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(DiagnosticLevel::Error, message)
    }

    /// Convenience constructor for a warning diagnostic.
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(DiagnosticLevel::Warning, message)
    }

    /// Convenience constructor for an informational note diagnostic.
    pub fn note(message: impl Into<String>) -> Self {
        Self::new(DiagnosticLevel::Note, message)
    }

    /// Sets an error or warning code (e.g. `"E0001"`).
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Sets the source span for this diagnostic.
    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    /// Sets the source file path for this diagnostic.
    pub fn with_file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Appends an informational note to the diagnostic.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Render location prefix if available
        match (&self.file_path, &self.span) {
            (Some(path), Some(span)) => write!(f, "{path}:{span}: ")?,
            (Some(path), None) => write!(f, "{path}: ")?,
            (None, Some(span)) => write!(f, "{span}: ")?,
            (None, None) => {}
        }

        // Render level and optional code
        if let Some(code) = &self.code {
            write!(f, "{}[{code}]: {}", self.level, self.message)?;
        } else {
            write!(f, "{}: {}", self.level, self.message)?;
        }

        // Render notes indented
        for note in &self.notes {
            write!(f, "\n  note: {note}")?;
        }

        Ok(())
    }
}

impl std::error::Error for Diagnostic {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_formatting() {
        let span = SourceSpan::single_line(4, 9, 1, 5, 10).unwrap();
        let diag = Diagnostic::error("unexpected token")
            .with_code("E0001")
            .with_file_path("test.qasm")
            .with_span(span)
            .with_note("expected gate identifier");

        let expected =
            "test.qasm:1:5-10: error[E0001]: unexpected token\n  note: expected gate identifier";
        assert_eq!(diag.to_string(), expected);
    }

    #[test]
    fn test_diagnostic_without_span_or_file() {
        let diag = Diagnostic::warning("unused classical register").with_code("W0002");
        assert_eq!(
            diag.to_string(),
            "warning[W0002]: unused classical register"
        );
    }
}
