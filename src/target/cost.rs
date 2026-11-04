//! Cost models, circuit metrics evaluation, and multi-objective optimization scoring.
//!
//! Evaluates gate counts, critical-path DAG depth, critical-path two-qubit depth,
//! critical-path duration, and circuit error rates.
//!
//! Adheres strictly to the Explicit Unknown Principle: circuits containing uncalibrated
//! operations yield [`DurationCost::Incomplete`] or [`DurationCost::Unknown`], **never**
//! silently substituting 0.0 ns.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::wire::QubitId;
use crate::target::metrics::{DurationCost, ErrorCost, MetricValue};
use crate::target::model::Target;
use crate::target::operation::{PhysicalQubitId, TargetOpId};

/// Characterized hardware execution cost for a single target operation invocation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OperationCost {
    /// Target operation identifier.
    pub operation: TargetOpId,
    /// Physical operand tuple.
    pub operands: Vec<PhysicalQubitId>,
    /// Quantum arity (qubit count).
    pub quantum_arity: usize,
    /// Whether this is a two-qubit operation.
    pub is_two_qubit: bool,
    /// Execution duration with tri-state certainty.
    pub duration: MetricValue<f64>,
    /// Infidelity / error estimate with tri-state certainty.
    pub error_estimate: MetricValue<f64>,
}

impl fmt::Display for OperationCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{:?} [dur: {}, err: {}]",
            self.operation, self.operands, self.duration, self.error_estimate
        )
    }
}

/// Aggregated hardware cost metrics for a physical circuit or operation sequence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CircuitCost {
    /// Total quantum gate count.
    pub total_gates: usize,
    /// Total single-qubit gate count.
    pub single_qubit_gates: usize,
    /// Total two-qubit gate count.
    pub two_qubit_gates: usize,
    /// Total multi-qubit gate count ($n \ge 2$).
    pub multi_qubit_gates: usize,
    /// Histogram of gate invocations by operation name.
    pub gate_counts: BTreeMap<TargetOpId, usize>,
    /// Critical path circuit depth in the dependency DAG.
    pub depth: usize,
    /// Critical path two-qubit depth.
    pub two_qubit_depth: usize,
    /// Critical path circuit execution duration.
    pub duration: DurationCost,
    /// Upper bound on circuit infidelity.
    pub error: ErrorCost,
}

impl CircuitCost {
    /// Constructs an empty cost evaluation for a zero-operation circuit.
    pub fn empty() -> Self {
        Self {
            total_gates: 0,
            single_qubit_gates: 0,
            two_qubit_gates: 0,
            multi_qubit_gates: 0,
            gate_counts: BTreeMap::new(),
            depth: 0,
            two_qubit_depth: 0,
            duration: DurationCost::Exact {
                total_ns: 0.0,
                is_measured_only: true,
            },
            error: ErrorCost::Exact {
                infidelity_bound: 0.0,
                is_measured_only: true,
            },
        }
    }
}

impl fmt::Display for CircuitCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "gates: {} (1Q: {}, 2Q: {}), depth: {} (2Q: {}), duration: {}, error: {}",
            self.total_gates,
            self.single_qubit_gates,
            self.two_qubit_gates,
            self.depth,
            self.two_qubit_depth,
            self.duration,
            self.error,
        )
    }
}

/// Multi-objective weight coefficients for candidate comparison and optimization scoring.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CostWeights {
    /// Weight on total quantum gate count.
    pub total_gates: f64,
    /// Weight on two-qubit gate count.
    pub two_qubit_gates: f64,
    /// Weight on critical path DAG depth.
    pub depth: f64,
    /// Weight on critical path two-qubit depth.
    pub two_qubit_depth: f64,
    /// Weight on total circuit duration in nanoseconds.
    pub duration_ns: f64,
    /// Weight on upper-bound circuit infidelity.
    pub error_rate: f64,
}

impl Default for CostWeights {
    fn default() -> Self {
        Self {
            total_gates: 1.0,
            two_qubit_gates: 10.0,
            depth: 2.0,
            two_qubit_depth: 5.0,
            duration_ns: 0.0,
            error_rate: 0.0,
        }
    }
}

/// Hardware-aware target cost model evaluating physical circuits and candidate sequences.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CostModel {
    /// Multi-objective scoring weights.
    pub weights: CostWeights,
}

impl CostModel {
    /// Creates a new cost model with the specified weight parameters.
    pub fn new(weights: CostWeights) -> Self {
        Self { weights }
    }

    /// Evaluates execution cost for a single target operation on a physical operand tuple.
    pub fn evaluate_operation(
        &self,
        target: &Target,
        op_name: &str,
        operands: &[PhysicalQubitId],
    ) -> OperationCost {
        let dur = target.query_duration(op_name, operands);
        let err = target.query_error(op_name, operands);
        let desc = target.operation(op_name);
        let arity = desc.map(|d| d.quantum_arity).unwrap_or(operands.len());
        let is_two_qubit = arity == 2;

        OperationCost {
            operation: TargetOpId::new(op_name),
            operands: operands.to_vec(),
            quantum_arity: arity,
            is_two_qubit,
            duration: dur,
            error_estimate: err,
        }
    }

    /// Evaluates cost for a sequence of physical operations with automatic concurrency tracking across wires.
    pub fn evaluate_physical_sequence<'a, I>(
        &self,
        target: &Target,
        ops: impl IntoIterator<Item = (&'a str, I)>,
    ) -> CircuitCost
    where
        I: AsRef<[PhysicalQubitId]>,
    {
        let mut total_gates = 0;
        let mut single_qubit_gates = 0;
        let mut two_qubit_gates = 0;
        let mut multi_qubit_gates = 0;
        let mut gate_counts: BTreeMap<TargetOpId, usize> = BTreeMap::new();

        let mut qubit_depths: HashMap<PhysicalQubitId, usize> = HashMap::new();
        let mut qubit_2q_depths: HashMap<PhysicalQubitId, usize> = HashMap::new();
        let mut qubit_end_times: HashMap<PhysicalQubitId, f64> = HashMap::new();

        let mut durations_measured = 0;
        let mut durations_estimated = 0;
        let mut durations_absent = 0;
        let mut known_durations_count = 0;

        let mut errors_measured = 0;
        let mut errors_estimated = 0;
        let mut errors_absent = 0;
        let mut known_errors_count = 0;
        let mut accumulated_error = 0.0;

        let mut has_operations = false;

        for (op_name, operands) in ops {
            let operands = operands.as_ref();
            has_operations = true;
            let n_qubits = operands.len();
            total_gates += 1;
            if n_qubits == 1 {
                single_qubit_gates += 1;
            } else if n_qubits == 2 {
                two_qubit_gates += 1;
                multi_qubit_gates += 1;
            } else if n_qubits > 2 {
                multi_qubit_gates += 1;
            }

            let op_id = TargetOpId::new(op_name.to_ascii_lowercase());
            *gate_counts.entry(op_id).or_default() += 1;

            // Wire-based DAG depth concurrency
            let start_depth = operands
                .iter()
                .map(|q| qubit_depths.get(q).copied().unwrap_or(0))
                .max()
                .unwrap_or(0);
            let node_depth = start_depth + 1;
            for &q in operands {
                qubit_depths.insert(q, node_depth);
            }

            // Wire-based 2Q depth concurrency
            let is_2q = n_qubits == 2;
            let start_2q_depth = operands
                .iter()
                .map(|q| qubit_2q_depths.get(q).copied().unwrap_or(0))
                .max()
                .unwrap_or(0);
            let node_2q_depth = start_2q_depth + if is_2q { 1 } else { 0 };
            for &q in operands {
                qubit_2q_depths.insert(q, node_2q_depth);
            }

            // Wire-based duration concurrency
            let dur_metric = target.query_duration(op_name, operands);
            let start_time = operands
                .iter()
                .map(|q| qubit_end_times.get(q).copied().unwrap_or(0.0))
                .fold(0.0f64, f64::max);

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
            let end_time = start_time + dur_val;
            for &q in operands {
                qubit_end_times.insert(q, end_time);
            }

            // Error rate tracking
            let err_metric = target.query_error(op_name, operands);
            match err_metric {
                MetricValue::Measured { value, .. } => {
                    errors_measured += 1;
                    known_errors_count += 1;
                    accumulated_error += value;
                }
                MetricValue::Estimated { value, .. } => {
                    errors_estimated += 1;
                    known_errors_count += 1;
                    accumulated_error += value;
                }
                MetricValue::Absent => {
                    errors_absent += 1;
                }
            }
        }

        if !has_operations {
            return CircuitCost::empty();
        }

        let depth = qubit_depths.values().copied().max().unwrap_or(0);
        let two_qubit_depth = qubit_2q_depths.values().copied().max().unwrap_or(0);
        let max_time = qubit_end_times.values().copied().fold(0.0f64, f64::max);

        let duration = if durations_absent == 0 {
            DurationCost::Exact {
                total_ns: max_time,
                is_measured_only: durations_measured > 0 && durations_estimated == 0,
            }
        } else if known_durations_count > 0 {
            DurationCost::Incomplete {
                accumulated_known_ns: max_time,
                uncalibrated_operations: durations_absent,
            }
        } else {
            DurationCost::Unknown
        };

        let error = if errors_absent == 0 {
            ErrorCost::Exact {
                infidelity_bound: accumulated_error,
                is_measured_only: errors_measured > 0 && errors_estimated == 0,
            }
        } else if known_errors_count > 0 {
            ErrorCost::Incomplete {
                known_infidelity: accumulated_error,
                uncalibrated_operations: errors_absent,
            }
        } else {
            ErrorCost::Unknown
        };

        CircuitCost {
            total_gates,
            single_qubit_gates,
            two_qubit_gates,
            multi_qubit_gates,
            gate_counts,
            depth,
            two_qubit_depth,
            duration,
            error,
        }
    }

    /// Evaluates hardware cost for a QccIR circuit assuming a 1-to-1 logical-to-physical qubit mapping.
    pub fn evaluate_circuit(&self, target: &Target, circuit: &Circuit) -> CircuitCost {
        self.evaluate_circuit_with_layout(target, circuit, &BTreeMap::new())
    }

    /// Evaluates hardware cost for a QccIR circuit with an explicit logical-to-physical qubit layout.
    pub fn evaluate_circuit_with_layout(
        &self,
        target: &Target,
        circuit: &Circuit,
        layout: &BTreeMap<QubitId, PhysicalQubitId>,
    ) -> CircuitCost {
        let order = match circuit.graph.deterministic_topological_order() {
            Ok(order) => order,
            Err(_) => return CircuitCost::empty(),
        };

        if order.is_empty() {
            return CircuitCost::empty();
        }

        let mut total_gates = 0;
        let mut single_qubit_gates = 0;
        let mut two_qubit_gates = 0;
        let mut multi_qubit_gates = 0;
        let mut gate_counts: BTreeMap<TargetOpId, usize> = BTreeMap::new();

        let mut depths: HashMap<OperationId, usize> = HashMap::with_capacity(order.len());
        let mut two_q_depths: HashMap<OperationId, usize> = HashMap::with_capacity(order.len());
        let mut end_times: HashMap<OperationId, f64> = HashMap::with_capacity(order.len());

        let mut durations_measured = 0;
        let mut durations_estimated = 0;
        let mut durations_absent = 0;
        let mut known_durations_count = 0;

        let mut errors_measured = 0;
        let mut errors_estimated = 0;
        let mut errors_absent = 0;
        let mut known_errors_count = 0;
        let mut accumulated_error = 0.0;

        for id in order {
            let Some(node) = circuit.graph.get_node(id) else {
                continue;
            };

            let predecessors = circuit.graph.predecessors(id);
            let max_pred_depth = predecessors
                .iter()
                .map(|(p, _)| depths.get(p).copied().unwrap_or(0))
                .max()
                .unwrap_or(0);
            let max_pred_2q_depth = predecessors
                .iter()
                .map(|(p, _)| two_q_depths.get(p).copied().unwrap_or(0))
                .max()
                .unwrap_or(0);
            let max_pred_time = predecessors
                .iter()
                .map(|(p, _)| end_times.get(p).copied().unwrap_or(0.0))
                .fold(0.0f64, f64::max);

            let node_depth = max_pred_depth + 1;
            depths.insert(id, node_depth);

            match &node.operation {
                Operation::Gate {
                    gate_name, qubits, ..
                } => {
                    let n_qubits = qubits.len();
                    total_gates += 1;
                    if n_qubits == 1 {
                        single_qubit_gates += 1;
                    } else if n_qubits == 2 {
                        two_qubit_gates += 1;
                        multi_qubit_gates += 1;
                    } else if n_qubits > 2 {
                        multi_qubit_gates += 1;
                    }

                    let op_id = TargetOpId::new(gate_name.to_ascii_lowercase());
                    *gate_counts.entry(op_id).or_default() += 1;

                    let is_2q = n_qubits == 2;
                    let node_2q_depth = max_pred_2q_depth + if is_2q { 1 } else { 0 };
                    two_q_depths.insert(id, node_2q_depth);

                    let phys_operands: Vec<PhysicalQubitId> = qubits
                        .iter()
                        .map(|q| {
                            layout
                                .get(q)
                                .copied()
                                .unwrap_or(PhysicalQubitId(q.0 as u32))
                        })
                        .collect();

                    let dur_metric = target.query_duration(gate_name, &phys_operands);
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
                    let end_time = max_pred_time + dur_val;
                    end_times.insert(id, end_time);

                    let err_metric = target.query_error(gate_name, &phys_operands);
                    match err_metric {
                        MetricValue::Measured { value, .. } => {
                            errors_measured += 1;
                            known_errors_count += 1;
                            accumulated_error += value;
                        }
                        MetricValue::Estimated { value, .. } => {
                            errors_estimated += 1;
                            known_errors_count += 1;
                            accumulated_error += value;
                        }
                        MetricValue::Absent => {
                            errors_absent += 1;
                        }
                    }
                }
                Operation::Measure { qubit, .. } => {
                    two_q_depths.insert(id, max_pred_2q_depth);
                    let p = layout
                        .get(qubit)
                        .copied()
                        .unwrap_or(PhysicalQubitId(qubit.0 as u32));

                    let dur_metric = target.query_readout_duration(p);
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
                    let end_time = max_pred_time + dur_val;
                    end_times.insert(id, end_time);

                    let err_metric = target.query_readout_error(p);
                    match err_metric {
                        MetricValue::Measured { value, .. } => {
                            errors_measured += 1;
                            known_errors_count += 1;
                            accumulated_error += value;
                        }
                        MetricValue::Estimated { value, .. } => {
                            errors_estimated += 1;
                            known_errors_count += 1;
                            accumulated_error += value;
                        }
                        MetricValue::Absent => {
                            errors_absent += 1;
                        }
                    }
                }
                Operation::Reset { qubit } => {
                    two_q_depths.insert(id, max_pred_2q_depth);
                    let p = layout
                        .get(qubit)
                        .copied()
                        .unwrap_or(PhysicalQubitId(qubit.0 as u32));

                    let dur_metric = target.query_duration("reset", &[p]);
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
                    let end_time = max_pred_time + dur_val;
                    end_times.insert(id, end_time);

                    let err_metric = target.query_error("reset", &[p]);
                    match err_metric {
                        MetricValue::Measured { value, .. } => {
                            errors_measured += 1;
                            known_errors_count += 1;
                            accumulated_error += value;
                        }
                        MetricValue::Estimated { value, .. } => {
                            errors_estimated += 1;
                            known_errors_count += 1;
                            accumulated_error += value;
                        }
                        MetricValue::Absent => {
                            errors_absent += 1;
                        }
                    }
                }
                Operation::Barrier { .. } | Operation::Classical { .. } => {
                    two_q_depths.insert(id, max_pred_2q_depth);
                    end_times.insert(id, max_pred_time);
                }
            }
        }

        let depth = depths.values().copied().max().unwrap_or(0);
        let two_qubit_depth = two_q_depths.values().copied().max().unwrap_or(0);
        let max_time = end_times.values().copied().fold(0.0f64, f64::max);

        let duration = if durations_absent == 0 {
            DurationCost::Exact {
                total_ns: max_time,
                is_measured_only: durations_measured > 0 && durations_estimated == 0,
            }
        } else if known_durations_count > 0 {
            DurationCost::Incomplete {
                accumulated_known_ns: max_time,
                uncalibrated_operations: durations_absent,
            }
        } else {
            DurationCost::Unknown
        };

        let error = if errors_absent == 0 {
            ErrorCost::Exact {
                infidelity_bound: accumulated_error,
                is_measured_only: errors_measured > 0 && errors_estimated == 0,
            }
        } else if known_errors_count > 0 {
            ErrorCost::Incomplete {
                known_infidelity: accumulated_error,
                uncalibrated_operations: errors_absent,
            }
        } else {
            ErrorCost::Unknown
        };

        CircuitCost {
            total_gates,
            single_qubit_gates,
            two_qubit_gates,
            multi_qubit_gates,
            gate_counts,
            depth,
            two_qubit_depth,
            duration,
            error,
        }
    }

    /// Computes the scalar multi-objective score according to the configured weights.
    ///
    /// Returns `None` if duration or error is assigned a strictly positive weight but is [`DurationCost::Unknown`]
    /// or [`ErrorCost::Unknown`].
    pub fn calculate_score(&self, cost: &CircuitCost) -> Option<f64> {
        let mut score = 0.0;
        score += self.weights.total_gates * (cost.total_gates as f64);
        score += self.weights.two_qubit_gates * (cost.two_qubit_gates as f64);
        score += self.weights.depth * (cost.depth as f64);
        score += self.weights.two_qubit_depth * (cost.two_qubit_depth as f64);

        if self.weights.duration_ns > 0.0 {
            match &cost.duration {
                DurationCost::Exact { total_ns, .. } => {
                    score += self.weights.duration_ns * total_ns;
                }
                DurationCost::Incomplete {
                    accumulated_known_ns,
                    ..
                } => {
                    score += self.weights.duration_ns * accumulated_known_ns;
                }
                DurationCost::Unknown => return None,
            }
        }

        if self.weights.error_rate > 0.0 {
            match &cost.error {
                ErrorCost::Exact {
                    infidelity_bound, ..
                } => {
                    score += self.weights.error_rate * infidelity_bound;
                }
                ErrorCost::Incomplete {
                    known_infidelity, ..
                } => {
                    score += self.weights.error_rate * known_infidelity;
                }
                ErrorCost::Unknown => return None,
            }
        }

        Some(score)
    }
}
