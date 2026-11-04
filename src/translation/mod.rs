//! Basis translation engine, translation rules, compilation passes, and candidate cost optimization.
//!
//! Translates high-level logical quantum circuits into equivalent physical circuits consisting
//! strictly of hardware-native operations supported by a generic [`crate::target::model::Target`].
//!
//! # Architecture
//!
//! - [`BasisTranslationEngine`]: Core translation engine performing recursive decomposition,
//!   cycle and recursion-depth protection, cost-based candidate selection, local unitary equivalence
//!   verification, and target legality enforcement.
//! - [`TranslationRule`]: Pluggable decomposition rule trait for decomposing unsupported operations.
//! - [`TranslationRuleRegistry`]: Extensible registry of decomposition rules with verified template
//!   wrappers, single-qubit synthesis, and symbolic parameter preservation.
//! - [`BasisTranslationPass`]: Compiler pass implementing [`crate::opt::pass::CompilerPass`].

pub mod engine;
pub mod pass;
pub mod rules;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ir::error::{IrError, IrValidationError};
use crate::ir::rewrite::RewriteError;
use crate::synthesis::single_qubit::SynthesisError;
use crate::synthesis::templates::TemplateError;
use crate::target::cost::CircuitCost;
use crate::target::operation::PhysicalQubitId;
use crate::target::query::LegalityError;

pub use engine::{BasisTranslationEngine, BasisTranslationEngineBuilder};
pub use pass::BasisTranslationPass;
pub use rules::{
    CxToCzRule, CzToCxRule, DecompositionCandidate, SingleQubitSynthesisRule, SwapToCxReversedRule,
    SwapToCxRule, SymbolicPhaseRule, SymbolicRxRule, SymbolicRyRule, ToffoliToCxRule,
    TranslationRule, TranslationRuleRegistry,
};

/// Structured error type for basis translation failures.
#[derive(Debug, Error)]
pub enum TranslationError {
    /// No viable decomposition rule exists or all decomposition candidates failed to translate.
    #[error("no viable translation found for operation '{op_name}' on physical operands {operands:?} (tried {candidates_tried} candidate paths): {reason}")]
    NoViableTranslation {
        /// Unsupported operation mnemonic.
        op_name: String,
        /// Physical qubit operands.
        operands: Vec<PhysicalQubitId>,
        /// Number of candidate decomposition paths attempted.
        candidates_tried: usize,
        /// Explanation of failure.
        reason: String,
    },

    /// A recursive decomposition cycle was detected (e.g. $A \to B \to A$).
    #[error("recursive decomposition cycle detected for operation '{op_name}': {cycle_path}")]
    CycleDetected {
        /// Operation at which cycle closed.
        op_name: String,
        /// Full decomposition trail showing the cycle.
        cycle_path: String,
    },

    /// Maximum recursive decomposition depth was exceeded.
    #[error("maximum recursion depth {max_depth} exceeded while decomposing operation '{op_name}' (current depth: {depth})")]
    MaxRecursionDepthExceeded {
        /// Operation being decomposed.
        op_name: String,
        /// Reached recursion depth.
        depth: usize,
        /// Maximum allowed depth.
        max_depth: usize,
    },

    /// Local unitary equivalence verification failed for a candidate decomposition.
    #[error("local unitary equivalence verification failed for decomposition of '{op_name}': max difference {max_diff:.3e} exceeds tolerance {tolerance:.3e}")]
    VerificationFailed {
        /// Decomposed operation mnemonic.
        op_name: String,
        /// Observed maximum element discrepancy.
        max_diff: f64,
        /// Allowed tolerance threshold.
        tolerance: f64,
    },

    /// Target legality violation for an emitted operation.
    #[error("target legality violation during basis translation: {0}")]
    Legality(#[from] LegalityError),

    /// Error originating from native single-qubit synthesis.
    #[error("single-qubit synthesis error: {0}")]
    Synthesis(#[from] SynthesisError),

    /// Error originating from decomposition templates.
    #[error("decomposition template error: {0}")]
    Template(#[from] TemplateError),

    /// Transactional DAG rewrite error.
    #[error("DAG rewrite error: {0}")]
    Rewrite(#[from] RewriteError),

    /// Intermediate representation error.
    #[error("IR error: {0}")]
    Ir(#[from] IrError),

    /// Circuit validation error.
    #[error("circuit validation error: {0}")]
    Validation(#[from] IrValidationError),
}

/// Telemetry metrics and diagnostics collected during basis translation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranslationReport {
    /// Number of operations examined in the input circuit.
    pub operations_examined: usize,
    /// Number of operations already supported by the target that were preserved without changes.
    pub operations_preserved: usize,
    /// Number of operations that were translated / decomposed.
    pub operations_decomposed: usize,
    /// Total number of operations emitted in the translated circuit.
    pub total_emitted_operations: usize,
    /// Total scalar global phase delta added to the circuit in radians.
    pub global_phase_delta: f64,
    /// Initial hardware execution cost before translation.
    pub initial_cost: CircuitCost,
    /// Final hardware execution cost after translation.
    pub final_cost: CircuitCost,
}
