//! Multi-candidate routing search configuration, evaluation objectives, candidate models,
//! multi-run SABRE execution engine, and deterministic candidate ranking and selection.
//!
//! Roadmap v3.1, Milestone M9 (Phases 74–76).
//!
//! Provides bounded candidate search configuration over SABRE seeds and initial layouts,
//! candidate provenance tracking, explicit candidate evaluation objectives, multi-run
//! candidate generation with candidate isolation, cumulative budget enforcement, validation evidence,
//! Pareto dominance analysis, deterministic tie-breaking, and explainability reporting.

pub mod candidate;
pub mod config;
pub mod error;
pub mod evaluator;
pub mod objective;
pub mod ranking;
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
pub use evaluator::{
    CandidateMetricsSnapshot, CandidateObjectiveScore, MetricEvaluationError, ObjectiveEvaluator,
};
pub use objective::{HardConstraint, ObjectiveFunction, ObjectiveMetric, RoutingObjective};
pub use ranking::{
    CandidateEvaluationRecord, CandidateRankEligibility, RankingEngine, RankingReport,
};
pub use runner::{
    BudgetExhaustionReason, CandidateExecutionRecord, CandidateExecutionStatus, CandidateSelection,
    CandidateValidationRecord, CheckStatus, MultiCandidateExecutionReport,
    MultiCandidateSearchRunner, ProvisionalCandidateSelection,
};
