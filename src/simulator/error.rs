use thiserror::Error;

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
}
