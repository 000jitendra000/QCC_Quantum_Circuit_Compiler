//! Quantum semantics, mathematical conventions, gate catalog, and matrix representations.

pub mod gate;
pub mod gate_matrix;
pub mod parameter;
pub mod semantics;

pub use gate::Gate;
pub use gate_matrix::{
    kron_2x2, matrix_cx, matrix_cx_c0_t1, matrix_cx_c1_t0, matrix_cz, matrix_h, matrix_rx,
    matrix_ry, matrix_rz, matrix_s, matrix_sdg, matrix_swap, matrix_t, matrix_tdg, matrix_x,
    matrix_y, matrix_z, Matrix2x2, Matrix4x4,
};
pub use parameter::{Angle, GlobalPhase};
pub use semantics::{
    basis_index_to_bits, bits_to_basis_index, cx_basis_action, format_basis_state,
    format_measurement_bits, parse_basis_state, ClassicalBitOrder, Endianness, OperandRole,
    OperationCategory, SemanticClass,
};
