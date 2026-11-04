//! Configurable multi-objective cost models for quantum circuit routing.
//!
//! Provides customizable weighting functions to evaluate routed circuit quality across
//! inserted SWAP count, total two-qubit entanglers, DAG depth, two-qubit depth,
//! operand direction corrections, and physical hardware duration.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::routing::metrics::RoutingMetrics;
use crate::target::metrics::DurationCost;

/// Errors that can occur during routing cost evaluation.
#[derive(Clone, Debug, PartialEq, Error, Serialize, Deserialize)]
pub enum RoutingCostError {
    /// Exact duration was required by the cost model, but some operations lacked calibration data.
    #[error(
        "exact duration required for cost evaluation, but {uncalibrated_operations} operations lack timing data"
    )]
    MissingTiming {
        /// Number of uncalibrated operations in the physical circuit.
        uncalibrated_operations: usize,
    },

    /// Exact duration was required by the cost model, but no timing metrics exist for the target.
    #[error(
        "exact duration required for cost evaluation, but target timing is completely unknown"
    )]
    UnknownTiming,

    /// A negative weight coefficient was supplied.
    #[error("cost weight for '{component}' cannot be negative: {value}")]
    NegativeWeight {
        /// Name of the cost component.
        component: &'static str,
        /// The invalid negative value.
        value: f64,
    },
}

/// Multi-objective weight coefficients for evaluating routing quality.
///
/// Each weight determines the relative penalty applied to a specific circuit attribute.
/// Component values are non-negative and have well-defined physical units.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutingCostWeights {
    /// Penalty per inserted physical routing SWAP operation.
    ///
    /// Reflects the overhead of moving non-adjacent operands into coupling adjacency.
    /// Default: 30.0 (commensurate with three CX gates).
    pub swap_count: f64,

    /// Penalty per two-qubit entangling gate in the physical circuit.
    ///
    /// Reflects hardware noise and decoherence susceptibility of entangling operations.
    /// Default: 10.0.
    pub two_qubit_gates: f64,

    /// Penalty per quantum operation (single-qubit, two-qubit, or multi-qubit).
    ///
    /// Reflects overall circuit size.
    /// Default: 1.0.
    pub total_gates: f64,

    /// Penalty per critical-path DAG depth level.
    ///
    /// Reflects sequential latency and coherence time pressure across all operations.
    /// Default: 2.0.
    pub depth: f64,

    /// Penalty per critical-path two-qubit DAG depth level.
    ///
    /// Reflects two-qubit gate scheduling congestion and depth bottleneck.
    /// Default: 5.0.
    pub two_qubit_depth: f64,

    /// Penalty per nanosecond of critical-path execution duration.
    ///
    /// Applied only when target timing metrics are available and exact.
    /// Default: 0.0 (opt-in when hardware timing calibration is available).
    pub duration_ns: f64,

    /// Penalty per direction correction operation (e.g. Hadamard reversal gates).
    ///
    /// Default: 2.0.
    pub direction_corrections: f64,
}

impl Default for RoutingCostWeights {
    fn default() -> Self {
        Self {
            swap_count: 30.0,
            two_qubit_gates: 10.0,
            total_gates: 1.0,
            depth: 2.0,
            two_qubit_depth: 5.0,
            duration_ns: 0.0,
            direction_corrections: 2.0,
        }
    }
}

impl RoutingCostWeights {
    /// Validates that all weight coefficients are non-negative and finite.
    pub fn validate(&self) -> Result<(), RoutingCostError> {
        let checks: [(&'static str, f64); 7] = [
            ("swap_count", self.swap_count),
            ("two_qubit_gates", self.two_qubit_gates),
            ("total_gates", self.total_gates),
            ("depth", self.depth),
            ("two_qubit_depth", self.two_qubit_depth),
            ("duration_ns", self.duration_ns),
            ("direction_corrections", self.direction_corrections),
        ];

        for (component, val) in checks {
            if val < 0.0 || val.is_nan() {
                return Err(RoutingCostError::NegativeWeight {
                    component,
                    value: val,
                });
            }
        }
        Ok(())
    }

    /// Creates a weight configuration that optimizes strictly for gate count and SWAPs.
    pub fn gate_count_only() -> Self {
        Self {
            swap_count: 30.0,
            two_qubit_gates: 10.0,
            total_gates: 1.0,
            depth: 0.0,
            two_qubit_depth: 0.0,
            duration_ns: 0.0,
            direction_corrections: 2.0,
        }
    }

    /// Creates a weight configuration that prioritizes critical-path DAG depth minimization.
    pub fn depth_focused() -> Self {
        Self {
            swap_count: 10.0,
            two_qubit_gates: 5.0,
            total_gates: 0.5,
            depth: 10.0,
            two_qubit_depth: 20.0,
            duration_ns: 0.0,
            direction_corrections: 2.0,
        }
    }

    /// Creates a weight configuration incorporating physical execution duration in nanoseconds.
    pub fn duration_focused(duration_ns_weight: f64) -> Self {
        Self {
            swap_count: 10.0,
            two_qubit_gates: 5.0,
            total_gates: 0.5,
            depth: 1.0,
            two_qubit_depth: 2.0,
            duration_ns: duration_ns_weight,
            direction_corrections: 2.0,
        }
    }
}

/// A configurable cost model for evaluating routing quality.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RoutingCostModel {
    /// Multi-objective weight coefficients.
    pub weights: RoutingCostWeights,
    /// Whether to require exact duration when `weights.duration_ns > 0.0`.
    pub require_exact_duration: bool,
}

impl RoutingCostModel {
    /// Creates a new routing cost model with specified weights.
    pub fn new(weights: RoutingCostWeights) -> Self {
        Self {
            weights,
            require_exact_duration: false,
        }
    }

    /// Sets whether exact duration is strictly required when duration weight is non-zero.
    pub fn with_exact_duration_required(mut self, required: bool) -> Self {
        self.require_exact_duration = required;
        self
    }

    /// Evaluates the composite weighted cost for the given [`RoutingMetrics`].
    ///
    /// If duration is incomplete or unknown, the duration component is omitted from the sum
    /// without fabricating 0.0 ns, adhering to the Explicit Unknown Principle.
    pub fn evaluate(&self, metrics: &RoutingMetrics) -> f64 {
        let mut cost = self.weights.swap_count * (metrics.routing_swaps_count as f64)
            + self.weights.two_qubit_gates * (metrics.two_qubit_gates as f64)
            + self.weights.total_gates * (metrics.total_gates as f64)
            + self.weights.depth * (metrics.circuit_depth as f64)
            + self.weights.two_qubit_depth * (metrics.two_qubit_depth as f64)
            + self.weights.direction_corrections * (metrics.direction_corrections_count as f64);

        if self.weights.duration_ns > 0.0 {
            if let Some(ns) = metrics.duration.exact_ns() {
                cost += self.weights.duration_ns * ns;
            }
        }

        cost
    }

    /// Evaluates the composite weighted cost, returning an error if validation fails
    /// or if exact duration is required but missing.
    pub fn evaluate_checked(&self, metrics: &RoutingMetrics) -> Result<f64, RoutingCostError> {
        self.weights.validate()?;

        if self.weights.duration_ns > 0.0 && self.require_exact_duration {
            match &metrics.duration {
                DurationCost::Exact { .. } => {}
                DurationCost::Incomplete {
                    uncalibrated_operations,
                    ..
                } => {
                    return Err(RoutingCostError::MissingTiming {
                        uncalibrated_operations: *uncalibrated_operations,
                    });
                }
                DurationCost::Unknown => {
                    return Err(RoutingCostError::UnknownTiming);
                }
            }
        }

        Ok(self.evaluate(metrics))
    }
}
