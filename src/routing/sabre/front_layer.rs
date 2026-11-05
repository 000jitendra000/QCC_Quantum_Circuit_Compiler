//! SABRE Front-Layer extraction and physical operand resolution.
//!
//! The Front Layer ($F$) contains all two-qubit operations that are currently dependency-ready
//! in the QccIR DAG. Logical operands are resolved to physical device locations dynamically
//! through the active layout.
//!
//! # Critical Invariants
//! - **Two-Qubit Only**: The front layer consists strictly of two-qubit entangling gates.
//!   Single-qubit operations are executed immediately upon readiness and do not block the front layer.
//! - **Dynamic Layout Resolution**: Physical operands are mapped via [`LayoutMap`] and
//!   must be updated whenever candidate layout permutations are explored to prevent stale mappings.
//! - **Deterministic Ordering**: Operations in the front layer are deterministically sorted
//!   by stable [`OperationId`].

use std::collections::BTreeSet;

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::wire::QubitId;
use crate::routing::layout_map::LayoutMap;
use crate::routing::pathfinding::RoutingError;
use crate::routing::sabre::frontier::SabreFrontier;
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;

/// A two-qubit operation residing in the SABRE front layer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrontLayerOp {
    /// Stable semantic identifier in the QccIR DAG.
    pub id: OperationId,
    /// First logical program qubit operand.
    pub logical_q1: QubitId,
    /// Second logical program qubit operand.
    pub logical_q2: QubitId,
    /// First physical hardware qubit operand mapped through current layout.
    pub physical_p1: PhysicalQubitId,
    /// Second physical hardware qubit operand mapped through current layout.
    pub physical_p2: PhysicalQubitId,
    /// Operation name (e.g. `"cx"`).
    pub name: String,
}

impl FrontLayerOp {
    /// Returns the logical operand pair `(q1, q2)`.
    #[inline]
    pub fn logical_operands(&self) -> (QubitId, QubitId) {
        (self.logical_q1, self.logical_q2)
    }

    /// Returns the physical operand pair `(p1, p2)`.
    #[inline]
    pub fn physical_operands(&self) -> (PhysicalQubitId, PhysicalQubitId) {
        (self.physical_p1, self.physical_p2)
    }

    /// Synchronously updates physical operands using the specified layout map.
    pub fn update_layout(&mut self, layout: &LayoutMap) -> Result<(), RoutingError> {
        self.physical_p1 = layout.get_physical(self.logical_q1).ok_or_else(|| {
            RoutingError::Layout(format!("qubit {} not assigned", self.logical_q1))
        })?;
        self.physical_p2 = layout.get_physical(self.logical_q2).ok_or_else(|| {
            RoutingError::Layout(format!("qubit {} not assigned", self.logical_q2))
        })?;
        Ok(())
    }

    /// Returns `true` if this operation can be executed immediately on `target` without SWAPs.
    #[inline]
    pub fn is_executable_on_target(&self, target: &Target) -> bool {
        target.is_valid_operand_tuple(&self.name, &[self.physical_p1, self.physical_p2])
    }
}

/// The set of two-qubit operations currently executable in the dependency DAG.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrontLayer {
    /// Deterministically ordered front-layer operations.
    operations: Vec<FrontLayerOp>,
}

impl FrontLayer {
    /// Constructs the front layer from the current dependency frontier and active layout.
    pub fn from_frontier(
        frontier: &SabreFrontier,
        circuit: &Circuit,
        layout: &LayoutMap,
    ) -> Result<Self, RoutingError> {
        let op_ids = frontier.executable_two_qubit_operations(circuit);
        let mut operations = Vec::with_capacity(op_ids.len());

        for id in op_ids {
            let op = circuit
                .graph
                .get_operation(id)
                .ok_or_else(|| RoutingError::Ir(format!("operation {id} not found in graph")))?;

            let qubits = match op {
                crate::ir::operation::Operation::Gate { qubits, .. } if qubits.len() == 2 => qubits,
                _ => continue,
            };

            let q1 = qubits[0];
            let q2 = qubits[1];

            let p1 = layout
                .get_physical(q1)
                .ok_or_else(|| RoutingError::Layout(format!("qubit {q1} not assigned")))?;
            let p2 = layout
                .get_physical(q2)
                .ok_or_else(|| RoutingError::Layout(format!("qubit {q2} not assigned")))?;

            operations.push(FrontLayerOp {
                id,
                logical_q1: q1,
                logical_q2: q2,
                physical_p1: p1,
                physical_p2: p2,
                name: op.name().to_string(),
            });
        }

        operations.sort_unstable_by_key(|op| op.id);
        Ok(Self { operations })
    }

    /// Re-evaluates all physical operand mappings under an updated layout.
    ///
    /// Guarantees that stale physical locations do not survive candidate layout changes.
    /// Updates are applied transactionally: if any operation fails to map, the front layer
    /// remains unchanged.
    pub fn update_layout(&mut self, layout: &LayoutMap) -> Result<(), RoutingError> {
        let mut updated = self.operations.clone();
        for op in &mut updated {
            op.update_layout(layout)?;
        }
        self.operations = updated;
        Ok(())
    }

    /// Returns a slice of the front-layer operations.
    #[inline]
    pub fn operations(&self) -> &[FrontLayerOp] {
        &self.operations
    }

    /// Returns `true` if the front layer contains no operations.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Returns the number of operations in the front layer.
    #[inline]
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Returns `true` if the front layer contains operation `id`.
    #[inline]
    pub fn contains(&self, id: OperationId) -> bool {
        self.operations.iter().any(|op| op.id == id)
    }

    /// Returns a reference to the front layer operation with ID `id`, if present.
    #[inline]
    pub fn get(&self, id: OperationId) -> Option<&FrontLayerOp> {
        self.operations.iter().find(|op| op.id == id)
    }

    /// Returns the set of all logical qubits actively involved in front-layer operations.
    pub fn logical_qubits(&self) -> BTreeSet<QubitId> {
        let mut set = BTreeSet::new();
        for op in &self.operations {
            set.insert(op.logical_q1);
            set.insert(op.logical_q2);
        }
        set
    }

    /// Returns the set of all physical qubits actively involved in front-layer operations.
    pub fn physical_qubits(&self) -> BTreeSet<PhysicalQubitId> {
        let mut set = BTreeSet::new();
        for op in &self.operations {
            set.insert(op.physical_p1);
            set.insert(op.physical_p2);
        }
        set
    }

    /// Returns all front-layer operations that are currently directly executable on `target`.
    pub fn executable_on_target<'a>(&'a self, target: &Target) -> Vec<&'a FrontLayerOp> {
        self.operations
            .iter()
            .filter(|op| op.is_executable_on_target(target))
            .collect()
    }
}
