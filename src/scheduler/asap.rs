//! ASAP (As Soon As Possible) instruction scheduler.
//!
//! Schedules routed physical operations at the earliest feasible timestamp permitted by:
//! 1. DAG predecessor dependencies (including classical RAW/WAR/WAW hazards and barriers).
//! 2. Target operation execution durations derived from hardware calibration.
//! 3. Qubit and coupler single-occupancy constraints.
//! 4. Hardware resource availability (shared drive lines, multiplexed feedline capacities).
//! 5. Target mutual exclusion rules (cross-talk avoidance, global device exclusivity).

use std::collections::{BTreeSet, HashMap};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::scheduler::schedule::{Schedule, ScheduledOperation};
use crate::scheduler::ScheduleError;
use crate::target::model::Target;
use crate::target::operation::{PhysicalQubitId, TargetOpId};
use crate::target::resources::{ResourceRequirement, ResourceUsageMode};

/// Configuration options for the ASAP scheduler.
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleConfig {
    /// Whether compiler synchronization barriers are supported.
    pub allow_barriers: bool,
    /// Numerical tolerance for floating-point timestamp comparisons in nanoseconds (default $10^{-9}$ ns).
    pub time_epsilon: f64,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            allow_barriers: true,
            time_epsilon: 1e-9,
        }
    }
}

/// ASAP (As Soon As Possible) quantum circuit scheduler.
#[derive(Clone, Debug, Default)]
pub struct AsapScheduler {
    config: ScheduleConfig,
}

impl AsapScheduler {
    /// Creates a new ASAP scheduler with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new ASAP scheduler with customized configuration.
    pub fn with_config(config: ScheduleConfig) -> Self {
        Self { config }
    }

    /// Returns a reference to the scheduler configuration.
    #[inline]
    pub fn config(&self) -> &ScheduleConfig {
        &self.config
    }

    /// Schedules a routed physical circuit onto the target device.
    ///
    /// # Algorithm
    /// 1. Verifies that the target provides a hardware resource model.
    /// 2. Computes a deterministic topological ordering using Kahn's algorithm with min-priority tie-breaking.
    /// 3. For each operation in topological order:
    ///    - Determines physical operands and hardware resource requirements.
    ///    - Resolves duration via [`Target::operation_duration`] (strictly rejecting unknown durations).
    ///    - Calculates the earliest start time $T_{\text{pred}}$ from the completion times of all predecessors.
    ///    - Scans candidate release times starting from $T_{\text{pred}}$ for the earliest timestamp where
    ///      qubits, couplers, shared lines, capacities, and mutual exclusions are satisfied.
    ///    - Binds the operation into the timeline and updates the resource calendar.
    ///
    /// # Errors
    /// - Returns [`ScheduleError::MissingResourceModel`] if the target has no resource model.
    /// - Returns [`ScheduleError::UnknownDuration`] if any operation has uncalibrated/unknown duration.
    /// - Returns [`ScheduleError::MissingOperationResources`] if an operation has missing resource metadata.
    /// - Returns [`ScheduleError::DagError`] if the circuit contains dependency cycles.
    pub fn schedule(&self, circuit: &Circuit, target: &Target) -> Result<Schedule, ScheduleError> {
        let resource_model =
            target
                .resources
                .as_ref()
                .ok_or_else(|| ScheduleError::MissingResourceModel {
                    target: target.name.clone(),
                })?;

        let clock_period = target.timing.as_ref().and_then(|t| t.clock_period_ns);

        let topological_order = circuit
            .graph
            .deterministic_topological_order()
            .map_err(|e| ScheduleError::DagError(e.to_string()))?;

        let mut schedule = Schedule::new(&circuit.name, &target.name);
        let mut completed_times: HashMap<OperationId, f64> =
            HashMap::with_capacity(topological_order.len());
        let mut scheduled_ops: Vec<ScheduledOperation> =
            Vec::with_capacity(topological_order.len());

        let eps = self.config.time_epsilon;

        for op_id in topological_order {
            let node = circuit
                .graph
                .get_node(op_id)
                .ok_or_else(|| ScheduleError::DagError(format!("missing node for op#{op_id}")))?;

            let op = &node.operation;
            let op_name = op.name();
            let is_barrier = op_name.eq_ignore_ascii_case("barrier");

            let operands: Vec<PhysicalQubitId> = op
                .qubits()
                .iter()
                .map(|q| PhysicalQubitId(q.0 as u32))
                .collect();

            // 1. Resolve duration
            let duration = if is_barrier {
                target.tuple_duration(op_name, &operands).unwrap_or(0.0)
            } else {
                let dur_enum = target.operation_duration(op_name, &operands);
                if dur_enum.is_unknown() {
                    return Err(ScheduleError::UnknownDuration {
                        node_id: op_id,
                        op_name: op_name.to_string(),
                        operands: operands.clone(),
                    });
                }
                let dur_ns = dur_enum
                    .to_nanoseconds(clock_period)
                    .map_err(ScheduleError::Target)?;
                dur_ns.ok_or_else(|| ScheduleError::UnknownDuration {
                    node_id: op_id,
                    op_name: op_name.to_string(),
                    operands: operands.clone(),
                })?
            };

            if !duration.is_finite() || duration < 0.0 {
                return Err(ScheduleError::Target(
                    crate::target::error::TargetError::InvalidDuration {
                        value: duration,
                        context: format!("scheduled operation '{op_name}'"),
                    },
                ));
            }

            // 2. Resolve resource requirements
            let requirements: Vec<ResourceRequirement> = if is_barrier {
                Vec::new()
            } else {
                let target_op_id = TargetOpId::new(op_name);
                if !resource_model.has_metadata_for(&target_op_id, &operands) {
                    return Err(ScheduleError::MissingOperationResources {
                        op_name: op_name.to_string(),
                        operands: operands.clone(),
                    });
                }
                resource_model.requirements_for(&target_op_id, &operands)
            };

            // 3. Predecessor completion barrier
            let mut t_pred = 0.0;
            for (pred_id, _) in circuit.graph.predecessors(op_id) {
                if let Some(&pred_end) = completed_times.get(&pred_id) {
                    if pred_end > t_pred {
                        t_pred = pred_end;
                    }
                }
            }

            // 4. Collect candidate start times: t_pred and end times of all scheduled operations >= t_pred
            let mut candidates = vec![t_pred];
            for scheduled in &scheduled_ops {
                if scheduled.end_time >= t_pred {
                    candidates.push(scheduled.end_time);
                }
            }
            candidates.sort_by(|a, b| a.total_cmp(b));
            let mut dedup_candidates: Vec<f64> = Vec::with_capacity(candidates.len());
            for c in candidates {
                if let Some(&last) = dedup_candidates.last() {
                    if (c - last).abs() <= eps {
                        continue;
                    }
                }
                dedup_candidates.push(c);
            }

            // 5. Scan candidate times in increasing chronological order
            let mut feasible_start_time = None;

            for candidate_t in dedup_candidates {
                let candidate_end = candidate_t + duration;

                if is_interval_feasible(
                    candidate_t,
                    candidate_end,
                    duration,
                    op_name,
                    &operands,
                    &requirements,
                    &scheduled_ops,
                    resource_model,
                    eps,
                ) {
                    feasible_start_time = Some(candidate_t);
                    break;
                }
            }

            let start_time = feasible_start_time.expect("candidate time search invariant");
            let end_time = start_time + duration;

            let effects = op.wire_effects();
            let scheduled_op = ScheduledOperation::new(
                op_id,
                node.sequence_number,
                op_name,
                operands,
                start_time,
                duration,
                requirements,
                circuit
                    .graph
                    .predecessors(op_id)
                    .into_iter()
                    .map(|(p, _)| p)
                    .collect(),
            )
            .with_classical_reads(effects.classical_reads)
            .with_classical_writes(effects.classical_writes);

            completed_times.insert(op_id, end_time);
            scheduled_ops.push(scheduled_op.clone());
            schedule.add_operation(scheduled_op);
        }

        Ok(schedule)
    }
}

/// Tests whether the proposed execution interval $[t_{\text{start}}, t_{\text{end}})$ is conflict-free.
#[allow(clippy::too_many_arguments)]
fn is_interval_feasible(
    candidate_start: f64,
    candidate_end: f64,
    duration: f64,
    op_name: &str,
    operands: &[PhysicalQubitId],
    requirements: &[ResourceRequirement],
    scheduled_ops: &[ScheduledOperation],
    resource_model: &crate::target::resources::ResourceModel,
    eps: f64,
) -> bool {
    if op_name.eq_ignore_ascii_case("barrier") {
        return true;
    }

    let dummy_op = ScheduledOperation::new(
        OperationId(0),
        0,
        op_name,
        operands.to_vec(),
        candidate_start,
        duration,
        requirements.to_vec(),
        Vec::new(),
    );

    // 1. Qubit single occupancy: no two operations on the same qubit can overlap
    for scheduled in scheduled_ops {
        if !scheduled.name.eq_ignore_ascii_case("barrier") && dummy_op.overlaps(scheduled, eps) {
            for &q in operands {
                if scheduled.touches_qubit(q) {
                    return false;
                }
            }
        }
    }

    // 2. Check pairwise mutual exclusions and pairwise conflicts via check_concurrency
    if !op_name.eq_ignore_ascii_case("barrier") {
        let op_id = TargetOpId::new(op_name);
        for scheduled in scheduled_ops {
            if !scheduled.name.eq_ignore_ascii_case("barrier") && dummy_op.overlaps(scheduled, eps)
            {
                let sched_op_id = TargetOpId::new(&scheduled.name);
                if resource_model
                    .check_concurrency((&op_id, operands), (&sched_op_id, &scheduled.operands))
                    .is_err()
                {
                    return false;
                }
            }
        }
    }

    // 3. Exclusive resource collisions (e.g. Coupler, DriveLine, or custom channels)
    for req in requirements {
        if req.mode == ResourceUsageMode::Exclusive {
            for scheduled in scheduled_ops {
                if dummy_op.overlaps(scheduled, eps)
                    && scheduled
                        .resources
                        .iter()
                        .any(|r| r.resource == req.resource)
                {
                    return false;
                }
            }
        }
    }

    // 4. Shared resource aggregate capacities across all concurrently active operations
    for req in requirements {
        if req.mode == ResourceUsageMode::Shared {
            let capacity = resource_model.capacity_of(&req.resource);

            // Collect all critical timestamps during the interval to evaluate aggregate capacity
            let mut event_times: BTreeSet<u64> = BTreeSet::new();
            event_times.insert((candidate_start / eps).round() as u64);

            for scheduled in scheduled_ops {
                if dummy_op.overlaps(scheduled, eps)
                    && scheduled.start_time >= candidate_start - eps
                    && scheduled.start_time < candidate_end - eps
                {
                    event_times.insert((scheduled.start_time / eps).round() as u64);
                }
            }

            for event_discrete in event_times {
                let event_t = (event_discrete as f64) * eps;
                let mut current_usage = req.amount;

                for scheduled in scheduled_ops {
                    if scheduled.is_active_at(event_t, eps) {
                        for sched_req in &scheduled.resources {
                            if sched_req.resource == req.resource {
                                current_usage += sched_req.amount;
                            }
                        }
                    }
                }

                if current_usage > capacity {
                    return false;
                }
            }
        }
    }

    true
}
