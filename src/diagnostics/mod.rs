//! Diagnostic reporting, severity levels, and source span representations.

pub mod diagnostic;
pub mod span;

pub use diagnostic::{Diagnostic, DiagnosticLevel};
pub use span::{SourceSpan, SpanError};

