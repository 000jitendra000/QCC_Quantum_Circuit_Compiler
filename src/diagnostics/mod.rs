//! Diagnostic reporting, severity levels, and source span representations.

pub mod diagnostic;
pub mod qasm;
pub mod span;
pub mod target;

pub use diagnostic::{Diagnostic, DiagnosticLevel};
pub use qasm::*;
pub use span::{SourceSpan, SpanError};
pub use target::{create_target_diagnostic, TargetDiagnosticCode};
