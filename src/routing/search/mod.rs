//! Multi-candidate routing search configuration, evaluation objectives, candidate models,
//! and multi-run SABRE execution engine.
//!
//! Roadmap v3.1, Milestone M9 (Phases 74–75).
//!
//! Provides bounded candidate search configuration over SABRE seeds and initial layouts,
//! candidate provenance tracking, explicit candidate evaluation objectives, and multi-run
//! candidate generation with candidate isolation, budget enforcement, and validation evidence.

pub mod candidate;
pub mod config;
pub mod error;
pub mod objective;
pub mod runner;

pub use candidate::{
    compute_sabre_config_hash, compute_target_hash, derive_canonical_candidate_id,
    CandidateDescriptor, CandidateIdentity,
};
pub use config::{
    FallbackPolicy, LayoutStrategy, MultiCandidateRoutingConfig,
    MultiCandidateRoutingConfigBuilder, SearchBudget, SeedStrategy, DEFAULT_MAX_CANDIDATES,
    HARD_MAX_CANDIDATE_LIMIT,
};
pub use error::{RoutingSearchConfigError, RoutingSearchExecutionError};
pub use objective::{HardConstraint, ObjectiveFunction, ObjectiveMetric, RoutingObjective};
pub use runner::{
    BudgetExhaustionReason, CandidateExecutionRecord, CandidateExecutionStatus,
    CandidateValidationRecord, CheckStatus, MultiCandidateExecutionReport,
    MultiCandidateSearchRunner, ProvisionalCandidateSelection,
};
