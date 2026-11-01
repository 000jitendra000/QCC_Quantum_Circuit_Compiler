//! Strongly typed error categories for the circuit builder API.

use thiserror::Error;

use crate::ir::error::{IrError, IrValidationError};
use crate::ir::wire::QubitId;

/// Errors produced during circuit construction through [`CircuitBuilder`](crate::builder::CircuitBuilder).
#[derive(Clone, Debug, PartialEq, Error)]
pub enum BuilderError {
    /// A handle from a different builder instance was passed.
    #[error("handle belongs to a different builder (expected builder #{expected}, got #{actual})")]
    ForeignHandle { expected: u64, actual: u64 },

    /// The referenced register name is not declared in this circuit.
    #[error("unknown register '{name}'")]
    UnknownRegister { name: String },

    /// A register with this name is already declared.
    #[error("register '{name}' is already declared")]
    DuplicateRegister { name: String },

    /// Register capacity must be strictly greater than zero.
    #[error("register size must be positive, got 0 for '{name}'")]
    EmptyRegister { name: String },

    /// Register index is out of bounds for the declared capacity.
    #[error("index {index} out of bounds for register '{register}' of size {size}")]
    IndexOutOfBounds {
        register: String,
        index: usize,
        size: usize,
    },

    /// Gate arity does not match expected arity from catalog.
    #[error("gate '{gate}' arity mismatch: expected {expected} qubits, got {actual}")]
    GateArityMismatch {
        gate: String,
        expected: usize,
        actual: usize,
    },

    /// Gate parameter count does not match expected parameter count.
    #[error("gate '{gate}' parameter mismatch: expected {expected} parameters, got {actual}")]
    GateParameterMismatch {
        gate: String,
        expected: usize,
        actual: usize,
    },

    /// Multi-qubit gate or barrier received duplicate operand qubits.
    #[error("duplicate operand qubit {qubit} in operation '{operation}'")]
    DuplicateOperandQubit { operation: String, qubit: QubitId },

    /// Circuit validation error during `finish()`.
    #[error("circuit validation failed: {0}")]
    Validation(#[from] IrValidationError),

    /// Underlying intermediate representation error.
    #[error("IR error: {0}")]
    Ir(#[from] IrError),
}
