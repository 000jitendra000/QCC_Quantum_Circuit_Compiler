//! Hardware resource modeling, concurrency constraints, and mutual exclusions.
//!
//! Prevents assuming disconnected gates can always run concurrently without explicit hardware
//! resource and cross-talk validation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::target::error::TargetError;
use crate::target::operation::{PhysicalQubitId, TargetOpId};

/// Strongly typed identifier for a hardware execution resource.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResourceId {
    /// Dedicated drive / control channel for a physical qubit.
    Qubit(PhysicalQubitId),
    /// Coupling channel / tunable bus between two physical qubits.
    Coupler(PhysicalQubitId, PhysicalQubitId),
    /// Multiplexed readout feedline or resonator bus.
    ReadoutLine(u32),
    /// Shared microwave control or drive line.
    DriveLine(u32),
    /// Signal acquisition unit or digitizer channel.
    AcquisitionUnit(u32),
    /// Custom named hardware resource.
    Custom(String),
}

impl ResourceId {
    /// Constructs a normalized symmetric coupler resource identifier.
    #[inline]
    pub fn coupler(q0: PhysicalQubitId, q1: PhysicalQubitId) -> Self {
        if q0 <= q1 {
            Self::Coupler(q0, q1)
        } else {
            Self::Coupler(q1, q0)
        }
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Qubit(q) => write!(f, "qubit:{q}"),
            Self::Coupler(q0, q1) => write!(f, "coupler:({q0}, {q1})"),
            Self::ReadoutLine(line) => write!(f, "readout_line:{line}"),
            Self::DriveLine(line) => write!(f, "drive_line:{line}"),
            Self::AcquisitionUnit(unit) => write!(f, "acquisition_unit:{unit}"),
            Self::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

/// Resource usage access mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResourceUsageMode {
    /// Exclusive access: resource cannot be shared with any concurrent operation.
    Exclusive,
    /// Shared access: multiple operations may access the resource up to its capacity limit.
    Shared,
}

impl fmt::Display for ResourceUsageMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exclusive => write!(f, "exclusive"),
            Self::Shared => write!(f, "shared"),
        }
    }
}

/// Hardware resource requirement for executing an operation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceRequirement {
    /// Resource identifier.
    pub resource: ResourceId,
    /// Access mode (exclusive or shared).
    pub mode: ResourceUsageMode,
    /// Number of units required (typically 1 for exclusive channels).
    pub amount: usize,
}

impl ResourceRequirement {
    /// Constructs an exclusive resource requirement with amount 1.
    pub fn exclusive(resource: ResourceId) -> Self {
        Self {
            resource,
            mode: ResourceUsageMode::Exclusive,
            amount: 1,
        }
    }

    /// Constructs a shared resource requirement with a specified capacity amount.
    pub fn shared(resource: ResourceId, amount: usize) -> Self {
        Self {
            resource,
            mode: ResourceUsageMode::Shared,
            amount,
        }
    }
}

impl fmt::Display for ResourceRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} x{})", self.resource, self.mode, self.amount)
    }
}

/// Specific exclusion condition for mutual exclusion rules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExclusionCondition {
    /// Pair of operations that cannot execute concurrently on designated operands.
    OperationPair {
        /// First operation name.
        op1: TargetOpId,
        /// Optional specific operands for first operation (`None` matches any operands).
        operands1: Option<Vec<PhysicalQubitId>>,
        /// Second operation name.
        op2: TargetOpId,
        /// Optional specific operands for second operation (`None` matches any operands).
        operands2: Option<Vec<PhysicalQubitId>>,
    },
    /// Disallowed simultaneous readout across physical qubits (e.g. frequency clash or cross-talk).
    ReadoutExclusionGroup {
        /// Qubits that cannot be measured simultaneously.
        qubits: BTreeSet<PhysicalQubitId>,
    },
    /// Global operation that requires complete hardware quiescence (no other gates can execute).
    GlobalExclusive {
        /// Target operation requiring global exclusivity.
        operation: TargetOpId,
    },
}

/// Mutual exclusion rule preventing concurrent execution of conflicting operations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutualExclusionRule {
    /// Name or documentation of the mutual exclusion constraint.
    pub name: String,
    /// Exclusion condition.
    pub condition: ExclusionCondition,
}

impl MutualExclusionRule {
    /// Constructs a new mutual exclusion rule.
    pub fn new(name: impl Into<String>, condition: ExclusionCondition) -> Self {
        Self {
            name: name.into(),
            condition,
        }
    }

    /// Creates an operation-pair exclusion rule.
    pub fn operation_pair(
        name: impl Into<String>,
        op1: impl Into<TargetOpId>,
        operands1: Option<Vec<PhysicalQubitId>>,
        op2: impl Into<TargetOpId>,
        operands2: Option<Vec<PhysicalQubitId>>,
    ) -> Self {
        Self::new(
            name,
            ExclusionCondition::OperationPair {
                op1: op1.into(),
                operands1,
                op2: op2.into(),
                operands2,
            },
        )
    }

    /// Creates a readout exclusion rule for a group of qubits.
    pub fn readout_exclusion(
        name: impl Into<String>,
        qubits: impl IntoIterator<Item = PhysicalQubitId>,
    ) -> Self {
        Self::new(
            name,
            ExclusionCondition::ReadoutExclusionGroup {
                qubits: qubits.into_iter().collect(),
            },
        )
    }

    /// Creates a global exclusive operation rule.
    pub fn global_exclusive(name: impl Into<String>, operation: impl Into<TargetOpId>) -> Self {
        Self::new(
            name,
            ExclusionCondition::GlobalExclusive {
                operation: operation.into(),
            },
        )
    }
}

/// Detailed description of a concurrency resource conflict.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ResourceConflict {
    /// Collision on an exclusively held resource.
    #[error("exclusive resource collision on '{resource}': operation '{op1}' and '{op2}' compete for exclusive access")]
    ExclusiveCollision {
        /// The conflicting resource.
        resource: ResourceId,
        /// First operation.
        op1: String,
        /// Second operation.
        op2: String,
    },

    /// Resource capacity was exceeded by concurrent demand.
    #[error(
        "resource capacity exceeded for '{resource}': requested {requested}, capacity {capacity}"
    )]
    CapacityExceeded {
        /// The overcommitted resource.
        resource: ResourceId,
        /// Requested concurrent capacity.
        requested: usize,
        /// Maximum supported capacity.
        capacity: usize,
    },

    /// Mutual exclusion rule was violated.
    #[error("mutual exclusion violation '{rule}': {reason}")]
    MutualExclusion {
        /// Name of the violated rule.
        rule: String,
        /// Explanation of the conflict.
        reason: String,
    },

    /// Disallowed simultaneous readout.
    #[error("readout exclusion conflict: qubits {qubit1} and {qubit2} cannot be measured simultaneously ({reason})")]
    ReadoutExclusion {
        /// First physical qubit.
        qubit1: PhysicalQubitId,
        /// Second physical qubit.
        qubit2: PhysicalQubitId,
        /// Failure reason.
        reason: String,
    },

    /// Target has no declared resource model.
    #[error(
        "target '{target}' has no declared resource model: cannot assume concurrent execution"
    )]
    MissingResourceModel {
        /// Target device name.
        target: String,
    },

    /// Missing resource metadata for an operation.
    #[error("missing resource metadata for operation '{op}' on operands {operands:?}: cannot assume unrestricted concurrency")]
    MissingOperationResources {
        /// Operation mnemonic.
        op: String,
        /// Physical operand tuple.
        operands: Vec<PhysicalQubitId>,
    },
}

/// Tuple-specific resource requirement override entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TupleResourceRequirementEntry {
    /// Target operation identifier.
    pub operation: TargetOpId,
    /// Physical operand tuple.
    pub operands: Vec<PhysicalQubitId>,
    /// Resource requirements for this tuple.
    pub requirements: Vec<ResourceRequirement>,
}

/// Entry declaring explicit capacity for a hardware resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceCapacityEntry {
    /// Resource identifier.
    pub resource: ResourceId,
    /// Maximum concurrent capacity ($\ge 1$).
    pub capacity: usize,
}

/// Association between a physical qubit and a multiplexed readout line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QubitReadoutLineEntry {
    /// Physical qubit identifier.
    pub qubit: PhysicalQubitId,
    /// Readout feedline identifier.
    pub line: u32,
}

/// Comprehensive target resource and concurrency constraint model.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResourceModel {
    /// Explicit capacity limits per resource. Resources not listed have a default capacity of 1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capacities: Vec<ResourceCapacityEntry>,
    /// Default resource requirements per target operation mnemonic.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub default_requirements: BTreeMap<TargetOpId, Vec<ResourceRequirement>>,
    /// Tuple-specific resource requirements (takes precedence over default requirements).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tuple_requirements: Vec<TupleResourceRequirementEntry>,
    /// Mutual exclusion rules between operations and qubits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mutual_exclusions: Vec<MutualExclusionRule>,
    /// Mapping of physical qubits to shared readout lines (feedlines).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qubit_readout_lines: Vec<QubitReadoutLineEntry>,
}

impl ResourceModel {
    /// Creates an empty resource model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets explicit capacity for a resource.
    pub fn set_capacity(&mut self, resource: ResourceId, capacity: usize) {
        if let Some(entry) = self.capacities.iter_mut().find(|e| e.resource == resource) {
            entry.capacity = capacity;
        } else {
            self.capacities
                .push(ResourceCapacityEntry { resource, capacity });
            self.capacities.sort_by(|a, b| a.resource.cmp(&b.resource));
        }
    }

    /// Returns the capacity limit for a resource, defaulting to 1 if not explicitly declared.
    pub fn capacity_of(&self, resource: &ResourceId) -> usize {
        self.capacities
            .iter()
            .find(|e| &e.resource == resource)
            .map(|e| e.capacity)
            .unwrap_or(1)
    }

    /// Sets default resource requirements for an operation mnemonic.
    pub fn set_default_requirements(
        &mut self,
        op: impl Into<TargetOpId>,
        requirements: Vec<ResourceRequirement>,
    ) {
        self.default_requirements.insert(op.into(), requirements);
    }

    /// Sets tuple-specific resource requirements.
    pub fn set_tuple_requirements(
        &mut self,
        op: impl Into<TargetOpId>,
        operands: impl IntoIterator<Item = PhysicalQubitId>,
        requirements: Vec<ResourceRequirement>,
    ) {
        let op_id = op.into();
        let ops: Vec<PhysicalQubitId> = operands.into_iter().collect();
        if let Some(entry) = self
            .tuple_requirements
            .iter_mut()
            .find(|e| e.operation == op_id && e.operands == ops)
        {
            entry.requirements = requirements;
        } else {
            self.tuple_requirements.push(TupleResourceRequirementEntry {
                operation: op_id,
                operands: ops,
                requirements,
            });
            self.tuple_requirements
                .sort_by(|a, b| (&a.operation, &a.operands).cmp(&(&b.operation, &b.operands)));
        }
    }

    /// Adds a mutual exclusion rule.
    pub fn add_mutual_exclusion(&mut self, rule: MutualExclusionRule) {
        self.mutual_exclusions.push(rule);
    }

    /// Maps a physical qubit to a shared readout line.
    pub fn map_qubit_readout_line(&mut self, qubit: PhysicalQubitId, line: u32) {
        if let Some(entry) = self
            .qubit_readout_lines
            .iter_mut()
            .find(|e| e.qubit == qubit)
        {
            entry.line = line;
        } else {
            self.qubit_readout_lines
                .push(QubitReadoutLineEntry { qubit, line });
            self.qubit_readout_lines.sort_by_key(|e| e.qubit);
        }
    }

    /// Returns the readout line mapped to a physical qubit, if any.
    pub fn get_qubit_readout_line(&self, qubit: PhysicalQubitId) -> Option<u32> {
        self.qubit_readout_lines
            .iter()
            .find(|e| e.qubit == qubit)
            .map(|e| e.line)
    }

    /// Resolves the comprehensive resource requirements for executing an operation on physical operands.
    ///
    /// Automatically synthesizes:
    /// 1. Explicit requirements from tuple or default specifications.
    /// 2. Dedicated exclusive [`ResourceId::Qubit`] locks for all operand qubits.
    /// 3. Normalised symmetric [`ResourceId::Coupler`] lock for two-qubit operations.
    /// 4. Readout line resource lock for measurement operations on mapped feedlines.
    pub fn requirements_for(
        &self,
        op: &TargetOpId,
        operands: &[PhysicalQubitId],
    ) -> Vec<ResourceRequirement> {
        let mut reqs = Vec::new();

        // 1. Check tuple requirements first
        if let Some(entry) = self
            .tuple_requirements
            .iter()
            .find(|e| &e.operation == op && e.operands.as_slice() == operands)
        {
            reqs.extend(entry.requirements.clone());
        } else if let Some(default_reqs) = self.default_requirements.get(op) {
            reqs.extend(default_reqs.clone());
        }

        // 2. Ensure each operand qubit has an exclusive Qubit resource requirement
        for &q in operands {
            let q_res = ResourceId::Qubit(q);
            if !reqs.iter().any(|r| r.resource == q_res) {
                reqs.push(ResourceRequirement::exclusive(q_res));
            }
        }

        // 3. For 2-qubit operations, ensure coupler channel requirement is present
        if operands.len() == 2 {
            let coupler_res = ResourceId::coupler(operands[0], operands[1]);
            if !reqs.iter().any(|r| r.resource == coupler_res) {
                reqs.push(ResourceRequirement::exclusive(coupler_res));
            }
        }

        // 4. For measurement, if mapped to a readout line, ensure ReadoutLine requirement is present
        if op.as_str() == "measure" {
            for &q in operands {
                if let Some(line) = self.get_qubit_readout_line(q) {
                    let line_res = ResourceId::ReadoutLine(line);
                    if !reqs.iter().any(|r| r.resource == line_res) {
                        reqs.push(ResourceRequirement::shared(line_res, 1));
                    }
                }
            }
        }

        reqs
    }

    /// Returns `true` if explicit or default resource metadata is declared for this operation.
    pub fn has_metadata_for(&self, op: &TargetOpId, operands: &[PhysicalQubitId]) -> bool {
        self.tuple_requirements
            .iter()
            .any(|e| &e.operation == op && e.operands.as_slice() == operands)
            || self.default_requirements.contains_key(op)
            || (op.as_str() == "measure"
                && operands
                    .iter()
                    .any(|q| self.get_qubit_readout_line(*q).is_some()))
            || self
                .mutual_exclusions
                .iter()
                .any(|rule| match &rule.condition {
                    ExclusionCondition::GlobalExclusive { operation } => operation == op,
                    ExclusionCondition::OperationPair { op1, op2, .. } => op1 == op || op2 == op,
                    ExclusionCondition::ReadoutExclusionGroup { qubits } => {
                        op.as_str() == "measure" && operands.iter().any(|q| qubits.contains(q))
                    }
                })
    }

    /// Verifies whether two physical operations can execute concurrently.
    ///
    /// Checks:
    /// 1. Presence of declared resource metadata (missing metadata prevents assuming concurrency).
    /// 2. Mutual exclusion rules (operation-pair and readout exclusion groups).
    /// 3. Exclusive resource collisions (e.g. shared qubit, coupler, or control line).
    /// 4. Aggregate resource capacity limits (e.g. maximum concurrent readouts per feedline).
    pub fn check_concurrency(
        &self,
        op1: (&TargetOpId, &[PhysicalQubitId]),
        op2: (&TargetOpId, &[PhysicalQubitId]),
    ) -> Result<(), ResourceConflict> {
        let (id1, ops1) = op1;
        let (id2, ops2) = op2;
        let desc1 = format!("{id1} on {ops1:?}");
        let desc2 = format!("{id2} on {ops2:?}");

        // Verify both operations have resource metadata declared
        if !self.has_metadata_for(id1, ops1) {
            return Err(ResourceConflict::MissingOperationResources {
                op: id1.as_str().to_string(),
                operands: ops1.to_vec(),
            });
        }
        if !self.has_metadata_for(id2, ops2) {
            return Err(ResourceConflict::MissingOperationResources {
                op: id2.as_str().to_string(),
                operands: ops2.to_vec(),
            });
        }

        // 1. Evaluate Mutual Exclusion Rules
        for rule in &self.mutual_exclusions {
            match &rule.condition {
                ExclusionCondition::GlobalExclusive { operation } => {
                    if id1 == operation || id2 == operation {
                        return Err(ResourceConflict::MutualExclusion {
                            rule: rule.name.clone(),
                            reason: format!(
                                "operation '{operation}' requires global device exclusivity; cannot run with '{desc1}' and '{desc2}'"
                            ),
                        });
                    }
                }
                ExclusionCondition::OperationPair {
                    op1: rop1,
                    operands1: rops1,
                    op2: rop2,
                    operands2: rops2,
                } => {
                    let matches_forward = id1 == rop1
                        && rops1.as_ref().map_or(true, |o| o.as_slice() == ops1)
                        && id2 == rop2
                        && rops2.as_ref().map_or(true, |o| o.as_slice() == ops2);

                    let matches_reverse = id1 == rop2
                        && rops2.as_ref().map_or(true, |o| o.as_slice() == ops1)
                        && id2 == rop1
                        && rops1.as_ref().map_or(true, |o| o.as_slice() == ops2);

                    if matches_forward || matches_reverse {
                        return Err(ResourceConflict::MutualExclusion {
                            rule: rule.name.clone(),
                            reason: format!(
                                "disallowed simultaneous operation pair '{desc1}' and '{desc2}'"
                            ),
                        });
                    }
                }
                ExclusionCondition::ReadoutExclusionGroup { qubits } => {
                    if id1.as_str() == "measure" && id2.as_str() == "measure" {
                        let in1: Vec<PhysicalQubitId> = ops1
                            .iter()
                            .copied()
                            .filter(|q| qubits.contains(q))
                            .collect();
                        let in2: Vec<PhysicalQubitId> = ops2
                            .iter()
                            .copied()
                            .filter(|q| qubits.contains(q))
                            .collect();
                        if !in1.is_empty() && !in2.is_empty() {
                            return Err(ResourceConflict::ReadoutExclusion {
                                qubit1: in1[0],
                                qubit2: in2[0],
                                reason: format!("qubits belong to exclusion group '{}'", rule.name),
                            });
                        }
                    }
                }
            }
        }

        // 2. Aggregate Resource Usage
        let reqs1 = self.requirements_for(id1, ops1);
        let reqs2 = self.requirements_for(id2, ops2);

        // Check for direct collisions
        for r1 in &reqs1 {
            for r2 in &reqs2 {
                if r1.resource == r2.resource
                    && (r1.mode == ResourceUsageMode::Exclusive
                        || r2.mode == ResourceUsageMode::Exclusive)
                {
                    return Err(ResourceConflict::ExclusiveCollision {
                        resource: r1.resource.clone(),
                        op1: desc1.clone(),
                        op2: desc2.clone(),
                    });
                }
            }
        }

        // Check shared capacities
        let mut aggregate_usage: BTreeMap<ResourceId, usize> = BTreeMap::new();
        for r in reqs1.iter().chain(reqs2.iter()) {
            *aggregate_usage.entry(r.resource.clone()).or_insert(0) += r.amount;
        }

        for (resource, requested) in aggregate_usage {
            let cap = self.capacity_of(&resource);
            if requested > cap {
                return Err(ResourceConflict::CapacityExceeded {
                    resource,
                    requested,
                    capacity: cap,
                });
            }
        }

        Ok(())
    }

    /// Validates the resource model consistency.
    pub fn validate(
        &self,
        target_capacity: usize,
        declared_ops: &BTreeSet<TargetOpId>,
    ) -> Result<(), TargetError> {
        for entry in &self.capacities {
            if entry.capacity == 0 {
                return Err(TargetError::InvalidResourceModel {
                    reason: format!("resource '{}' declared with zero capacity", entry.resource),
                });
            }
            match &entry.resource {
                ResourceId::Qubit(q) => {
                    if q.index() >= target_capacity {
                        return Err(TargetError::QubitOutOfBounds {
                            qubit: *q,
                            target: "ResourceModel".to_string(),
                            capacity: target_capacity,
                        });
                    }
                }
                ResourceId::Coupler(q0, q1) => {
                    if q0.index() >= target_capacity {
                        return Err(TargetError::QubitOutOfBounds {
                            qubit: *q0,
                            target: "ResourceModel".to_string(),
                            capacity: target_capacity,
                        });
                    }
                    if q1.index() >= target_capacity {
                        return Err(TargetError::QubitOutOfBounds {
                            qubit: *q1,
                            target: "ResourceModel".to_string(),
                            capacity: target_capacity,
                        });
                    }
                    if q0 == q1 {
                        return Err(TargetError::SelfLoopEdge {
                            operation: "coupler_resource".to_string(),
                            qubit: *q0,
                        });
                    }
                }
                _ => {}
            }
        }

        for (op_id, reqs) in &self.default_requirements {
            if !declared_ops.contains(op_id) {
                return Err(TargetError::UnknownOperation {
                    operation: op_id.as_str().to_string(),
                    target: "ResourceModel".to_string(),
                });
            }
            for req in reqs {
                if req.amount == 0 {
                    return Err(TargetError::InvalidResourceModel {
                        reason: format!(
                            "operation '{op_id}' has requirement for '{}' with 0 amount",
                            req.resource
                        ),
                    });
                }
            }
        }

        for entry in &self.tuple_requirements {
            if !declared_ops.contains(&entry.operation) {
                return Err(TargetError::UnknownOperation {
                    operation: entry.operation.as_str().to_string(),
                    target: "ResourceModel".to_string(),
                });
            }
            for &q in &entry.operands {
                if q.index() >= target_capacity {
                    return Err(TargetError::QubitOutOfBounds {
                        qubit: q,
                        target: "ResourceModel".to_string(),
                        capacity: target_capacity,
                    });
                }
            }
        }

        for entry in &self.qubit_readout_lines {
            if entry.qubit.index() >= target_capacity {
                return Err(TargetError::QubitOutOfBounds {
                    qubit: entry.qubit,
                    target: "ResourceModel".to_string(),
                    capacity: target_capacity,
                });
            }
        }

        for rule in &self.mutual_exclusions {
            match &rule.condition {
                ExclusionCondition::GlobalExclusive { operation } => {
                    if !declared_ops.contains(operation) {
                        return Err(TargetError::UnknownOperation {
                            operation: operation.as_str().to_string(),
                            target: "ResourceModel".to_string(),
                        });
                    }
                }
                ExclusionCondition::OperationPair { op1, op2, .. } => {
                    if !declared_ops.contains(op1) {
                        return Err(TargetError::UnknownOperation {
                            operation: op1.as_str().to_string(),
                            target: "ResourceModel".to_string(),
                        });
                    }
                    if !declared_ops.contains(op2) {
                        return Err(TargetError::UnknownOperation {
                            operation: op2.as_str().to_string(),
                            target: "ResourceModel".to_string(),
                        });
                    }
                }
                ExclusionCondition::ReadoutExclusionGroup { qubits } => {
                    for &q in qubits {
                        if q.index() >= target_capacity {
                            return Err(TargetError::QubitOutOfBounds {
                                qubit: q,
                                target: "ResourceModel".to_string(),
                                capacity: target_capacity,
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
