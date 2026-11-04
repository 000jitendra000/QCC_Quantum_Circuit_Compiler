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
pub mod cli;
pub mod commutation;
pub mod decomposition;
pub mod diagnostics;
pub mod errors;
pub mod frontend;
pub mod gate_catalog;
pub mod gate_signature;
pub mod ir;
pub mod opt;
pub mod optimizer;
pub mod quantum;
pub mod routing;
pub mod simulator;
pub mod synthesis;
pub mod target;
pub mod testkit;
pub mod translation;
pub mod verification;

pub use backend::{
    emit_openqasm, EmitterOptions, GlobalPhasePolicy, MeasurementSyntax, OpenQasmEmitter,
    OpenQasmError,
};

pub use builder::{
    BuilderError, CircuitBuilder, CircuitView, ClassicalBitHandle, ClassicalRegisterHandle,
    QuantumRegisterHandle, QubitHandle,
};
pub use commutation::{
    CommutationPrecondition, CommutationRegistry, CommutationResult, CommutationRule,
    NonCommutationReason,
};
pub use decomposition::{
    decompose_euler, decompose_euler_zxz, decompose_euler_zyz, DecompositionError, EulerBasis,
    EulerDecomposition, DEFAULT_EULER_TOLERANCE, DEFAULT_GIMBAL_TOLERANCE,
};
pub use diagnostics::{
    create_target_diagnostic, duplicate_declaration, duplicate_qubit_operand, gate_param_arity,
    gate_qubit_arity, index_out_of_bounds, indexed_scalar, invalid_parameter_domain,
    invalid_register_size, lowering_error, measurement_size_mismatch, non_finite_parameter, qasm,
    register_type_mismatch, reserved_keyword, undeclared_register, unindexed_array, unknown_gate,
    unresolved_symbol, unsupported_3q_gate, unsupported_feature, unsupported_gate, Diagnostic,
    DiagnosticLevel, SourceSpan, SpanError, TargetDiagnosticCode, QASM_LOWERING_ERROR,
};
pub use errors::{QccError, Result};
pub use frontend::{
    classify_unsupported_token, evaluate_expression, find_similar_gate, is_literal_zero,
    lookup_gate_signature, lower_barrier_statement, lower_classical_register_decl,
    lower_gate_statement, lower_global_phase_statement, lower_measure_statement,
    lower_parameter_expr, lower_qasm_str, lower_quantum_register_decl, lower_reset_statement,
    AstBarrier, AstBitRef, AstExprKind, AstExpression, AstGateCall, AstGlobalPhase, AstInclude,
    AstMeasure, AstProgram, AstQubitRef, AstRegisterDecl, AstRegisterKind, AstReset, AstStatement,
    AstUnsupported, AstVersion, BarrierParser, CircuitLowerer, ClassicalRegisterId,
    DeclarationParser, ExpressionEvaluationError, ExpressionParser, Frontend, FrontendError,
    FrontendResult, GateParser, LexResult, Lexer, LexerLimits, MeasurementParser, OpenQasmFrontend,
    OperandParser, QuantumRegisterId, RegisterEntry, RegisterError, RegisterKind, RegisterTable,
    ResetParser, ResolvedBit, ResolvedQubit, SemanticAnalyzer, SemanticChecker, SourceBuffer,
    SymbolTable, Token, TokenKind, UnsupportedConstruct, SUPPORTED_GATES,
};
pub use gate_catalog::{GateCatalog, CANONICAL_GATES, KNOWN_TARGET_NATIVE_CANDIDATES};
pub use gate_signature::{GateClassification, GateId, GateSignature, InverseKind, ParameterDomain};
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
    are_inverses, are_inverses_with_config, classify_rotation_angle, compose_rotation_angles,
    compute_circuit_depth, invert_gate, invert_gate_name, invert_operation, is_self_inverse,
    normalize_rotation_angle_2pi, normalize_rotation_angle_exact, simplify_parameter_expr,
    AlgebraError, CircuitMetrics, CommutationReorderConfig, CommutationReorderPass,
    CommutationReorderStats, GateCategory, GateIdentity, GateMatrix, IdentityCategory,
    IdentityEliminationConfig, IdentityEliminationPass, IdentityEliminationStats, IdentityKind,
    IdentityRegistry, InverseCancellationConfig, InverseCancellationPass, InverseCancellationStats,
    OneQubitFusionConfig, OneQubitFusionPass, OneQubitFusionStats, OptimizationLevel,
    OptimizerConfig, OptimizerError, OptimizerPipeline, OptimizerReport, PassExecutionReport,
    PipelineTermination, ReorderStrategy, RotationAxis, RotationIdentityKind, RotationMergeConfig,
    RotationMergePass, RotationMergeStats, SymbolicMergePolicy, DEFAULT_ANGLE_TOLERANCE,
    DEFAULT_CANCELLATION_TOLERANCE, DEFAULT_FUSION_TOLERANCE, DEFAULT_MAX_ITERATIONS,
    DEFAULT_MAX_OPTIMIZATION_ITERATIONS, DEFAULT_MAX_REWRITES, DEFAULT_OPTIMIZATION_TOLERANCE,
    DEFAULT_PIPELINE_REORDER_MAX_REWRITES, DEFAULT_PIPELINE_REORDER_WINDOW,
    DEFAULT_REORDER_TOLERANCE, DEFAULT_REORDER_WINDOW, DEFAULT_ROTATION_TOLERANCE,
};
pub use quantum::{
    Angle, ClassicalBitOrder, Gate, GlobalPhase, Matrix2x2, Matrix4x4, OperandRole,
    OperationCategory, SemanticClass,
};
pub use routing::{
    are_physical_neighbors, emit_physical_swap, ensure_legal_orientation, find_shortest_path,
    path_length, place_best_of, place_circuit, place_circuit_with_coupling,
    route_operands_along_path, BaselineRouter, BaselineRouterConfig, BaselineRouterReport,
    CircuitLayout, DegreePlacer, IdentityPlacer, InteractionGraph, Layout, LayoutError, LayoutMap,
    LogicalQubitId, PathSwapStrategy, PlacementError, PlacementResult, PlacementScore,
    PlacementStrategy, RandomPlacer, RoutingError, TopologyDistances, WeightedPlacer,
};
pub use simulator::{
    execute_circuit, execute_circuit_with_initial_state, sample_circuit, CircuitExecutor,
    ExecutionError, ExecutionResult, ExecutionStep, ExecutionTrace, MeasurementRecord,
    MeasurementSelection, ProbabilityDistribution, SeededRng, ShotResult, SimulatorError,
    SplitMix64, StateVector, TerminalSampler,
};
pub use synthesis::{
    normalize_angle_with_spinor_shift, CxToCzTemplate, CzToCxTemplate, DecompositionTemplate,
    Matrix8x8, Precondition, SingleQubitSynthesis, SingleQubitSynthesizer, SwapToCxTemplate,
    SynthesisError, SynthesizedOneQubitSequence, TemplateError, TemplateId, TemplateRegistry,
    ToffoliToCxTemplate,
};
pub use target::{
    CircuitCost, CostModel, CostWeights, CouplingEdge, CouplingGraph, DirectionError,
    DirectionStatus, Directionality, DurationCost, ErrorCost, LegalityError, MetricKind,
    MetricValue, OperationConnectivity, OperationCost, PhysicalQubitId, Provenance,
    QubitCalibration, QubitCalibrationEntry, SingleQubitBasis, Target, TargetBuilder,
    TargetCalibration, TargetError, TargetLegalityError, TargetLegalityPass, TargetLegalityReport,
    TargetLegalityValidator, TargetOpId, TargetOperationBuilder, TargetOperationDescriptor,
    TargetQuery, TargetResult, TargetViolation, TupleCalibration, TupleCalibrationEntry,
};
pub use translation::{
    verify_cx_reversal_matrix_c0_t1, verify_cx_reversal_matrix_c1_t0, BasisTranslationEngine,
    BasisTranslationEngineBuilder, BasisTranslationPass, CxDirectionCorrectionRule, CxToCzRule,
    CzDirectionCommutationRule, CzToCxRule, DecompositionCandidate, DirectionCorrectionRegistry,
    DirectionCorrectionReport, SingleQubitSynthesisRule, SwapToCxReversedRule, SwapToCxRule,
    SymbolicPhaseRule, SymbolicRxRule, SymbolicRyRule, TargetDirectionRule, ToffoliToCxRule,
    TranslationError, TranslationReport, TranslationRule, TranslationRuleRegistry,
    TwoQubitDirectionCorrector, TwoQubitDirectionPass,
};
pub use verification::{
    angular_distance, check_equivalence, check_layout_equivalence, embed_logical_state,
    shortest_angle_difference, EquivalenceChecker, EquivalenceVerdict, GlobalPhaseEstimator,
    LayoutEquivalenceChecker, MismatchDiagnostic, PhaseEstimationResult, VerificationError,
    VerificationMethod, VerificationTolerances,
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
