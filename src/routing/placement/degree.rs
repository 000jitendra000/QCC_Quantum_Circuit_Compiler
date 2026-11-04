//! Degree-based initial placement.
//!
//! Maps high-degree logical circuit qubits to high-degree physical target qubits,
//! prioritizing placement of logical interaction hubs onto central/high-connectivity
//! physical device locations.

use std::collections::BTreeSet;

use crate::ir::wire::QubitId;
use crate::routing::layout_map::Layout;
use crate::routing::placement::distance::TopologyDistances;
use crate::routing::placement::interaction_graph::InteractionGraph;
use crate::routing::placement::score::PlacementScore;
use crate::routing::placement::strategy::{PlacementError, PlacementResult, PlacementStrategy};
use crate::target::operation::PhysicalQubitId;

/// Degree-based placement matching logical interaction degrees to physical topology degrees.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DegreePlacer;

impl DegreePlacer {
    /// Creates a new `DegreePlacer`.
    pub fn new() -> Self {
        Self
    }

    /// Computes the degree-based placement.
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
                strategy: PlacementStrategy::Degree,
                score,
                logical_qubits: 0,
                physical_qubits: n,
            });
        }

        // Rank logical qubits by unweighted degree descending, then weighted degree descending, then ID ascending
        let mut logical_ranked: Vec<QubitId> = (0..m).map(QubitId).collect();
        logical_ranked.sort_by(|&a, &b| {
            let u_a = interactions.unweighted_degree(a);
            let u_b = interactions.unweighted_degree(b);
            let w_a = interactions.weighted_degree(a);
            let w_b = interactions.weighted_degree(b);

            u_b.cmp(&u_a)
                .then_with(|| w_b.cmp(&w_a))
                .then_with(|| a.cmp(&b))
        });

        // Rank physical qubits by physical topology degree descending, then ID ascending
        let physical_ranked = distances.qubits_by_degree_descending();

        let mut assigned_physical = BTreeSet::new();
        let mut mappings: Vec<(QubitId, PhysicalQubitId)> = Vec::with_capacity(m);

        // Place the highest degree logical qubit on the highest degree physical qubit
        let root_logical = logical_ranked[0];
        let root_physical = physical_ranked[0];
        mappings.push((root_logical, root_physical));
        assigned_physical.insert(root_physical);

        let mut placed_logical = BTreeSet::new();
        placed_logical.insert(root_logical);

        // For remaining logical qubits in degree order, prioritize placing adjacent logical neighbors
        // onto available physical neighbors of already-placed neighbors that have high physical degree
        for &logical_q in &logical_ranked[1..] {
            // Find already placed logical neighbors of logical_q
            let placed_neighbors: Vec<QubitId> = interactions
                .neighbors(logical_q)
                .into_iter()
                .filter(|nbr| placed_logical.contains(nbr))
                .collect();

            let mut best_physical = None;
            let mut best_score = None; // (min_distance_to_placed_neighbors, -physical_degree, physical_id)

            // Evaluate all candidate unassigned physical qubits
            for &candidate in &physical_ranked {
                if assigned_physical.contains(&candidate) {
                    continue;
                }

                let phys_deg = distances.degree(candidate);

                if placed_neighbors.is_empty() {
                    // If logical_q has no placed neighbors, prefer highest physical degree
                    let score = (0usize, usize::MAX - phys_deg, candidate);
                    if best_score.is_none() || score < best_score.unwrap() {
                        best_score = Some(score);
                        best_physical = Some(candidate);
                    }
                } else {
                    // Distance sum to already-placed logical neighbors
                    let mut dist_sum = 0;
                    for &log_nbr in &placed_neighbors {
                        // Find where log_nbr is mapped
                        let nbr_phys = mappings
                            .iter()
                            .find(|(q, _)| *q == log_nbr)
                            .map(|(_, p)| *p)
                            .unwrap();

                        dist_sum += distances.distance_or(candidate, nbr_phys, n * 10);
                    }

                    let score = (dist_sum, usize::MAX - phys_deg, candidate);
                    if best_score.is_none() || score < best_score.unwrap() {
                        best_score = Some(score);
                        best_physical = Some(candidate);
                    }
                }
            }

            let chosen_phys = best_physical.expect("must find available physical qubit");
            mappings.push((logical_q, chosen_phys));
            assigned_physical.insert(chosen_phys);
            placed_logical.insert(logical_q);
        }

        let layout = Layout::from_mappings(m, n, mappings)?;
        let score = PlacementScore::evaluate(&layout, interactions, distances);

        Ok(PlacementResult {
            layout,
            strategy: PlacementStrategy::Degree,
            score,
            logical_qubits: m,
            physical_qubits: n,
        })
    }
}
