//! Dependency frontier tracking for SABRE circuit traversal.
//!
//! Tracks completed operations and remaining predecessor in-degrees along directed
//! dependency edges in the QccIR DAG, isolating currently executable operations.
//!
//! # Critical Invariants
//! - **Actual Dependency In-Degrees**: Readiness is determined strictly from predecessor
//!   dependency edges in QccIR, never merely from topological array indices.
//! - **Deterministic Traversal**: All ready sets are maintained in [`BTreeSet`] ordered
//!   by stable [`OperationId`] to guarantee reproducible execution orders.

use std::collections::{BTreeSet, HashMap};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::routing::pathfinding::RoutingError;

/// Dependency frontier tracking operation execution and readiness in a circuit DAG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SabreFrontier {
    /// Set of operations that have already been executed / satisfied.
    completed: BTreeSet<OperationId>,
    /// Remaining unsatisfied predecessor dependencies for each operation in the circuit.
    in_degrees: HashMap<OperationId, usize>,
    /// Operations whose predecessor dependencies are currently 100% satisfied (`in_degree == 0`)
    /// and have not yet been marked completed.
    executable: BTreeSet<OperationId>,
    /// Total number of operations in the underlying circuit.
    total_operations: usize,
}

impl SabreFrontier {
    /// Initializes a new dependency frontier from a logical [`Circuit`].
    ///
    /// Computes initial predecessor dependency in-degrees for every operation in the DAG.
    /// Operations with zero incoming dependencies are placed into the executable set.
    pub fn new(circuit: &Circuit) -> Result<Self, RoutingError> {
        let order = circuit
            .graph
            .deterministic_topological_order()
            .map_err(|e| RoutingError::Ir(e.to_string()))?;

        let mut in_degrees = HashMap::with_capacity(order.len());
        let mut executable = BTreeSet::new();

        for &id in &order {
            let preds = circuit.graph.predecessors(id);
            let deg = preds.len();
            in_degrees.insert(id, deg);
            if deg == 0 {
                executable.insert(id);
            }
        }

        Ok(Self {
            completed: BTreeSet::new(),
            in_degrees,
            executable,
            total_operations: order.len(),
        })
    }

    /// Returns `true` if operation `id` has already been executed.
    #[inline]
    pub fn is_completed(&self, id: OperationId) -> bool {
        self.completed.contains(&id)
    }

    /// Returns `true` if operation `id` is currently dependency-ready for execution.
    #[inline]
    pub fn is_executable(&self, id: OperationId) -> bool {
        self.executable.contains(&id)
    }

    /// Returns the remaining unsatisfied predecessor dependency count for `id`, if tracked.
    #[inline]
    pub fn remaining_in_degree(&self, id: OperationId) -> Option<usize> {
        self.in_degrees.get(&id).copied()
    }

    /// Returns an iterator over all currently executable operations in deterministic order.
    #[inline]
    pub fn executable_operations(&self) -> impl Iterator<Item = &OperationId> {
        self.executable.iter()
    }

    /// Returns all currently executable single-qubit operations in deterministic order.
    pub fn executable_single_qubit_operations(&self, circuit: &Circuit) -> Vec<OperationId> {
        self.executable
            .iter()
            .copied()
            .filter(|&id| {
                circuit
                    .graph
                    .get_operation(id)
                    .map(|op| {
                        !matches!(
                            op,
                            crate::ir::operation::Operation::Gate { qubits, .. } if qubits.len() == 2
                        )
                    })
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Returns all currently executable two-qubit operations in deterministic order.
    pub fn executable_two_qubit_operations(&self, circuit: &Circuit) -> Vec<OperationId> {
        self.executable
            .iter()
            .copied()
            .filter(|&id| {
                circuit
                    .graph
                    .get_operation(id)
                    .map(|op| {
                        matches!(
                            op,
                            crate::ir::operation::Operation::Gate { qubits, .. } if qubits.len() == 2
                        )
                    })
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Returns the number of operations remaining to be executed.
    #[inline]
    pub fn remaining_operations_count(&self) -> usize {
        self.total_operations.saturating_sub(self.completed.len())
    }

    /// Returns `true` if every operation in the circuit has completed.
    #[inline]
    pub fn is_fully_completed(&self) -> bool {
        self.completed.len() >= self.total_operations
    }

    /// Returns the number of completed operations.
    #[inline]
    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    /// Advances the frontier by marking `id` as executed.
    ///
    /// Decrements the unsatisfied in-degree of all direct successors of `id` in the DAG.
    /// Successors whose in-degree drops to zero enter the executable set.
    ///
    /// # Errors
    /// Returns [`RoutingError::Ir`] if `id` is not currently in the executable set or not in the DAG.
    pub fn advance(
        &mut self,
        circuit: &Circuit,
        id: OperationId,
    ) -> Result<Vec<OperationId>, RoutingError> {
        if !self.executable.remove(&id) {
            return Err(RoutingError::Ir(format!(
                "cannot advance frontier with non-executable operation {id}"
            )));
        }

        self.completed.insert(id);

        let mut newly_executable = Vec::new();
        for (succ_id, _) in circuit.graph.successors(id) {
            if let Some(deg) = self.in_degrees.get_mut(&succ_id) {
                if *deg > 0 {
                    *deg -= 1;
                    if *deg == 0 {
                        self.executable.insert(succ_id);
                        newly_executable.push(succ_id);
                    }
                }
            }
        }

        newly_executable.sort_unstable();
        newly_executable.dedup();
        Ok(newly_executable)
    }

    /// Advances the frontier by marking `id` as executed (alias for [`advance`]).
    #[inline]
    pub fn mark_executed(
        &mut self,
        circuit: &Circuit,
        id: OperationId,
    ) -> Result<Vec<OperationId>, RoutingError> {
        self.advance(circuit, id)
    }

    /// Advances the frontier through multiple operations sequentially.
    pub fn advance_many<'a>(
        &mut self,
        circuit: &Circuit,
        ids: impl IntoIterator<Item = &'a OperationId>,
    ) -> Result<Vec<OperationId>, RoutingError> {
        let mut all_new = Vec::new();
        for &id in ids {
            let new_ready = self.advance(circuit, id)?;
            all_new.extend(new_ready);
        }
        all_new.sort_unstable();
        all_new.dedup();
        Ok(all_new)
    }

    /// Creates an isolated lightweight clone of the frontier state for lookahead simulation.
    pub fn clone_for_lookahead(&self) -> Self {
        self.clone()
    }
}
