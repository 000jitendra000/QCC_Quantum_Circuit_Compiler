//! Error types for the generic target data model.

use thiserror::Error;

use crate::target::operation::PhysicalQubitId;

/// Result alias for target modeling operations.
pub type TargetResult<T> = Result<T, TargetError>;

/// Errors encountered during target construction, validation, and serialization.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum TargetError {
    /// Target was declared with zero physical qubits.
    #[error("target qubit count must be greater than zero, got 0")]
    ZeroQubits,

    /// Target was declared without any supported operations.
    #[error("target '{0}' must declare at least one supported operation")]
    NoOperations(String),

    /// A physical qubit index was referenced that equals or exceeds the target qubit capacity.
    #[error("physical qubit {qubit} out of bounds for target '{target}' with capacity {capacity}")]
    QubitOutOfBounds {
        /// Out-of-bounds physical qubit.
        qubit: PhysicalQubitId,
        /// Name of the target.
        target: String,
        /// Qubit capacity of the target.
        capacity: usize,
    },

    /// An operation declared an invalid quantum arity.
    #[error("operation '{name}' on target '{target}' has invalid quantum arity {arity} (must be in range 1..={capacity})")]
    InvalidArity {
        /// Name of the operation.
        name: String,
        /// Target name.
        target: String,
        /// Declared arity.
        arity: usize,
        /// Physical qubit capacity.
        capacity: usize,
    },

    /// The number of physical qubits in an operand tuple does not match the operation's declared arity.
    #[error("operation '{operation}' expects quantum arity {expected}, but operand tuple has length {actual}: {operands:?}")]
    ArityMismatch {
        /// Operation name.
        operation: String,
        /// Declared arity.
        expected: usize,
        /// Actual operand count.
        actual: usize,
        /// The invalid operand tuple.
        operands: Vec<PhysicalQubitId>,
    },

    /// An operand tuple contains duplicate physical qubits (e.g. `(p0, p0)`).
    #[error("operation '{operation}' operand tuple contains duplicate physical qubit {qubit}: {operands:?}")]
    DuplicateOperandQubit {
        /// Operation name.
        operation: String,
        /// The duplicated physical qubit.
        qubit: PhysicalQubitId,
        /// The invalid operand tuple.
        operands: Vec<PhysicalQubitId>,
    },

    /// A coupling edge connects a physical qubit to itself.
    #[error(
        "operation '{operation}' connectivity edge ({qubit}, {qubit}) is an invalid self-loop"
    )]
    SelfLoopEdge {
        /// Operation name.
        operation: String,
        /// The looped physical qubit.
        qubit: PhysicalQubitId,
    },

    /// An error estimate (infidelity) was outside the physical probability range $[0.0, 1.0]$.
    #[error(
        "invalid error estimate {value} for {context}: must be finite and in range [0.0, 1.0]"
    )]
    InvalidErrorEstimate {
        /// Invalid value.
        value: f64,
        /// Context description.
        context: String,
    },

    /// A duration was negative, infinite, or NaN.
    #[error("invalid duration {value} for {context}: must be finite and non-negative")]
    InvalidDuration {
        /// Invalid value.
        value: f64,
        /// Context description.
        context: String,
    },

    /// Calibration data references an operation that is not declared on the target.
    #[error("calibration data references unknown operation '{operation}' on target '{target}'")]
    UnknownOperation {
        /// Unknown operation name.
        operation: String,
        /// Target name.
        target: String,
    },

    /// Target description serialization failed.
    #[error("target serialization failed: {0}")]
    Serialization(String),

    /// Target description deserialization failed or produced an invalid target.
    #[error("target deserialization failed: {0}")]
    Deserialization(String),
}
