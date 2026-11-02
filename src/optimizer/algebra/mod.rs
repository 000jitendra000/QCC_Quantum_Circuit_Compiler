//! Gate algebra and symbolic identity registry.
//!
//! Provides mathematically exact gate inverse relationships, typed identity registries,
//! rotation composition rules, and parameter normalization distinguishing exact matrix
//! equality from global-phase equivalence.

pub mod identities;
pub mod inverses;

pub use identities::{
    compose_rotation_angles, normalize_rotation_angle_2pi, normalize_rotation_angle_exact,
    simplify_parameter_expr, GateIdentity, GateMatrix, IdentityCategory, IdentityKind,
    IdentityRegistry,
};
pub use inverses::{
    are_inverses, are_inverses_with_config, invert_gate, invert_gate_name, invert_operation,
    is_self_inverse,
};

/// Errors arising in gate algebra and symbolic identity validation.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum AlgebraError {
    /// Attempted to invert an uncatalogued or custom gate without a registered inverse.
    #[error("gate '{name}' has no registered inverse")]
    NoInverseRegistered {
        /// Name of the unsupported gate.
        name: String,
    },

    /// Attempted to invert a non-unitary operation (e.g. measurement, reset, barrier).
    #[error("operation is non-unitary and cannot be inverted")]
    NonUnitaryOperation,

    /// Error evaluating or simplifying parameter expressions.
    #[error("parameter algebra error: {0}")]
    ParameterError(String),

    /// Mathematical validation of an identity failed against concrete matrices.
    #[error("identity verification failed for '{identity_id}': {reason}")]
    IdentityVerificationFailed {
        /// Identifier of the failed identity.
        identity_id: String,
        /// Mathematical reason for failure.
        reason: String,
    },
}
