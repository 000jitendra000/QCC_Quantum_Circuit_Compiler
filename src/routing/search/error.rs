//! Structured error types for multi-candidate routing configuration and validation.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::routing::search::objective::ObjectiveMetric;
use crate::routing::search::runner::MultiCandidateExecutionReport;

/// Structured error produced when parsing or validating multi-candidate routing search configurations.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum RoutingSearchConfigError {
    /// Candidate count is zero or exceeds the configured maximum candidate budget.
    #[error("invalid candidate count {count}: must be between {min} and {max}")]
    InvalidCandidateCount {
        /// Configured candidate count.
        count: usize,
        /// Minimum permitted candidate count (usually 1).
        min: usize,
        /// Upper bound candidate limit.
        max: usize,
    },

    /// Seed configuration is empty or malformed.
    #[error("invalid seed configuration: {0}")]
    InvalidSeedConfiguration(String),

    /// Layout candidate strategy is empty or invalid.
    #[error("invalid layout candidate configuration: {0}")]
    InvalidLayoutConfiguration(String),

    /// Search budget specifies impossible or contradictory values.
    #[error("invalid search budget: {0}")]
    InvalidBudget(String),

    /// Objective configuration is contradictory or invalid.
    #[error("invalid routing objective: {0}")]
    InvalidObjective(String),

    /// An objective requires target timing information, but the target lacks a timing model.
    ///
    /// Adheres to the "Explicit Unknown Principle" (docs/05-target-routing-and-layout.md):
    /// missing hardware timing data is never assumed to be zero.
    #[error(
        "routing objective requires metric '{metric:?}', but target '{target}' provides no hardware timing model"
    )]
    MissingTimingModel {
        /// Metric that could not be evaluated.
        metric: ObjectiveMetric,
        /// Name of the target device lacking timing data.
        target: String,
    },

    /// Conflicting configuration options detected.
    #[error("conflicting routing search configuration: {0}")]
    ConflictingConfiguration(String),
}

/// Structured error produced during multi-candidate routing search execution (Phase 75).
///
/// Preserves the accumulated [`MultiCandidateExecutionReport`] across all search failure modes
/// so that callers retain complete evidence for successful, failed, and skipped candidates.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum RoutingSearchExecutionError {
    /// Search was aborted by the `Abort` fallback policy due to failure or budget exhaustion.
    #[error("search aborted by Abort fallback policy: {reason}")]
    Aborted {
        /// Reason for abortion.
        reason: String,
        /// Accumulated candidate execution report.
        report: Box<MultiCandidateExecutionReport>,
    },

    /// The designated deterministic baseline candidate failed or was invalid, preventing fallback.
    #[error("deterministic baseline candidate failed or was invalid, preventing baseline fallback: {reason}")]
    InvalidBaselineFallback {
        /// Failure details of the baseline candidate.
        reason: String,
        /// Accumulated candidate execution report.
        report: Box<MultiCandidateExecutionReport>,
    },

    /// All executed candidates encountered routing failures.
    #[error("all {total} candidates failed routing execution: {details}")]
    AllCandidatesFailed {
        /// Total number of candidates executed.
        total: usize,
        /// Summary details of the failures.
        details: String,
        /// Accumulated candidate execution report.
        report: Box<MultiCandidateExecutionReport>,
    },

    /// Candidates completed routing but none satisfied all mandatory hard constraints.
    #[error("no candidate satisfied all mandatory validation constraints (target legality, semantic equivalence, clean ancillae)")]
    NoValidCandidates {
        /// Total number of candidates executed.
        total_executed: usize,
        /// Accumulated candidate execution report.
        report: Box<MultiCandidateExecutionReport>,
    },

    /// Configuration or target validation failed upfront before candidates could run.
    #[error("configuration error: {0}")]
    Config(#[from] RoutingSearchConfigError),
}

impl RoutingSearchExecutionError {
    /// Accesses the accumulated candidate execution report preserved in the failure, if available.
    pub fn execution_report(&self) -> Option<&MultiCandidateExecutionReport> {
        match self {
            Self::Aborted { report, .. }
            | Self::InvalidBaselineFallback { report, .. }
            | Self::AllCandidatesFailed { report, .. }
            | Self::NoValidCandidates { report, .. } => Some(report),
            Self::Config(_) => None,
        }
    }
}
