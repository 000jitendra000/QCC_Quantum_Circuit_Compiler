//! QccIR basic-block dependency DAG encapsulating petgraph internally.
//!
//! # Critical Invariants
//! - **No Exposed Graph Indices**: `petgraph::graph::NodeIndex` and `EdgeIndex` are strictly
//!   private. All external identifiers use QCC-owned stable [`OperationId`].
//! - **Per-Wire Data Hazard Modeling**: Dependency edges are induced using [`WireAccessState`]
//!   to correctly model Read-After-Write (RAW), Write-After-Read (WAR), and Write-After-Write (WAW).
//! - **Deterministic Topological Traversal**: Graph traversals use Kahn's algorithm with
//!   a deterministic priority queue ordered by [`OperationId`] / sequence number.

use std::collections::{BTreeSet, HashMap};

use petgraph::algo::is_cyclic_directed;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableDiGraph;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use petgraph::Direction;

use crate::ir::edge::{DependencyKind, WireDependency};
use crate::ir::error::IrError;
use crate::ir::node::{NodeData, OperationId};
use crate::ir::operation::Operation;
use crate::ir::wire::{WireAccessState, WireId};

/// Canonical QccIR basic-block dependency directed acyclic graph.
#[derive(Clone, Debug)]
pub struct CircuitGraph {
    /// Internal petgraph stable directed graph.
    graph: StableDiGraph<NodeData, WireDependency>,
    /// Map from stable semantic ID to internal petgraph node index.
    node_map: HashMap<OperationId, NodeIndex>,
    /// Map from internal node index to stable semantic ID.
    id_map: HashMap<NodeIndex, OperationId>,
    /// Per-wire access state for quantum and classical wires.
    access_states: HashMap<WireId, WireAccessState>,
    /// Next available operation ID number.
    next_id: u64,
    /// Next available sequence number.
    next_sequence: u64,
}

impl Default for CircuitGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitGraph {
    /// Creates a new empty `CircuitGraph`.
    pub fn new() -> Self {
        Self {
            graph: StableDiGraph::new(),
            node_map: HashMap::new(),
            id_map: HashMap::new(),
            access_states: HashMap::new(),
            next_id: 1,
            next_sequence: 1,
        }
    }

    /// Number of operations (nodes) in the graph.
    #[inline]
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Number of dependency edges in the graph.
    #[inline]
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Returns `true` if the graph contains no operations.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.graph.node_count() == 0
    }

    /// Returns `true` if operation with the given ID exists in the graph.
    #[inline]
    pub fn contains_operation(&self, id: OperationId) -> bool {
        self.node_map.contains_key(&id)
    }

    /// Returns node data for the operation, if found.
    pub fn get_node(&self, id: OperationId) -> Option<&NodeData> {
        self.node_map
            .get(&id)
            .and_then(|&idx| self.graph.node_weight(idx))
    }

    /// Returns a reference to the operation payload, if found.
    pub fn get_operation(&self, id: OperationId) -> Option<&Operation> {
        self.get_node(id).map(|node| &node.operation)
    }

    /// Adds an operation to the graph, automatically inducing typed dependency edges
    /// on affected quantum and classical wires using [`WireAccessState`].
    pub fn add_operation(&mut self, operation: Operation) -> Result<OperationId, IrError> {
        let op_id = OperationId(self.next_id);
        self.next_id += 1;

        let seq = self.next_sequence;
        self.next_sequence += 1;

        let effects = operation.wire_effects();

        // Reject duplicate qubits in quantum operations to prevent self-loops
        let mut distinct = std::collections::HashSet::new();
        for &q in &effects.quantum_writes {
            if !distinct.insert(q) {
                return Err(IrError::Validation(
                    crate::ir::error::IrValidationError::WireDependencyInconsistent {
                        reason: format!(
                            "operation '{}' references duplicate qubit {}",
                            operation.name(),
                            q
                        ),
                    },
                ));
            }
        }

        let node_data = NodeData::new(op_id, seq, operation);
        let node_idx = self.graph.add_node(node_data);

        self.node_map.insert(op_id, node_idx);
        self.id_map.insert(node_idx, op_id);

        // 1. Quantum wire dependencies
        for &qubit in &effects.quantum_writes {
            let wire = WireId::Quantum(qubit);
            let state = self.access_states.entry(wire).or_default();
            let (prev_writer, prev_readers) = state.record_quantum_update(op_id);

            if let Some(prev) = prev_writer {
                let is_prev_barrier = self
                    .get_operation(prev)
                    .is_some_and(|op| matches!(op, Operation::Barrier { .. }));
                let kind = if effects.is_barrier || is_prev_barrier {
                    DependencyKind::BarrierBoundary
                } else {
                    DependencyKind::QuantumOrder
                };
                self.add_edge_internal(prev, op_id, WireDependency::new(wire, kind))?;
            }
            for reader in prev_readers {
                let is_reader_barrier = self
                    .get_operation(reader)
                    .is_some_and(|op| matches!(op, Operation::Barrier { .. }));
                let kind = if effects.is_barrier || is_reader_barrier {
                    DependencyKind::BarrierBoundary
                } else {
                    DependencyKind::QuantumOrder
                };
                self.add_edge_internal(reader, op_id, WireDependency::new(wire, kind))?;
            }
        }

        // 2. Classical wire reads (RAW)
        for &bit in &effects.classical_reads {
            let wire = WireId::Classical(bit);
            let state = self.access_states.entry(wire).or_default();
            if let Some(prev_writer) = state.record_read(op_id) {
                self.add_edge_internal(
                    prev_writer,
                    op_id,
                    WireDependency::new(wire, DependencyKind::ClassicalRaw),
                )?;
            }
        }

        // 3. Classical wire writes (WAW and WAR)
        for &bit in &effects.classical_writes {
            let wire = WireId::Classical(bit);
            let state = self.access_states.entry(wire).or_default();
            let (prev_writer, prev_readers) = state.record_write(op_id);

            // Write-After-Write
            if let Some(prev) = prev_writer {
                self.add_edge_internal(
                    prev,
                    op_id,
                    WireDependency::new(wire, DependencyKind::ClassicalWaw),
                )?;
            }

            // Write-After-Read (to all outstanding readers)
            for reader in prev_readers {
                self.add_edge_internal(
                    reader,
                    op_id,
                    WireDependency::new(wire, DependencyKind::ClassicalWar),
                )?;
            }
        }

        Ok(op_id)
    }

    /// Internal helper to insert a dependency edge between two operations.
    fn add_edge_internal(
        &mut self,
        from: OperationId,
        to: OperationId,
        dep: WireDependency,
    ) -> Result<(), IrError> {
        let from_idx = *self
            .node_map
            .get(&from)
            .ok_or(IrError::UnknownOperation { id: from })?;
        let to_idx = *self
            .node_map
            .get(&to)
            .ok_or(IrError::UnknownOperation { id: to })?;

        // Prevent self-loops
        if from_idx == to_idx {
            return Err(IrError::CycleDetected);
        }

        // Avoid duplicate identical edges
        let already_exists = self
            .graph
            .edges_connecting(from_idx, to_idx)
            .any(|e| *e.weight() == dep);

        if !already_exists {
            self.graph.add_edge(from_idx, to_idx, dep);
        }

        Ok(())
    }

    /// Directly adds a typed dependency edge between two operations.
    pub fn add_dependency_edge(
        &mut self,
        from: OperationId,
        to: OperationId,
        dep: WireDependency,
    ) -> Result<(), IrError> {
        self.add_edge_internal(from, to, dep)?;
        if is_cyclic_directed(&self.graph) {
            return Err(IrError::CycleDetected);
        }
        Ok(())
    }

    /// Returns `true` if the graph contains any directed cycle.
    pub fn is_cyclic(&self) -> bool {
        is_cyclic_directed(&self.graph)
    }

    /// Returns an iterator over all direct predecessors of `id` and their connecting dependency edge.
    pub fn predecessors(&self, id: OperationId) -> Vec<(OperationId, WireDependency)> {
        let mut preds = Vec::new();
        if let Some(&node_idx) = self.node_map.get(&id) {
            for edge in self.graph.edges_directed(node_idx, Direction::Incoming) {
                let source_idx = edge.source();
                if let Some(&source_id) = self.id_map.get(&source_idx) {
                    preds.push((source_id, edge.weight().clone()));
                }
            }
        }
        preds
    }

    /// Returns an iterator over all direct successors of `id` and their connecting dependency edge.
    pub fn successors(&self, id: OperationId) -> Vec<(OperationId, WireDependency)> {
        let mut succs = Vec::new();
        if let Some(&node_idx) = self.node_map.get(&id) {
            for edge in self.graph.edges_directed(node_idx, Direction::Outgoing) {
                let target_idx = edge.target();
                if let Some(&target_id) = self.id_map.get(&target_idx) {
                    succs.push((target_id, edge.weight().clone()));
                }
            }
        }
        succs
    }

    /// Returns all edges in the graph as tuples `(from, to, dependency)`.
    pub fn all_edges(&self) -> Vec<(OperationId, OperationId, WireDependency)> {
        let mut edges = Vec::with_capacity(self.graph.edge_count());
        for edge in self.graph.edge_references() {
            if let (Some(&from_id), Some(&to_id)) = (
                self.id_map.get(&edge.source()),
                self.id_map.get(&edge.target()),
            ) {
                edges.push((from_id, to_id, edge.weight().clone()));
            }
        }
        edges
    }

    /// Computes a strictly deterministic topological ordering of all operations in the graph.
    ///
    /// Uses Kahn's algorithm with a [`BTreeSet`] min-priority queue ordered by [`OperationId`].
    /// Any tie-break is resolved deterministically by operation ID sequence number, guaranteeing
    /// identical output sequences across repeated executions and platforms.
    ///
    /// # Errors
    /// Returns [`IrError::CycleDetected`] if a cycle exists in the graph.
    pub fn deterministic_topological_order(&self) -> Result<Vec<OperationId>, IrError> {
        let node_count = self.graph.node_count();
        if node_count == 0 {
            return Ok(Vec::new());
        }

        // 1. Calculate in-degrees for all nodes
        let mut in_degrees: HashMap<NodeIndex, usize> = HashMap::with_capacity(node_count);
        for idx in self.graph.node_indices() {
            let deg = self.graph.edges_directed(idx, Direction::Incoming).count();
            in_degrees.insert(idx, deg);
        }

        // 2. Collect all ready nodes (in-degree == 0) in deterministic BTreeSet ordered by OperationId
        let mut ready_queue: BTreeSet<OperationId> = BTreeSet::new();
        for (&idx, &deg) in &in_degrees {
            if deg == 0 {
                if let Some(&op_id) = self.id_map.get(&idx) {
                    ready_queue.insert(op_id);
                }
            }
        }

        let mut order = Vec::with_capacity(node_count);

        // 3. Process ready queue in deterministic order
        while let Some(op_id) = ready_queue.pop_first() {
            order.push(op_id);
            let node_idx = *self
                .node_map
                .get(&op_id)
                .expect("node_map consistency invariant");

            for edge in self.graph.edges_directed(node_idx, Direction::Outgoing) {
                let succ_idx = edge.target();
                let deg = in_degrees
                    .get_mut(&succ_idx)
                    .expect("in_degrees consistency invariant");
                *deg = deg.saturating_sub(1);
                if *deg == 0 {
                    let succ_id = *self
                        .id_map
                        .get(&succ_idx)
                        .expect("id_map consistency invariant");
                    ready_queue.insert(succ_id);
                }
            }
        }

        if order.len() != node_count {
            return Err(IrError::CycleDetected);
        }

        Ok(order)
    }

    /// Returns an iterator over all node payloads in the graph.
    pub fn nodes(&self) -> impl Iterator<Item = &NodeData> {
        self.graph.node_weights()
    }

    /// Reconstructs a `CircuitGraph` from explicit nodes and dependency edges.
    ///
    /// Restores internal mappings, sequence counters, access states, and verifies acyclicity.
    pub fn from_nodes_and_edges(
        nodes: Vec<NodeData>,
        edges: Vec<(OperationId, OperationId, WireDependency)>,
    ) -> Result<Self, IrError> {
        let mut graph = StableDiGraph::new();
        let mut node_map = HashMap::new();
        let mut id_map = HashMap::new();
        let mut max_id: u64 = 0;
        let mut max_seq: u64 = 0;

        for node_data in nodes {
            let op_id = node_data.id;
            let seq = node_data.sequence_number;
            if op_id.0 > max_id {
                max_id = op_id.0;
            }
            if seq > max_seq {
                max_seq = seq;
            }
            let idx = graph.add_node(node_data);
            node_map.insert(op_id, idx);
            id_map.insert(idx, op_id);
        }

        let mut circuit_graph = Self {
            graph,
            node_map,
            id_map,
            access_states: HashMap::new(),
            next_id: max_id + 1,
            next_sequence: max_seq + 1,
        };

        for (from, to, dep) in edges {
            circuit_graph.add_edge_internal(from, to, dep)?;
        }

        if circuit_graph.is_cyclic() {
            return Err(IrError::CycleDetected);
        }

        if let Ok(order) = circuit_graph.deterministic_topological_order() {
            for op_id in order {
                if let Some(op) = circuit_graph.get_operation(op_id) {
                    let effects = op.wire_effects();
                    for &qubit in &effects.quantum_writes {
                        let wire = WireId::Quantum(qubit);
                        circuit_graph
                            .access_states
                            .entry(wire)
                            .or_default()
                            .record_quantum_update(op_id);
                    }
                    for &bit in &effects.classical_reads {
                        let wire = WireId::Classical(bit);
                        circuit_graph
                            .access_states
                            .entry(wire)
                            .or_default()
                            .record_read(op_id);
                    }
                    for &bit in &effects.classical_writes {
                        let wire = WireId::Classical(bit);
                        circuit_graph
                            .access_states
                            .entry(wire)
                            .or_default()
                            .record_write(op_id);
                    }
                }
            }
        }

        Ok(circuit_graph)
    }
}

impl PartialEq for CircuitGraph {
    fn eq(&self, other: &Self) -> bool {
        if self.node_count() != other.node_count() || self.edge_count() != other.edge_count() {
            return false;
        }
        for node in self.nodes() {
            match other.get_node(node.id) {
                Some(other_node) if node == other_node => {}
                _ => return false,
            }
        }
        let mut self_edges = self.all_edges();
        let mut other_edges = other.all_edges();
        self_edges.sort();
        other_edges.sort();
        self_edges == other_edges
    }
}

impl Eq for CircuitGraph {}
