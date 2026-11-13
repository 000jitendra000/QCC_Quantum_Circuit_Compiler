//! Candidate evaluation objectives, metrics, and hard constraints for multi-candidate routing.
//!
//! Separates mandatory hard constraints (target legality, semantic equivalence, ancilla zero-state)
//! from ranking objectives (CX count, SWAP count, depth, makespan).
//!
//! # Critical Invariants
//! - **Hard Constraints vs Soft Ranking**: A candidate violating a hard constraint is disqualified
//!   regardless of its objective score.
//! - **Explicit Unknown Principle**: Missing timing or calibration metrics are modeled as `None` (`Unknown`),
//!   **never assumed to be zero**. Objectives requiring duration on targets lacking timing models are rejected.
//! - **Supported vs Unsupported Metrics**: Metrics directly measurable from QccIR topological structures
//!   are fully supported. Metrics requiring uninstantiated models (e.g. noise fidelity, crosstalk) are classified
//!   as unsupported.

use serde::{Deserialize, Serialize};

use crate::routing::search::error::RoutingSearchConfigError;

/// Specific routing quality metric evaluated when scoring candidate routes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjectiveMetric {
    /// Number of physical SWAP gates inserted during circuit routing.
    /// (Directly measurable from router report and `RoutingMetrics`).
    SwapCount,
    /// Total count of physical CX gates in the routed circuit.
    /// (Directly measurable from circuit DAG analysis).
    CxCount,
    /// Total count of two-qubit operations in the routed circuit.
    /// (Directly measurable from circuit DAG analysis).
    TwoQubitGateCount,
    /// Total operations count (1Q + 2Q gates) in the routed circuit.
    /// (Directly measurable from circuit DAG analysis).
    TotalGateCount,
    /// Topological critical path circuit depth.
    /// (Directly measurable from `CircuitMetrics::circuit_depth`).
    CircuitDepth,
    /// Post-routing physical makespan / estimated duration in nanoseconds.
    /// (Requires target hardware `TimingModel` and post-routing scheduler;
    ///  otherwise explicit Unknown/Unavailable).
    EstimatedDuration,
    /// Explicitly unsupported or deferred metric (e.g. noise fidelity, cross-talk).
    Unsupported(String),
}

impl ObjectiveMetric {
    /// Human-readable identifier for the metric.
    pub fn name(&self) -> &'static str {
        match self {
            Self::SwapCount => "swap_count",
            Self::CxCount => "cx_count",
            Self::TwoQubitGateCount => "two_qubit_gate_count",
            Self::TotalGateCount => "total_gate_count",
            Self::CircuitDepth => "circuit_depth",
            Self::EstimatedDuration => "estimated_duration_ns",
            Self::Unsupported(_) => "unsupported",
        }
    }

    /// Returns `true` if this metric can be directly computed from topological QccIR.
    pub fn is_topological(&self) -> bool {
        matches!(
            self,
            Self::SwapCount
                | Self::CxCount
                | Self::TwoQubitGateCount
                | Self::TotalGateCount
                | Self::CircuitDepth
        )
    }

    /// Returns `true` if this metric requires a hardware timing model from the target device.
    pub fn requires_timing_model(&self) -> bool {
        matches!(self, Self::EstimatedDuration)
    }

    /// Returns `true` if the metric is currently supported by QCC compiler infrastructure.
    pub fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported(_))
    }
}

/// Evaluation objective function specifying how candidate routes are ranked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ObjectiveFunction {
    /// Minimize a single metric (e.g. `ObjectiveMetric::SwapCount`).
    Single(ObjectiveMetric),
    /// Lexicographic multi-objective ranking.
    /// Candidates are ranked primarily by the first metric; ties are broken
    /// by the second metric, then third, etc.
    Lexicographic(Vec<ObjectiveMetric>),
    /// Weighted linear sum: score = $\sum w_i \cdot m_i$.
    /// All weights must be finite and $\ge 0.0$, with at least one weight $> 0.0$.
    Weighted(Vec<(ObjectiveMetric, f64)>),
}

impl ObjectiveFunction {
    /// Validates the objective function parameters.
    pub fn validate(&self) -> Result<(), RoutingSearchConfigError> {
        match self {
            Self::Single(metric) => {
                if let ObjectiveMetric::Unsupported(reason) = metric {
                    return Err(RoutingSearchConfigError::InvalidObjective(format!(
                        "metric '{reason}' is unsupported in single-metric objective"
                    )));
                }
                Ok(())
            }
            Self::Lexicographic(metrics) => {
                if metrics.is_empty() {
                    return Err(RoutingSearchConfigError::InvalidObjective(
                        "lexicographic objective requires at least one metric".to_string(),
                    ));
                }
                for m in metrics {
                    if let ObjectiveMetric::Unsupported(reason) = m {
                        return Err(RoutingSearchConfigError::InvalidObjective(format!(
                            "metric '{reason}' is unsupported in lexicographic objective"
                        )));
                    }
                }
                Ok(())
            }
            Self::Weighted(weights) => {
                if weights.is_empty() {
                    return Err(RoutingSearchConfigError::InvalidObjective(
                        "weighted objective requires at least one (metric, weight) pair"
                            .to_string(),
                    ));
                }
                let mut has_positive_weight = false;
                let mut total_weight = 0.0f64;
                for (metric, weight) in weights {
                    if let ObjectiveMetric::Unsupported(reason) = metric {
                        return Err(RoutingSearchConfigError::InvalidObjective(format!(
                            "metric '{reason}' is unsupported in weighted objective"
                        )));
                    }
                    if !weight.is_finite() || *weight < 0.0 {
                        return Err(RoutingSearchConfigError::InvalidObjective(format!(
                            "weight for metric '{:?}' must be finite and non-negative; got {weight}",
                            metric
                        )));
                    }
                    if *weight > 0.0 {
                        has_positive_weight = true;
                    }
                    total_weight += *weight;
                    if !total_weight.is_finite() {
                        return Err(RoutingSearchConfigError::InvalidObjective(
                            "sum of weights in weighted objective overflowed to non-finite"
                                .to_string(),
                        ));
                    }
                }
                if !has_positive_weight {
                    return Err(RoutingSearchConfigError::InvalidObjective(
                        "weighted objective requires at least one strictly positive weight"
                            .to_string(),
                    ));
                }
                Ok(())
            }
        }
    }

    /// Returns all metrics referenced by this objective function.
    pub fn referenced_metrics(&self) -> Vec<ObjectiveMetric> {
        match self {
            Self::Single(m) => vec![m.clone()],
            Self::Lexicographic(ms) => ms.clone(),
            Self::Weighted(pairs) => pairs.iter().map(|(m, _)| m.clone()).collect(),
        }
    }
}

/// Hard constraints that MUST be strictly satisfied for a routing candidate to be valid.
///
/// Hard constraints act as non-negotiable filters, never soft penalty terms:
/// an invalid candidate is immediately disqualified, regardless of its objective score.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HardConstraint {
    /// Physical gates and connectivity must satisfy TargetLegalityValidator fail-closed.
    TargetLegality,
    /// Routed circuit must satisfy layout-aware unitary equivalence against logical reference.
    SemanticEquivalence,
    /// Every unassigned physical qubit outside the layout must return to |0> (ADR-0001).
    AncillaZeroState,
}

impl HardConstraint {
    /// Human-readable description of the constraint.
    pub fn description(&self) -> &'static str {
        match self {
            Self::TargetLegality => "Target physical gate and connectivity legality",
            Self::SemanticEquivalence => {
                "Layout-aware unitary equivalence against logical reference"
            }
            Self::AncillaZeroState => "ADR-0001 clean ancilla zero-state preservation",
        }
    }
}

/// The complete evaluation objective contract for candidate ranking and filtering.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutingObjective {
    /// Primary objective function used to rank valid candidates.
    pub function: ObjectiveFunction,
    /// Set of mandatory hard constraints.
    pub hard_constraints: Vec<HardConstraint>,
}

impl Default for RoutingObjective {
    fn default() -> Self {
        Self::minimize_swaps()
    }
}

impl RoutingObjective {
    /// Creates a single-metric minimization objective with default mandatory hard constraints.
    pub fn single(metric: ObjectiveMetric) -> Self {
        Self {
            function: ObjectiveFunction::Single(metric),
            hard_constraints: vec![
                HardConstraint::TargetLegality,
                HardConstraint::SemanticEquivalence,
                HardConstraint::AncillaZeroState,
            ],
        }
    }

    /// Creates a default objective minimizing inserted physical SWAP count.
    pub fn minimize_swaps() -> Self {
        Self {
            function: ObjectiveFunction::Single(ObjectiveMetric::SwapCount),
            hard_constraints: vec![
                HardConstraint::TargetLegality,
                HardConstraint::SemanticEquivalence,
                HardConstraint::AncillaZeroState,
            ],
        }
    }

    /// Creates an objective minimizing topological critical path circuit depth.
    pub fn minimize_depth() -> Self {
        Self {
            function: ObjectiveFunction::Single(ObjectiveMetric::CircuitDepth),
            hard_constraints: vec![
                HardConstraint::TargetLegality,
                HardConstraint::SemanticEquivalence,
                HardConstraint::AncillaZeroState,
            ],
        }
    }

    /// Creates a lexicographic multi-objective ranking.
    pub fn lexicographic(metrics: Vec<ObjectiveMetric>) -> Self {
        Self {
            function: ObjectiveFunction::Lexicographic(metrics),
            hard_constraints: vec![
                HardConstraint::TargetLegality,
                HardConstraint::SemanticEquivalence,
                HardConstraint::AncillaZeroState,
            ],
        }
    }

    /// Creates a weighted linear composite objective.
    pub fn weighted(weights: Vec<(ObjectiveMetric, f64)>) -> Self {
        Self {
            function: ObjectiveFunction::Weighted(weights),
            hard_constraints: vec![
                HardConstraint::TargetLegality,
                HardConstraint::SemanticEquivalence,
                HardConstraint::AncillaZeroState,
            ],
        }
    }

    /// Appends a hard constraint to this objective.
    pub fn with_hard_constraint(mut self, constraint: HardConstraint) -> Self {
        if !self.hard_constraints.contains(&constraint) {
            self.hard_constraints.push(constraint);
        }
        self
    }

    /// Validates the objective contract.
    ///
    /// # Mandatory Hard Constraints
    /// All three hard constraints are strictly mandatory and cannot be omitted:
    /// - [`HardConstraint::TargetLegality`]: physical gates and connectivity legality.
    /// - [`HardConstraint::SemanticEquivalence`]: layout-aware unitary equivalence.
    /// - [`HardConstraint::AncillaZeroState`]: ADR-0001 clean ancilla zero-state preservation.
    ///
    /// Soft ranking objectives (single, lexicographic, or weighted) cannot weaken or override them.
    pub fn validate(&self) -> Result<(), RoutingSearchConfigError> {
        self.function.validate()?;
        if !self
            .hard_constraints
            .contains(&HardConstraint::TargetLegality)
        {
            return Err(RoutingSearchConfigError::InvalidObjective(
                "hard constraint 'TargetLegality' is mandatory and cannot be omitted".to_string(),
            ));
        }
        if !self
            .hard_constraints
            .contains(&HardConstraint::SemanticEquivalence)
        {
            return Err(RoutingSearchConfigError::InvalidObjective(
                "hard constraint 'SemanticEquivalence' is mandatory and cannot be omitted"
                    .to_string(),
            ));
        }
        if !self
            .hard_constraints
            .contains(&HardConstraint::AncillaZeroState)
        {
            return Err(RoutingSearchConfigError::InvalidObjective(
                "hard constraint 'AncillaZeroState' (ADR-0001) is mandatory and cannot be omitted"
                    .to_string(),
            ));
        }
        Ok(())
    }
}
