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
use crate::target::resources::ResourceRequirement;

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
    /// # Algorithm
    /// 1. Runs an initial ASAP baseline pass to determine:
    ///    - The minimum feasible makespan $M_{\text{ASAP}}$.
    ///    - The earliest possible start time $\text{start}_{\text{ASAP}}(u)$ for each operation.
    /// 2. Resolves the target makespan window $M$:
    ///    - If [`AlapMakespanPolicy::DerivedFromAsap`], $M = M_{\text{ASAP}}$.
    ///    - If [`AlapMakespanPolicy::Explicit`], requires $M \ge M_{\text{ASAP}} - \epsilon$; returns
    ///      [`ScheduleError::InfeasibleMakespan`] if the requested window is too tight.
    /// 3. Obtains a deterministic topological ordering using Kahn's algorithm with min-priority tie-breaking
    ///    and traverses it in **reverse** order (`dag_order.iter().rev()`).
    /// 4. For each operation in reverse order:
    ///    - Resolves duration and hardware resource requirements from the target model.
    ///    - Computes latest completion time $T_{\text{succ}}(u)$:
    ///      - If $u$ has successors in the DAG, $T_{\text{succ}}(u) = \min_{v \in \text{succ}(u)} \text{start}(v)$.
    ///      - If $u$ has no successors (terminal node), $T_{\text{succ}}(u) = M$.
    ///    - Checks that $T_{\text{succ}}(u) - d_u \ge -\epsilon$; returns [`ScheduleError::NegativeSlack`] otherwise.
    ///    - Identifies candidate end times in $[0, T_{\text{succ}}(u)]$ based on critical resource transitions
    ///      ($T_{\text{succ}}$, already scheduled operation starts and ends, and duration offsets).
    ///    - Scans candidates in descending order to locate the latest feasible conflict-free placement
    ///      under exact half-open interval semantics.
    ///    - Computes operation slack $\text{Slack}(u) = \text{start}_{\text{ALAP}}(u) - \text{start}_{\text{ASAP}}(u)$.
    ///      Strictly reports [`ScheduleError::NegativeSlack`] if $\text{Slack} < -\epsilon$.
    /// 5. Assembles the final [`Schedule`], sorted chronologically by start time, end time, sequence number, and ID.
    ///
    /// # Errors
    /// - [`ScheduleError::MissingResourceModel`] if the target has no resource model.
    /// - [`ScheduleError::UnknownDuration`] if an operation has uncalibrated/unknown duration.
    /// - [`ScheduleError::MissingOperationResources`] if an operation lacks resource metadata.
    /// - [`ScheduleError::InfeasibleMakespan`] if the requested makespan window is shorter than $M_{\text{ASAP}}$.
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

        let mut schedule =
            Schedule::new(&circuit.name, &target.name).with_requested_makespan(target_makespan);
        let mut scheduled_start_times: HashMap<OperationId, f64> =
            HashMap::with_capacity(topological_order.len());
        let mut scheduled_ops: Vec<ScheduledOperation> =
            Vec::with_capacity(topological_order.len());

        let eps = self.config.time_epsilon;

        // 3. Reverse topological traversal
        for &op_id in topological_order.iter().rev() {
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

            // Duration resolution
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

            // Resource requirement resolution
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

            // Successor latest completion time $T_{\text{succ}}$
            let successors = circuit.graph.successors(op_id);
            let t_succ = if successors.is_empty() {
                target_makespan
            } else {
                let mut min_succ_start = target_makespan;
                for (succ_id, _) in successors {
                    if let Some(&succ_start) = scheduled_start_times.get(&succ_id) {
                        if succ_start < min_succ_start {
                            min_succ_start = succ_start;
                        }
                    }
                }
                min_succ_start
            };

            let asap_start = asap_starts.get(&op_id).copied().unwrap_or(0.0);

            // Pre-check for impossible completion before earliest feasible start
            if t_succ - duration < -eps {
                let slack = (t_succ - duration) - asap_start;
                return Err(ScheduleError::NegativeSlack {
                    op_id,
                    op_name: op_name.to_string(),
                    operands: operands.clone(),
                    slack_ns: slack,
                    latest_start_ns: t_succ - duration,
                });
            }

            // Collect candidate end times $\le T_{\text{succ}}$
            let mut candidates = vec![t_succ];
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
                    op_name,
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
                    let slack = (t_succ - duration) - asap_start;
                    if slack < -eps {
                        return Err(ScheduleError::NegativeSlack {
                            op_id,
                            op_name: op_name.to_string(),
                            operands: operands.clone(),
                            slack_ns: slack,
                            latest_start_ns: t_succ - duration,
                        });
                    } else {
                        return Err(ScheduleError::InfeasibleMakespan {
                            requested_ns: target_makespan,
                            minimum_ns: min_makespan,
                        });
                    }
                }
            };

            // Calculate operation slack
            let mut slack = start_time - asap_start;
            if slack < -eps {
                return Err(ScheduleError::NegativeSlack {
                    op_id,
                    op_name: op_name.to_string(),
                    operands: operands.clone(),
                    slack_ns: slack,
                    latest_start_ns: start_time,
                });
            }
            if slack < 0.0 && slack >= -eps {
                slack = 0.0;
            }

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
