//! Candidate routing quality metric extraction, snapshotting, and objective scoring.
//!
//! Roadmap v3.1, Milestone M9 (Phase 76).
//!
//! Provides strongly typed metrics snapshotting for routed candidates, strict evaluation of
//! topological and hardware-dependent quality metrics (with fail-closed behavior for unknown
//! or uncalibrated durations), and evaluation of single, lexicographic, and weighted objectives.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::operation::Operation;
use crate::routing::sabre::SabreRouterReport;
use crate::routing::search::objective::{ObjectiveFunction, ObjectiveMetric};
use crate::target::model::Target;

/// Error evaluating an objective metric for a candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricEvaluationError {
    /// Metric requires target hardware timing, but timing data is absent or incomplete.
    DurationUnavailable {
        /// Diagnostic reason why the duration metric cannot be evaluated.
        reason: String,
    },
    /// The metric is not supported by QCC compiler infrastructure.
    UnsupportedMetric {
        /// Metric name.
        name: String,
        /// Reason metric is unsupported.
        reason: String,
    },
}

impl fmt::Display for MetricEvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DurationUnavailable { reason } => {
                write!(f, "estimated duration unavailable: {reason}")
            }
            Self::UnsupportedMetric { name, reason } => {
                write!(f, "metric '{name}' is unsupported: {reason}")
            }
        }
    }
}

impl std::error::Error for MetricEvaluationError {}

/// Captured quantitative metrics for a single successfully routed circuit candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CandidateMetricsSnapshot {
    /// Number of physical SWAP gates inserted by the router.
    pub swap_count: usize,
    /// Total count of physical CX gates in the routed circuit DAG.
    pub cx_count: usize,
    /// Total count of two-qubit operations in the routed circuit DAG.
    pub two_qubit_gate_count: usize,
    /// Total number of operations (1Q + 2Q gates, measurements, etc.) in the physical circuit DAG.
    pub total_gate_count: usize,
    /// Critical path circuit depth in the routed circuit DAG.
    pub circuit_depth: usize,
    /// Critical path makespan in nanoseconds, or `None` if uncalibrated/unknown.
    pub estimated_duration_ns: Option<f64>,
}

impl CandidateMetricsSnapshot {
    /// Extracts metrics snapshot directly from the routed physical circuit, SABRE report, and target.
    ///
    /// # Critical Invariants
    /// - **Explicit Unknown Principle**: `estimated_duration_ns` is populated only when the target
    ///   provides a calibrated timing model and all operations in the routed circuit have exact known
    ///   durations (`DurationCost::Exact`). Missing or uncalibrated durations are preserved as `None`,
    ///   **never assumed or defaulted to zero**.
    pub fn from_circuit_and_report(
        circuit: &Circuit,
        report: &SabreRouterReport,
        target: &Target,
    ) -> Self {
        let swap_count = report.swaps_inserted;

        // Count CX gates directly from circuit DAG nodes
        let cx_count = circuit
            .graph
            .nodes()
            .filter(|n| match &n.operation {
                Operation::Gate { gate_name, .. } => gate_name.eq_ignore_ascii_case("cx"),
                _ => false,
            })
            .count();

        // Count 2Q gates directly from circuit DAG nodes
        let two_qubit_gate_count = circuit
            .graph
            .nodes()
            .filter(|n| match &n.operation {
                Operation::Gate { qubits, .. } => qubits.len() == 2,
                _ => false,
            })
            .count();

        let total_gate_count = circuit.num_operations();
        let circuit_depth = report.metrics.circuit_depth;

        // Extract duration if completely and exactly known under target timing model
        let estimated_duration_ns = if target.timing.is_some() {
            report.metrics.duration.exact_ns()
        } else {
            None
        };

        Self {
            swap_count,
            cx_count,
            two_qubit_gate_count,
            total_gate_count,
            circuit_depth,
            estimated_duration_ns,
        }
    }

    /// Evaluates the value of a specific [`ObjectiveMetric`] against this snapshot.
    ///
    /// # Errors
    /// Returns [`MetricEvaluationError::DurationUnavailable`] if `EstimatedDuration` is requested
    /// but durations are uncalibrated or missing.
    pub fn get_metric(&self, metric: &ObjectiveMetric) -> Result<f64, MetricEvaluationError> {
        match metric {
            ObjectiveMetric::SwapCount => Ok(self.swap_count as f64),
            ObjectiveMetric::CxCount => Ok(self.cx_count as f64),
            ObjectiveMetric::TwoQubitGateCount => Ok(self.two_qubit_gate_count as f64),
            ObjectiveMetric::TotalGateCount => Ok(self.total_gate_count as f64),
            ObjectiveMetric::CircuitDepth => Ok(self.circuit_depth as f64),
            ObjectiveMetric::EstimatedDuration => match self.estimated_duration_ns {
                Some(ns) => Ok(ns),
                None => Err(MetricEvaluationError::DurationUnavailable {
                    reason: "hardware timing model is missing or circuit contains uncalibrated operations"
                        .to_string(),
                }),
            },
            ObjectiveMetric::Unsupported(reason) => Err(MetricEvaluationError::UnsupportedMetric {
                name: "unsupported".to_string(),
                reason: reason.clone(),
            }),
        }
    }
}

/// Evaluated objective score for a candidate under a declared objective function.
///
/// In all variants, **lower is better** (minimization direction).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CandidateObjectiveScore {
    /// Single metric minimization (e.g. `SwapCount`).
    Single {
        /// Evaluated metric.
        metric: ObjectiveMetric,
        /// Metric numeric value.
        value: f64,
    },
    /// Lexicographic multi-objective ranking vector.
    Lexicographic {
        /// Evaluated metrics in descending priority order.
        metrics: Vec<ObjectiveMetric>,
        /// Evaluated metric values corresponding to `metrics`.
        values: Vec<f64>,
    },
    /// Weighted linear composite score ($\sum w_i \cdot m_i$).
    Weighted {
        /// Composite score $\sum w_i \cdot m_i$.
        composite_score: f64,
        /// Breakdown of constituent metrics, weights, and raw values.
        components: Vec<(ObjectiveMetric, f64, f64)>,
    },
}

impl CandidateObjectiveScore {
    /// Primary scalar score representation for display and summary tables.
    #[inline]
    pub fn scalar_value(&self) -> f64 {
        match self {
            Self::Single { value, .. } => *value,
            Self::Lexicographic { values, .. } => values.first().copied().unwrap_or(0.0),
            Self::Weighted {
                composite_score, ..
            } => *composite_score,
        }
    }

    /// Compares two candidate objective scores deterministically (lower is better).
    ///
    /// Uses [`f64::total_cmp`] to guarantee a total ordering across all IEEE-754 values.
    pub fn cmp_score(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Single { value: a, .. }, Self::Single { value: b, .. }) => a.total_cmp(b),
            (Self::Lexicographic { values: a, .. }, Self::Lexicographic { values: b, .. }) => {
                for (va, vb) in a.iter().zip(b.iter()) {
                    let ord = va.total_cmp(vb);
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                a.len().cmp(&b.len())
            }
            (
                Self::Weighted {
                    composite_score: a, ..
                },
                Self::Weighted {
                    composite_score: b, ..
                },
            ) => a.total_cmp(b),
            _ => std::cmp::Ordering::Equal,
        }
    }
}

impl fmt::Display for CandidateObjectiveScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single { metric, value } => write!(f, "{} = {:.2}", metric.name(), value),
            Self::Lexicographic { metrics, values } => {
                let parts: Vec<String> = metrics
                    .iter()
                    .zip(values.iter())
                    .map(|(m, v)| format!("{}={:.2}", m.name(), v))
                    .collect();
                write!(f, "lex({})", parts.join(", "))
            }
            Self::Weighted {
                composite_score, ..
            } => write!(f, "{:.4}", composite_score),
        }
    }
}

/// Evaluator computing candidate objective scores from metrics snapshots.
pub struct ObjectiveEvaluator;

impl ObjectiveEvaluator {
    /// Evaluates candidate metrics under the specified objective function.
    ///
    /// # Errors
    /// Returns [`MetricEvaluationError`] if any metric required by the objective cannot be evaluated
    /// (e.g. `EstimatedDuration` on an uncalibrated target or circuit).
    pub fn evaluate(
        metrics: &CandidateMetricsSnapshot,
        objective: &ObjectiveFunction,
    ) -> Result<CandidateObjectiveScore, MetricEvaluationError> {
        match objective {
            ObjectiveFunction::Single(metric) => {
                let value = metrics.get_metric(metric)?;
                Ok(CandidateObjectiveScore::Single {
                    metric: metric.clone(),
                    value,
                })
            }
            ObjectiveFunction::Lexicographic(metrics_list) => {
                let mut values = Vec::with_capacity(metrics_list.len());
                for m in metrics_list {
                    let val = metrics.get_metric(m)?;
                    values.push(val);
                }
                Ok(CandidateObjectiveScore::Lexicographic {
                    metrics: metrics_list.clone(),
                    values,
                })
            }
            ObjectiveFunction::Weighted(weights) => {
                let mut composite_score = 0.0;
                let mut components = Vec::with_capacity(weights.len());
                for (metric, weight) in weights {
                    let val = metrics.get_metric(metric)?;
                    composite_score += weight * val;
                    components.push((metric.clone(), *weight, val));
                }
                Ok(CandidateObjectiveScore::Weighted {
                    composite_score,
                    components,
                })
            }
        }
    }
}
