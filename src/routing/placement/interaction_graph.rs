//! Logical circuit interaction graph representation for initial placement.
//!
//! Extracts multi-qubit interactions from a logical [`Circuit`], tracking weighted
//! interaction frequencies per pair of logical qubits to guide placement decisions.

use std::collections::{BTreeMap, BTreeSet};

use crate::ir::circuit::Circuit;
use crate::ir::operation::Operation;
use crate::ir::wire::QubitId;

/// An undirected weighted interaction graph representing multi-qubit interactions in a circuit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractionGraph {
    num_qubits: usize,
    /// Interaction edges mapped to their interaction count (weight).
    /// Stored with canonical ordering $(q_1 < q_2)$.
    edges: BTreeMap<(QubitId, QubitId), usize>,
    /// Incident edges per logical qubit.
    adjacency: BTreeMap<QubitId, BTreeSet<QubitId>>,
}

impl InteractionGraph {
    /// Creates an empty interaction graph for a circuit with `num_qubits` logical qubits.
    pub fn new(num_qubits: usize) -> Self {
        let mut adjacency = BTreeMap::new();
        for i in 0..num_qubits {
            adjacency.insert(QubitId(i), BTreeSet::new());
        }
        Self {
            num_qubits,
            edges: BTreeMap::new(),
            adjacency,
        }
    }

    /// Extracts the logical interaction graph by scanning all multi-qubit operations in `circuit`.
    pub fn from_circuit(circuit: &Circuit) -> Self {
        let num_qubits = circuit.num_qubits();
        let mut graph = Self::new(num_qubits);

        for node in circuit.graph.nodes() {
            if let Operation::Gate { qubits, .. } = &node.operation {
                if qubits.len() == 2 {
                    let q0 = qubits[0];
                    let q1 = qubits[1];
                    if q0 != q1 && q0.0 < num_qubits && q1.0 < num_qubits {
                        graph.add_interaction(q0, q1, 1);
                    }
                } else if qubits.len() > 2 {
                    // General multi-qubit gate: add interaction for all pairs
                    for i in 0..qubits.len() {
                        for j in (i + 1)..qubits.len() {
                            let qi = qubits[i];
                            let qj = qubits[j];
                            if qi != qj && qi.0 < num_qubits && qj.0 < num_qubits {
                                graph.add_interaction(qi, qj, 1);
                            }
                        }
                    }
                }
            }
        }

        graph
    }

    /// Adds or increments an interaction between logical qubits `q0` and `q1` with the given `weight`.
    pub fn add_interaction(&mut self, q0: QubitId, q1: QubitId, weight: usize) {
        if q0 == q1 || weight == 0 {
            return;
        }
        let u = q0.min(q1);
        let v = q0.max(q1);

        *self.edges.entry((u, v)).or_insert(0) += weight;
        self.adjacency.entry(u).or_default().insert(v);
        self.adjacency.entry(v).or_default().insert(u);
    }

    /// Returns the total logical qubit count.
    #[inline]
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Returns the interaction weight between logical qubits `q0` and `q1`.
    /// Returns 0 if they do not interact.
    pub fn edge_weight(&self, q0: QubitId, q1: QubitId) -> usize {
        let u = q0.min(q1);
        let v = q0.max(q1);
        self.edges.get(&(u, v)).copied().unwrap_or(0)
    }

    /// Returns all interaction edges with their respective weights, canonically ordered $q_1 < q_2$.
    #[inline]
    pub fn edges(&self) -> &BTreeMap<(QubitId, QubitId), usize> {
        &self.edges
    }

    /// Returns the unweighted degree (number of distinct logical neighbors) of `qubit`.
    pub fn unweighted_degree(&self, qubit: QubitId) -> usize {
        self.adjacency.get(&qubit).map(|s| s.len()).unwrap_or(0)
    }

    /// Returns the weighted degree (sum of interaction counts on all incident edges) of `qubit`.
    pub fn weighted_degree(&self, qubit: QubitId) -> usize {
        if let Some(neighbors) = self.adjacency.get(&qubit) {
            neighbors
                .iter()
                .map(|&nbr| self.edge_weight(qubit, nbr))
                .sum()
        } else {
            0
        }
    }

    /// Returns the sorted interacting neighbors of `qubit`.
    pub fn neighbors(&self, qubit: QubitId) -> Vec<QubitId> {
        self.adjacency
            .get(&qubit)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Returns the total interaction count across all edges in the circuit.
    pub fn total_interactions(&self) -> usize {
        self.edges.values().sum()
    }

    /// Returns `true` if there are any two-qubit interactions in this circuit.
    #[inline]
    pub fn has_interactions(&self) -> bool {
        !self.edges.is_empty()
    }

    /// Returns all logical qubits sorted by:
    /// 1. Weighted degree descending
    /// 2. Unweighted degree descending
    /// 3. Qubit ID ascending (for strict determinism)
    pub fn logical_qubits_by_degree_descending(&self) -> Vec<QubitId> {
        let mut qubits: Vec<QubitId> = (0..self.num_qubits).map(QubitId).collect();
        qubits.sort_by(|&a, &b| {
            let w_a = self.weighted_degree(a);
            let w_b = self.weighted_degree(b);
            let u_a = self.unweighted_degree(a);
            let u_b = self.unweighted_degree(b);

            w_b.cmp(&w_a)
                .then_with(|| u_b.cmp(&u_a))
                .then_with(|| a.cmp(&b))
        });
        qubits
    }

    /// Returns all interaction edges sorted by weight descending,
    /// with ties broken deterministically by qubit IDs ascending.
    pub fn sorted_edges_by_weight_descending(&self) -> Vec<((QubitId, QubitId), usize)> {
        let mut edge_list: Vec<((QubitId, QubitId), usize)> =
            self.edges.iter().map(|(&k, &v)| (k, v)).collect();
        edge_list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        edge_list
    }
}
