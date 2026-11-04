//! Simulation kernels for quantum operations.
//!
//! Includes in-place single-qubit amplitude-pair and two-qubit amplitude-quartet transformation kernels.

pub mod controlled;
pub mod single_qubit;
pub mod two_qubit;

pub use controlled::{apply_ccx, apply_controlled_2x2_kernel, apply_cx, apply_cz};
pub use single_qubit::*;
pub use two_qubit::{
    apply_4x4_kernel, apply_swap, apply_two_qubit_gate_kernel, quartet_indices, AmplitudeQuartet,
    QuartetIterator,
};
