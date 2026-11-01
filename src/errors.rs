use thiserror::Error;

use crate::diagnostics::Diagnostic;

/// Standard result type alias for operations producing a [`QccError`].
pub type Result<T> = std::result::Result<T, QccError>;

/// Top-level typed error categories for the QCC compiler and runtime.
///
/// Subsystem-specific errors (I/O, CLI, Diagnostics, Configuration) are represented
/// as structured variants with contextual payloads. Subsystems implemented in future
/// roadmap phases (such as QccIR, verification, target translation, routing, and
/// simulation) will introduce dedicated error categories under this hierarchy.
#[derive(Debug, Error)]
pub enum QccError {
    /// Command-line argument parsing or execution error.
    #[error("CLI error: {message}")]
    Cli {
        /// Human-readable explanation of the CLI failure.
        message: String,
    },

    /// Low-level input/output error during file or stream operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Compiler diagnostic error associated with a source span or validation failure.
    #[error("Diagnostic error: {0}")]
    Diagnostic(#[from] Diagnostic),

    /// System or pipeline configuration error.
    #[error("Configuration error: {message}")]
    Config {
        /// Explanation of the configuration mismatch.
        message: String,
    },
}

impl QccError {
    /// Creates a new [`QccError::Cli`] with a custom message.
    pub fn cli(message: impl Into<String>) -> Self {
        Self::Cli {
            message: message.into(),
        }
    }

    /// Creates a new [`QccError::Config`] with a custom message.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = QccError::cli("unrecognized flag `--foo`");
        assert_eq!(err.to_string(), "CLI error: unrecognized flag `--foo`");

        let cfg_err = QccError::config("missing target profile");
        assert_eq!(
            cfg_err.to_string(),
            "Configuration error: missing target profile"
        );
    }
}
