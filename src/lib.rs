//! # QCC — Quantum Circuit Compiler
//!
//! QCC is a Rust-first quantum circuit compiler and simulator designed for
//! semantics-preserving optimization, target-aware basis translation,
//! circuit routing, and scheduling.
//!
//! ## Subsystems
//!
//! - [`builder`]: Public Rust circuit construction API with typed handles and safety validation.
//! - [`ir`]: Canonical intermediate representation (QccIR) with dependency DAG and hazard tracking.
//! - [`quantum`]: Formal quantum semantics, endianness conventions, gate definitions, and unitary matrices.
//! - [`simulator`]: High-performance statevector simulator with in-place kernels.
//! - [`errors`]: Top-level typed error categories and standard [`Result`] type.
//! - [`diagnostics`]: Source span representations, diagnostic levels, and deterministic formatting.

pub mod builder;
pub mod diagnostics;
pub mod errors;
pub mod ir;
pub mod quantum;
pub mod simulator;

pub use builder::{
    BuilderError, CircuitBuilder, CircuitView, ClassicalBitHandle, ClassicalRegisterHandle,
    QuantumRegisterHandle, QubitHandle,
};
pub use diagnostics::{Diagnostic, DiagnosticLevel, SourceSpan, SpanError};
pub use errors::{QccError, Result};
pub use ir::{
    circuit_from_json, circuit_to_json, circuit_to_json_pretty, Circuit, CircuitGraph,
    ClassicalBitId, ClassicalRegister, DependencyKind, IrError, IrValidationError, NodeData,
    Operation, OperationId, ParameterExpr, QuantumRegister, QubitId, WireAccessState,
    WireDependency, WireId,
};
pub use quantum::{
    Angle, Gate, GlobalPhase, Matrix2x2, Matrix4x4, OperandRole, OperationCategory, SemanticClass,
};
pub use simulator::{
    ExecutionResult, MeasurementSelection, ProbabilityDistribution, SeededRng, ShotResult,
    SimulatorError, SplitMix64, StateVector, TerminalSampler,
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
