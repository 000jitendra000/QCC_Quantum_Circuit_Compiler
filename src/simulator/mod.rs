//! Quantum statevector simulator.
//!
//! Provides pure statevector representation with in-place $\mathcal{O}(2^n)$ kernels
//! conforming to Little-Endian qubit indexing and QCC's non-negotiable architectural invariants:
//! - Little-endian qubit indexing ($q_0$ is LSB).
//! - Column vectors and left multiplication ($B \cdot A$).
//! - In-place amplitude-pair and quartet kernels without allocating dense $2^n \times 2^n$ operators.
//! - Non-normalized state validation policy without silent renormalization.
//! - Resource limit checks against memory bounds.
//! - Terminal measurement probability calculation and seeded CDF sampling.

pub mod error;
pub mod executor;
pub mod kernels;
pub mod measurement;
pub mod operation_dispatch;
pub mod result;
pub mod sampling;
pub mod statevector;
pub mod trace;

pub use error::{ExecutionError, SimulatorError};
pub use executor::{execute_circuit, sample_circuit, CircuitExecutor};
pub use kernels::{controlled, single_qubit, two_qubit};
pub use measurement::{MeasurementSelection, ProbabilityDistribution, PROBABILITY_NORM_TOLERANCE};
pub use result::{ExecutionResult, ShotResult};
pub use sampling::{CumulativeDistribution, SeededRng, SplitMix64, TerminalSampler};
pub use statevector::{StateVector, DEFAULT_MAX_QUBITS, DEFAULT_NORM_TOLERANCE};
pub use trace::{ExecutionStep, ExecutionTrace, MeasurementRecord};
