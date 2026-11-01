//! # QCC — Quantum Circuit Compiler
//!
//! QCC is a Rust-first quantum circuit compiler and simulator designed for
//! semantics-preserving optimization, target-aware basis translation,
//! circuit routing, and scheduling.
//!
//! ## Subsystems
//!
//! - [`quantum`]: Formal quantum semantics, endianness conventions, gate definitions, and unitary matrices.
//! - [`errors`]: Top-level typed error categories and standard [`Result`] type.
//! - [`diagnostics`]: Source span representations, diagnostic levels, and deterministic formatting.

pub mod diagnostics;
pub mod errors;
pub mod quantum;

pub use diagnostics::{Diagnostic, DiagnosticLevel, SourceSpan, SpanError};
pub use errors::{QccError, Result};
pub use quantum::{
    Angle, Gate, GlobalPhase, Matrix2x2, Matrix4x4, OperandRole, OperationCategory, SemanticClass,
};

/// Package name extracted from Cargo package metadata.
pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");

/// Package version extracted from Cargo package metadata.
pub const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the formatted version string (e.g. `"qcc 0.1.0"`).
pub fn version_string() -> String {
    format!("{PKG_NAME} {PKG_VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_string() {
        assert_eq!(
            version_string(),
            format!("qcc {}", env!("CARGO_PKG_VERSION"))
        );
    }
}
