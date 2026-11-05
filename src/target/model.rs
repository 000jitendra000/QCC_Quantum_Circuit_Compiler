//! The top-level generic quantum hardware target data model.
//!
//! Provides provider-neutral hardware description capturing physical qubit counts,
//! supported operation descriptors, per-operation directed/undirected connectivity,
//! and optional calibration/timing databases with provenance.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::target::calibration::{Provenance, TargetCalibration};
use crate::target::connectivity::{CouplingGraph, Directionality};
use crate::target::error::TargetError;
use crate::target::operation::{PhysicalQubitId, TargetOpId, TargetOperationDescriptor};
use crate::target::resources::{ResourceConflict, ResourceModel, ResourceRequirement};
use crate::target::synthesis_basis::SingleQubitBasis;
use crate::target::timing::{OperationDuration, TimingModel};

/// Authoritative provider-neutral description of a quantum hardware device.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Target {
    /// Human-readable device or target identifier (e.g. `"ibm_eagle_synthetic"`, `"rigetti_aspen_synthetic"`).
    pub name: String,
    /// Total count of physical hardware qubits ($n \ge 1$).
    pub num_qubits: usize,
    /// Map of target-native operations supported by the device.
    pub operations: BTreeMap<TargetOpId, TargetOperationDescriptor>,
    /// Optional calibration and characterization metrics database.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration: Option<TargetCalibration>,
    /// Optional provenance tracking the target definition itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    /// Optional detailed hardware timing model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<TimingModel>,
    /// Optional hardware resource and concurrency model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceModel>,
}

impl Target {
    /// Creates a new target with the given name and physical qubit capacity.
    pub fn new(name: impl Into<String>, num_qubits: usize) -> Self {
        Self {
            name: name.into(),
            num_qubits,
            operations: BTreeMap::new(),
            calibration: None,
            provenance: None,
            timing: None,
            resources: None,
        }
    }

    /// Returns a new fluent builder for constructing a [`Target`].
    pub fn builder(name: impl Into<String>, num_qubits: usize) -> TargetBuilder {
        TargetBuilder::new(name, num_qubits)
    }

    /// Returns the target's name.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the total physical qubit capacity of the target.
    #[inline]
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Returns a reference to the operations map.
    #[inline]
    pub fn operations(&self) -> &BTreeMap<TargetOpId, TargetOperationDescriptor> {
        &self.operations
    }

    /// Adds a supported operation descriptor to the target.
    pub fn add_operation(&mut self, descriptor: TargetOperationDescriptor) {
        self.operations.insert(descriptor.name.clone(), descriptor);
    }

    /// Retrieves an operation descriptor by its string mnemonic.
    pub fn operation(&self, name: &str) -> Option<&TargetOperationDescriptor> {
        self.operations.get(&TargetOpId::new(name))
    }

    /// Returns `true` if this target supports an operation with the given mnemonic.
    pub fn is_operation_supported(&self, name: &str) -> bool {
        self.operations.contains_key(&TargetOpId::new(name))
    }

    /// Checks whether an operation is supported on the given physical operand tuple.
    pub fn is_valid_operand_tuple(&self, op_name: &str, operands: &[PhysicalQubitId]) -> bool {
        self.operation(op_name)
            .map(|op| op.is_valid_operands(operands))
            .unwrap_or(false)
    }

    /// Resolves the operation duration as an explicit [`OperationDuration`].
    ///
    /// Checks:
    /// 1. Detailed timing model (`TimingModel`).
    /// 2. Tuple-specific calibration database (`TargetCalibration`).
    /// 3. Nominal operation descriptor default duration.
    ///
    /// Following the Explicit Unknown Principle, returns [`OperationDuration::Unknown`] if uncalibrated,
    /// **never zero**.
    pub fn operation_duration(
        &self,
        op_name: &str,
        operands: &[PhysicalQubitId],
    ) -> OperationDuration {
        let op_id = TargetOpId::new(op_name);
        if let Some(timing) = &self.timing {
            let dur = timing.duration_of(&op_id, operands);
            if !dur.is_unknown() {
                return dur;
            }
        }
        if let Some(cal) = &self.calibration {
            if let Some(dur) = cal.tuple_duration(&op_id, operands) {
                return OperationDuration::known_ns(dur);
            }
        }
        if let Some(op) = self.operation(op_name) {
            if let Some(dur) = op.duration {
                return OperationDuration::estimated_ns(dur);
            }
        }
        OperationDuration::Unknown
    }

    /// Returns the execution duration in nanoseconds for an operation on a physical operand tuple.
    ///
    /// Following the Explicit Unknown Principle, returns `None` if unknown (never zero).
    pub fn tuple_duration(&self, op_name: &str, operands: &[PhysicalQubitId]) -> Option<f64> {
        let dur = self.operation_duration(op_name, operands);
        let clock_period = self.timing.as_ref().and_then(|t| t.clock_period_ns);
        dur.to_nanoseconds(clock_period).ok().flatten()
    }

    /// Resolves the hardware execution resource requirements for an operation on physical operands.
    ///
    /// Following the Explicit Unknown Principle, returns `None` if the target does not define
    /// a resource model.
    pub fn operation_resources(
        &self,
        op_name: &str,
        operands: &[PhysicalQubitId],
    ) -> Option<Vec<ResourceRequirement>> {
        let op_id = TargetOpId::new(op_name);
        self.resources
            .as_ref()
            .map(|res| res.requirements_for(&op_id, operands))
    }

    /// Checks whether two operations on physical operands can execute concurrently.
    ///
    /// # Errors
    /// - Returns [`ResourceConflict::MissingResourceModel`] if the target has no resource model
    ///   (missing resource metadata is never silently interpreted as unrestricted concurrency).
    /// - Returns [`ResourceConflict`] if concurrency violates mutual exclusions, collisions, or capacity.
    pub fn check_concurrency(
        &self,
        op1: (&str, &[PhysicalQubitId]),
        op2: (&str, &[PhysicalQubitId]),
    ) -> Result<(), ResourceConflict> {
        let Some(res) = &self.resources else {
            return Err(ResourceConflict::MissingResourceModel {
                target: self.name.clone(),
            });
        };
        let id1 = TargetOpId::new(op1.0);
        let id2 = TargetOpId::new(op2.0);
        res.check_concurrency((&id1, op1.1), (&id2, op2.1))
    }

    /// Returns the error estimate (infidelity) for an operation on a physical operand tuple.
    ///
    /// Checks tuple-specific calibration first, falling back to operation-level nominal error.
    /// Returns `None` if unknown (never zero).
    pub fn tuple_error(&self, op_name: &str, operands: &[PhysicalQubitId]) -> Option<f64> {
        let op_id = TargetOpId::new(op_name);
        if let Some(cal) = &self.calibration {
            if let Some(err) = cal.tuple_error(&op_id, operands) {
                return Some(err);
            }
        }
        self.operation(op_name).and_then(|op| op.error_estimate)
    }

    /// Returns all physical two-qubit coupling edges permitted for a specific operation.
    pub fn operation_edges(&self, op_name: &str) -> Vec<(PhysicalQubitId, PhysicalQubitId)> {
        let Some(op) = self.operation(op_name) else {
            return Vec::new();
        };
        let tuples = op.connectivity.allowed_tuples(self.num_qubits, 2);
        tuples
            .into_iter()
            .filter_map(|t| {
                if t.len() == 2 {
                    Some((t[0], t[1]))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Computes the union coupling graph of all two-qubit operations supported by the target.
    pub fn combined_coupling_graph(&self) -> CouplingGraph {
        let mut graph = CouplingGraph::new(self.num_qubits, Directionality::Directed);
        for op in self.operations.values() {
            if op.is_two_qubit() {
                let edges = op.connectivity.allowed_tuples(self.num_qubits, 2);
                for edge in edges {
                    if edge.len() == 2 {
                        let _ = graph.add_edge(edge[0], edge[1]);
                    }
                }
            }
        }
        graph
    }

    /// Thoroughly validates the target model against all invariants.
    pub fn validate(&self) -> Result<(), TargetError> {
        if self.num_qubits == 0 {
            return Err(TargetError::ZeroQubits);
        }

        if self.operations.is_empty() {
            return Err(TargetError::NoOperations(self.name.clone()));
        }

        let mut declared_ops = BTreeSet::new();
        for (op_id, desc) in &self.operations {
            declared_ops.insert(op_id.clone());
            desc.validate(self.num_qubits)?;
        }

        if let Some(cal) = &self.calibration {
            cal.validate(&self.name, self.num_qubits, &declared_ops)?;
        }

        if let Some(timing) = &self.timing {
            timing.validate(self.num_qubits, &declared_ops)?;
        }

        if let Some(resources) = &self.resources {
            resources.validate(self.num_qubits, &declared_ops)?;
        }

        Ok(())
    }

    /// Serializes this target description into deterministic, compact JSON.
    pub fn to_json(&self) -> Result<String, TargetError> {
        serde_json::to_string(self).map_err(|e| TargetError::Serialization(e.to_string()))
    }

    /// Serializes this target description into deterministic, formatted JSON.
    pub fn to_json_pretty(&self) -> Result<String, TargetError> {
        serde_json::to_string_pretty(self).map_err(|e| TargetError::Serialization(e.to_string()))
    }

    /// Deserializes a target description from JSON and validates all invariants.
    pub fn from_json(json_str: &str) -> Result<Self, TargetError> {
        let target: Self = serde_json::from_str(json_str)
            .map_err(|e| TargetError::Deserialization(e.to_string()))?;
        target.validate()?;
        Ok(target)
    }

    /// Returns all [`SingleQubitBasis`] families supported by this target on physical `qubit`.
    pub fn supported_single_qubit_bases(&self, qubit: PhysicalQubitId) -> Vec<SingleQubitBasis> {
        let all = [
            SingleQubitBasis::RzSx,
            SingleQubitBasis::Zxz,
            SingleQubitBasis::Zyz,
            SingleQubitBasis::U3,
        ];
        all.into_iter()
            .filter(|b| b.is_supported_by(self, qubit))
            .collect()
    }

    /// Selects the preferred single-qubit synthesis basis for physical `qubit`.
    ///
    /// Priority order:
    /// 1. `RzSx` (target-native virtual/microwave transmon family)
    /// 2. `Zxz` (native rotation family)
    /// 3. `Zyz` (generic rotation family)
    /// 4. `U3` (single-gate universal family)
    pub fn preferred_single_qubit_basis(&self, qubit: PhysicalQubitId) -> Option<SingleQubitBasis> {
        self.supported_single_qubit_bases(qubit).into_iter().next()
    }
}

/// Fluent builder for constructing and validating a [`Target`].
#[derive(Clone, Debug)]
pub struct TargetBuilder {
    name: String,
    num_qubits: usize,
    operations: BTreeMap<TargetOpId, TargetOperationDescriptor>,
    calibration: Option<TargetCalibration>,
    provenance: Option<Provenance>,
    timing: Option<TimingModel>,
    resources: Option<ResourceModel>,
}

impl TargetBuilder {
    /// Creates a new builder for a target with the given name and physical qubit capacity.
    pub fn new(name: impl Into<String>, num_qubits: usize) -> Self {
        Self {
            name: name.into(),
            num_qubits,
            operations: BTreeMap::new(),
            calibration: None,
            provenance: None,
            timing: None,
            resources: None,
        }
    }

    /// Adds a target-native operation descriptor.
    pub fn add_operation(mut self, descriptor: TargetOperationDescriptor) -> Self {
        self.operations.insert(descriptor.name.clone(), descriptor);
        self
    }

    /// Sets the target calibration database.
    pub fn calibration(mut self, calibration: TargetCalibration) -> Self {
        self.calibration = Some(calibration);
        self
    }

    /// Sets target-level provenance metadata.
    pub fn provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    /// Sets the hardware timing model.
    pub fn timing(mut self, timing: TimingModel) -> Self {
        self.timing = Some(timing);
        self
    }

    /// Sets the hardware resource and concurrency model.
    pub fn resources(mut self, resources: ResourceModel) -> Self {
        self.resources = Some(resources);
        self
    }

    /// Builds and validates the [`Target`].
    pub fn build(self) -> Result<Target, TargetError> {
        let target = Target {
            name: self.name,
            num_qubits: self.num_qubits,
            operations: self.operations,
            calibration: self.calibration,
            provenance: self.provenance,
            timing: self.timing,
            resources: self.resources,
        };
        target.validate()?;
        Ok(target)
    }
}
