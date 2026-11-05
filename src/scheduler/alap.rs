//! ALAP (As Late As Possible) instruction scheduler.
//!
//! Milestone M6 (Phase 56): Schedules routed physical operations as late as possible
//! within a defined or derived makespan window, preserving all DAG dependencies, physical qubit
//! single-occupancy, target duration calibrations, and hardware resource constraints.
//!
//! Computes and exposes operation slack ($\text{Slack} = \text{start}_{\text{ALAP}} - \text{start}_{\text{ASAP}}$),
//! identifies critical-path operations ($\text{Slack} = 0$), and detects infeasible schedule windows
//! or negative slack.

use std::collections::HashMap;

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::scheduler::asap::{self, AsapScheduler, ScheduleConfig};
use crate::scheduler::schedule::{Schedule, ScheduledOperation};
use crate::scheduler::ScheduleError;
use crate::target::model::Target;
use crate::target::operation::{PhysicalQubitId, TargetOpId};
use crate::target::resources::{ResourceId, ResourceModel, ResourceRequirement, ResourceUsageMode};

/// Determines if two operations have a direct target resource conflict preventing concurrent execution.
fn operations_conflict(
    op1_name: &str,
    op1_operands: &[PhysicalQubitId],
    op1_reqs: &[ResourceRequirement],
    op2_name: &str,
    op2_operands: &[PhysicalQubitId],
    op2_reqs: &[ResourceRequirement],
    resource_model: &ResourceModel,
) -> bool {
    let is_b1 = op1_name.eq_ignore_ascii_case("barrier");
    let is_b2 = op2_name.eq_ignore_ascii_case("barrier");

    if is_b1 || is_b2 {
        return false;
    }

    // 1. Qubit single occupancy: concurrent operations on the same physical qubit cannot overlap
    for &q1 in op1_operands {
        if op2_operands.contains(&q1) {
            return true;
        }
    }

    // 2. Physical coupler collision for 2-qubit operations
    if op1_operands.len() == 2 && op2_operands.len() == 2 {
        let c1 = ResourceId::coupler(op1_operands[0], op1_operands[1]);
        let c2 = ResourceId::coupler(op2_operands[0], op2_operands[1]);
        if c1 == c2 {
            return true;
        }
    }

    // 3. Exclusive hardware resource collisions
    for r1 in op1_reqs {
        for r2 in op2_reqs {
            if r1.resource == r2.resource
                && (r1.mode == ResourceUsageMode::Exclusive
                    || r2.mode == ResourceUsageMode::Exclusive)
            {
                return true;
            }
        }
    }

    // 4. Target mutual exclusion rules
    let id1 = TargetOpId::new(op1_name);
    let id2 = TargetOpId::new(op2_name);
    if resource_model
        .check_concurrency((&id1, op1_operands), (&id2, op2_operands))
        .is_err()
    {
        return true;
    }

    // 5. Shared resource capacity: sum of concurrent requirements exceeds capacity
    for r1 in op1_reqs {
        if r1.mode == ResourceUsageMode::Shared {
            for r2 in op2_reqs {
                if r2.mode == ResourceUsageMode::Shared && r1.resource == r2.resource {
                    let cap = resource_model.capacity_of(&r1.resource);
                    if r1.amount + r2.amount > cap {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Policy for determining the schedule makespan in ALAP scheduling.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum AlapMakespanPolicy {
    /// Derive makespan automatically by running an initial ASAP scheduling pass.
    #[default]
    DerivedFromAsap,
    /// Enforce an explicit target makespan in nanoseconds.
    ///
    /// Must be $\ge$ the minimum feasible makespan (from ASAP); otherwise scheduling fails
    /// with [`ScheduleError::InfeasibleMakespan`].
    Explicit(f64),
}

/// ALAP (As Late As Possible) quantum circuit scheduler.
#[derive(Clone, Debug, Default)]
pub struct AlapScheduler {
    config: ScheduleConfig,
    policy: AlapMakespanPolicy,
}

impl AlapScheduler {
    /// Creates a new ALAP scheduler with default configuration and ASAP-derived makespan.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an ALAP scheduler with an explicit makespan target in nanoseconds.
    pub fn with_makespan(mut self, makespan: f64) -> Self {
        self.policy = AlapMakespanPolicy::Explicit(makespan);
        self
    }

    /// Sets the makespan determination policy.
    pub fn with_policy(mut self, policy: AlapMakespanPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Configures scheduler options using [`ScheduleConfig`].
    pub fn with_config(mut self, config: ScheduleConfig) -> Self {
        self.config = config;
        self
    }

    /// Configures floating-point epsilon tolerance in nanoseconds.
    pub fn with_epsilon(mut self, epsilon: f64) -> Self {
        self.config.time_epsilon = epsilon;
        self
    }

    /// Configures calibration freshness check against an ISO 8601 or epoch timestamp string.
    pub fn with_freshness_check(
        mut self,
        reference_timestamp: impl Into<String>,
        max_age_secs: Option<u64>,
    ) -> Self {
        self.config.reference_timestamp = Some(reference_timestamp.into());
        self.config.max_calibration_age_secs = max_age_secs;
        self
    }

    /// Returns a reference to the scheduler configuration.
    #[inline]
    pub fn config(&self) -> &ScheduleConfig {
        &self.config
    }

    /// Returns the active makespan policy.
    #[inline]
    pub fn policy(&self) -> AlapMakespanPolicy {
        self.policy
    }

    /// Schedules a routed physical circuit as late as possible within the specified or derived makespan.
    ///
    /// # Algorithm (RC-CPM: Resource-Constrained Critical Path Method)
    /// 1. Runs an initial ASAP baseline pass to determine:
    ///    - The minimum feasible makespan $M_{\text{ASAP}}$.
    ///    - The earliest possible start and end times $\text{start}_{\text{ASAP}}(u)$, $\text{end}_{\text{ASAP}}(u)$.
    /// 2. Resolves the target makespan window $M$:
    ///    - If [`AlapMakespanPolicy::DerivedFromAsap`], $M = M_{\text{ASAP}}$.
    ///    - If [`AlapMakespanPolicy::Explicit`], requires $M \ge M_{\text{ASAP}} - \epsilon$; returns
    ///      [`ScheduleError::InfeasibleMakespan`] if the requested window is mathematically infeasible.
    /// 3. Reconstructs hardware resource requirements and identifies resource precedences:
    ///    - For every pair of operations $(u, v)$ with a target resource conflict,
    ///      the forward ASAP pass established a temporal ordering $u \prec_{\text{res}} v$ if $u$ finished
    ///      before $v$ started. ALAP preserves this resource-ordering equivalence class.
    /// 4. Sorts operations in descending order of `(asap_end, asap_start, seq_num, op_id)`:
    ///    - This guarantees that for every edge $u \prec^* v$ (DAG or resource precedence), $v$ is visited
    ///      and placed before $u$.
    /// 5. For each operation $u$ in reverse order:
    ///    - Computes latest completion time $T_{\text{succ}}^*(u)$:
    ///      $T_{\text{succ}}^*(u) = \min (\{ M \} \cup \{ \text{start}_{\text{ALAP}}(v) \mid v \in \text{DAG-succ}(u) \cup \text{Resource-succ}(u) \})$.
    ///    - Identifies candidate end times in $[0, T_{\text{succ}}^*(u)]$ based on critical resource transitions
    ///      and explicitly includes $\text{end}_{\text{ASAP}}(u)$.
    ///    - Scans candidates in descending order to locate the latest feasible conflict-free placement
    ///      under exact half-open interval semantics.
    ///    - If placement fails, returns [`ScheduleError::AlgorithmicPlacementFailure`].
    ///    - Computes operation slack $\text{Slack}(u) = \text{start}_{\text{ALAP}}(u) - \text{start}_{\text{ASAP}}(u)$.
    ///      Under RC-CPM, $\text{Slack}(u) \ge 0$ is mathematically guaranteed for all $M \ge M_{\text{ASAP}}$.
    /// 6. Assembles the final [`Schedule`], sorted chronologically by start time, end time, sequence number, and ID.
    ///
    /// # Errors
    /// - [`ScheduleError::MissingResourceModel`] if the target has no resource model.
    /// - [`ScheduleError::UnknownDuration`] if an operation has uncalibrated/unknown duration.
    /// - [`ScheduleError::MissingOperationResources`] if an operation lacks resource metadata.
    /// - [`ScheduleError::InfeasibleMakespan`] if the requested makespan window is shorter than $M_{\text{ASAP}}$.
    /// - [`ScheduleError::AlgorithmicPlacementFailure`] if heuristic search fails to place an operation.
    /// - [`ScheduleError::NegativeSlack`] if an operation's latest start time precedes its earliest start time.
    /// - [`ScheduleError::DagError`] if the circuit DAG has invalid dependencies or cycles.
    pub fn schedule(&self, circuit: &Circuit, target: &Target) -> Result<Schedule, ScheduleError> {
        let resource_model =
            target
                .resources
                .as_ref()
                .ok_or_else(|| ScheduleError::MissingResourceModel {
                    target: target.name.clone(),
                })?;

        // 1. Run baseline ASAP pass
        let asap_scheduler = AsapScheduler::with_config(self.config.clone());
        let asap_schedule = asap_scheduler.schedule(circuit, target)?;
        let min_makespan = asap_schedule.makespan();

        let asap_starts: HashMap<OperationId, f64> = asap_schedule
            .operations()
            .iter()
            .map(|op| (op.id, op.start_time))
            .collect();
        let asap_ends: HashMap<OperationId, f64> = asap_schedule
            .operations()
            .iter()
            .map(|op| (op.id, op.end_time))
            .collect();

        // 2. Resolve target makespan
        let target_makespan = match self.policy {
            AlapMakespanPolicy::DerivedFromAsap => min_makespan,
            AlapMakespanPolicy::Explicit(m) => {
                if !m.is_finite() || m < 0.0 {
                    return Err(ScheduleError::Target(
                        crate::target::error::TargetError::InvalidDuration {
                            value: m,
                            context: "explicit ALAP makespan".to_string(),
                        },
                    ));
                }
                if m + self.config.time_epsilon < min_makespan {
                    return Err(ScheduleError::InfeasibleMakespan {
                        requested_ns: m,
                        minimum_ns: min_makespan,
                    });
                }
                m
            }
        };

        let clock_period = target.timing.as_ref().and_then(|t| t.clock_period_ns);

        let topological_order = circuit
            .graph
            .deterministic_topological_order()
            .map_err(|e| ScheduleError::DagError(e.to_string()))?;

        let eps = self.config.time_epsilon;

        // 3. Extract metadata, durations, and resource requirements for all operations
        let mut op_requirements: HashMap<OperationId, Vec<ResourceRequirement>> =
            HashMap::with_capacity(topological_order.len());
        let mut op_operands: HashMap<OperationId, Vec<PhysicalQubitId>> =
            HashMap::with_capacity(topological_order.len());
        let mut op_names: HashMap<OperationId, String> =
            HashMap::with_capacity(topological_order.len());
        let mut op_durations: HashMap<OperationId, f64> =
            HashMap::with_capacity(topological_order.len());
        let mut op_seq_nums: HashMap<OperationId, u64> =
            HashMap::with_capacity(topological_order.len());

        for &op_id in &topological_order {
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

            op_names.insert(op_id, op_name.to_string());
            op_operands.insert(op_id, operands);
            op_requirements.insert(op_id, requirements);
            op_durations.insert(op_id, duration);
            op_seq_nums.insert(op_id, node.sequence_number);
        }

        // 4. Build resource successors based on ASAP schedule and resource conflicts (RC-CPM)
        let mut resource_successors: HashMap<OperationId, Vec<OperationId>> =
            HashMap::with_capacity(topological_order.len());
        for &u in &topological_order {
            let u_end = asap_ends.get(&u).copied().unwrap_or(0.0);
            let u_start = asap_starts.get(&u).copied().unwrap_or(0.0);
            let u_name = &op_names[&u];
            let u_ops = &op_operands[&u];
            let u_reqs = &op_requirements[&u];

            let mut succs = Vec::new();
            for &v in &topological_order {
                if u == v {
                    continue;
                }
                let v_start = asap_starts.get(&v).copied().unwrap_or(0.0);
                let v_end = asap_ends.get(&v).copied().unwrap_or(0.0);

                let u_before_v = if (u_end - v_start).abs() <= eps && (u_start - v_end).abs() <= eps
                {
                    op_seq_nums[&u] < op_seq_nums[&v]
                } else {
                    u_end <= v_start + eps
                };

                if u_before_v {
                    let v_name = &op_names[&v];
                    let v_ops = &op_operands[&v];
                    let v_reqs = &op_requirements[&v];

                    if operations_conflict(
                        u_name,
                        u_ops,
                        u_reqs,
                        v_name,
                        v_ops,
                        v_reqs,
                        resource_model,
                    ) {
                        succs.push(v);
                    }
                }
            }
            resource_successors.insert(u, succs);
        }

        // 5. Reverse traversal order sorted descending by (asap_end, asap_start, seq_num, op_id)
        let mut reverse_order = topological_order.clone();
        reverse_order.sort_by(|&a, &b| {
            let a_end = asap_ends.get(&a).copied().unwrap_or(0.0);
            let b_end = asap_ends.get(&b).copied().unwrap_or(0.0);
            let a_start = asap_starts.get(&a).copied().unwrap_or(0.0);
            let b_start = asap_starts.get(&b).copied().unwrap_or(0.0);
            let a_seq = op_seq_nums.get(&a).copied().unwrap_or(0);
            let b_seq = op_seq_nums.get(&b).copied().unwrap_or(0);

            b_end
                .total_cmp(&a_end)
                .then_with(|| b_start.total_cmp(&a_start))
                .then_with(|| b_seq.cmp(&a_seq))
                .then_with(|| b.0.cmp(&a.0))
        });

        let mut schedule =
            Schedule::new(&circuit.name, &target.name).with_requested_makespan(target_makespan);
        let mut scheduled_start_times: HashMap<OperationId, f64> =
            HashMap::with_capacity(topological_order.len());
        let mut scheduled_ops: Vec<ScheduledOperation> =
            Vec::with_capacity(topological_order.len());

        // 6. Reverse scheduling traversal
        for &op_id in &reverse_order {
            let op_name = op_names[&op_id].clone();
            let operands = op_operands[&op_id].clone();
            let requirements = op_requirements[&op_id].clone();
            let duration = op_durations[&op_id];

            // Successor latest completion time T_succ
            let mut min_succ_start = target_makespan;

            // DAG successors
            for (succ_id, _) in circuit.graph.successors(op_id) {
                if let Some(&succ_start) = scheduled_start_times.get(&succ_id) {
                    if succ_start < min_succ_start {
                        min_succ_start = succ_start;
                    }
                }
            }

            // Resource successors (RC-CPM)
            if let Some(r_succs) = resource_successors.get(&op_id) {
                for &r_succ_id in r_succs {
                    if let Some(&succ_start) = scheduled_start_times.get(&r_succ_id) {
                        if succ_start < min_succ_start {
                            min_succ_start = succ_start;
                        }
                    }
                }
            }

            let t_succ = min_succ_start;
            let asap_start = asap_starts.get(&op_id).copied().unwrap_or(0.0);

            // Pre-check for impossible completion before earliest feasible start
            if t_succ - duration < -eps {
                let slack = (t_succ - duration) - asap_start;
                return Err(ScheduleError::NegativeSlack {
                    op_id,
                    op_name: op_name.clone(),
                    operands: operands.clone(),
                    slack_ns: slack,
                    latest_start_ns: t_succ - duration,
                });
            }

            // Collect candidate end times <= T_succ
            let mut candidates = vec![t_succ];
            if let Some(&asap_end) = asap_ends.get(&op_id) {
                if asap_end <= t_succ + eps {
                    candidates.push(asap_end.min(t_succ));
                }
            }
            for scheduled in &scheduled_ops {
                if scheduled.start_time <= t_succ {
                    candidates.push(scheduled.start_time);
                }
                if scheduled.end_time <= t_succ {
                    candidates.push(scheduled.end_time);
                }
                let end_offset = scheduled.end_time + duration;
                if end_offset <= t_succ {
                    candidates.push(end_offset);
                }
                let start_offset = scheduled.start_time + duration;
                if start_offset <= t_succ {
                    candidates.push(start_offset);
                }
            }

            // Filter candidates where candidate_end - duration >= -eps
            candidates.retain(|&c| c - duration >= -eps);

            // Sort descending to find the latest feasible timestamp first
            candidates.sort_by(|a, b| b.total_cmp(a));
            let mut dedup_candidates: Vec<f64> = Vec::with_capacity(candidates.len());
            for c in candidates {
                if let Some(&last) = dedup_candidates.last() {
                    if (c - last).abs() <= 1e-12 {
                        continue;
                    }
                }
                dedup_candidates.push(c);
            }

            // Search for latest feasible placement
            let mut feasible_placement = None;

            for candidate_end in dedup_candidates {
                let mut candidate_start = candidate_end - duration;
                if candidate_start < 0.0 && candidate_start >= -eps {
                    candidate_start = 0.0;
                }
                if candidate_start < 0.0 {
                    continue;
                }
                let actual_end = candidate_start + duration;

                if asap::is_interval_feasible(
                    candidate_start,
                    actual_end,
                    duration,
                    &op_name,
                    &operands,
                    &requirements,
                    &scheduled_ops,
                    resource_model,
                ) {
                    feasible_placement = Some((candidate_start, actual_end));
                    break;
                }
            }

            let (start_time, _end_time) = match feasible_placement {
                Some((s, e)) => (s, e),
                None => {
                    return Err(ScheduleError::AlgorithmicPlacementFailure {
                        op_id,
                        op_name: op_name.clone(),
                        operands: operands.clone(),
                    });
                }
            };

            // Calculate operation slack
            let mut slack = start_time - asap_start;
            if slack < -eps {
                return Err(ScheduleError::NegativeSlack {
                    op_id,
                    op_name: op_name.clone(),
                    operands: operands.clone(),
                    slack_ns: slack,
                    latest_start_ns: start_time,
                });
            }
            if slack < 0.0 && slack >= -eps {
                slack = 0.0;
            }

            let node = circuit
                .graph
                .get_node(op_id)
                .expect("node in topological order must exist");
            let op = &node.operation;
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
            .with_slack(slack)
            .with_classical_reads(effects.classical_reads)
            .with_classical_writes(effects.classical_writes);

            scheduled_start_times.insert(op_id, start_time);
            scheduled_ops.push(scheduled_op.clone());
            schedule.add_operation(scheduled_op);
        }

        Ok(schedule)
    }
}
