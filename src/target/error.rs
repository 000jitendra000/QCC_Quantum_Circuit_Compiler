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

    /// An invalid duration unit was supplied (e.g. cycles without clock period).
    #[error("invalid duration unit '{unit}' for {context}")]
    InvalidDurationUnit {
        /// The invalid unit description.
        unit: String,
        /// Context description.
        context: String,
    },

    /// Timing specification is inconsistent (e.g. pulse duration exceeds total duration).
    #[error("inconsistent timing for {context}: {reason}")]
    InconsistentTiming {
        /// Context description.
        context: String,
        /// Inconsistency reason.
        reason: String,
    },

    /// Timing data is missing when strict timing availability is required.
    #[error("missing timing data for operation '{operation}' on operands {operands:?}")]
    MissingTimingData {
        /// Operation name.
        operation: String,
        /// Physical operand tuple.
        operands: Vec<PhysicalQubitId>,
    },

    /// Calibration timestamp format was invalid.
    #[error("invalid calibration timestamp '{timestamp}': {reason}")]
    InvalidTimestamp {
        /// Timestamp string.
        timestamp: String,
        /// Reason for parsing failure.
        reason: String,
    },

    /// Calibration data is stale (exceeds allowable age or is expired).
    #[error(
        "stale calibration for {context}: timestamp '{timestamp}' is {age_seconds}s old relative to '{reference}' (max allowed: {max_age_seconds}s)"
    )]
    StaleCalibration {
        /// Context description.
        context: String,
        /// Calibration timestamp.
        timestamp: String,
        /// Reference comparison timestamp.
        reference: String,
        /// Observed age in seconds.
        age_seconds: u64,
        /// Maximum allowed age in seconds.
        max_age_seconds: u64,
    },

    /// Resource capacity was exceeded by concurrent operations.
    #[error(
        "resource capacity exceeded for '{resource}': requested {requested}, capacity {capacity}"
    )]
    ResourceCapacityExceeded {
        /// Resource description.
        resource: String,
        /// Requested capacity.
        requested: usize,
        /// Maximum capacity.
        capacity: usize,
    },

    /// Resource conflict between two operations.
    #[error("resource conflict between '{op1}' and '{op2}': {reason}")]
    ResourceConflictError {
        /// First operation.
        op1: String,
        /// Second operation.
        op2: String,
        /// Reason for conflict.
        reason: String,
    },

    /// Resource model configuration is invalid.
    #[error("invalid resource model: {reason}")]
    InvalidResourceModel {
        /// Failure reason.
        reason: String,
    },

    /// Target description serialization failed.
    #[error("target serialization failed: {0}")]
    Serialization(String),

    /// Target description deserialization failed or produced an invalid target.
    #[error("target deserialization failed: {0}")]
    Deserialization(String),
}
