//! Hardware resource timeline calendar and reservation tracking engine.
//!
//! Tracks exact half-open execution intervals $[t_{\text{start}}, t_{\text{end}})$ across dedicated physical
//! qubits, coupler channels, shared control lines, multiplexed readout feedlines, and custom hardware resources.
//! Enforces aggregate capacity limits and mutual exclusion rules, providing explainable diagnostics for resource conflicts.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ir::node::OperationId;
use crate::scheduler::schedule::ScheduledOperation;
use crate::scheduler::validator::ScheduleValidationError;
use crate::target::model::Target;
use crate::target::operation::{PhysicalQubitId, TargetOpId};
use crate::target::resources::{
    ExclusionCondition, ResourceId, ResourceModel, ResourceRequirement, ResourceUsageMode,
};

/// Normalizes floating-point timestamps so that `-0.0` becomes `+0.0`.
#[inline]
fn normalize_time(t: f64) -> f64 {
    if t == 0.0 {
        0.0
    } else {
        t
    }
}

/// Exact floating-point comparator normalizing signed zeros.
#[inline]
fn exact_cmp(a: f64, b: f64) -> std::cmp::Ordering {
    normalize_time(a).total_cmp(&normalize_time(b))
}

/// Concrete hardware resource reservation over a half-open time interval.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceReservation {
    /// Stable operation node identifier.
    pub op_id: OperationId,
    /// Operation mnemonic.
    pub op_name: String,
    /// Physical operand qubits.
    pub operands: Vec<PhysicalQubitId>,
    /// Reserved hardware resource identifier.
    pub resource: ResourceId,
    /// Reservation start time in nanoseconds.
    pub start_time: f64,
    /// Reservation end time in nanoseconds.
    pub end_time: f64,
    /// Physical operation duration in nanoseconds.
    pub duration: f64,
    /// Usage access mode (exclusive or shared).
    pub mode: ResourceUsageMode,
    /// Number of resource units reserved.
    pub amount: usize,
}

impl ResourceReservation {
    /// Constructs a new validated resource reservation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        op_id: OperationId,
        op_name: impl Into<String>,
        operands: Vec<PhysicalQubitId>,
        resource: ResourceId,
        start_time: f64,
        duration: f64,
        mode: ResourceUsageMode,
        amount: usize,
    ) -> Self {
        let start_time = normalize_time(start_time);
        let duration = normalize_time(duration);
        let end_time = normalize_time(start_time + duration);

        Self {
            op_id,
            op_name: op_name.into(),
            operands,
            resource,
            start_time,
            end_time,
            duration,
            mode,
            amount,
        }
    }

    /// Determines whether this reservation overlaps with a proposed half-open time interval
    /// $[t_{\text{start}}, t_{\text{end}})$ under exact half-open interval semantics.
    pub fn overlaps(&self, start: f64, end: f64, duration: f64) -> bool {
        let start = normalize_time(start);
        let end = normalize_time(end);
        let duration = normalize_time(duration);

        let max_start = if self.start_time >= start {
            self.start_time
        } else {
            start
        };
        let min_end = if self.end_time <= end {
            self.end_time
        } else {
            end
        };

        if self.duration == 0.0 && duration == 0.0 {
            self.start_time == start
        } else if self.duration == 0.0 {
            self.start_time > start && self.start_time < end
        } else if duration == 0.0 {
            start > self.start_time && start < self.end_time
        } else {
            max_start < min_end
        }
    }

    /// Returns `true` if this reservation is actively executing at timestamp $t$.
    pub fn is_active_at(&self, t: f64) -> bool {
        let t = normalize_time(t);
        if self.duration == 0.0 {
            self.start_time == t
        } else {
            t >= self.start_time && t < self.end_time
        }
    }
}

/// Summary of an existing operation contributing to a resource collision or capacity limit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockingOpSummary {
    /// Operation node identifier.
    pub op_id: OperationId,
    /// Operation mnemonic.
    pub op_name: String,
    /// Physical operand qubits.
    pub operands: Vec<PhysicalQubitId>,
    /// Number of units held on the contested resource.
    pub amount: usize,
    /// Reserved interval start time formatted in nanoseconds.
    pub start_ns: String,
    /// Reserved interval end time formatted in nanoseconds.
    pub end_ns: String,
}

/// Detailed reason why an operation cannot be placed in a given interval.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ResourceConflictReason {
    /// Dedicated operand qubit is already occupied by a concurrent operation.
    QubitOccupied {
        /// The occupied physical qubit.
        qubit: PhysicalQubitId,
        /// Operation currently holding the qubit.
        blocking_op_id: OperationId,
        /// Name of the blocking operation.
        blocking_op_name: String,
        /// Reserved interval of the blocking operation.
        blocking_interval: (f64, f64),
    },

    /// Normalized physical coupler channel is already in use by a two-qubit operation.
    CouplerOccupied {
        /// Contested coupler pair.
        coupler: (PhysicalQubitId, PhysicalQubitId),
        /// Operation currently holding the coupler channel.
        blocking_op_id: OperationId,
        /// Name of the blocking operation.
        blocking_op_name: String,
        /// Reserved interval of the blocking operation.
        blocking_interval: (f64, f64),
    },

    /// An exclusively held resource channel (e.g. drive line or acquisition unit) collides with an overlapping operation.
    ExclusiveCollision {
        /// Contested resource identifier.
        resource: ResourceId,
        /// Operation currently holding the resource.
        blocking_op_id: OperationId,
        /// Name of the blocking operation.
        blocking_op_name: String,
        /// Reserved interval of the blocking operation.
        blocking_interval: (f64, f64),
    },

    /// Aggregate concurrent demand on a shared resource exceeds its declared capacity.
    CapacityExceeded {
        /// The overcommitted resource.
        resource: ResourceId,
        /// Maximum capacity limit.
        capacity: usize,
        /// Requested units for the candidate operation.
        requested: usize,
        /// Active usage at the contested timestamp.
        active_usage: usize,
        /// Timestamp in nanoseconds where overcommitment occurred.
        timestamp: f64,
        /// Summary of concurrently active operations contributing to usage.
        blocking_ops: Vec<BlockingOpSummary>,
    },

    /// Violated target mutual exclusion rule.
    MutualExclusion {
        /// Name of the mutual exclusion rule.
        rule: String,
        /// Blocking operation ID.
        blocking_op_id: OperationId,
        /// Blocking operation name.
        blocking_op_name: String,
        /// Explanatory diagnostic.
        reason: String,
    },

    /// Disallowed simultaneous readout across physical qubits.
    ReadoutExclusion {
        /// First physical qubit.
        qubit1: PhysicalQubitId,
        /// Second physical qubit.
        qubit2: PhysicalQubitId,
        /// Detailed explanation.
        reason: String,
    },

    /// Target has no declared resource metadata for the candidate operation.
    MissingMetadata {
        /// Candidate operation name.
        op_name: String,
        /// Physical operand qubits.
        operands: Vec<PhysicalQubitId>,
    },
}

impl fmt::Display for ResourceConflictReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QubitOccupied {
                qubit,
                blocking_op_id,
                blocking_op_name,
                blocking_interval,
            } => {
                write!(
                    f,
                    "qubit {qubit} occupied by '{blocking_op_name}' (op#{}) in interval [{:.2}, {:.2})ns",
                    blocking_op_id.0, blocking_interval.0, blocking_interval.1
                )
            }
            Self::CouplerOccupied {
                coupler,
                blocking_op_id,
                blocking_op_name,
                blocking_interval,
            } => {
                write!(
                    f,
                    "coupler ({}, {}) occupied by '{blocking_op_name}' (op#{}) in interval [{:.2}, {:.2})ns",
                    coupler.0, coupler.1, blocking_op_id.0, blocking_interval.0, blocking_interval.1
                )
            }
            Self::ExclusiveCollision {
                resource,
                blocking_op_id,
                blocking_op_name,
                blocking_interval,
            } => {
                write!(
                    f,
                    "exclusive resource '{resource}' collision with '{blocking_op_name}' (op#{}) in interval [{:.2}, {:.2})ns",
                    blocking_op_id.0, blocking_interval.0, blocking_interval.1
                )
            }
            Self::CapacityExceeded {
                resource,
                capacity,
                requested,
                active_usage,
                timestamp,
                blocking_ops,
            } => {
                write!(
                    f,
                    "capacity exceeded on '{resource}' at {:.2}ns: requested {requested} + active {active_usage} > capacity {capacity} (active ops: {})",
                    timestamp,
                    blocking_ops
                        .iter()
                        .map(|b| format!("{}(op#{})", b.op_name, b.op_id.0))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Self::MutualExclusion {
                rule,
                blocking_op_id,
                blocking_op_name,
                reason,
            } => {
                write!(
                    f,
                    "mutual exclusion rule '{rule}' violated by '{blocking_op_name}' (op#{}): {reason}",
                    blocking_op_id.0
                )
            }
            Self::ReadoutExclusion {
                qubit1,
                qubit2,
                reason,
            } => {
                write!(
                    f,
                    "simultaneous readout excluded between qubit {qubit1} and {qubit2}: {reason}"
                )
            }
            Self::MissingMetadata { op_name, operands } => {
                write!(
                    f,
                    "missing resource metadata for '{op_name}' on operands {operands:?}"
                )
            }
        }
    }
}

/// Structured diagnostic report describing why a candidate interval is unavailable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Error)]
#[error("resource conflict in interval [{:.2}, {:.2})ns: {reason}", contested_interval.0, contested_interval.1)]
pub struct ResourceConflictReport {
    /// Contested hardware resource identifier, if specific to a single resource.
    pub conflicted_resource: Option<ResourceId>,
    /// Proposed or contested time interval in nanoseconds $[t_{\text{start}}, t_{\text{end}})$.
    pub contested_interval: (f64, f64),
    /// Detailed structured conflict reason.
    pub reason: Box<ResourceConflictReason>,
}

impl ResourceConflictReport {
    /// Constructs a new conflict report boxing the reason.
    pub fn new(
        conflicted_resource: Option<ResourceId>,
        contested_interval: (f64, f64),
        reason: ResourceConflictReason,
    ) -> Self {
        Self {
            conflicted_resource,
            contested_interval,
            reason: Box::new(reason),
        }
    }
}

/// Comprehensive hardware resource timeline calendar.
///
/// Maintains exact reservations across all hardware execution channels and validates candidate
/// intervals against physical qubit single-occupancy, dedicated coupler channels, exclusive control
/// lines, aggregate shared-resource capacities, and mutual exclusion rules.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResourceCalendar {
    /// Underlying target resource and concurrency model.
    resource_model: ResourceModel,
    /// Timeline of reservations indexed by ResourceId.
    reservations: BTreeMap<ResourceId, Vec<ResourceReservation>>,
    /// Registered scheduled operations indexed by OperationId.
    scheduled_ops: BTreeMap<OperationId, ScheduledOperation>,
    /// Overall schedule makespan in nanoseconds.
    makespan: f64,
}

impl ResourceCalendar {
    /// Creates a new empty resource calendar backed by the given resource model.
    pub fn new(resource_model: ResourceModel) -> Self {
        Self {
            resource_model,
            reservations: BTreeMap::new(),
            scheduled_ops: BTreeMap::new(),
            makespan: 0.0,
        }
    }

    /// Returns a reference to the calendar's underlying resource model.
    #[inline]
    pub fn resource_model(&self) -> &ResourceModel {
        &self.resource_model
    }

    /// Returns a mutable reference to the calendar's underlying resource model.
    #[inline]
    pub fn resource_model_mut(&mut self) -> &mut ResourceModel {
        &mut self.resource_model
    }

    /// Returns the current schedule makespan in nanoseconds.
    #[inline]
    pub fn makespan(&self) -> f64 {
        self.makespan
    }

    /// Returns the total number of scheduled operations registered in the calendar.
    #[inline]
    pub fn len(&self) -> usize {
        self.scheduled_ops.len()
    }

    /// Returns `true` if no operations are scheduled in the calendar.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.scheduled_ops.is_empty()
    }

    /// Returns all distinct hardware resources currently tracked by the calendar.
    pub fn all_resources(&self) -> BTreeSet<ResourceId> {
        let mut resources: BTreeSet<ResourceId> = self.reservations.keys().cloned().collect();
        for cap in &self.resource_model.capacities {
            resources.insert(cap.resource.clone());
        }
        resources
    }

    /// Returns the list of reservations recorded for a specific resource.
    pub fn timeline_for(&self, resource: &ResourceId) -> &[ResourceReservation] {
        self.reservations
            .get(resource)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns a reference to a registered scheduled operation by its node identifier.
    #[inline]
    pub fn get_operation(&self, op_id: OperationId) -> Option<&ScheduledOperation> {
        self.scheduled_ops.get(&op_id)
    }

    /// Returns an iterator over all scheduled operations registered in the calendar.
    pub fn scheduled_operations(&self) -> impl Iterator<Item = &ScheduledOperation> {
        self.scheduled_ops.values()
    }

    /// Tests whether a proposed interval $[t_{\text{start}}, t_{\text{end}})$ is conflict-free.
    pub fn is_interval_available(
        &self,
        start: f64,
        end: f64,
        duration: f64,
        op_name: &str,
        operands: &[PhysicalQubitId],
        requirements: &[ResourceRequirement],
    ) -> bool {
        self.check_interval(start, end, duration, op_name, operands, requirements)
            .is_ok()
    }

    /// Checks whether a proposed interval $[t_{\text{start}}, t_{\text{end}})$ is conflict-free,
    /// returning a detailed structured diagnostic report on conflict.
    pub fn check_interval(
        &self,
        start: f64,
        end: f64,
        duration: f64,
        op_name: &str,
        operands: &[PhysicalQubitId],
        requirements: &[ResourceRequirement],
    ) -> Result<(), ResourceConflictReport> {
        let start = normalize_time(start);
        let end = normalize_time(end);
        let duration = normalize_time(duration);
        let interval = (start, end);

        if op_name.eq_ignore_ascii_case("barrier") {
            return Ok(());
        }

        let target_op_id = TargetOpId::new(op_name);

        // 1. Explicit Unknown check: verify metadata exists
        if !self
            .resource_model
            .has_metadata_for(&target_op_id, operands)
        {
            return Err(ResourceConflictReport::new(
                None,
                interval,
                ResourceConflictReason::MissingMetadata {
                    op_name: op_name.to_string(),
                    operands: operands.to_vec(),
                },
            ));
        }

        // 2. Qubit single-occupancy check
        for &q in operands {
            let q_res = ResourceId::Qubit(q);
            if let Some(res_list) = self.reservations.get(&q_res) {
                for r in res_list {
                    if r.overlaps(start, end, duration) {
                        return Err(ResourceConflictReport::new(
                            Some(q_res),
                            interval,
                            ResourceConflictReason::QubitOccupied {
                                qubit: q,
                                blocking_op_id: r.op_id,
                                blocking_op_name: r.op_name.clone(),
                                blocking_interval: (r.start_time, r.end_time),
                            },
                        ));
                    }
                }
            }
        }

        // 3. Normalized Coupler channel check for two-qubit operations
        if operands.len() == 2 {
            let coupler_res = ResourceId::coupler(operands[0], operands[1]);
            if let Some(res_list) = self.reservations.get(&coupler_res) {
                for r in res_list {
                    if r.overlaps(start, end, duration) {
                        return Err(ResourceConflictReport::new(
                            Some(coupler_res),
                            interval,
                            ResourceConflictReason::CouplerOccupied {
                                coupler: (operands[0], operands[1]),
                                blocking_op_id: r.op_id,
                                blocking_op_name: r.op_name.clone(),
                                blocking_interval: (r.start_time, r.end_time),
                            },
                        ));
                    }
                }
            }
        }

        // 4. Mutual exclusion rules (pairwise check against all overlapping scheduled operations)
        for scheduled in self.scheduled_ops.values() {
            if scheduled.name.eq_ignore_ascii_case("barrier") {
                continue;
            }

            // Check if intervals overlap
            let overlaps = {
                let max_start = if scheduled.start_time >= start {
                    scheduled.start_time
                } else {
                    start
                };
                let min_end = if scheduled.end_time <= end {
                    scheduled.end_time
                } else {
                    end
                };
                if scheduled.duration == 0.0 && duration == 0.0 {
                    scheduled.start_time == start
                } else if scheduled.duration == 0.0 {
                    scheduled.start_time > start && scheduled.start_time < end
                } else if duration == 0.0 {
                    start > scheduled.start_time && start < scheduled.end_time
                } else {
                    max_start < min_end
                }
            };

            if overlaps {
                let sched_op_id = TargetOpId::new(&scheduled.name);

                for rule in &self.resource_model.mutual_exclusions {
                    match &rule.condition {
                        ExclusionCondition::GlobalExclusive { operation } => {
                            if &target_op_id == operation || &sched_op_id == operation {
                                return Err(ResourceConflictReport::new(
                                    None,
                                    interval,
                                    ResourceConflictReason::MutualExclusion {
                                        rule: rule.name.clone(),
                                        blocking_op_id: scheduled.id,
                                        blocking_op_name: scheduled.name.clone(),
                                        reason: format!(
                                            "operation '{operation}' requires global hardware quiescence; cannot overlap with '{}'",
                                            scheduled.name
                                        ),
                                    },
                                ));
                            }
                        }
                        ExclusionCondition::OperationPair {
                            op1,
                            operands1,
                            op2,
                            operands2,
                        } => {
                            let match_forward = &target_op_id == op1
                                && operands1
                                    .as_ref()
                                    .map(|ops| ops.as_slice() == operands)
                                    .unwrap_or(true)
                                && &sched_op_id == op2
                                && operands2
                                    .as_ref()
                                    .map(|ops| ops.as_slice() == scheduled.operands.as_slice())
                                    .unwrap_or(true);

                            let match_reverse = &target_op_id == op2
                                && operands2
                                    .as_ref()
                                    .map(|ops| ops.as_slice() == operands)
                                    .unwrap_or(true)
                                && &sched_op_id == op1
                                && operands1
                                    .as_ref()
                                    .map(|ops| ops.as_slice() == scheduled.operands.as_slice())
                                    .unwrap_or(true);

                            if match_forward || match_reverse {
                                return Err(ResourceConflictReport::new(
                                    None,
                                    interval,
                                    ResourceConflictReason::MutualExclusion {
                                        rule: rule.name.clone(),
                                        blocking_op_id: scheduled.id,
                                        blocking_op_name: scheduled.name.clone(),
                                        reason: format!(
                                            "operation pair exclusion between '{op_name}' and '{}'",
                                            scheduled.name
                                        ),
                                    },
                                ));
                            }
                        }
                        ExclusionCondition::ReadoutExclusionGroup { qubits } => {
                            if op_name == "measure" && scheduled.name == "measure" {
                                for &q_cand in operands {
                                    if qubits.contains(&q_cand) {
                                        for &q_sched in &scheduled.operands {
                                            if q_cand != q_sched && qubits.contains(&q_sched) {
                                                return Err(ResourceConflictReport::new(
                                                    None,
                                                    interval,
                                                    ResourceConflictReason::ReadoutExclusion {
                                                        qubit1: q_cand,
                                                        qubit2: q_sched,
                                                        reason: format!(
                                                            "qubits {q_cand} and {q_sched} belong to readout exclusion group '{}'",
                                                            rule.name
                                                        ),
                                                    },
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 5. Exclusive and Shared resource checks across all requirements
        for req in requirements {
            // Check exclusive collisions
            if let Some(res_list) = self.reservations.get(&req.resource) {
                for r in res_list {
                    if r.overlaps(start, end, duration)
                        && (req.mode == ResourceUsageMode::Exclusive
                            || r.mode == ResourceUsageMode::Exclusive)
                    {
                        return Err(ResourceConflictReport::new(
                            Some(req.resource.clone()),
                            interval,
                            ResourceConflictReason::ExclusiveCollision {
                                resource: req.resource.clone(),
                                blocking_op_id: r.op_id,
                                blocking_op_name: r.op_name.clone(),
                                blocking_interval: (r.start_time, r.end_time),
                            },
                        ));
                    }
                }
            }

            // Check aggregate capacity for shared resources
            if req.mode == ResourceUsageMode::Shared {
                let capacity = self.resource_model.capacity_of(&req.resource);

                if let Some(res_list) = self.reservations.get(&req.resource) {
                    // Gather all critical event timestamps in [start, end)
                    let mut event_times = vec![start];
                    for r in res_list {
                        if r.overlaps(start, end, duration)
                            && r.start_time >= start
                            && r.start_time < end
                        {
                            event_times.push(r.start_time);
                        }
                    }

                    event_times.sort_by(|a, b| exact_cmp(*a, *b));
                    let mut dedup_events: Vec<f64> = Vec::with_capacity(event_times.len());
                    for t in event_times {
                        if let Some(&last) = dedup_events.last() {
                            if exact_cmp(t, last).is_eq() {
                                continue;
                            }
                        }
                        dedup_events.push(t);
                    }

                    for event_t in dedup_events {
                        let mut active_usage = 0;
                        let mut blocking_ops = Vec::new();

                        for r in res_list {
                            if r.is_active_at(event_t) {
                                active_usage += r.amount;
                                blocking_ops.push(BlockingOpSummary {
                                    op_id: r.op_id,
                                    op_name: r.op_name.clone(),
                                    operands: r.operands.clone(),
                                    amount: r.amount,
                                    start_ns: format!("{:.2}", r.start_time),
                                    end_ns: format!("{:.2}", r.end_time),
                                });
                            }
                        }

                        if active_usage + req.amount > capacity {
                            return Err(ResourceConflictReport::new(
                                Some(req.resource.clone()),
                                interval,
                                ResourceConflictReason::CapacityExceeded {
                                    resource: req.resource.clone(),
                                    capacity,
                                    requested: req.amount,
                                    active_usage,
                                    timestamp: event_t,
                                    blocking_ops,
                                },
                            ));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Reserves hardware resources for an operation according to its scheduled execution interval.
    pub fn reserve(&mut self, op: &ScheduledOperation) -> Result<(), ResourceConflictReport> {
        let op_name = &op.name;
        let operands = &op.operands;
        let start = normalize_time(op.start_time);
        let end = normalize_time(op.end_time);
        let duration = normalize_time(op.duration);

        if !op_name.eq_ignore_ascii_case("barrier") {
            // First check interval availability
            self.check_interval(start, end, duration, op_name, operands, &op.resources)?;

            // Record reservation for every required resource
            for req in &op.resources {
                let res = req.resource.clone();
                let reservation = ResourceReservation::new(
                    op.id,
                    op.name.clone(),
                    op.operands.clone(),
                    res.clone(),
                    start,
                    duration,
                    req.mode,
                    req.amount,
                );

                let list = self.reservations.entry(res).or_default();
                list.push(reservation);
                list.sort_by(|a, b| {
                    exact_cmp(a.start_time, b.start_time)
                        .then_with(|| exact_cmp(a.end_time, b.end_time))
                        .then_with(|| a.op_id.cmp(&b.op_id))
                });
            }
        }

        self.scheduled_ops.insert(op.id, op.clone());
        if end > self.makespan {
            self.makespan = end;
        }

        Ok(())
    }

    /// Removes all reservations associated with an operation node and recalculates makespan.
    pub fn remove_reservation(&mut self, op_id: OperationId) -> bool {
        if self.scheduled_ops.remove(&op_id).is_none() {
            return false;
        }

        for list in self.reservations.values_mut() {
            list.retain(|r| r.op_id != op_id);
        }

        // Recalculate makespan
        self.makespan = self
            .scheduled_ops
            .values()
            .map(|op| op.end_time)
            .fold(0.0, f64::max);

        true
    }

    /// Finds the earliest feasible start time $t \ge t_{\text{min}}$ where the operation can be placed.
    pub fn find_earliest_feasible_time(
        &self,
        min_start: f64,
        duration: f64,
        op_name: &str,
        operands: &[PhysicalQubitId],
        requirements: &[ResourceRequirement],
    ) -> f64 {
        let min_start = normalize_time(min_start);
        let duration = normalize_time(duration);

        if op_name.eq_ignore_ascii_case("barrier") {
            return min_start;
        }

        // Collect candidate transition timestamps: min_start, and end times of all scheduled operations >= min_start
        let mut candidates = vec![min_start];
        for op in self.scheduled_ops.values() {
            if op.end_time >= min_start {
                candidates.push(op.end_time);
            }
        }

        // Also collect end times from resource reservations
        for req in requirements {
            if let Some(res_list) = self.reservations.get(&req.resource) {
                for r in res_list {
                    if r.end_time >= min_start {
                        candidates.push(r.end_time);
                    }
                }
            }
        }

        candidates.sort_by(|a, b| exact_cmp(*a, *b));
        let mut dedup_candidates: Vec<f64> = Vec::with_capacity(candidates.len());
        for c in candidates {
            if let Some(&last) = dedup_candidates.last() {
                if exact_cmp(c, last).is_eq() {
                    continue;
                }
            }
            dedup_candidates.push(c);
        }

        for candidate_t in dedup_candidates {
            let candidate_end = candidate_t + duration;
            if self
                .check_interval(
                    candidate_t,
                    candidate_end,
                    duration,
                    op_name,
                    operands,
                    requirements,
                )
                .is_ok()
            {
                return candidate_t;
            }
        }

        min_start
    }

    /// Finds the latest feasible start time $t \le t_{\text{max}} - \text{duration}$ where $t \ge t_{\text{min}}$.
    pub fn find_latest_feasible_time(
        &self,
        max_end: f64,
        min_start: f64,
        duration: f64,
        op_name: &str,
        operands: &[PhysicalQubitId],
        requirements: &[ResourceRequirement],
    ) -> Option<f64> {
        let max_end = normalize_time(max_end);
        let min_start = normalize_time(min_start);
        let duration = normalize_time(duration);

        if max_end < min_start + duration {
            return None;
        }

        if op_name.eq_ignore_ascii_case("barrier") {
            return Some(max_end - duration);
        }

        // Collect candidate end times: max_end, and start times / end times of scheduled ops in [min_start + duration, max_end]
        let mut candidates = vec![max_end];
        for op in self.scheduled_ops.values() {
            if op.start_time >= min_start + duration && op.start_time <= max_end {
                candidates.push(op.start_time);
            }
            if op.end_time >= min_start + duration && op.end_time <= max_end {
                candidates.push(op.end_time);
            }
        }

        for req in requirements {
            if let Some(res_list) = self.reservations.get(&req.resource) {
                for r in res_list {
                    if r.start_time >= min_start + duration && r.start_time <= max_end {
                        candidates.push(r.start_time);
                    }
                    if r.end_time >= min_start + duration && r.end_time <= max_end {
                        candidates.push(r.end_time);
                    }
                }
            }
        }

        // Sort descending
        candidates.sort_by(|a, b| exact_cmp(*b, *a));
        let mut dedup_candidates: Vec<f64> = Vec::with_capacity(candidates.len());
        for c in candidates {
            if let Some(&last) = dedup_candidates.last() {
                if exact_cmp(c, last).is_eq() {
                    continue;
                }
            }
            dedup_candidates.push(c);
        }

        for candidate_end in dedup_candidates {
            let candidate_start = candidate_end - duration;
            if candidate_start < min_start {
                continue;
            }

            if self
                .check_interval(
                    candidate_start,
                    candidate_end,
                    duration,
                    op_name,
                    operands,
                    requirements,
                )
                .is_ok()
            {
                return Some(candidate_start);
            }
        }

        None
    }

    /// Returns the active usage (sum of units) on a resource at timestamp $t$.
    pub fn active_usage_at(&self, resource: &ResourceId, t: f64) -> usize {
        let t = normalize_time(t);
        self.reservations
            .get(resource)
            .map(|list| {
                list.iter()
                    .filter(|r| r.is_active_at(t))
                    .map(|r| r.amount)
                    .sum()
            })
            .unwrap_or(0)
    }

    /// Returns all reservations actively executing on a resource at timestamp $t$.
    pub fn active_reservations_at(
        &self,
        resource: &ResourceId,
        t: f64,
    ) -> Vec<&ResourceReservation> {
        let t = normalize_time(t);
        self.reservations
            .get(resource)
            .map(|list| list.iter().filter(|r| r.is_active_at(t)).collect())
            .unwrap_or_default()
    }

    /// Computes the peak concurrent usage of a resource and the earliest timestamp where peak usage occurs.
    pub fn peak_usage(&self, resource: &ResourceId) -> (usize, f64) {
        let list = match self.reservations.get(resource) {
            Some(l) if !l.is_empty() => l,
            _ => return (0, 0.0),
        };

        let mut event_times: Vec<f64> = list.iter().map(|r| r.start_time).collect();
        event_times.sort_by(|a, b| exact_cmp(*a, *b));
        event_times.dedup_by(|a, b| exact_cmp(*a, *b).is_eq());

        let mut peak_val = 0;
        let mut peak_t = 0.0;

        for t in event_times {
            let usage = self.active_usage_at(resource, t);
            if usage > peak_val {
                peak_val = usage;
                peak_t = t;
            }
        }

        (peak_val, peak_t)
    }

    /// Returns all scheduled operations actively executing at timestamp $t$.
    pub fn operations_active_at(&self, t: f64) -> Vec<&ScheduledOperation> {
        let t = normalize_time(t);
        self.scheduled_ops
            .values()
            .filter(|op| op.is_active_at(t))
            .collect()
    }

    /// Constructs a validated `ResourceCalendar` from an existing `Schedule` and `Target`.
    ///
    /// Validates all scheduled operations and returns any collision or capacity violation
    /// as a structured `ScheduleValidationError`.
    pub fn from_schedule(
        schedule: &crate::scheduler::schedule::Schedule,
        target: &Target,
    ) -> Result<Self, ScheduleValidationError> {
        let resource_model = target.resources.as_ref().cloned().ok_or_else(|| {
            ScheduleValidationError::MissingResourceModel {
                target: target.name.clone(),
            }
        })?;

        let mut calendar = Self::new(resource_model);

        for op in schedule.operations() {
            if let Err(report) = calendar.reserve(op) {
                // Map ResourceConflictReport to ScheduleValidationError
                return Err(match *report.reason {
                    ResourceConflictReason::QubitOccupied {
                        qubit,
                        blocking_op_id,
                        ..
                    } => ScheduleValidationError::QubitCollision {
                        op1: blocking_op_id,
                        op2: op.id,
                        qubit,
                        interval: (op.start_time, op.end_time),
                    },
                    ResourceConflictReason::CouplerOccupied {
                        coupler,
                        blocking_op_id,
                        ..
                    } => ScheduleValidationError::CouplerCollision {
                        op1: blocking_op_id,
                        op2: op.id,
                        coupler,
                        interval: (op.start_time, op.end_time),
                    },
                    ResourceConflictReason::ExclusiveCollision {
                        resource,
                        blocking_op_id,
                        ..
                    } => ScheduleValidationError::ExclusiveResourceCollision {
                        op1: blocking_op_id,
                        op2: op.id,
                        resource,
                        interval: (op.start_time, op.end_time),
                    },
                    ResourceConflictReason::CapacityExceeded {
                        resource,
                        capacity,
                        active_usage,
                        timestamp,
                        ..
                    } => ScheduleValidationError::CapacityExceeded {
                        resource,
                        capacity,
                        usage: active_usage + 1,
                        time: timestamp,
                    },
                    ResourceConflictReason::MutualExclusion {
                        rule,
                        blocking_op_id,
                        reason,
                        ..
                    } => ScheduleValidationError::MutualExclusionViolation {
                        rule,
                        op1: blocking_op_id,
                        op2: op.id,
                        reason,
                    },
                    ResourceConflictReason::ReadoutExclusion {
                        qubit1: _,
                        qubit2: _,
                        reason,
                    } => ScheduleValidationError::MutualExclusionViolation {
                        rule: "readout_exclusion".to_string(),
                        op1: op.id,
                        op2: op.id,
                        reason,
                    },
                    ResourceConflictReason::MissingMetadata { op_name, operands } => {
                        ScheduleValidationError::MissingOperationResources { op_name, operands }
                    }
                });
            }
        }

        Ok(calendar)
    }
}
