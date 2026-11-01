use thiserror::Error;

use crate::ir::error::{IrError, IrValidationError};
use crate::ir::node::OperationId;

/// Specific error conditions encountered during quantum statevector simulation.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum SimulatorError {
    /// The specified qubit count is invalid (e.g. 0 or exceeding maximum limit).
    #[error("invalid qubit count: {num_qubits} (must be between 1 and {max_allowed})")]
    InvalidQubitCount {
        num_qubits: usize,
        max_allowed: usize,
    },

    /// The length of provided amplitude vector does not match $2^n$.
    #[error(
        "dimension mismatch: expected {expected} amplitudes for {num_qubits} qubits, got {actual}"
    )]
    DimensionMismatch {
        num_qubits: usize,
        expected: usize,
        actual: usize,
    },

    /// State is not normalized within the required tolerance.
    #[error("statevector is not normalized: norm squared = {norm_sqr:.8}, expected 1.0 (tolerance = {tolerance:.1e})")]
    NonNormalizedState { norm_sqr: f64, tolerance: f64 },

    /// An amplitude value is invalid (e.g. NaN or infinite).
    #[error("invalid amplitude at index {index}: {reason}")]
    InvalidAmplitude { index: usize, reason: &'static str },

    /// Target qubit index is out of bounds for the register size.
    #[error("target qubit index {target} is out of bounds for register of size {num_qubits}")]
    TargetOutOfBounds { target: usize, num_qubits: usize },

    /// Multi-qubit gate operands must refer to distinct qubits.
    #[error("identical qubit operands: operand 0 and operand 1 both refer to qubit {qubit}")]
    IdenticalQubitOperands { qubit: usize },

    /// A gate operation is unsupported or incompatible with the kernel.
    #[error("unsupported gate {gate}: {reason}")]
    UnsupportedGate { gate: String, reason: &'static str },

    /// Computational basis state index exceeds $2^n - 1$.
    #[error(
        "basis index {index} is out of bounds for {num_qubits} qubits (max index is {max_index})"
    )]
    BasisIndexOutOfBounds {
        index: usize,
        num_qubits: usize,
        max_index: usize,
    },

    /// Memory required for the statevector exceeds configured resource limits.
    #[error("resource limit exceeded: requested {requested_bytes} bytes for {num_qubits} qubits, maximum allowed is {max_allowed_bytes} bytes")]
    ResourceLimitExceeded {
        num_qubits: usize,
        requested_bytes: usize,
        max_allowed_bytes: usize,
    },

    /// Measurement selection must specify at least one qubit.
    #[error("empty measurement selection: at least one qubit must be measured")]
    EmptyMeasurementSelection,

    /// Measurement selection contains duplicate qubits.
    #[error("duplicate qubit in measurement selection: qubit {qubit} specified multiple times")]
    DuplicateMeasurementQubit { qubit: usize },

    /// Shot count must be non-zero.
    #[error("invalid shot count: requested {count} shots (must be at least 1)")]
    InvalidShotCount { count: usize },

    /// Probability distribution is invalid (e.g. non-normalized, contains negative or NaN values).
    #[error("invalid probability distribution: {reason}")]
    InvalidProbabilityDistribution { reason: &'static str },

    /// Mid-circuit measurement is unsupported in terminal sampling mode.
    #[error(
        "mid-circuit measurement or dynamic feedback is unsupported in terminal sampling mode"
    )]
    MidCircuitMeasurementUnsupported,
}

/// Errors produced during QccIR DAG execution by the simulator adapter.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum ExecutionError {
    /// The circuit failed pre-execution semantic or structural validation.
    #[error("circuit validation failed: {0}")]
    Validation(#[from] IrValidationError),

    /// An underlying IR or graph error occurred during traversal.
    #[error("IR error: {0}")]
    Ir(#[from] IrError),

    /// A low-level simulator kernel or memory error occurred.
    #[error("simulator error: {0}")]
    Simulator(#[from] SimulatorError),

    /// An operation type is unsupported by the execution adapter.
    #[error("unsupported operation '{operation}': {reason}")]
    UnsupportedOperation { operation: String, reason: String },

    /// A gate is unsupported by the simulator kernels.
    #[error("unsupported gate '{gate}': {reason}")]
    UnsupportedGate { gate: String, reason: String },

    /// A symbolic parameter expression could not be evaluated due to a missing variable.
    #[error(
        "symbolic parameter '{symbol}' could not be evaluated (variable not found in environment)"
    )]
    UnevaluatedParameter { symbol: String },

    /// A mid-circuit measurement was detected on a qubit that underwent subsequent operations.
    #[error("mid-circuit measurement or dynamic feedback detected on operation {op_id}: {reason}")]
    MidCircuitMeasurement { op_id: OperationId, reason: String },

    /// A classical operation depends dynamically on measurement outcomes.
    #[error("dynamic feedback unsupported: operation {op_id} depends on measurement outcome")]
    DynamicFeedbackUnsupported { op_id: OperationId },

    /// The circuit contains 0 declared qubits.
    #[error("execution requires at least one qubit, but circuit declares 0 qubits")]
    ZeroQubitCircuit,
}
