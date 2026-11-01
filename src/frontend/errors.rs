//! Frontend result and error types isolating parser diagnostics from compiler core.

use thiserror::Error;

use crate::diagnostics::{Diagnostic, DiagnosticLevel, SourceSpan, SpanError};

/// Specific error conditions encountered across frontend parsing and adaptation boundaries.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum FrontendError {
    /// Parsing failed with one or more error-level diagnostics.
    #[error("frontend parsing failed with {count} error(s)")]
    ParseFailed {
        /// Total number of error-level diagnostics encountered during parsing.
        count: usize,
    },

    /// An invalid source span coordinate or calculation occurred.
    #[error("source span error: {0}")]
    Span(#[from] SpanError),

    /// An unsupported OpenQASM construct or language feature was encountered.
    #[error("unsupported frontend construct: {message}")]
    Unsupported {
        /// Explanation of why the construct is unsupported.
        message: String,
        /// The optional source location of the unsupported construct.
        span: Option<SourceSpan>,
    },

    /// An I/O error occurred while reading source files.
    #[error("I/O error: {message}")]
    Io {
        /// Message describing the I/O failure.
        message: String,
    },
}

impl From<std::io::Error> for FrontendError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            message: err.to_string(),
        }
    }
}

/// Result returned by frontend operations, bundling optional output with compiler diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct FrontendResult<T> {
    /// The produced AST or intermediate representation, if parsing succeeded.
    pub output: Option<T>,
    /// All diagnostics (errors, warnings, notes) emitted during frontend analysis.
    pub diagnostics: Vec<Diagnostic>,
}

impl<T> FrontendResult<T> {
    /// Creates a new [`FrontendResult`] with optional output and diagnostics.
    pub fn new(output: Option<T>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            output,
            diagnostics,
        }
    }

    /// Constructs a successful [`FrontendResult`] with zero diagnostics.
    pub fn success(output: T) -> Self {
        Self {
            output: Some(output),
            diagnostics: Vec::new(),
        }
    }

    /// Constructs a successful [`FrontendResult`] carrying diagnostics.
    pub fn with_diagnostics(output: T, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            output: Some(output),
            diagnostics,
        }
    }

    /// Constructs a failure [`FrontendResult`] with no output and given diagnostics.
    pub fn failure(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            output: None,
            diagnostics,
        }
    }

    /// Returns `true` if parsing produced output and zero [`DiagnosticLevel::Error`] diagnostics.
    pub fn is_ok(&self) -> bool {
        self.output.is_some() && !self.has_errors()
    }

    /// Returns `true` if any diagnostic has [`DiagnosticLevel::Error`] severity.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Error)
    }

    /// Returns `true` if any diagnostic has [`DiagnosticLevel::Warning`] severity.
    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Warning)
    }

    /// Returns an iterator over all error diagnostics.
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Error)
    }

    /// Returns an iterator over all warning diagnostics.
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Warning)
    }

    /// Returns a slice of all diagnostics in order of discovery.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Unwraps the output or returns a [`FrontendError::ParseFailed`] containing the error count.
    pub fn into_result(self) -> Result<T, FrontendError> {
        let err_count = self
            .diagnostics
            .iter()
            .filter(|d| d.level == DiagnosticLevel::Error)
            .count();
        if err_count > 0 {
            Err(FrontendError::ParseFailed { count: err_count })
        } else {
            match self.output {
                Some(out) => Ok(out),
                None => Err(FrontendError::ParseFailed { count: 1 }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frontend_result_success() {
        let res: FrontendResult<i32> = FrontendResult::success(42);
        assert!(res.is_ok());
        assert!(!res.has_errors());
        assert!(!res.has_warnings());
        assert_eq!(res.into_result().unwrap(), 42);
    }

    #[test]
    fn test_frontend_result_failure() {
        let diag = Diagnostic::error("syntax error");
        let res: FrontendResult<i32> = FrontendResult::failure(vec![diag]);
        assert!(!res.is_ok());
        assert!(res.has_errors());
        assert_eq!(res.errors().count(), 1);
        let err = res.into_result().unwrap_err();
        assert_eq!(err, FrontendError::ParseFailed { count: 1 });
    }

    #[test]
    fn test_frontend_result_with_warnings() {
        let warn = Diagnostic::warning("deprecated syntax");
        let res: FrontendResult<i32> = FrontendResult::with_diagnostics(100, vec![warn]);
        assert!(res.is_ok());
        assert!(!res.has_errors());
        assert!(res.has_warnings());
        assert_eq!(res.warnings().count(), 1);
        assert_eq!(res.into_result().unwrap(), 100);
    }
}
