//! Error definitions for OpenQASM 3 export.

use thiserror::Error;

use crate::ir::error::{IrError, IrValidationError};
use crate::ir::node::OperationId;

/// Specific error conditions encountered when exporting a QccIR circuit to OpenQASM 3.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum OpenQasmError {
    /// The circuit failed pre-export semantic or structural validation.
    #[error("circuit validation failed before OpenQASM export: {0}")]
    Validation(#[from] IrValidationError),

    /// An underlying IR or graph error occurred during topological traversal.
    #[error("IR error during OpenQASM export: {0}")]
    Ir(#[from] IrError),

    /// A qubit operand could not be resolved to any declared quantum register.
    #[error("qubit ID {qubit} could not be resolved to any declared quantum register (total qubits: {total})")]
    UnresolvedQubit {
        /// The flat qubit index that failed resolution.
        qubit: usize,
        /// Total declared qubits in the circuit.
        total: usize,
    },

    /// A classical bit operand could not be resolved to any declared classical register.
    #[error("classical bit ID {bit} could not be resolved to any declared classical register (total bits: {total})")]
    UnresolvedClassicalBit {
        /// The flat classical bit index that failed resolution.
        bit: usize,
        /// Total declared classical bits in the circuit.
        total: usize,
    },

    /// An operation type is unsupported for OpenQASM 3 export.
    #[error("unsupported operation '{operation}' (op {op_id:?}): {reason}")]
    UnsupportedOperation {
        /// Semantic operation ID if known.
        op_id: Option<OperationId>,
        /// Name of the unsupported operation.
        operation: String,
        /// Reason why this operation cannot be emitted.
        reason: String,
    },

    /// A gate mnemonic is unsupported or not in the recognized standard OpenQASM 3 set.
    #[error("unsupported gate '{gate}' (op {op_id:?}): {reason}")]
    UnsupportedGate {
        /// Semantic operation ID if known.
        op_id: Option<OperationId>,
        /// Gate mnemonic that is unsupported.
        gate: String,
        /// Reason why this gate cannot be emitted.
        reason: String,
    },

    /// Gate arity does not match expected operand count for the gate.
    #[error("gate '{gate}' arity mismatch: expected {expected} qubits, found {found}")]
    GateArityMismatch {
        /// Gate mnemonic.
        gate: String,
        /// Expected qubit count.
        expected: usize,
        /// Provided qubit count.
        found: usize,
    },

    /// Gate parameter count does not match expected parameters for the gate.
    #[error(
        "gate '{gate}' parameter count mismatch: expected {expected} parameters, found {found}"
    )]
    GateParameterMismatch {
        /// Gate mnemonic.
        gate: String,
        /// Expected parameter count.
        expected: usize,
        /// Provided parameter count.
        found: usize,
    },

    /// A numeric parameter is non-finite (NaN or infinite).
    #[error("non-finite numeric parameter value encountered: {value}")]
    NonFiniteParameter {
        /// The non-finite float value.
        value: f64,
    },

    /// An identifier is not a valid OpenQASM 3 identifier.
    #[error("invalid OpenQASM 3 identifier: '{identifier}'")]
    InvalidIdentifier {
        /// The invalid identifier string.
        identifier: String,
    },

    /// Circuit contains a non-zero global phase when `GlobalPhasePolicy::RejectNonZero` is active.
    #[error("non-zero global phase cannot be emitted under active policy: '{phase}'")]
    NonZeroGlobalPhase {
        /// Formatted global phase expression.
        phase: String,
    },

    /// The circuit contains 0 declared qubits.
    #[error("circuit contains 0 declared qubits; cannot emit OpenQASM 3")]
    EmptyCircuit,
}
