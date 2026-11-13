//! Multi-candidate routing search configuration, evaluation objectives, and candidate models.
//!
//! Roadmap v3.1, Milestone M9 (Phase 74).
//!
//! Provides bounded candidate search configuration over SABRE seeds and initial layouts,
//! candidate provenance tracking, and explicit candidate evaluation objectives.

pub mod candidate;
pub mod config;
pub mod error;
pub mod objective;

pub use candidate::{
    compute_sabre_config_hash, compute_target_hash, derive_canonical_candidate_id,
    CandidateDescriptor, CandidateIdentity,
};
pub use config::{
    FallbackPolicy, LayoutStrategy, MultiCandidateRoutingConfig,
    MultiCandidateRoutingConfigBuilder, SearchBudget, SeedStrategy, DEFAULT_MAX_CANDIDATES,
    HARD_MAX_CANDIDATE_LIMIT,
};
pub use error::RoutingSearchConfigError;
pub use objective::{HardConstraint, ObjectiveFunction, ObjectiveMetric, RoutingObjective};
