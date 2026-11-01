//! Quantum statevector simulator.
//!
//! Provides pure statevector representation with in-place $\mathcal{O}(2^n)$ kernels
//! conforming to Little-Endian qubit indexing and QCC's non-negotiable architectural invariants:
//! - Little-endian qubit indexing ($q_0$ is LSB).
//! - Column vectors and left multiplication ($B \cdot A$).
//! - In-place amplitude-pair and quartet kernels without allocating dense $2^n \times 2^n$ operators.
//! - Non-normalized state validation policy without silent renormalization.
//! - Resource limit checks against memory bounds.

pub mod error;
pub mod kernels;
pub mod statevector;

pub use error::SimulatorError;
pub use kernels::{controlled, single_qubit, two_qubit};
pub use statevector::{StateVector, DEFAULT_MAX_QUBITS, DEFAULT_NORM_TOLERANCE};
