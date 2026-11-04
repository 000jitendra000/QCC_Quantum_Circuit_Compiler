//! Physical topology distances and shortest path computation for placement.
//!
//! Computes all-pairs shortest path distance matrices on physical coupling graphs
//! using breadth-first search (BFS), treating physical coupling edges as undirected
//! communication channels.

use std::collections::{BTreeSet, VecDeque};

use crate::target::connectivity::CouplingGraph;
use crate::target::operation::PhysicalQubitId;

/// All-pairs shortest path distance matrix and connectivity metrics for a target topology.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopologyDistances {
    num_qubits: usize,
    /// Distance matrix stored as a flattened vector `distances[u * num_qubits + v]`.
    /// `None` indicates disconnected / unreachable physical qubits.
    distances: Vec<Option<usize>>,
    /// Undirected adjacency list: physical qubit -> sorted set of physical neighbors.
    adjacency: Vec<Vec<PhysicalQubitId>>,
}

impl TopologyDistances {
    /// Computes the all-pairs shortest paths from a target's [`CouplingGraph`].
    pub fn from_coupling_graph(coupling: &CouplingGraph) -> Self {
        let n = coupling.num_qubits;
        let mut adj_sets: Vec<BTreeSet<PhysicalQubitId>> = vec![BTreeSet::new(); n];

        // Treat edges as undirected for physical communication distances
        for &(u, v) in &coupling.edges {
            if u.index() < n && v.index() < n && u != v {
                adj_sets[u.index()].insert(v);
                adj_sets[v.index()].insert(u);
            }
        }

        let adjacency: Vec<Vec<PhysicalQubitId>> = adj_sets
            .into_iter()
            .map(|set| set.into_iter().collect())
            .collect();

        let mut distances = vec![None; n * n];

        // Run BFS from each physical qubit
        for src_idx in 0..n {
            let src = PhysicalQubitId::new(src_idx as u32);
            let mut queue = VecDeque::new();
            distances[src_idx * n + src_idx] = Some(0);
            queue.push_back((src, 0));

            while let Some((curr, d)) = queue.pop_front() {
                for &neighbor in &adjacency[curr.index()] {
                    let n_idx = neighbor.index();
                    let target_slot = src_idx * n + n_idx;
                    if distances[target_slot].is_none() {
                        distances[target_slot] = Some(d + 1);
                        queue.push_back((neighbor, d + 1));
                    }
                }
            }
        }

        Self {
            num_qubits: n,
            distances,
            adjacency,
        }
    }

    /// Returns the total number of physical qubits in this topology.
    #[inline]
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Returns the shortest path distance in edge hops between two physical qubits.
    ///
    /// Returns `Some(0)` if `p1 == p2`.
    /// Returns `None` if `p1` and `p2` belong to disconnected components,
    /// or if either qubit index is out of bounds.
    #[inline]
    pub fn distance(&self, p1: PhysicalQubitId, p2: PhysicalQubitId) -> Option<usize> {
        let i = p1.index();
        let j = p2.index();
        if i < self.num_qubits && j < self.num_qubits {
            self.distances[i * self.num_qubits + j]
        } else {
            None
        }
    }

    /// Returns the shortest path distance between two physical qubits, or a default penalty
    /// if they are disconnected or out of bounds.
    #[inline]
    pub fn distance_or(&self, p1: PhysicalQubitId, p2: PhysicalQubitId, penalty: usize) -> usize {
        self.distance(p1, p2).unwrap_or(penalty)
    }

    /// Returns the standard penalty distance applied when physical qubits are in disconnected components.
    #[inline]
    pub fn disconnected_penalty(&self) -> usize {
        self.num_qubits.max(1) * 1000
    }

    /// Returns the undirected degree (number of physical neighbors) of `qubit`.
    #[inline]
    pub fn degree(&self, qubit: PhysicalQubitId) -> usize {
        if qubit.index() < self.num_qubits {
            self.adjacency[qubit.index()].len()
        } else {
            0
        }
    }

    /// Returns the neighbors of `qubit`.
    #[inline]
    pub fn neighbors(&self, qubit: PhysicalQubitId) -> &[PhysicalQubitId] {
        if qubit.index() < self.num_qubits {
            &self.adjacency[qubit.index()]
        } else {
            &[]
        }
    }

    /// Returns whether the physical topology is fully connected.
    pub fn is_connected(&self) -> bool {
        if self.num_qubits <= 1 {
            return true;
        }
        for i in 0..self.num_qubits {
            for j in 0..self.num_qubits {
                if self.distances[i * self.num_qubits + j].is_none() {
                    return false;
                }
            }
        }
        true
    }

    /// Returns the maximum finite distance between any two reachable physical qubits.
    pub fn diameter(&self) -> usize {
        let mut max_d = 0;
        for &d in &self.distances {
            if let Some(hops) = d {
                if hops > max_d {
                    max_d = hops;
                }
            }
        }
        max_d
    }

    /// Returns a list of all physical qubits sorted by degree descending,
    /// with ties broken deterministically by physical qubit ID ascending.
    pub fn qubits_by_degree_descending(&self) -> Vec<PhysicalQubitId> {
        let mut qubits: Vec<PhysicalQubitId> = (0..self.num_qubits as u32)
            .map(PhysicalQubitId::new)
            .collect();
        qubits.sort_by(|&a, &b| {
            let deg_a = self.degree(a);
            let deg_b = self.degree(b);
            deg_b.cmp(&deg_a).then_with(|| a.cmp(&b))
        });
        qubits
    }
}
