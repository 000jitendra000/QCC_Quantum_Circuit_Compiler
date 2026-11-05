//! Hardware concurrency constraints, readout groups, and explainability engine.
//!
//! Models device-level shared control lines, multiplexed readout feedlines, and mutual exclusion
//! policies. Provides rich, structured diagnostics explaining *why* operations cannot execute concurrently
//! or why an operation was delayed during physical scheduling.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::scheduler::asap::ScheduleConfig;
use crate::scheduler::resource_calendar::ResourceCalendar;
use crate::scheduler::schedule::{Schedule, ScheduledOperation};
use crate::scheduler::ScheduleError;
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

/// Synchronization policy for multiplexed readout groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ReadoutSynchronizationPolicy {
    /// Measurements on distinct qubits in the feedline group can start independently,
    /// bounded only by the aggregate feedline capacity limit.
    Independent,
    /// Concurrent measurements in the readout group must start at the exact same timestamp
    /// ($|t_1 - t_2| \le \epsilon$). Staggered or partially overlapping measurements are prohibited.
    SimultaneousOnly,
    /// Only a single measurement in the readout group may execute at any given time (capacity 1).
    StrictlySerialized,
}

impl fmt::Display for ReadoutSynchronizationPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Independent => write!(f, "independent"),
            Self::SimultaneousOnly => write!(f, "simultaneous-only"),
            Self::StrictlySerialized => write!(f, "strictly-serialized"),
        }
    }
}

/// Multiplexed readout feedline group sharing physical measurement infrastructure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadoutGroup {
    /// Unique group identifier.
    pub id: u32,
    /// Human-readable group name.
    pub name: String,
    /// Underlying physical readout feedline identifier.
    pub feedline: u32,
    /// Set of physical qubits multiplexed onto this feedline.
    pub qubits: BTreeSet<PhysicalQubitId>,
    /// Maximum concurrent measurement capacity on this feedline.
    pub capacity: usize,
    /// Group readout synchronization policy.
    pub sync_policy: ReadoutSynchronizationPolicy,
}

impl ReadoutGroup {
    /// Constructs a new readout group with default independent concurrency.
    pub fn new(
        id: u32,
        feedline: u32,
        qubits: impl IntoIterator<Item = PhysicalQubitId>,
        capacity: usize,
    ) -> Self {
        Self {
            id,
            name: format!("readout_group_{id}"),
            feedline,
            qubits: qubits.into_iter().collect(),
            capacity: capacity.max(1),
            sync_policy: ReadoutSynchronizationPolicy::Independent,
        }
    }

    /// Configures the synchronization policy for this readout group.
    pub fn with_sync_policy(mut self, sync_policy: ReadoutSynchronizationPolicy) -> Self {
        self.sync_policy = sync_policy;
        self
    }

    /// Sets a descriptive name for this readout group.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Returns `true` if this readout group contains the specified physical qubit.
    #[inline]
    pub fn contains_qubit(&self, qubit: PhysicalQubitId) -> bool {
        self.qubits.contains(&qubit)
    }

    /// Returns the associated `ResourceId` for this readout group's feedline.
    #[inline]
    pub fn resource_id(&self) -> ResourceId {
        ResourceId::ReadoutLine(self.feedline)
    }
}

/// Hardware control channel shared across multiple physical qubits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedControlGroup {
    /// Unique group identifier.
    pub id: u32,
    /// Human-readable group name.
    pub name: String,
    /// Underlying hardware resource (e.g. drive line or local oscillator).
    pub channel_id: ResourceId,
    /// Set of physical qubits sharing this control channel.
    pub qubits: BTreeSet<PhysicalQubitId>,
    /// Usage access mode (exclusive or shared).
    pub mode: ResourceUsageMode,
    /// Channel capacity limit.
    pub capacity: usize,
}

impl SharedControlGroup {
    /// Constructs a new shared control channel group.
    pub fn new(
        id: u32,
        channel_id: ResourceId,
        qubits: impl IntoIterator<Item = PhysicalQubitId>,
        mode: ResourceUsageMode,
        capacity: usize,
    ) -> Self {
        Self {
            id,
            name: format!("control_group_{id}"),
            channel_id,
            qubits: qubits.into_iter().collect(),
            mode,
            capacity: capacity.max(1),
        }
    }

    /// Sets a descriptive name for this control group.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Returns `true` if this control group contains the specified physical qubit.
    #[inline]
    pub fn contains_qubit(&self, qubit: PhysicalQubitId) -> bool {
        self.qubits.contains(&qubit)
    }
}

/// Specific hardware constraint inhibiting concurrent execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InhibitingConstraint {
    /// Two operations collide on the same physical qubit.
    SharedQubit(PhysicalQubitId),
    /// Two operations collide on the same physical coupler channel.
    SharedCoupler(PhysicalQubitId, PhysicalQubitId),
    /// Contention on an exclusively held hardware resource (e.g. drive line or bus).
    ExclusiveResource(ResourceId),
    /// Aggregate concurrent demand exceeds resource capacity.
    CapacityExceeded {
        /// Overcommitted resource.
        resource: ResourceId,
        /// Maximum capacity limit.
        capacity: usize,
        /// Total combined demand.
        combined_demand: usize,
    },
    /// Violated target mutual exclusion rule.
    MutualExclusion {
        /// Name of the mutual exclusion rule.
        rule: String,
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
    /// Unsynchronized concurrent readouts on a feedline requiring simultaneous start.
    ReadoutDesynchronization {
        /// Feedline identifier.
        line: u32,
        /// Detailed explanation.
        reason: String,
    },
    /// Operation lacks declared resource metadata.
    MissingMetadata {
        /// Operation mnemonic.
        op_name: String,
        /// Physical operand qubits.
        operands: Vec<PhysicalQubitId>,
    },
}

impl fmt::Display for InhibitingConstraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SharedQubit(q) => write!(f, "qubit {q} collision"),
            Self::SharedCoupler(q0, q1) => write!(f, "coupler ({q0}, {q1}) collision"),
            Self::ExclusiveResource(r) => write!(f, "exclusive resource '{r}' collision"),
            Self::CapacityExceeded {
                resource,
                capacity,
                combined_demand,
            } => {
                write!(
                    f,
                    "capacity exceeded on '{resource}' (demand {combined_demand} > capacity {capacity})"
                )
            }
            Self::MutualExclusion { rule, reason } => {
                write!(f, "mutual exclusion '{rule}': {reason}")
            }
            Self::ReadoutExclusion {
                qubit1,
                qubit2,
                reason,
            } => {
                write!(
                    f,
                    "readout exclusion between qubit {qubit1} and {qubit2}: {reason}"
                )
            }
            Self::ReadoutDesynchronization { line, reason } => {
                write!(f, "feedline {line} desynchronization: {reason}")
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

/// Comprehensive explainability analysis for concurrent execution of two operations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConcurrencyAnalysis {
    /// `true` if the operations can execute concurrently without resource conflicts.
    pub can_run_concurrently: bool,
    /// List of all hardware constraints that inhibit concurrent execution.
    pub inhibiting_constraints: Vec<InhibitingConstraint>,
}

impl ConcurrencyAnalysis {
    /// Returns `true` if the operations are completely compatible.
    #[inline]
    pub fn is_compatible(&self) -> bool {
        self.can_run_concurrently && self.inhibiting_constraints.is_empty()
    }

    /// Returns `true` if any hardware constraint prevents concurrency.
    #[inline]
    pub fn has_conflict(&self) -> bool {
        !self.can_run_concurrently
    }
}

/// Description of an active blocker that delayed an operation's scheduled placement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacementBlocker {
    /// Candidate timestamp where placement was attempted.
    pub candidate_time: f64,
    /// Operation node identifier holding the contested resource, if known.
    pub blocking_op_id: Option<OperationId>,
    /// Name of the blocking operation, if known.
    pub blocking_op_name: Option<String>,
    /// Contested hardware resource identifier.
    pub blocking_resource: ResourceId,
    /// Detailed inhibiting constraint.
    pub constraint: InhibitingConstraint,
    /// Expected timestamp when the blocker will release the resource.
    pub expected_release_time: f64,
}

impl fmt::Display for PlacementBlocker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let (Some(id), Some(name)) = (self.blocking_op_id, &self.blocking_op_name) {
            write!(
                f,
                "blocked at {:.2}ns by '{name}' (op#{}) on '{}' until {:.2}ns ({})",
                self.candidate_time,
                id.0,
                self.blocking_resource,
                self.expected_release_time,
                self.constraint
            )
        } else {
            write!(
                f,
                "blocked at {:.2}ns on '{}' until {:.2}ns ({})",
                self.candidate_time,
                self.blocking_resource,
                self.expected_release_time,
                self.constraint
            )
        }
    }
}

/// Diagnostic trace entry recording why an operation was delayed during scheduling.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SchedulingTraceEntry {
    /// Operation node identifier.
    pub op_id: OperationId,
    /// Operation mnemonic.
    pub op_name: String,
    /// Physical operand qubits.
    pub operands: Vec<PhysicalQubitId>,
    /// Predecessor completion bound in nanoseconds ($T_{\text{pred}}$).
    pub predecessor_bound: f64,
    /// Actual scheduled start time in nanoseconds.
    pub scheduled_start: f64,
    /// Operation duration in nanoseconds.
    pub duration: f64,
    /// Delay incurred due to resource contention ($T_{\text{start}} - T_{\text{pred}}$).
    pub delay: f64,
    /// List of placement blockers encountered before final placement.
    pub blockers: Vec<PlacementBlocker>,
}

/// Centralized manager for hardware concurrency constraints, readout groups, and explainability.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResourceConstraintManager {
    /// Underlying hardware resource model.
    resource_model: ResourceModel,
    /// Declared multiplexed readout groups indexed by group ID.
    readout_groups: BTreeMap<u32, ReadoutGroup>,
    /// Declared shared control channel groups indexed by group ID.
    control_groups: BTreeMap<u32, SharedControlGroup>,
}

impl ResourceConstraintManager {
    /// Constructs a constraint manager backed by a target resource model.
    pub fn new(resource_model: ResourceModel) -> Self {
        Self {
            resource_model,
            readout_groups: BTreeMap::new(),
            control_groups: BTreeMap::new(),
        }
    }

    /// Constructs a constraint manager extracted from a `Target` specification.
    pub fn from_target(target: &Target) -> Result<Self, ScheduleError> {
        let resource_model = target.resources.as_ref().cloned().ok_or_else(|| {
            ScheduleError::MissingResourceModel {
                target: target.name.clone(),
            }
        })?;

        let mut manager = Self::new(resource_model);

        // Populate readout groups from qubit feedline mappings
        let mut feedline_map: BTreeMap<u32, BTreeSet<PhysicalQubitId>> = BTreeMap::new();
        for entry in &manager.resource_model.qubit_readout_lines {
            feedline_map
                .entry(entry.line)
                .or_default()
                .insert(entry.qubit);
        }

        for (line_id, qubits) in feedline_map {
            let cap = manager
                .resource_model
                .capacity_of(&ResourceId::ReadoutLine(line_id));
            let group = ReadoutGroup::new(line_id, line_id, qubits, cap);
            manager.add_readout_group(group);
        }

        Ok(manager)
    }

    /// Returns a reference to the underlying resource model.
    #[inline]
    pub fn resource_model(&self) -> &ResourceModel {
        &self.resource_model
    }

    /// Returns a mutable reference to the underlying resource model.
    #[inline]
    pub fn resource_model_mut(&mut self) -> &mut ResourceModel {
        &mut self.resource_model
    }

    /// Registers a multiplexed readout group.
    pub fn add_readout_group(&mut self, group: ReadoutGroup) {
        // Synchronize with resource model
        for &q in &group.qubits {
            self.resource_model
                .map_qubit_readout_line(q, group.feedline);
        }
        self.resource_model
            .set_capacity(group.resource_id(), group.capacity);

        self.readout_groups.insert(group.id, group);
    }

    /// Registers a shared control channel group.
    pub fn add_control_group(&mut self, group: SharedControlGroup) {
        self.resource_model
            .set_capacity(group.channel_id.clone(), group.capacity);
        self.control_groups.insert(group.id, group);
    }

    /// Returns the readout group associated with a physical qubit, if any.
    pub fn readout_group_for(&self, qubit: PhysicalQubitId) -> Option<&ReadoutGroup> {
        self.readout_groups
            .values()
            .find(|g| g.contains_qubit(qubit))
    }

    /// Returns all shared control groups associated with a physical qubit.
    pub fn control_groups_for(&self, qubit: PhysicalQubitId) -> Vec<&SharedControlGroup> {
        self.control_groups
            .values()
            .filter(|g| g.contains_qubit(qubit))
            .collect()
    }

    /// Resolves the comprehensive physical resource requirements for an operation.
    pub fn resolve_requirements(
        &self,
        op_name: &str,
        operands: &[PhysicalQubitId],
    ) -> Result<Vec<ResourceRequirement>, ScheduleError> {
        let target_op_id = TargetOpId::new(op_name);

        if !self
            .resource_model
            .has_metadata_for(&target_op_id, operands)
        {
            return Err(ScheduleError::MissingOperationResources {
                op_name: op_name.to_string(),
                operands: operands.to_vec(),
            });
        }

        let mut reqs = self
            .resource_model
            .requirements_for(&target_op_id, operands);

        // Add control group requirements if not already present
        for &q in operands {
            for cg in self.control_groups_for(q) {
                if !reqs.iter().any(|r| r.resource == cg.channel_id) {
                    reqs.push(match cg.mode {
                        ResourceUsageMode::Exclusive => {
                            ResourceRequirement::exclusive(cg.channel_id.clone())
                        }
                        ResourceUsageMode::Shared => {
                            ResourceRequirement::shared(cg.channel_id.clone(), 1)
                        }
                    });
                }
            }
        }

        Ok(reqs)
    }

    /// Analyzes whether two operations can execute concurrently, reporting all inhibiting constraints.
    pub fn analyze_concurrency(
        &self,
        op1: (&str, &[PhysicalQubitId]),
        op2: (&str, &[PhysicalQubitId]),
    ) -> ConcurrencyAnalysis {
        let (name1, ops1) = op1;
        let (name2, ops2) = op2;
        let id1 = TargetOpId::new(name1);
        let id2 = TargetOpId::new(name2);

        let mut constraints = Vec::new();

        // 1. Verify resource metadata presence
        if !self.resource_model.has_metadata_for(&id1, ops1) {
            constraints.push(InhibitingConstraint::MissingMetadata {
                op_name: name1.to_string(),
                operands: ops1.to_vec(),
            });
        }
        if !self.resource_model.has_metadata_for(&id2, ops2) {
            constraints.push(InhibitingConstraint::MissingMetadata {
                op_name: name2.to_string(),
                operands: ops2.to_vec(),
            });
        }

        if !constraints.is_empty() {
            return ConcurrencyAnalysis {
                can_run_concurrently: false,
                inhibiting_constraints: constraints,
            };
        }

        // 2. Qubit single-occupancy
        for &q1 in ops1 {
            for &q2 in ops2 {
                if q1 == q2 {
                    constraints.push(InhibitingConstraint::SharedQubit(q1));
                }
            }
        }

        // 3. Coupler channel collision for two-qubit operations
        if ops1.len() == 2 && ops2.len() == 2 {
            let c1 = (ops1[0].min(ops1[1]), ops1[0].max(ops1[1]));
            let c2 = (ops2[0].min(ops2[1]), ops2[0].max(ops2[1]));
            if c1 == c2 {
                constraints.push(InhibitingConstraint::SharedCoupler(c1.0, c1.1));
            }
        }

        // 4. Mutual exclusion rules
        for rule in &self.resource_model.mutual_exclusions {
            match &rule.condition {
                ExclusionCondition::GlobalExclusive { operation } => {
                    if &id1 == operation || &id2 == operation {
                        constraints.push(InhibitingConstraint::MutualExclusion {
                            rule: rule.name.clone(),
                            reason: format!(
                                "operation '{operation}' requires global device quiescence"
                            ),
                        });
                    }
                }
                ExclusionCondition::OperationPair {
                    op1: r_op1,
                    operands1: r_ops1,
                    op2: r_op2,
                    operands2: r_ops2,
                } => {
                    let match_fwd = &id1 == r_op1
                        && r_ops1
                            .as_ref()
                            .map(|o| o.as_slice() == ops1)
                            .unwrap_or(true)
                        && &id2 == r_op2
                        && r_ops2
                            .as_ref()
                            .map(|o| o.as_slice() == ops2)
                            .unwrap_or(true);

                    let match_rev = &id2 == r_op1
                        && r_ops1
                            .as_ref()
                            .map(|o| o.as_slice() == ops2)
                            .unwrap_or(true)
                        && &id1 == r_op2
                        && r_ops2
                            .as_ref()
                            .map(|o| o.as_slice() == ops1)
                            .unwrap_or(true);

                    if match_fwd || match_rev {
                        constraints.push(InhibitingConstraint::MutualExclusion {
                            rule: rule.name.clone(),
                            reason: format!(
                                "operation pair exclusion between '{name1}' and '{name2}'"
                            ),
                        });
                    }
                }
                ExclusionCondition::ReadoutExclusionGroup { qubits } => {
                    if name1 == "measure" && name2 == "measure" {
                        for &q1 in ops1 {
                            for &q2 in ops2 {
                                if q1 != q2 && qubits.contains(&q1) && qubits.contains(&q2) {
                                    constraints.push(InhibitingConstraint::ReadoutExclusion {
                                        qubit1: q1,
                                        qubit2: q2,
                                        reason: format!(
                                            "qubits belong to readout exclusion group '{}'",
                                            rule.name
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // 5. Exclusive resources and aggregate shared capacities
        let reqs1 = self.resource_model.requirements_for(&id1, ops1);
        let reqs2 = self.resource_model.requirements_for(&id2, ops2);

        for r1 in &reqs1 {
            for r2 in &reqs2 {
                if r1.resource == r2.resource {
                    if r1.mode == ResourceUsageMode::Exclusive
                        || r2.mode == ResourceUsageMode::Exclusive
                    {
                        constraints
                            .push(InhibitingConstraint::ExclusiveResource(r1.resource.clone()));
                    } else {
                        let cap = self.resource_model.capacity_of(&r1.resource);
                        let total_demand = r1.amount + r2.amount;
                        if total_demand > cap {
                            constraints.push(InhibitingConstraint::CapacityExceeded {
                                resource: r1.resource.clone(),
                                capacity: cap,
                                combined_demand: total_demand,
                            });
                        }
                    }
                }
            }
        }

        // 6. Readout Group synchronization checks
        if name1 == "measure" && name2 == "measure" {
            for &q1 in ops1 {
                for &q2 in ops2 {
                    if let (Some(g1), Some(g2)) =
                        (self.readout_group_for(q1), self.readout_group_for(q2))
                    {
                        if g1.id == g2.id
                            && g1.sync_policy == ReadoutSynchronizationPolicy::StrictlySerialized
                        {
                            constraints.push(InhibitingConstraint::CapacityExceeded {
                                resource: g1.resource_id(),
                                capacity: 1,
                                combined_demand: 2,
                            });
                        }
                    }
                }
            }
        }

        let can_run = constraints.is_empty();
        ConcurrencyAnalysis {
            can_run_concurrently: can_run,
            inhibiting_constraints: constraints,
        }
    }

    /// Explains all placement blockers preventing an operation from starting at `candidate_t`.
    pub fn explain_placement_blockers(
        &self,
        calendar: &ResourceCalendar,
        candidate_t: f64,
        duration: f64,
        op_name: &str,
        operands: &[PhysicalQubitId],
    ) -> Vec<PlacementBlocker> {
        let candidate_t = normalize_time(candidate_t);
        let duration = normalize_time(duration);
        let end_t = candidate_t + duration;

        let mut blockers = Vec::new();
        let target_op_id = TargetOpId::new(op_name);

        if !self
            .resource_model
            .has_metadata_for(&target_op_id, operands)
        {
            blockers.push(PlacementBlocker {
                candidate_time: candidate_t,
                blocking_op_id: None,
                blocking_op_name: None,
                blocking_resource: ResourceId::Custom("metadata".to_string()),
                constraint: InhibitingConstraint::MissingMetadata {
                    op_name: op_name.to_string(),
                    operands: operands.to_vec(),
                },
                expected_release_time: f64::INFINITY,
            });
            return blockers;
        }

        // Check Qubit single-occupancy blockers
        for &q in operands {
            let q_res = ResourceId::Qubit(q);
            for r in calendar.timeline_for(&q_res) {
                if r.overlaps(candidate_t, end_t, duration) {
                    blockers.push(PlacementBlocker {
                        candidate_time: candidate_t,
                        blocking_op_id: Some(r.op_id),
                        blocking_op_name: Some(r.op_name.clone()),
                        blocking_resource: q_res.clone(),
                        constraint: InhibitingConstraint::SharedQubit(q),
                        expected_release_time: r.end_time,
                    });
                }
            }
        }

        // Check Coupler blockers
        if operands.len() == 2 {
            let coupler_res = ResourceId::coupler(operands[0], operands[1]);
            for r in calendar.timeline_for(&coupler_res) {
                if r.overlaps(candidate_t, end_t, duration) {
                    blockers.push(PlacementBlocker {
                        candidate_time: candidate_t,
                        blocking_op_id: Some(r.op_id),
                        blocking_op_name: Some(r.op_name.clone()),
                        blocking_resource: coupler_res.clone(),
                        constraint: InhibitingConstraint::SharedCoupler(operands[0], operands[1]),
                        expected_release_time: r.end_time,
                    });
                }
            }
        }

        // Check Exclusive resource blockers
        let reqs = self
            .resolve_requirements(op_name, operands)
            .unwrap_or_default();

        for req in &reqs {
            for r in calendar.timeline_for(&req.resource) {
                if r.overlaps(candidate_t, end_t, duration)
                    && (req.mode == ResourceUsageMode::Exclusive
                        || r.mode == ResourceUsageMode::Exclusive)
                {
                    blockers.push(PlacementBlocker {
                        candidate_time: candidate_t,
                        blocking_op_id: Some(r.op_id),
                        blocking_op_name: Some(r.op_name.clone()),
                        blocking_resource: req.resource.clone(),
                        constraint: InhibitingConstraint::ExclusiveResource(req.resource.clone()),
                        expected_release_time: r.end_time,
                    });
                }
            }

            // Check capacity blockers
            if req.mode == ResourceUsageMode::Shared {
                let cap = self.resource_model.capacity_of(&req.resource);
                let active = calendar.active_usage_at(&req.resource, candidate_t);
                if active + req.amount > cap {
                    let active_res = calendar.active_reservations_at(&req.resource, candidate_t);
                    let earliest_release = active_res
                        .iter()
                        .map(|r| r.end_time)
                        .fold(f64::INFINITY, f64::min);

                    blockers.push(PlacementBlocker {
                        candidate_time: candidate_t,
                        blocking_op_id: active_res.first().map(|r| r.op_id),
                        blocking_op_name: active_res.first().map(|r| r.op_name.clone()),
                        blocking_resource: req.resource.clone(),
                        constraint: InhibitingConstraint::CapacityExceeded {
                            resource: req.resource.clone(),
                            capacity: cap,
                            combined_demand: active + req.amount,
                        },
                        expected_release_time: earliest_release,
                    });
                }
            }
        }

        blockers
    }
}

/// Result produced by `ResourceConstrainedScheduler`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceScheduleResult {
    /// Resulting physical schedule.
    pub schedule: Schedule,
    /// Populated resource calendar.
    pub calendar: ResourceCalendar,
    /// Diagnostic scheduling traces explaining delays.
    pub traces: Vec<SchedulingTraceEntry>,
}

/// Advanced resource-constrained quantum instruction scheduler.
///
/// Coordinates physical execution timelines with `ResourceCalendar` and `ResourceConstraintManager`,
/// enforcing hardware capacities, shared drive lines, and multiplexed feedlines with full explainability.
pub struct ResourceConstrainedScheduler {
    manager: ResourceConstraintManager,
    config: ScheduleConfig,
}

impl ResourceConstrainedScheduler {
    /// Constructs a resource-constrained scheduler with default configuration.
    pub fn new(manager: ResourceConstraintManager) -> Self {
        Self {
            manager,
            config: ScheduleConfig::default(),
        }
    }

    /// Constructs a resource-constrained scheduler with explicit configuration.
    pub fn with_config(manager: ResourceConstraintManager, config: ScheduleConfig) -> Self {
        Self { manager, config }
    }

    /// Schedules a QccIR circuit DAG under physical timing and hardware resource constraints.
    pub fn schedule(
        &self,
        circuit: &Circuit,
        target: &Target,
    ) -> Result<ResourceScheduleResult, ScheduleError> {
        let clock_period = target.timing.as_ref().and_then(|t| t.clock_period_ns);
        let mut calendar = ResourceCalendar::new(self.manager.resource_model.clone());
        let mut schedule = Schedule::new(&circuit.name, &target.name);

        let topological_order = circuit
            .graph
            .deterministic_topological_order()
            .map_err(|e| ScheduleError::DagError(e.to_string()))?;

        let mut completed_times: BTreeMap<OperationId, f64> = BTreeMap::new();
        let mut traces = Vec::new();

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
            let duration = normalize_time(duration);

            // 2. Resolve requirements
            let requirements = if is_barrier {
                Vec::new()
            } else {
                self.manager.resolve_requirements(op_name, &operands)?
            };

            // 3. Predecessor completion bound
            let mut t_pred = 0.0;
            for (pred_id, _) in circuit.graph.predecessors(op_id) {
                if let Some(&pred_end) = completed_times.get(&pred_id) {
                    if pred_end > t_pred {
                        t_pred = pred_end;
                    }
                }
            }
            let t_pred = normalize_time(t_pred);

            // 4. Find earliest feasible start time in calendar
            let start_time = calendar.find_earliest_feasible_time(
                t_pred,
                duration,
                op_name,
                &operands,
                &requirements,
            );
            let start_time = normalize_time(start_time);
            let end_time = normalize_time(start_time + duration);

            // 5. If delayed, record explainability trace
            let delay = normalize_time(start_time - t_pred);
            if delay > self.config.time_epsilon {
                let blockers = self
                    .manager
                    .explain_placement_blockers(&calendar, t_pred, duration, op_name, &operands);
                traces.push(SchedulingTraceEntry {
                    op_id,
                    op_name: op_name.to_string(),
                    operands: operands.clone(),
                    predecessor_bound: t_pred,
                    scheduled_start: start_time,
                    duration,
                    delay,
                    blockers,
                });
            }

            // 6. Build and register scheduled operation
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

            calendar.reserve(&scheduled_op).map_err(|e| {
                ScheduleError::DagError(format!("calendar reservation failure for op#{op_id}: {e}"))
            })?;
            completed_times.insert(op_id, end_time);
            schedule.add_operation(scheduled_op);
        }

        Ok(ResourceScheduleResult {
            schedule,
            calendar,
            traces,
        })
    }
}
