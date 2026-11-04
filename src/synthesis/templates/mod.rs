//! Fixed decomposition template library for target translation and synthesis.
//!
//! Provides mathematically verified, circuit-preserving decomposition templates for
//! discrete multi-qubit gates (SWAP, CX/CZ, Toffoli) before general two-qubit synthesis.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::rewrite::{replace_sequence, RewriteError};
use crate::ir::wire::QubitId;

pub mod cx_cz;
pub mod matrix;
pub mod registry;
pub mod swap;
pub mod toffoli;

pub use cx_cz::{CxToCzTemplate, CzToCxTemplate};
pub use matrix::Matrix8x8;
pub use registry::TemplateRegistry;
pub use swap::SwapToCxTemplate;
pub use toffoli::ToffoliToCxTemplate;

/// Unique identifier for fixed decomposition templates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TemplateId {
    /// Decomposes two-qubit `SWAP(a, b)` into three `CX` gates: `CX(a, b); CX(b, a); CX(a, b)`.
    SwapToCx,
    /// Decomposes `CX(c, t)` into `H(t); CZ(c, t); H(t)`.
    CxToCz,
    /// Decomposes `CZ(c, t)` into `H(t); CX(c, t); H(t)`.
    CzToCx,
    /// Decomposes three-qubit `Toffoli(c1, c2, t)` (CCX) into 6 CX, 2 H, 4 T, and 3 Tdg gates.
    ToffoliToCx,
}

impl fmt::Display for TemplateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SwapToCx => write!(f, "SwapToCx"),
            Self::CxToCz => write!(f, "CxToCz"),
            Self::CzToCx => write!(f, "CzToCx"),
            Self::ToffoliToCx => write!(f, "ToffoliToCx"),
        }
    }
}

/// Precondition constraint required before applying a decomposition template.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Precondition {
    /// Operands must be mutually distinct qubits.
    DistinctOperands,
    /// Operation must match the expected quantum arity.
    QuantumArity(usize),
    /// Operation must require zero continuous parameters.
    ZeroParameters,
    /// Operation must be unitary.
    UnitaryTarget,
}

impl fmt::Display for Precondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DistinctOperands => write!(f, "DistinctOperands"),
            Self::QuantumArity(n) => write!(f, "QuantumArity({n})"),
            Self::ZeroParameters => write!(f, "ZeroParameters"),
            Self::UnitaryTarget => write!(f, "UnitaryTarget"),
        }
    }
}

/// Errors encountered during template verification, expansion, or application.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum TemplateError {
    /// The number of operand qubits does not match the template arity.
    #[error("template '{template}' requires {expected} operand qubits, got {actual}")]
    ArityMismatch {
        /// Template name.
        template: String,
        /// Expected operand count.
        expected: usize,
        /// Actual operand count.
        actual: usize,
    },

    /// The operand list contains duplicate qubits.
    #[error("template '{template}' requires distinct operand qubits, but found duplicates: {operands:?}")]
    DuplicateOperands {
        /// Template name.
        template: String,
        /// Operand list containing duplicates.
        operands: Vec<QubitId>,
    },

    /// The target gate has parameters when zero were expected.
    #[error("template '{template}' expects 0 parameters, but operation has {actual} parameters")]
    NonZeroParameters {
        /// Template name.
        template: String,
        /// Encountered parameter count.
        actual: usize,
    },

    /// Template cannot be applied to a non-unitary operation.
    #[error("template '{template}' cannot be applied to non-unitary operation '{operation}'")]
    NonUnitaryOperation {
        /// Template name.
        template: String,
        /// Target operation name.
        operation: String,
    },

    /// The operation is not supported by this template.
    #[error("template '{template}' does not support operation '{operation}'")]
    UnsupportedOperation {
        /// Template name.
        template: String,
        /// Unsupported operation name.
        operation: String,
    },

    /// Mathematical verification against independent reference matrices failed.
    #[error("template verification failed for '{template}': max difference {max_diff} exceeds tolerance {tolerance}")]
    VerificationFailed {
        /// Template name.
        template: String,
        /// Maximum matrix element discrepancy.
        max_diff: f64,
        /// Verification tolerance.
        tolerance: f64,
    },

    /// Transactional DAG rewrite error during template substitution.
    #[error("transaction rewrite error during template expansion: {0}")]
    Rewrite(#[from] RewriteError),
}

/// Trait implemented by all verified fixed decomposition templates.
pub trait DecompositionTemplate: Send + Sync {
    /// Returns the unique template identifier.
    fn id(&self) -> TemplateId;

    /// Returns the human-readable template name.
    fn name(&self) -> &'static str;

    /// Returns the canonical mnemonic of the target gate decomposed by this template.
    fn target_gate(&self) -> &'static str;

    /// Number of qubit operands required by this template.
    fn quantum_arity(&self) -> usize;

    /// Circuit-level global phase angle contribution of this decomposition in radians.
    fn phase_contribution(&self) -> f64;

    /// List of formal preconditions required to apply this template.
    fn preconditions(&self) -> &'static [Precondition];

    /// Validates all preconditions against a concrete operand slice.
    fn check_preconditions(&self, operands: &[QubitId]) -> Result<(), TemplateError> {
        let expected_arity = self.quantum_arity();
        if operands.len() != expected_arity {
            return Err(TemplateError::ArityMismatch {
                template: self.name().to_string(),
                expected: expected_arity,
                actual: operands.len(),
            });
        }

        for i in 0..operands.len() {
            for j in (i + 1)..operands.len() {
                if operands[i] == operands[j] {
                    return Err(TemplateError::DuplicateOperands {
                        template: self.name().to_string(),
                        operands: operands.to_vec(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Expands the template into a concrete sequence of constituent QccIR operations.
    fn expand(&self, operands: &[QubitId]) -> Result<Vec<Operation>, TemplateError>;

    /// Transactionally replaces a matching node in `circuit` with this template's constituent operations.
    ///
    /// Preserves explicit circuit global phase and DAG convexity with automatic rollback on error.
    fn apply(
        &self,
        circuit: &mut Circuit,
        target_id: OperationId,
    ) -> Result<Vec<OperationId>, TemplateError> {
        let node = circuit
            .graph
            .get_node(target_id)
            .ok_or(RewriteError::OperationNotFound { id: target_id })?;

        let (gate_name, operands) = match &node.operation {
            Operation::Gate {
                gate_name,
                qubits,
                parameters,
            } => {
                if !parameters.is_empty() {
                    return Err(TemplateError::NonZeroParameters {
                        template: self.name().to_string(),
                        actual: parameters.len(),
                    });
                }
                (gate_name.as_str(), qubits.as_slice())
            }
            Operation::Measure { .. }
            | Operation::Reset { .. }
            | Operation::Barrier { .. }
            | Operation::Classical { .. } => {
                return Err(TemplateError::NonUnitaryOperation {
                    template: self.name().to_string(),
                    operation: node.operation.name().to_string(),
                });
            }
        };

        if !gate_name.eq_ignore_ascii_case(self.target_gate()) {
            return Err(TemplateError::UnsupportedOperation {
                template: self.name().to_string(),
                operation: gate_name.to_string(),
            });
        }

        self.check_preconditions(operands)?;
        let replacement_ops = self.expand(operands)?;

        let new_ids = replace_sequence(circuit, &[target_id], replacement_ops)?;

        let phase = self.phase_contribution();
        if phase.abs() > 1e-15 {
            circuit.add_global_phase(ParameterExpr::Constant(phase));
        }

        Ok(new_ids)
    }

    /// Verifies the decomposition template against an independently constructed unitary reference matrix.
    fn verify_matrix(&self) -> Result<(), TemplateError>;
}
