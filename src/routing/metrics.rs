//! Comprehensive routing quality metrics and critical-path DAG analysis.
//!
//! Evaluates physical circuits across DAG depth, two-qubit critical depth, gate counts,
//! inserted routing SWAPs, native entanglers, direction corrections, hardware execution duration,
//! and multi-stage layout evolution.
//!
//! # Critical Invariants
//! - **DAG-Based Depth**: Depth is computed along directed dependency edges in the QccIR DAG,
//!   reflecting true physical concurrency rather than sequential instruction counts.
//! - **Explicit Unknown Principle**: Circuits containing uncalibrated operations yield
//!   [`DurationCost::Incomplete`] or [`DurationCost::Unknown`], **never** fabricating 0.0 ns.
//! - **Reproducibility**: All metrics excluding wall-clock run time are strictly deterministic
//!   for identical circuits, targets, initial layouts, and placement seeds.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::routing::cost::RoutingCostWeights;
use crate::routing::layout::CircuitLayout;
use crate::routing::layout_map::LayoutMap;
use crate::routing::provenance::{RouteOperationKind, RouteProvenanceTracker};
use crate::target::metrics::{DurationCost, MetricValue};
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;

/// Comprehensive metrics characterizing the quality and overhead of a routed quantum circuit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutingMetrics {
    /// Critical path depth in the QccIR dependency DAG.
    pub circuit_depth: usize,

    /// Critical path depth considering only two-qubit entangling operations.
    pub two_qubit_depth: usize,

    /// Total number of operations (gates, measurements, etc.) in the physical circuit.
    pub total_gates: usize,

    /// Number of single-qubit quantum operations.
    pub single_qubit_gates: usize,

    /// Number of two-qubit quantum operations.
    pub two_qubit_gates: usize,

    /// Number of multi-qubit operations ($n > 2$).
    pub multi_qubit_gates: usize,

    /// Number of logical routing SWAP movements inserted along paths.
    pub routing_swaps_count: usize,

    /// Number of physical quantum gates emitted to implement the inserted SWAPs
    /// (e.g. 1 gate per native SWAP, or 3 CX gates per decomposed SWAP).
    pub routing_swap_gates: usize,

    /// Number of two-qubit operations originating from the original logical circuit.
    pub original_entanglers_count: usize,

    /// Number of single-qubit operations inserted for operand direction correction (e.g. Hadamards).
    pub direction_corrections_count: usize,

    /// Total overhead operations inserted purely for routing (`routing_swap_gates + direction_corrections_count`).
    pub total_routing_overhead_ops: usize,

    /// Histogram of physical operations by gate/instruction name.
    pub gate_counts: BTreeMap<String, usize>,

    /// Critical path execution duration with tri-state calibration certainty.
    pub duration: DurationCost,

    /// Initial logical-to-physical qubit layout.
    pub initial_layout: LayoutMap,

    /// Final logical-to-physical qubit layout after all routing SWAPs.
    pub final_layout: LayoutMap,

    /// Randomization seed used during placement or routing, if applicable.
    pub seed: Option<u64>,

    /// Wall-clock execution time elapsed during routing, if recorded.
    pub run_duration: Option<Duration>,

    /// Composite weighted routing cost score under configured weights.
    pub weighted_cost: f64,
}

impl fmt::Display for RoutingMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "depth: {} (2Q: {}), gates: {} (1Q: {}, 2Q: {}), swaps: {} (overhead ops: {}), duration: {}, cost: {:.2}",
            self.circuit_depth,
            self.two_qubit_depth,
            self.total_gates,
            self.single_qubit_gates,
            self.two_qubit_gates,
            self.routing_swaps_count,
            self.total_routing_overhead_ops,
            self.duration,
            self.weighted_cost,
        )
    }
}

impl RoutingMetrics {
    /// Returns `true` if all deterministic metric fields match between `self` and `other`,
    /// deliberately ignoring inherently variable measurements such as wall-clock `run_duration`.
    pub fn is_deterministic_match(&self, other: &Self) -> bool {
        self.circuit_depth == other.circuit_depth
            && self.two_qubit_depth == other.two_qubit_depth
            && self.total_gates == other.total_gates
            && self.single_qubit_gates == other.single_qubit_gates
            && self.two_qubit_gates == other.two_qubit_gates
            && self.multi_qubit_gates == other.multi_qubit_gates
            && self.routing_swaps_count == other.routing_swaps_count
            && self.routing_swap_gates == other.routing_swap_gates
            && self.original_entanglers_count == other.original_entanglers_count
            && self.direction_corrections_count == other.direction_corrections_count
            && self.total_routing_overhead_ops == other.total_routing_overhead_ops
            && self.gate_counts == other.gate_counts
            && self.duration == other.duration
            && self.initial_layout == other.initial_layout
            && self.final_layout == other.final_layout
            && self.seed == other.seed
            && (self.weighted_cost - other.weighted_cost).abs() < 1e-9
    }

    /// Evaluates metrics for a routed physical circuit using default cost weights.
    pub fn evaluate(
        circuit: &Circuit,
        target: &Target,
        layout: &CircuitLayout,
        provenance: &RouteProvenanceTracker,
        seed: Option<u64>,
        run_duration: Option<Duration>,
    ) -> Self {
        Self::evaluate_with_weights(
            circuit,
            target,
            layout,
            provenance,
            &RoutingCostWeights::default(),
            seed,
            run_duration,
        )
    }

    /// Evaluates metrics for a routed physical circuit with specified cost weights.
    pub fn evaluate_with_weights(
        circuit: &Circuit,
        target: &Target,
        layout: &CircuitLayout,
        provenance: &RouteProvenanceTracker,
        weights: &RoutingCostWeights,
        seed: Option<u64>,
        run_duration: Option<Duration>,
    ) -> Self {
        let order = circuit
            .graph
            .deterministic_topological_order()
            .unwrap_or_default();

        let mut depths: HashMap<OperationId, usize> = HashMap::new();
        let mut depths_2q: HashMap<OperationId, usize> = HashMap::new();
        let mut end_times: HashMap<OperationId, f64> = HashMap::new();

        let mut total_gates = 0;
        let mut single_qubit_gates = 0;
        let mut two_qubit_gates = 0;
        let mut multi_qubit_gates = 0;
        let mut gate_counts: BTreeMap<String, usize> = BTreeMap::new();

        let mut durations_measured = 0;
        let mut durations_estimated = 0;
        let mut durations_absent = 0;
        let mut known_durations_count = 0;

        let mut routing_swap_gates = 0;
        let mut direction_corrections_count = 0;
        let mut original_entanglers_count = 0;
        let mut unique_swaps: BTreeSet<(usize, Option<OperationId>)> = BTreeSet::new();

        for &id in &order {
            let node = match circuit.graph.get_node(id) {
                Some(n) => n,
                None => continue,
            };

            let op = &node.operation;
            let op_name = op.name().to_string();
            *gate_counts.entry(op_name.clone()).or_insert(0) += 1;
            total_gates += 1;

            let writes = &op.wire_effects().quantum_writes;
            let arity = writes.len();
            let is_2q = arity == 2;

            if arity == 1 {
                single_qubit_gates += 1;
            } else if arity == 2 {
                two_qubit_gates += 1;
            } else if arity > 2 {
                multi_qubit_gates += 1;
            }

            // Provenance classification
            if let Some(record) = provenance.get(id) {
                match record.kind {
                    RouteOperationKind::Logical { .. } => {
                        if is_2q {
                            original_entanglers_count += 1;
                        }
                    }
                    RouteOperationKind::RoutingSwap { caused_by, step } => {
                        routing_swap_gates += 1;
                        unique_swaps.insert((step, caused_by));
                    }
                    RouteOperationKind::DirectionCorrection { .. } => {
                        direction_corrections_count += 1;
                    }
                }
            }

            // DAG critical path depth
            let preds = circuit.graph.predecessors(id);
            let pred_depth = preds
                .iter()
                .map(|(p, _)| depths.get(p).copied().unwrap_or(0))
                .max()
                .unwrap_or(0);
            let node_depth = pred_depth + 1;
            depths.insert(id, node_depth);

            // DAG critical path 2Q depth
            let pred_2q_depth = preds
                .iter()
                .map(|(p, _)| depths_2q.get(p).copied().unwrap_or(0))
                .max()
                .unwrap_or(0);
            let node_2q_depth = pred_2q_depth + if is_2q { 1 } else { 0 };
            depths_2q.insert(id, node_2q_depth);

            // Hardware critical path duration
            let pred_time = preds
                .iter()
                .map(|(p, _)| end_times.get(p).copied().unwrap_or(0.0))
                .fold(0.0f64, f64::max);

            let phys_operands: Vec<PhysicalQubitId> = writes
                .iter()
                .map(|&q| PhysicalQubitId::new(q.0 as u32))
                .collect();
            let dur_metric = target.query_duration(&op_name, &phys_operands);

            let dur_val = match dur_metric {
                MetricValue::Measured { value, .. } => {
                    durations_measured += 1;
                    known_durations_count += 1;
                    value
                }
                MetricValue::Estimated { value, .. } => {
                    durations_estimated += 1;
                    known_durations_count += 1;
                    value
                }
                MetricValue::Absent => {
                    durations_absent += 1;
                    0.0
                }
            };

            let node_end_time = pred_time + dur_val;
            end_times.insert(id, node_end_time);
        }

        let circuit_depth = depths.values().copied().max().unwrap_or(0);
        let two_qubit_depth = depths_2q.values().copied().max().unwrap_or(0);
        let max_duration_ns = end_times.values().copied().fold(0.0f64, f64::max);

        let duration = if total_gates == 0 {
            DurationCost::Exact {
                total_ns: 0.0,
                is_measured_only: true,
            }
        } else if durations_absent == 0 {
            DurationCost::Exact {
                total_ns: max_duration_ns,
                is_measured_only: durations_measured > 0 && durations_estimated == 0,
            }
        } else if known_durations_count > 0 {
            DurationCost::Incomplete {
                accumulated_known_ns: max_duration_ns,
                uncalibrated_operations: durations_absent,
            }
        } else {
            DurationCost::Unknown
        };

        let routing_swaps_count = unique_swaps.len();
        let total_routing_overhead_ops = routing_swap_gates + direction_corrections_count;

        let initial_layout = layout.initial().clone();
        let final_layout = layout.current().clone();

        // Calculate weighted cost
        let mut weighted_cost = weights.swap_count * (routing_swaps_count as f64)
            + weights.two_qubit_gates * (two_qubit_gates as f64)
            + weights.total_gates * (total_gates as f64)
            + weights.depth * (circuit_depth as f64)
            + weights.two_qubit_depth * (two_qubit_depth as f64)
            + weights.direction_corrections * (direction_corrections_count as f64);

        if weights.duration_ns > 0.0 {
            if let Some(ns) = duration.exact_ns() {
                weighted_cost += weights.duration_ns * ns;
            }
        }

        Self {
            circuit_depth,
            two_qubit_depth,
            total_gates,
            single_qubit_gates,
            two_qubit_gates,
            multi_qubit_gates,
            routing_swaps_count,
            routing_swap_gates,
            original_entanglers_count,
            direction_corrections_count,
            total_routing_overhead_ops,
            gate_counts,
            duration,
            initial_layout,
            final_layout,
            seed,
            run_duration,
            weighted_cost,
        }
    }
}
