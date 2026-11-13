//! Structured error types for multi-candidate routing configuration and validation.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::routing::search::objective::ObjectiveMetric;

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
