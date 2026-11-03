//! Single- and multi-qubit unitary decomposition engines.

pub mod euler;

pub use euler::{
    decompose_euler, decompose_euler_zxz, decompose_euler_zyz, DecompositionError, EulerBasis,
    EulerDecomposition, DEFAULT_EULER_TOLERANCE, DEFAULT_GIMBAL_TOLERANCE,
};
