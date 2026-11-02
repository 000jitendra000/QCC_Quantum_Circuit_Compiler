//! # QCC — Quantum Circuit Compiler
//!
//! QCC is a Rust-first quantum circuit compiler and simulator designed for
//! semantics-preserving optimization, target-aware basis translation,
//! circuit routing, and scheduling.
//!
//! ## Subsystems
//!
//! - [`builder`]: Public Rust circuit construction API with typed handles and safety validation.
//! - [`backend`]: Code generation and serialization backends, including OpenQASM 3.
//! - [`frontend`]: Parser-independent OpenQASM frontend adapter boundary and AST.
//! - [`ir`]: Canonical intermediate representation (QccIR) with dependency DAG and hazard tracking.
//! - [`quantum`]: Formal quantum semantics, endianness conventions, gate definitions, and unitary matrices.
//! - [`simulator`]: High-performance statevector simulator with in-place kernels.
//! - [`opt`]: Deterministic compiler pass framework, analysis invalidation, and execution pipelines.
//! - [`optimizer`]: Circuit optimization, gate algebra, identity registry, and rewrite rules.
//! - [`verification`]: Global-phase-aware unitary equivalence checker and correctness oracle.
//! - [`testkit`]: Random circuit and normalized statevector generators for property testing.
//! - [`errors`]: Top-level typed error categories and standard [`Result`] type.
//! - [`diagnostics`]: Source span representations, diagnostic levels, and deterministic formatting.

pub mod backend;
pub mod builder;
pub mod diagnostics;
pub mod errors;
pub mod frontend;
pub mod ir;
pub mod opt;
pub mod optimizer;
pub mod quantum;
pub mod simulator;
pub mod testkit;
pub mod verification;

pub use backend::{
    emit_openqasm, EmitterOptions, GlobalPhasePolicy, MeasurementSyntax, OpenQasmEmitter,
    OpenQasmError,
};

pub use builder::{
    BuilderError, CircuitBuilder, CircuitView, ClassicalBitHandle, ClassicalRegisterHandle,
    QuantumRegisterHandle, QubitHandle,
};
pub use diagnostics::{Diagnostic, DiagnosticLevel, SourceSpan, SpanError};
pub use errors::{QccError, Result};
pub use frontend::{
    evaluate_expression, find_similar_gate, is_literal_zero, lookup_gate_signature, AstBarrier,
    AstBitRef, AstExprKind, AstExpression, AstGateCall, AstGlobalPhase, AstInclude, AstMeasure,
    AstProgram, AstQubitRef, AstRegisterDecl, AstRegisterKind, AstReset, AstStatement,
    AstUnsupported, AstVersion, BarrierParser, ClassicalRegisterId, DeclarationParser,
    ExpressionEvaluationError, ExpressionParser, Frontend, FrontendError, FrontendResult,
    GateParser, GateSignature, LexResult, Lexer, LexerLimits, MeasurementParser, OpenQasmFrontend,
    OperandParser, QuantumRegisterId, RegisterEntry, RegisterError, RegisterKind, RegisterTable,
    ResetParser, ResolvedBit, ResolvedQubit, SourceBuffer, Token, TokenKind, SUPPORTED_GATES,
};
pub use ir::{
    check_convexity, circuit_from_json, circuit_to_json, circuit_to_json_pretty, expand_operation,
    fuse_operations, insert_after, insert_at_end, insert_at_start, insert_before, remove_operation,
    replace_operation, Circuit, CircuitGraph, CircuitTransaction, ClassicalBitId,
    ClassicalRegister, DependencyKind, IrError, IrValidationError, NodeData, Operation,
    OperationId, ParameterExpr, QuantumRegister, QubitId, RewriteError, RewriteResult,
    WireAccessState, WireDependency, WireId,
};
pub use opt::{
    AnalysisId, CompilerPass, PassContext, PassError, PassExecutionStatus, PassExecutionTrace,
    PassId, PassManager, PassManagerError, PassManagerReport, PassMutation, PassResult,
    PassStatistics, PassTraceStep, PreservedAnalyses, SemanticDomain,
};
pub use optimizer::{
    are_inverses, compose_rotation_angles, invert_gate, invert_gate_name, invert_operation,
    is_self_inverse, normalize_rotation_angle_2pi, normalize_rotation_angle_exact,
    simplify_parameter_expr, AlgebraError, GateIdentity, GateMatrix, IdentityCategory,
    IdentityKind, IdentityRegistry,
};
pub use quantum::{
    Angle, ClassicalBitOrder, Gate, GlobalPhase, Matrix2x2, Matrix4x4, OperandRole,
    OperationCategory, SemanticClass,
};
pub use simulator::{
    execute_circuit, execute_circuit_with_initial_state, sample_circuit, CircuitExecutor,
    ExecutionError, ExecutionResult, ExecutionStep, ExecutionTrace, MeasurementRecord,
    MeasurementSelection, ProbabilityDistribution, SeededRng, ShotResult, SimulatorError,
    SplitMix64, StateVector, TerminalSampler,
};
pub use verification::{
    angular_distance, check_equivalence, shortest_angle_difference, EquivalenceChecker,
    EquivalenceVerdict, GlobalPhaseEstimator, MismatchDiagnostic, PhaseEstimationResult,
    VerificationError, VerificationMethod, VerificationTolerances,
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
