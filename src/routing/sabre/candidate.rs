//! Candidate SWAP generation for SABRE routing.
//!
//! Generates candidate physical SWAPs on hardware coupling edges incident to
//! physical qubits currently occupied by front-layer operations that are not
//! directly adjacent on the hardware topology.
//!
//! # Critical Invariants
//! - **Canonical Undirected Pairs**: Candidate SWAPs are canonicalized such that
//!   `u < v` (by [`PhysicalQubitId`]) so that duplicate undirected edges `(u, v)`
//!   and `(v, u)` are collapsed.
//! - **Deterministic Ordering**: Candidates are deduplicated and returned in strictly
//!   deterministic lexicographical order.
//! - **Front-Layer Grounding**: Candidates are generated strictly from the physical
//!   neighborhood of front-layer operands with distance $> 1$. Operations already
//!   adjacent require no SWAPs.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::routing::placement::distance::TopologyDistances;
use crate::routing::sabre::front_layer::FrontLayer;
use crate::target::connectivity::CouplingGraph;
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;

/// A candidate physical SWAP between two physically adjacent hardware qubits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CandidateSwap {
    /// First physical hardware qubit (strictly `u < v`).
    pub u: PhysicalQubitId,
    /// Second physical hardware qubit (strictly `v > u`).
    pub v: PhysicalQubitId,
}

impl CandidateSwap {
    /// Constructs a new candidate SWAP between physical qubits `p1` and `p2`.
    ///
    /// Canonicalizes the pair such that `u = min(p1, p2)` and `v = max(p1, p2)`.
    #[inline]
    pub fn new(p1: PhysicalQubitId, p2: PhysicalQubitId) -> Self {
        if p1 <= p2 {
            Self { u: p1, v: p2 }
        } else {
            Self { u: p2, v: p1 }
        }
    }

    /// Returns the canonical physical operand pair `(u, v)`.
    #[inline]
    pub fn operands(&self) -> (PhysicalQubitId, PhysicalQubitId) {
        (self.u, self.v)
    }

    /// Returns `true` if this candidate SWAP touches the specified physical qubit.
    #[inline]
    pub fn touches(&self, p: PhysicalQubitId) -> bool {
        self.u == p || self.v == p
    }

    /// Returns `true` if this candidate is a non-trivial SWAP (`u != v`).
    #[inline]
    pub fn is_non_trivial(&self) -> bool {
        self.u != self.v
    }
}

/// Candidate SWAP generator evaluating coupling edges adjacent to front-layer gates.
pub struct CandidateGenerator;

impl CandidateGenerator {
    /// Generates candidate SWAPs for the current front layer on the hardware target.
    ///
    /// For each two-qubit gate in the front layer:
    /// - If the gate's physical operands are already adjacent on the target coupling graph,
    ///   no routing SWAPs are required for that gate.
    /// - If the physical operands are distance $> 1$ apart, candidate SWAPs are collected
    ///   along all hardware coupling edges incident to `p1` and `p2`.
    ///
    /// All candidates are canonicalized (`u < v`), deduplicated, and returned in
    /// deterministic lexicographical order.
    pub fn generate_candidates(
        front_layer: &FrontLayer,
        target: &Target,
        distances: &TopologyDistances,
    ) -> Vec<CandidateSwap> {
        let coupling = target.combined_coupling_graph();
        Self::generate_candidates_with_coupling(front_layer, &coupling, distances)
    }

    /// Generates candidate SWAPs directly from a [`CouplingGraph`].
    pub fn generate_candidates_with_coupling(
        front_layer: &FrontLayer,
        coupling: &CouplingGraph,
        distances: &TopologyDistances,
    ) -> Vec<CandidateSwap> {
        let mut candidate_set = BTreeSet::new();

        for op in front_layer.operations() {
            let (p1, p2) = op.physical_operands();

            // Only generate SWAP candidates for operations that are not already adjacent
            let dist = distances.distance(p1, p2);
            if dist == Some(1) || dist == Some(0) {
                continue;
            }

            // Neighbors of p1
            for &neighbor in distances.neighbors(p1) {
                if coupling.has_edge(p1, neighbor) || coupling.has_edge(neighbor, p1) {
                    candidate_set.insert(CandidateSwap::new(p1, neighbor));
                }
            }

            // Neighbors of p2
            for &neighbor in distances.neighbors(p2) {
                if coupling.has_edge(p2, neighbor) || coupling.has_edge(neighbor, p2) {
                    candidate_set.insert(CandidateSwap::new(p2, neighbor));
                }
            }
        }

        candidate_set.into_iter().collect()
    }
}
