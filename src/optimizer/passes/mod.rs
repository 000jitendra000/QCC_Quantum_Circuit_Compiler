//! Compiler optimization passes.
//!
//! Provides modular circuit optimization passes implementing the [`CompilerPass`](crate::opt::pass::CompilerPass)
//! trait, operating on QccIR DAG representations.

pub mod cancellation;
pub mod identity;

pub use cancellation::{
    GateCategory, InverseCancellationConfig, InverseCancellationPass, InverseCancellationStats,
    DEFAULT_CANCELLATION_TOLERANCE, DEFAULT_MAX_ITERATIONS,
};
pub use identity::{
    classify_rotation_angle, IdentityEliminationConfig, IdentityEliminationPass,
    IdentityEliminationStats, RotationIdentityKind, DEFAULT_ANGLE_TOLERANCE,
};
