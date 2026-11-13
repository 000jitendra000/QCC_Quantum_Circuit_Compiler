//! Compiler optimization passes.
//!
//! Provides modular circuit optimization passes implementing the [`CompilerPass`](crate::opt::pass::CompilerPass)
//! trait, operating on QccIR DAG representations.

pub mod cancellation;
pub mod commutation_reorder;
pub mod commutative_cancellation;
pub mod identity;
pub mod one_qubit_fusion;
pub mod rotation_merge;

pub use cancellation::{
    GateCategory, InverseCancellationConfig, InverseCancellationPass, InverseCancellationStats,
    DEFAULT_CANCELLATION_TOLERANCE, DEFAULT_MAX_ITERATIONS,
};
pub use commutation_reorder::{
    CommutationReorderConfig, CommutationReorderPass, CommutationReorderStats, ReorderStrategy,
    DEFAULT_MAX_REWRITES, DEFAULT_REORDER_TOLERANCE, DEFAULT_REORDER_WINDOW,
};
pub use commutative_cancellation::{
    CommutativeCancellationConfig, CommutativeCancellationStats, CommutativeRejectionReason,
    CommutativeTwoQubitCancellationPass, DEFAULT_COMMUTATIVE_CANCELLATION_MAX_ITERATIONS,
    DEFAULT_COMMUTATIVE_CANCELLATION_MAX_SEARCH_DEPTH,
};
pub use identity::{
    classify_rotation_angle, IdentityEliminationConfig, IdentityEliminationPass,
    IdentityEliminationStats, RotationIdentityKind, DEFAULT_ANGLE_TOLERANCE,
};
pub use one_qubit_fusion::{
    OneQubitFusionConfig, OneQubitFusionPass, OneQubitFusionStats, DEFAULT_FUSION_TOLERANCE,
};
pub use rotation_merge::{
    RotationAxis, RotationMergeConfig, RotationMergePass, RotationMergeStats, SymbolicMergePolicy,
    DEFAULT_ROTATION_TOLERANCE,
};
