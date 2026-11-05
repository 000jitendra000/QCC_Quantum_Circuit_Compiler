//! Independent schedule verification and validation engine.
//!
//! Validates scheduled execution timelines against QccIR DAG dependencies, physical qubit
//! single occupancy, target duration calibrations, shared resource capacities, and mutual exclusions.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::scheduler::schedule::{Schedule, ScheduledOperation};
use crate::target::model::Target;
use crate::target::operation::{PhysicalQubitId, TargetOpId};
use crate::target::resources::{ResourceId, ResourceRequirement, ResourceUsageMode};

/// Specific schedule validation error variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScheduleValidationError {
    /// An operation from the circuit DAG was missing from the schedule.
    MissingOperation {
        /// Stable node identifier.
        op_id: OperationId,
        /// Operation mnemonic.
        op_name: String,
    },

    /// The schedule contains a duplicate entry for an operation.
    DuplicateOperation {
        /// Stable node identifier.
        op_id: OperationId,
    },

    /// The schedule contains an operation not present in the circuit DAG.
    UnexpectedOperation {
        /// Stable node identifier.
        op_id: OperationId,
    },

    /// Scheduled operation name does not match the DAG node operation.
    OperationMismatch {
        /// Stable node identifier.
        op_id: OperationId,
        /// Expected name from circuit DAG.
        expected_name: String,
        /// Actual name recorded in schedule.
        actual_name: String,
    },

    /// Scheduled operands do not match the DAG node operands.
    OperandMismatch {
        /// Stable node identifier.
        op_id: OperationId,
        /// Expected physical operands from circuit DAG.
        expected_operands: Vec<PhysicalQubitId>,
        /// Actual physical operands recorded in schedule.
        actual_operands: Vec<PhysicalQubitId>,
    },

    /// An operation starts before one of its DAG predecessors has completed.
    PredecessorViolation {
        /// Dependent operation ID.
        op_id: OperationId,
        /// Predecessor operation ID that finished too late.
        pred_id: OperationId,
        /// Actual start time of dependent operation in nanoseconds.
        op_start: f64,
        /// Actual end time of predecessor operation in nanoseconds.
        pred_end: f64,
    },

    /// Scheduled operation has invalid or negative timing values.
    InvalidTiming {
        /// Operation ID.
        op_id: OperationId,
        /// Detailed explanation.
        reason: String,
    },

    /// Scheduled duration does not match calibrated duration from target.
    DurationMismatch {
        /// Operation ID.
        op_id: OperationId,
        /// Expected duration in nanoseconds from target.
        expected_ns: f64,
        /// Actual duration recorded in the schedule in nanoseconds.
        actual_ns: f64,
    },

    /// Operation has unknown or uncalibrated duration.
    UnknownDuration {
        /// Operation ID.
        op_id: OperationId,
        /// Operation mnemonic.
        op_name: String,
        /// Physical operands.
        operands: Vec<PhysicalQubitId>,
    },

    /// Two concurrent operations violate physical qubit single-occupancy.
    QubitCollision {
        /// First operation ID.
        op1: OperationId,
        /// Second operation ID.
        op2: OperationId,
        /// Contested physical qubit.
        qubit: PhysicalQubitId,
        /// Overlapping time interval in nanoseconds [start, end].
        interval: (f64, f64),
    },

    /// Two concurrent operations collide on the same physical coupler channel.
    CouplerCollision {
        /// First operation ID.
        op1: OperationId,
        /// Second operation ID.
        op2: OperationId,
        /// Contested normalized coupler.
        coupler: (PhysicalQubitId, PhysicalQubitId),
        /// Overlapping time interval in nanoseconds.
        interval: (f64, f64),
    },

    /// Two concurrent operations collide on an exclusively held resource.
    ExclusiveResourceCollision {
        /// First operation ID.
        op1: OperationId,
        /// Second operation ID.
        op2: OperationId,
        /// Contested exclusive resource.
        resource: ResourceId,
        /// Overlapping time interval in nanoseconds.
        interval: (f64, f64),
    },

    /// Concurrently active operations exceed aggregate resource capacity.
    CapacityExceeded {
        /// Overcommitted resource.
        resource: ResourceId,
        /// Maximum capacity limit.
        capacity: usize,
        /// Requested concurrent usage amount.
        usage: usize,
        /// Timestamp in nanoseconds where overcommitment occurred.
        time: f64,
    },

    /// Concurrently active operations violate a target mutual exclusion rule.
    MutualExclusionViolation {
        /// Rule name or description.
        rule: String,
        /// First conflicting operation ID.
        op1: OperationId,
        /// Second conflicting operation ID.
        op2: OperationId,
        /// Detailed reason.
        reason: String,
    },

    /// Target has no declared resource model.
    MissingResourceModel {
        /// Target machine name.
        target: String,
    },

    /// Operation lacks declared resource metadata.
    MissingOperationResources {
        /// Operation mnemonic.
        op_name: String,
        /// Physical operand qubits.
        operands: Vec<PhysicalQubitId>,
    },

    /// Scheduled operation resource requirements do not match target requirements.
    ResourceMetadataMismatch {
        /// Operation ID.
        op_id: OperationId,
        /// Expected requirements from target model.
        expected: Vec<ResourceRequirement>,
        /// Actual requirements in schedule.
        actual: Vec<ResourceRequirement>,
    },

    /// Scheduled operation predecessor IDs do not match DAG predecessors.
    PredecessorMetadataMismatch {
        /// Operation ID.
        op_id: OperationId,
        /// Expected predecessor IDs from circuit DAG.
        expected: Vec<OperationId>,
        /// Actual predecessor IDs in schedule.
        actual: Vec<OperationId>,
    },

    /// Target calibration data is stale or expired.
    StaleCalibration {
        /// Target machine name.
        target: String,
        /// Detailed staleness or expiration reason.
        reason: String,
    },
}

impl fmt::Display for ScheduleValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOperation { op_id, op_name } => {
                write!(f, "missing operation '{op_name}' (op#{op_id}) in schedule")
            }
            Self::DuplicateOperation { op_id } => {
                write!(f, "duplicate operation op#{op_id} in schedule")
            }
            Self::UnexpectedOperation { op_id } => {
                write!(
                    f,
                    "unexpected operation op#{op_id} not present in circuit DAG"
                )
            }
            Self::OperationMismatch {
                op_id,
                expected_name,
                actual_name,
            } => {
                write!(
                    f,
                    "operation mismatch on op#{op_id}: expected '{expected_name}' from DAG, schedule declared '{actual_name}'"
                )
            }
            Self::OperandMismatch {
                op_id,
                expected_operands,
                actual_operands,
            } => {
                write!(
                    f,
                    "operand mismatch on op#{op_id}: expected {expected_operands:?} from DAG, schedule declared {actual_operands:?}"
                )
            }
            Self::PredecessorViolation {
                op_id,
                pred_id,
                op_start,
                pred_end,
            } => {
                write!(
                    f,
                    "predecessor violation: op#{op_id} starts at {op_start:.2}ns before predecessor op#{pred_id} ends at {pred_end:.2}ns"
                )
            }
            Self::InvalidTiming { op_id, reason } => {
                write!(f, "invalid timing on op#{op_id}: {reason}")
            }
            Self::DurationMismatch {
                op_id,
                expected_ns,
                actual_ns,
            } => {
                write!(
                    f,
                    "duration mismatch on op#{op_id}: expected {expected_ns:.2}ns from target calibration, got {actual_ns:.2}ns"
                )
            }
            Self::UnknownDuration {
                op_id,
                op_name,
                operands,
            } => {
                write!(
                    f,
                    "unknown duration on op#{op_id} ('{op_name}' on {operands:?}): duration must be calibrated"
                )
            }
            Self::QubitCollision {
                op1,
                op2,
                qubit,
                interval,
            } => {
                write!(
                    f,
                    "qubit collision on {qubit}: op#{op1} and op#{op2} overlap during [{:.2}ns, {:.2}ns]",
                    interval.0, interval.1
                )
            }
            Self::CouplerCollision {
                op1,
                op2,
                coupler,
                interval,
            } => {
                write!(
                    f,
                    "coupler collision on ({}, {}): op#{op1} and op#{op2} overlap during [{:.2}ns, {:.2}ns]",
                    coupler.0, coupler.1, interval.0, interval.1
                )
            }
            Self::ExclusiveResourceCollision {
                op1,
                op2,
                resource,
                interval,
            } => {
                write!(
                    f,
                    "exclusive resource collision on '{resource}': op#{op1} and op#{op2} overlap during [{:.2}ns, {:.2}ns]",
                    interval.0, interval.1
                )
            }
            Self::CapacityExceeded {
                resource,
                capacity,
                usage,
                time,
            } => {
                write!(
                    f,
                    "capacity exceeded on '{resource}' at {time:.2}ns: usage {usage} > capacity {capacity}"
                )
            }
            Self::MutualExclusionViolation {
                rule,
                op1,
                op2,
                reason,
            } => {
                write!(
                    f,
                    "mutual exclusion violation '{rule}' between op#{op1} and op#{op2}: {reason}"
                )
            }
            Self::MissingResourceModel { target } => {
                write!(
                    f,
                    "target '{target}' has no resource model: cannot verify schedule legality"
                )
            }
            Self::MissingOperationResources { op_name, operands } => {
                write!(
                    f,
                    "missing resource metadata for operation '{op_name}' on operands {operands:?}"
                )
            }
            Self::ResourceMetadataMismatch {
                op_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "resource metadata mismatch on op#{op_id}: expected {expected:?} from target, schedule declared {actual:?}"
                )
            }
            Self::PredecessorMetadataMismatch {
                op_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "predecessor metadata mismatch on op#{op_id}: expected {expected:?} from DAG, schedule declared {actual:?}"
                )
            }
            Self::StaleCalibration { target, reason } => {
                write!(f, "stale calibration on target '{target}': {reason}")
            }
        }
    }
}

/// Comprehensive validation report produced by [`ScheduleValidator`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduleValidationReport {
    /// Name of the validated circuit.
    pub circuit_name: String,
    /// Name of the target device.
    pub target_name: String,
    /// Number of operations examined.
    pub operations_checked: usize,
    /// True if the schedule satisfies all constraints.
    pub is_valid: bool,
    /// List of detected validation errors.
    pub violations: Vec<ScheduleValidationError>,
}

impl ScheduleValidationReport {
    /// Returns `true` if zero violations were detected.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.is_valid && self.violations.is_empty()
    }

    /// Number of detected violations.
    #[inline]
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }

    /// Formats a readable summary of all violations.
    pub fn summary(&self) -> String {
        if self.is_valid() {
            format!(
                "Schedule validation passed for circuit '{}' on target '{}' ({} operations).",
                self.circuit_name, self.target_name, self.operations_checked
            )
        } else {
            let mut s = format!(
                "Schedule validation failed with {} violations for circuit '{}' on target '{}':\n",
                self.violations.len(),
                self.circuit_name,
                self.target_name
            );
            for (idx, v) in self.violations.iter().enumerate() {
                s.push_str(&format!("  {}. {}\n", idx + 1, v));
            }
            s
        }
    }
}

/// Independent validator for quantum circuit execution schedules.
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleValidator {
    /// Allow compiler barriers without target descriptors.
    pub allow_barriers: bool,
    /// Timestamp comparison tolerance in nanoseconds (default $10^{-9}$ ns).
    pub time_epsilon: f64,
    /// Optional ISO 8601 or epoch timestamp string against which to check calibration freshness.
    pub reference_timestamp: Option<String>,
    /// Optional maximum calibration age in seconds (default: 86,400s).
    pub max_calibration_age_secs: Option<u64>,
}

impl Default for ScheduleValidator {
    fn default() -> Self {
        Self {
            allow_barriers: true,
            time_epsilon: 1e-9,
            reference_timestamp: None,
            max_calibration_age_secs: None,
        }
    }
}

impl ScheduleValidator {
    /// Creates a new validator with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures floating-point epsilon tolerance in nanoseconds.
    pub fn with_epsilon(mut self, epsilon: f64) -> Self {
        self.time_epsilon = epsilon;
        self
    }

    /// Configures calibration freshness check against an epoch or ISO 8601 timestamp string.
    pub fn with_freshness_check(
        mut self,
        reference_timestamp: impl Into<String>,
        max_age_secs: Option<u64>,
    ) -> Self {
        self.reference_timestamp = Some(reference_timestamp.into());
        self.max_calibration_age_secs = max_age_secs;
        self
    }

    /// Independently validates a schedule against the circuit DAG and target specifications.
    pub fn validate(
        &self,
        schedule: &Schedule,
        circuit: &Circuit,
        target: &Target,
    ) -> ScheduleValidationReport {
        let mut violations = Vec::new();
        let eps = self.time_epsilon;
        let clock_period = target.timing.as_ref().and_then(|t| t.clock_period_ns);

        let Some(resource_model) = &target.resources else {
            violations.push(ScheduleValidationError::MissingResourceModel {
                target: target.name.clone(),
            });
            return ScheduleValidationReport {
                circuit_name: circuit.name.clone(),
                target_name: target.name.clone(),
                operations_checked: schedule.len(),
                is_valid: false,
                violations,
            };
        };

        // 0. Optional Calibration Freshness Check
        if let Some(ref_ts) = &self.reference_timestamp {
            let max_age = self.max_calibration_age_secs.unwrap_or(86_400);
            if let Some(cal) = &target.calibration {
                if let Err(e) = cal.validate_freshness(ref_ts, max_age) {
                    violations.push(ScheduleValidationError::StaleCalibration {
                        target: target.name.clone(),
                        reason: e.to_string(),
                    });
                }
            }
            if let Some(prov) = &target.provenance {
                if let Err(e) = prov.validate_freshness(ref_ts, max_age, &target.name) {
                    violations.push(ScheduleValidationError::StaleCalibration {
                        target: target.name.clone(),
                        reason: e.to_string(),
                    });
                }
            }
        }

        // 1. Validate Schedule Makespan
        if !schedule.makespan.is_finite() || schedule.makespan < 0.0 {
            violations.push(ScheduleValidationError::InvalidTiming {
                op_id: OperationId(0),
                reason: format!(
                    "schedule makespan must be non-negative and finite, got {}",
                    schedule.makespan
                ),
            });
        }

        let expected_makespan = schedule
            .operations
            .iter()
            .map(|o| o.end_time)
            .fold(0.0_f64, f64::max);
        if (schedule.makespan - expected_makespan).abs() > eps {
            violations.push(ScheduleValidationError::InvalidTiming {
                op_id: OperationId(0),
                reason: format!(
                    "schedule makespan {} does not match maximum operation end time {}",
                    schedule.makespan, expected_makespan
                ),
            });
        }

        // 2. Bijective node coverage check
        let mut circuit_node_ids = HashSet::new();
        let mut dag_order = Vec::new();
        if let Ok(order) = circuit.graph.deterministic_topological_order() {
            dag_order = order;
            for &id in &dag_order {
                circuit_node_ids.insert(id);
            }
        }

        let mut seen_in_schedule = HashSet::new();
        for op in schedule.operations() {
            if !seen_in_schedule.insert(op.id) {
                violations.push(ScheduleValidationError::DuplicateOperation { op_id: op.id });
            }
            if !circuit_node_ids.contains(&op.id) {
                violations.push(ScheduleValidationError::UnexpectedOperation { op_id: op.id });
            }
        }

        for &id in &dag_order {
            if !seen_in_schedule.contains(&id) {
                let name = circuit
                    .graph
                    .get_operation(id)
                    .map(|o| o.name().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                violations.push(ScheduleValidationError::MissingOperation {
                    op_id: id,
                    op_name: name,
                });
            }
        }

        // Fast map from OperationId to scheduled operation
        let sched_map: HashMap<OperationId, &ScheduledOperation> =
            schedule.operations().iter().map(|op| (op.id, op)).collect();

        // 3. Predecessor dependencies, operation matching, and individual timing checks
        let mut independent_resources: HashMap<OperationId, Vec<ResourceRequirement>> =
            HashMap::with_capacity(dag_order.len());

        for &id in &dag_order {
            if let Some(op) = sched_map.get(&id) {
                let dag_node = circuit
                    .graph
                    .get_node(id)
                    .expect("node in dag_order must exist in circuit");
                let dag_op = &dag_node.operation;
                let expected_name = dag_op.name();
                let expected_operands: Vec<PhysicalQubitId> = dag_op
                    .qubits()
                    .iter()
                    .map(|q| PhysicalQubitId(q.0 as u32))
                    .collect();

                // Operation name and operand check against DAG
                if op.name != expected_name {
                    violations.push(ScheduleValidationError::OperationMismatch {
                        op_id: id,
                        expected_name: expected_name.to_string(),
                        actual_name: op.name.clone(),
                    });
                }
                if op.operands != expected_operands {
                    violations.push(ScheduleValidationError::OperandMismatch {
                        op_id: id,
                        expected_operands: expected_operands.clone(),
                        actual_operands: op.operands.clone(),
                    });
                }

                // Check predecessor metadata on ScheduledOperation against actual DAG predecessors
                let actual_dag_preds: BTreeSet<OperationId> = circuit
                    .graph
                    .predecessors(id)
                    .into_iter()
                    .map(|(p, _)| p)
                    .collect();
                let recorded_sched_preds: BTreeSet<OperationId> =
                    op.predecessors.iter().copied().collect();
                if actual_dag_preds != recorded_sched_preds {
                    violations.push(ScheduleValidationError::PredecessorMetadataMismatch {
                        op_id: id,
                        expected: actual_dag_preds.into_iter().collect(),
                        actual: op.predecessors.clone(),
                    });
                }

                // Reconstruct true resource requirements from Target
                let is_barrier = expected_name.eq_ignore_ascii_case("barrier");
                let expected_reqs = if is_barrier {
                    Vec::new()
                } else {
                    let target_op_id = TargetOpId::new(expected_name);
                    if !resource_model.has_metadata_for(&target_op_id, &expected_operands) {
                        violations.push(ScheduleValidationError::MissingOperationResources {
                            op_name: expected_name.to_string(),
                            operands: expected_operands.clone(),
                        });
                        Vec::new()
                    } else {
                        resource_model.requirements_for(&target_op_id, &expected_operands)
                    }
                };

                // Verify schedule's recorded resources match target's expected requirements
                let mut sorted_actual = op.resources.clone();
                let mut sorted_expected = expected_reqs.clone();
                sorted_actual.sort();
                sorted_expected.sort();
                if sorted_actual != sorted_expected {
                    violations.push(ScheduleValidationError::ResourceMetadataMismatch {
                        op_id: id,
                        expected: expected_reqs.clone(),
                        actual: op.resources.clone(),
                    });
                }

                independent_resources.insert(id, expected_reqs);

                // Timing sanity
                if !op.start_time.is_finite() || op.start_time < 0.0 {
                    violations.push(ScheduleValidationError::InvalidTiming {
                        op_id: id,
                        reason: format!(
                            "start_time must be non-negative and finite, got {}",
                            op.start_time
                        ),
                    });
                }
                if !op.duration.is_finite() || op.duration < 0.0 {
                    violations.push(ScheduleValidationError::InvalidTiming {
                        op_id: id,
                        reason: format!(
                            "duration must be non-negative and finite, got {}",
                            op.duration
                        ),
                    });
                }
                if !op.end_time.is_finite() || op.end_time < 0.0 {
                    violations.push(ScheduleValidationError::InvalidTiming {
                        op_id: id,
                        reason: format!(
                            "end_time must be non-negative and finite, got {}",
                            op.end_time
                        ),
                    });
                }
                if (op.end_time - (op.start_time + op.duration)).abs() > eps {
                    violations.push(ScheduleValidationError::InvalidTiming {
                        op_id: id,
                        reason: format!(
                            "end_time {} != start_time {} + duration {}",
                            op.end_time, op.start_time, op.duration
                        ),
                    });
                }

                // Target duration calibration check
                if is_barrier {
                    let expected_dur = target.tuple_duration(&op.name, &op.operands).unwrap_or(0.0);
                    if (op.duration - expected_dur).abs() > eps {
                        violations.push(ScheduleValidationError::DurationMismatch {
                            op_id: id,
                            expected_ns: expected_dur,
                            actual_ns: op.duration,
                        });
                    }
                } else {
                    let dur_enum = target.operation_duration(&op.name, &op.operands);
                    if dur_enum.is_unknown() {
                        violations.push(ScheduleValidationError::UnknownDuration {
                            op_id: id,
                            op_name: op.name.clone(),
                            operands: op.operands.clone(),
                        });
                    } else if let Ok(Some(expected_ns)) = dur_enum.to_nanoseconds(clock_period) {
                        if (op.duration - expected_ns).abs() > eps {
                            violations.push(ScheduleValidationError::DurationMismatch {
                                op_id: id,
                                expected_ns,
                                actual_ns: op.duration,
                            });
                        }
                    }
                }

                // Predecessor completion checks
                for (pred_id, _) in circuit.graph.predecessors(id) {
                    if let Some(pred_op) = sched_map.get(&pred_id) {
                        if op.start_time < pred_op.end_time - eps {
                            violations.push(ScheduleValidationError::PredecessorViolation {
                                op_id: id,
                                pred_id,
                                op_start: op.start_time,
                                pred_end: pred_op.end_time,
                            });
                        }
                    }
                }
            }
        }

        // 4. Concurrency constraints (qubits, couplers, exclusive resources, mutual exclusions)
        let ops = schedule.operations();
        for i in 0..ops.len() {
            let op1 = &ops[i];

            for op2 in ops.iter().skip(i + 1) {
                let is_b1 = op1.name.eq_ignore_ascii_case("barrier");
                let is_b2 = op2.name.eq_ignore_ascii_case("barrier");

                let overlaps = op1.overlaps(op2, eps);

                if overlaps {
                    let overlap_interval = (
                        op1.start_time.max(op2.start_time),
                        op1.end_time.min(op2.end_time),
                    );

                    // a. Qubit collision
                    if !is_b1 && !is_b2 {
                        for &q in &op1.operands {
                            if op2.touches_qubit(q) {
                                violations.push(ScheduleValidationError::QubitCollision {
                                    op1: op1.id,
                                    op2: op2.id,
                                    qubit: q,
                                    interval: overlap_interval,
                                });
                            }
                        }
                    }

                    // b. Coupler collision
                    if op1.operands.len() == 2 && op2.operands.len() == 2 {
                        let c1 = ResourceId::coupler(op1.operands[0], op1.operands[1]);
                        let c2 = ResourceId::coupler(op2.operands[0], op2.operands[1]);
                        if c1 == c2 {
                            if let (ResourceId::Coupler(q0, q1), _) = (c1, c2) {
                                violations.push(ScheduleValidationError::CouplerCollision {
                                    op1: op1.id,
                                    op2: op2.id,
                                    coupler: (q0, q1),
                                    interval: overlap_interval,
                                });
                            }
                        }
                    }

                    // c. Exclusive resource collision (using independent target-reconstructed requirements)
                    let empty_reqs = Vec::new();
                    let reqs1 = independent_resources.get(&op1.id).unwrap_or(&empty_reqs);
                    let reqs2 = independent_resources.get(&op2.id).unwrap_or(&empty_reqs);

                    for req1 in reqs1 {
                        for req2 in reqs2 {
                            if req1.resource == req2.resource
                                && (req1.mode == ResourceUsageMode::Exclusive
                                    || req2.mode == ResourceUsageMode::Exclusive)
                            {
                                violations.push(
                                    ScheduleValidationError::ExclusiveResourceCollision {
                                        op1: op1.id,
                                        op2: op2.id,
                                        resource: req1.resource.clone(),
                                        interval: overlap_interval,
                                    },
                                );
                                break;
                            }
                        }
                    }

                    // d. Mutual exclusion rules (checked for any concurrent execution)
                    if !is_b1 && !is_b2 {
                        let id1 = TargetOpId::new(&op1.name);
                        let id2 = TargetOpId::new(&op2.name);
                        if let Err(conflict) = resource_model
                            .check_concurrency((&id1, &op1.operands), (&id2, &op2.operands))
                        {
                            violations.push(ScheduleValidationError::MutualExclusionViolation {
                                rule: format!("{conflict}"),
                                op1: op1.id,
                                op2: op2.id,
                                reason: conflict.to_string(),
                            });
                        }
                    }
                }
            }
        }

        // 5. Shared resource capacity limit checks (using independently reconstructed resources)
        let mut shared_resources: HashSet<ResourceId> = HashSet::new();
        for reqs in independent_resources.values() {
            for req in reqs {
                if req.mode == ResourceUsageMode::Shared {
                    shared_resources.insert(req.resource.clone());
                }
            }
        }

        // Collect all critical event start timestamps
        let mut event_times: Vec<f64> = ops.iter().map(|o| o.start_time).collect();
        event_times.sort_by(|a, b| a.total_cmp(b));
        let mut dedup_events: Vec<f64> = Vec::with_capacity(event_times.len());
        for et in event_times {
            if let Some(&last) = dedup_events.last() {
                if (et - last).abs() <= eps {
                    continue;
                }
            }
            dedup_events.push(et);
        }

        for resource in shared_resources {
            let capacity = resource_model.capacity_of(&resource);

            for t in dedup_events.iter().copied() {
                let mut current_usage = 0;

                for op in ops {
                    if op.is_active_at(t, eps) {
                        if let Some(reqs) = independent_resources.get(&op.id) {
                            for req in reqs {
                                if req.resource == resource {
                                    current_usage += req.amount;
                                }
                            }
                        }
                    }
                }

                if current_usage > capacity {
                    violations.push(ScheduleValidationError::CapacityExceeded {
                        resource: resource.clone(),
                        capacity,
                        usage: current_usage,
                        time: t,
                    });
                    break;
                }
            }
        }

        let is_valid = violations.is_empty();
        ScheduleValidationReport {
            circuit_name: circuit.name.clone(),
            target_name: target.name.clone(),
            operations_checked: schedule.len(),
            is_valid,
            violations,
        }
    }
}
