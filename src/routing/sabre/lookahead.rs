//! Extended Lookahead Set construction for SABRE routing.
//!
//! Evaluates upcoming two-qubit operations beyond the current front layer by simulating
//! forward DAG progression within a bounded lookahead budget/window $W$.
//!
//! # Critical Invariants
//! - **DAG-Induced Readiness**: Upcoming operations in the extended set must be actual
//!   DAG descendants of the front layer, resolved along dependency edges.
//! - **Single-Qubit Gate Transparent Advancement**: Intervening single-qubit operations
//!   have zero coupling constraints and are transparently satisfied in lookahead simulation.
//! - **Deterministic Bounded Window**: Lookahead collection terminates when $|E| \ge W$
//!   or the DAG is exhausted, preserving deterministic tie-breaking by [`OperationId`].
//! - **Dynamic Layout Resolution**: Physical operands are mapped via [`LayoutMap`] and
//!   synchronized whenever candidate layout mutations are evaluated.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::wire::QubitId;
use crate::routing::layout_map::LayoutMap;
use crate::routing::pathfinding::RoutingError;
use crate::routing::sabre::frontier::SabreFrontier;
use crate::target::operation::PhysicalQubitId;

/// Configuration parameters controlling extended lookahead set generation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LookaheadConfig {
    /// Maximum number of two-qubit operations to include in the extended lookahead set ($W$).
    ///
    /// Standard SABRE literature typically uses $W \in [10, 30]$.
    /// Default: 20.
    pub max_size: usize,

    /// Optional maximum layer depth beyond the front layer to explore.
    ///
    /// If `None`, lookahead depth is unbounded and exploration terminates only when
    /// `max_size` is reached or the circuit DAG is exhausted.
    /// Default: `None`.
    pub max_depth: Option<usize>,
}

impl Default for LookaheadConfig {
    fn default() -> Self {
        Self {
            max_size: 20,
            max_depth: None,
        }
    }
}

impl LookaheadConfig {
    /// Creates a new lookahead configuration with the given maximum window size $W$.
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            max_size,
            max_depth: None,
        }
    }

    /// Constrains lookahead exploration to at most `max_depth` layers beyond the front layer.
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = Some(max_depth);
        self
    }
}

/// A two-qubit operation residing in the extended lookahead set.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LookaheadOp {
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
    /// Lookahead layer depth (1 = direct successor layer of front layer).
    pub layer: usize,
}

impl LookaheadOp {
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
}

/// The extended lookahead set ($E$) containing upcoming two-qubit operations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LookaheadSet {
    /// Ordered list of lookahead operations.
    operations: Vec<LookaheadOp>,
}

impl LookaheadSet {
    /// Builds the extended lookahead set by simulating forward DAG progression from the current frontier.
    ///
    /// # Algorithm
    /// 1. Clones the current dependency frontier to avoid mutating active compiler state.
    /// 2. Transparently retires all currently ready single-qubit operations.
    /// 3. Retires the front-layer two-qubit operations to simulate their hypothetical execution.
    /// 4. As new operations become ready in simulation:
    ///    - Single-qubit operations are immediately retired to uncover further dependencies.
    ///    - Two-qubit operations enter lookahead layer $k$.
    /// 5. Exploration proceeds layer-by-layer until `max_size` is satisfied or the DAG is exhausted.
    pub fn build(
        frontier: &SabreFrontier,
        circuit: &Circuit,
        layout: &LayoutMap,
        config: &LookaheadConfig,
    ) -> Result<Self, RoutingError> {
        if config.max_size == 0 {
            return Ok(Self {
                operations: Vec::new(),
            });
        }

        let mut sim_frontier = frontier.clone_for_lookahead();

        // 1. Transparently retire all currently ready single-qubit operations
        loop {
            let ready_1q = sim_frontier.executable_single_qubit_operations(circuit);
            if ready_1q.is_empty() {
                break;
            }
            for op_id in ready_1q {
                sim_frontier.advance(circuit, op_id)?;
            }
        }

        // 2. Front layer is the currently ready two-qubit operations
        let front_layer_2q = sim_frontier.executable_two_qubit_operations(circuit);
        if front_layer_2q.is_empty() {
            return Ok(Self {
                operations: Vec::new(),
            });
        }

        let mut lookahead_ops: Vec<LookaheadOp> = Vec::new();

        // Queue holds batches of two-qubit operations to simulate, along with their layer index
        let mut layer_queue: VecDeque<(Vec<OperationId>, usize)> = VecDeque::new();

        // Layer 0 is the front layer itself; advancing it produces lookahead Layer 1
        layer_queue.push_back((front_layer_2q, 0));

        while let Some((batch, current_layer)) = layer_queue.pop_front() {
            let next_layer = current_layer + 1;
            if let Some(max_d) = config.max_depth {
                if next_layer > max_d {
                    break;
                }
            }

            let mut unblocked_in_this_layer: Vec<OperationId> = Vec::new();

            // Simulate execution of operations in batch
            for op_id in batch {
                let newly_ready = sim_frontier.advance(circuit, op_id)?;
                unblocked_in_this_layer.extend(newly_ready);
            }

            // Unblock and advance intervening single-qubit operations recursively
            let mut newly_ready_2q = Vec::new();
            let mut pending_scan = unblocked_in_this_layer;

            while !pending_scan.is_empty() {
                let mut next_pending = Vec::new();
                for id in pending_scan {
                    if let Some(op) = circuit.graph.get_operation(id) {
                        if matches!(
                            op,
                            crate::ir::operation::Operation::Gate { qubits, .. } if qubits.len() == 2
                        ) {
                            newly_ready_2q.push(id);
                        } else {
                            // Non-2Q gate: advance immediately in simulation
                            let unblocked = sim_frontier.advance(circuit, id)?;
                            next_pending.extend(unblocked);
                        }
                    }
                }
                pending_scan = next_pending;
            }

            newly_ready_2q.sort_unstable();
            newly_ready_2q.dedup();

            // Register newly unblocked 2Q operations into lookahead set
            let mut layer_batch_to_enqueue = Vec::new();
            for id in newly_ready_2q {
                if lookahead_ops.len() >= config.max_size {
                    break;
                }

                if let Some(op) = circuit.graph.get_operation(id) {
                    if let crate::ir::operation::Operation::Gate { qubits, .. } = op {
                        if qubits.len() == 2 {
                            let q1 = qubits[0];
                            let q2 = qubits[1];
                            let p1 = layout.get_physical(q1).ok_or_else(|| {
                                RoutingError::Layout(format!("qubit {q1} not assigned"))
                            })?;
                            let p2 = layout.get_physical(q2).ok_or_else(|| {
                                RoutingError::Layout(format!("qubit {q2} not assigned"))
                            })?;

                            lookahead_ops.push(LookaheadOp {
                                id,
                                logical_q1: q1,
                                logical_q2: q2,
                                physical_p1: p1,
                                physical_p2: p2,
                                name: op.name().to_string(),
                                layer: next_layer,
                            });
                            layer_batch_to_enqueue.push(id);
                        }
                    }
                }
            }

            if lookahead_ops.len() >= config.max_size {
                break;
            }

            if !layer_batch_to_enqueue.is_empty() {
                layer_queue.push_back((layer_batch_to_enqueue, next_layer));
            }
        }

        Ok(Self {
            operations: lookahead_ops,
        })
    }

    /// Re-evaluates all physical operand mappings under an updated layout.
    ///
    /// Guarantees that stale physical locations do not survive candidate layout changes.
    /// Updates are applied transactionally: if any operation fails to map, the lookahead set
    /// remains unchanged.
    pub fn update_layout(&mut self, layout: &LayoutMap) -> Result<(), RoutingError> {
        let mut updated = self.operations.clone();
        for op in &mut updated {
            op.update_layout(layout)?;
        }
        self.operations = updated;
        Ok(())
    }

    /// Returns a slice of operations in the extended lookahead set.
    #[inline]
    pub fn operations(&self) -> &[LookaheadOp] {
        &self.operations
    }

    /// Returns `true` if the lookahead set is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Returns the number of operations in the lookahead set.
    #[inline]
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Returns `true` if the lookahead set contains operation `id`.
    #[inline]
    pub fn contains(&self, id: OperationId) -> bool {
        self.operations.iter().any(|op| op.id == id)
    }

    /// Returns a reference to the lookahead operation with ID `id`, if present.
    #[inline]
    pub fn get(&self, id: OperationId) -> Option<&LookaheadOp> {
        self.operations.iter().find(|op| op.id == id)
    }

    /// Returns all lookahead operations belonging to a specific lookahead layer.
    pub fn operations_in_layer(&self, layer: usize) -> Vec<&LookaheadOp> {
        self.operations
            .iter()
            .filter(|op| op.layer == layer)
            .collect()
    }

    /// Returns the maximum lookahead layer index present in the set.
    pub fn max_layer(&self) -> usize {
        self.operations.iter().map(|op| op.layer).max().unwrap_or(0)
    }
}
