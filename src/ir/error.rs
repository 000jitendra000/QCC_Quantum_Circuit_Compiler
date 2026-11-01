//! Error types for QccIR data structures, graph construction, and validation.

use thiserror::Error;

use crate::ir::node::OperationId;

/// Primary error enum for QccIR construction and manipulation operations.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum IrError {
    /// A directed cycle was detected in the dependency graph.
    #[error("cycle detected in QccIR dependency graph")]
    CycleDetected,

    /// Operation with the specified ID was not found in the graph.
    #[error("operation ID {id:?} not found in circuit graph")]
    UnknownOperation { id: OperationId },

    /// Referenced qubit is out of bounds for declared registers.
    #[error("qubit index {qubit} is out of bounds (total declared qubits: {total})")]
    InvalidQubit { qubit: usize, total: usize },

    /// Referenced classical bit is out of bounds for declared registers.
    #[error("classical bit index {bit} is out of bounds (total declared classical bits: {total})")]
    InvalidClassicalBit { bit: usize, total: usize },

    /// Register name is unknown.
    #[error("unknown register '{name}'")]
    UnknownRegister { name: String },

    /// Duplicate register declaration.
    #[error("register '{name}' is already declared")]
    DuplicateRegister { name: String },

    /// Gate arity does not match expected arity from catalog.
    #[error("gate '{gate}' arity mismatch: expected {expected} qubits, got {actual}")]
    GateArityMismatch {
        gate: String,
        expected: usize,
        actual: usize,
    },

    /// Gate parameter count does not match expected count from catalog.
    #[error("gate '{gate}' parameter mismatch: expected {expected} parameters, got {actual}")]
    GateParameterMismatch {
        gate: String,
        expected: usize,
        actual: usize,
    },

    /// Measurement operation has invalid operands.
    #[error("invalid measurement: {reason}")]
    InvalidMeasurement { reason: &'static str },

    /// Dependency edge references an invalid endpoint or wire.
    #[error("invalid dependency edge: {reason}")]
    InvalidEdge { reason: String },

    /// Serialization or deserialization error.
    #[error("serialization error: {message}")]
    SerializationError { message: String },

    /// Validation error during executable semantic checks.
    #[error("validation error: {0}")]
    Validation(#[from] IrValidationError),
}

/// Detailed validation error categories for executable semantic checks.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum IrValidationError {
    /// Graph contains a directed cycle.
    #[error("acyclicity invariant violated: directed cycle detected")]
    CycleDetected,

    /// Duplicate or out-of-order operation IDs.
    #[error("invalid operation ID sequence: {message}")]
    InvalidOperationIdSequence { message: String },

    /// Qubit reference exceeds declared capacity.
    #[error("undeclared qubit {qubit} referenced by operation {op_id:?}")]
    UndeclaredQubit { op_id: OperationId, qubit: usize },

    /// Classical bit reference exceeds declared capacity.
    #[error("undeclared classical bit {bit} referenced by operation {op_id:?}")]
    UndeclaredClassicalBit { op_id: OperationId, bit: usize },

    /// Gate arity mismatch in validation.
    #[error("gate '{gate}' in operation {op_id:?} has invalid arity (expected {expected}, found {actual})")]
    GateArityMismatch {
        op_id: OperationId,
        gate: String,
        expected: usize,
        actual: usize,
    },

    /// Gate parameter count mismatch in validation.
    #[error("gate '{gate}' in operation {op_id:?} has invalid parameter count (expected {expected}, found {actual})")]
    GateParameterMismatch {
        op_id: OperationId,
        gate: String,
        expected: usize,
        actual: usize,
    },

    /// Wire dependency inconsistency.
    #[error("wire dependency error: {reason}")]
    WireDependencyInconsistent { reason: String },

    /// Empty register or invalid register configuration.
    #[error("invalid register configuration: {reason}")]
    InvalidRegisterConfiguration { reason: String },
}
