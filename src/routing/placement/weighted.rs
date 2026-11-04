//! Interaction-weighted initial placement.
//!
//! Optimizes physical layout by directly prioritizing frequently interacting
//! pairs of logical qubits, anchoring heavy interaction edges across candidate
//! target sites and minimizing total weighted physical routing distance.

use std::collections::BTreeSet;

use crate::ir::wire::QubitId;
use crate::routing::layout_map::Layout;
use crate::routing::placement::distance::TopologyDistances;
use crate::routing::placement::interaction_graph::InteractionGraph;
use crate::routing::placement::score::PlacementScore;
use crate::routing::placement::strategy::{PlacementError, PlacementResult, PlacementStrategy};
use crate::target::operation::PhysicalQubitId;

/// Interaction-weighted placement directly minimizing weighted physical distance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeightedPlacer;

impl WeightedPlacer {
    /// Creates a new `WeightedPlacer`.
    pub fn new() -> Self {
        Self
    }

    /// Computes the interaction-weighted placement by evaluating candidate physical anchor edges
    /// for the heaviest logical interaction edge and greedily growing the layout.
    pub fn place(
        &self,
        interactions: &InteractionGraph,
        distances: &TopologyDistances,
    ) -> Result<PlacementResult, PlacementError> {
        let m = interactions.num_qubits();
        let n = distances.num_qubits();

        if m > n {
            return Err(PlacementError::InsufficientCapacity {
                required: m,
                available: n,
            });
        }

        if m == 0 {
            let layout = Layout::new(0, n);
            let score = PlacementScore::evaluate(&layout, interactions, distances);
            return Ok(PlacementResult {
                layout,
                strategy: PlacementStrategy::Weighted,
                score,
                logical_qubits: 0,
                physical_qubits: n,
            });
        }

        // If no interactions exist at all in the circuit, fall back to sequential physical assignment
        if !interactions.has_interactions() || m == 1 {
            let mut mappings = Vec::with_capacity(m);
            for i in 0..m {
                mappings.push((QubitId(i), PhysicalQubitId::new(i as u32)));
            }
            let layout = Layout::from_mappings(m, n, mappings)?;
            let score = PlacementScore::evaluate(&layout, interactions, distances);
            return Ok(PlacementResult {
                layout,
                strategy: PlacementStrategy::Weighted,
                score,
                logical_qubits: m,
                physical_qubits: n,
            });
        }

        let sorted_edges = interactions.sorted_edges_by_weight_descending();
        let (q_a, q_b) = sorted_edges[0].0;

        // Collect all available physical edges (distance = 1) as candidate anchors
        let mut candidate_anchors: Vec<(PhysicalQubitId, PhysicalQubitId)> = Vec::new();
        for u_idx in 0..n {
            let u = PhysicalQubitId::new(u_idx as u32);
            for &v in distances.neighbors(u) {
                candidate_anchors.push((u, v));
            }
        }

        // If topology has no edges, fallback to first two physical qubits
        if candidate_anchors.is_empty() {
            candidate_anchors.push((PhysicalQubitId::new(0), PhysicalQubitId::new(1)));
        }

        let mut best_result: Option<PlacementResult> = None;

        for (p_a, p_b) in candidate_anchors {
            let res =
                self.place_with_anchor(interactions, distances, m, n, (q_a, q_b), (p_a, p_b))?;

            match &best_result {
                None => best_result = Some(res),
                Some(curr) => {
                    if res.score < curr.score {
                        best_result = Some(res);
                    }
                }
            }
        }

        Ok(best_result.expect("candidate anchors list was non-empty"))
    }

    /// Constructs a placement grown greedily starting from an anchored pair `(q_a -> p_a, q_b -> p_b)`.
    fn place_with_anchor(
        &self,
        interactions: &InteractionGraph,
        distances: &TopologyDistances,
        m: usize,
        n: usize,
        anchor_q: (QubitId, QubitId),
        anchor_p: (PhysicalQubitId, PhysicalQubitId),
    ) -> Result<PlacementResult, PlacementError> {
        let (q_a, q_b) = anchor_q;
        let (p_a, p_b) = anchor_p;

        let mut assigned_physical = BTreeSet::new();
        let mut placed_logical = BTreeSet::new();
        let mut mappings: Vec<(QubitId, PhysicalQubitId)> = Vec::with_capacity(m);

        mappings.push((q_a, p_a));
        assigned_physical.insert(p_a);
        placed_logical.insert(q_a);

        mappings.push((q_b, p_b));
        assigned_physical.insert(p_b);
        placed_logical.insert(q_b);

        // Iteratively place remaining logical qubits
        while placed_logical.len() < m {
            let mut best_candidate_q = None;
            let mut best_affinity = None;

            for q_idx in 0..m {
                let q = QubitId(q_idx);
                if placed_logical.contains(&q) {
                    continue;
                }

                // Compute affinity: sum of interaction weights with already placed qubits
                let mut affinity = 0;
                for &placed_q in &placed_logical {
                    affinity += interactions.edge_weight(q, placed_q);
                }

                let total_w = interactions.weighted_degree(q);
                let score_tuple = (affinity, total_w, usize::MAX - q_idx);

                if best_affinity.is_none() || score_tuple > best_affinity.unwrap() {
                    best_affinity = Some(score_tuple);
                    best_candidate_q = Some(q);
                }
            }

            let next_q = best_candidate_q.expect("must find unplaced logical qubit");

            // Find the best available physical location that minimizes weighted distance to placed neighbors
            let mut best_target_p = None;
            let mut best_cost = None; // (weighted_distance_cost, usize::MAX - physical_degree, p_id)

            for p_idx in 0..n {
                let candidate_p = PhysicalQubitId::new(p_idx as u32);
                if assigned_physical.contains(&candidate_p) {
                    continue;
                }

                let mut weighted_dist_sum = 0;
                for &(placed_q, placed_p) in &mappings {
                    let w = interactions.edge_weight(next_q, placed_q);
                    if w > 0 {
                        let d = distances.distance_or(
                            candidate_p,
                            placed_p,
                            distances.disconnected_penalty(),
                        );
                        weighted_dist_sum += w * d;
                    }
                }

                let phys_deg = distances.degree(candidate_p);
                let cost_tuple = (weighted_dist_sum, usize::MAX - phys_deg, candidate_p);

                if best_cost.is_none() || cost_tuple < best_cost.unwrap() {
                    best_cost = Some(cost_tuple);
                    best_target_p = Some(candidate_p);
                }
            }

            let chosen_p = best_target_p.expect("must find available physical qubit");
            mappings.push((next_q, chosen_p));
            assigned_physical.insert(chosen_p);
            placed_logical.insert(next_q);
        }

        let layout = Layout::from_mappings(m, n, mappings)?;
        let score = PlacementScore::evaluate(&layout, interactions, distances);

        Ok(PlacementResult {
            layout,
            strategy: PlacementStrategy::Weighted,
            score,
            logical_qubits: m,
            physical_qubits: n,
        })
    }
}
