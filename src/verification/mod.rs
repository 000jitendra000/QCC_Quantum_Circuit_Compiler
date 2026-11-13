//! Verification and equivalence checking subsystem.
//!
//! Provides the primary correctness oracle for QCC: certifying that candidate circuits
//! preserve unitary action up to exactly one single common global phase.

pub mod global_phase;
pub mod layout_equivalence;
pub mod tolerance;
pub mod unitary;

pub use global_phase::{
    angular_distance, shortest_angle_difference, GlobalPhaseEstimator, PhaseEstimationResult,
};
pub use layout_equivalence::{
    check_layout_equivalence, embed_logical_state, LayoutEquivalenceChecker,
    LayoutEquivalenceReport,
};
pub use tolerance::VerificationTolerances;
pub use unitary::{
    check_equivalence, EquivalenceChecker, EquivalenceVerdict, MismatchDiagnostic,
    VerificationError, VerificationMethod,
};
