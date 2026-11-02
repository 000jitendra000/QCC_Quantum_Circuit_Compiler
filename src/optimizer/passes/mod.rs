//! Compiler optimization passes.
//!
//! Provides modular circuit optimization passes implementing the [`CompilerPass`](crate::opt::pass::CompilerPass)
//! trait, operating on QccIR DAG representations.

pub mod identity;

pub use identity::{
    classify_rotation_angle, IdentityEliminationConfig, IdentityEliminationPass,
    IdentityEliminationStats, RotationIdentityKind, DEFAULT_ANGLE_TOLERANCE,
};
