//! Placement scoring metric and evaluation.
//!
//! Provides a deterministic quantitative score for candidate physical placements,
//! evaluating total routing distance, interaction-weighted distance, and direct edge fraction.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::routing::layout_map::Layout;
use crate::routing::placement::distance::TopologyDistances;
use crate::routing::placement::interaction_graph::InteractionGraph;

/// Deterministic evaluation score for a candidate initial placement.
///
/// Implements [`Ord`] such that lower/better placement scores compare as less-than
/// (i.e. `score_a < score_b` indicates `score_a` is a superior placement).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementScore {
    /// Number of interacting logical pairs mapped to mutually disconnected physical qubits.
    pub disconnected_pairs: usize,
    /// Total interaction-weighted physical distance $\sum w(q_i, q_j) \cdot \operatorname{dist}(p_i, p_j)$.
    pub weighted_distance_sum: usize,
    /// Unweighted physical distance sum $\sum \operatorname{dist}(p_i, p_j)$ across all interacting pairs.
    pub unweighted_distance_sum: usize,
    /// Maximum physical distance between any interacting logical qubit pair.
    pub max_distance: usize,
    /// Number of distinct interacting pairs that are placed directly adjacent ($\operatorname{dist} = 1$).
    pub direct_edges: usize,
    /// Sum of interaction counts on directly adjacent edges.
    pub direct_interactions: usize,
    /// Total two-qubit interaction count in the circuit.
    pub total_interactions: usize,
    /// Total distinct interacting logical pairs in the circuit.
    pub total_edges: usize,
}

impl PlacementScore {
    /// Evaluates the placement score of `layout` against the circuit's `interactions`
    /// and the target's `distances`.
    pub fn evaluate(
        layout: &Layout,
        interactions: &InteractionGraph,
        distances: &TopologyDistances,
    ) -> Self {
        let disconnected_penalty = distances.num_qubits() * 1000;
        let mut disconnected_pairs = 0;
        let mut weighted_distance_sum = 0;
        let mut unweighted_distance_sum = 0;
        let mut max_distance = 0;
        let mut direct_edges = 0;
        let mut direct_interactions = 0;

        for (&(q0, q1), &weight) in interactions.edges() {
            let p0 = layout.get_physical(q0);
            let p1 = layout.get_physical(q1);

            match (p0, p1) {
                (Some(phys0), Some(phys1)) => match distances.distance(phys0, phys1) {
                    Some(d) => {
                        weighted_distance_sum += weight * d;
                        unweighted_distance_sum += d;
                        if d > max_distance {
                            max_distance = d;
                        }
                        if d == 1 {
                            direct_edges += 1;
                            direct_interactions += weight;
                        }
                    }
                    None => {
                        disconnected_pairs += 1;
                        weighted_distance_sum += weight * disconnected_penalty;
                        unweighted_distance_sum += disconnected_penalty;
                        max_distance = max_distance.max(disconnected_penalty);
                    }
                },
                _ => {
                    disconnected_pairs += 1;
                    weighted_distance_sum += weight * disconnected_penalty;
                    unweighted_distance_sum += disconnected_penalty;
                    max_distance = max_distance.max(disconnected_penalty);
                }
            }
        }

        Self {
            disconnected_pairs,
            weighted_distance_sum,
            unweighted_distance_sum,
            max_distance,
            direct_edges,
            direct_interactions,
            total_interactions: interactions.total_interactions(),
            total_edges: interactions.edges().len(),
        }
    }

    /// Returns the fraction of total interactions placed on directly adjacent physical edges ($[0.0, 1.0]$).
    pub fn direct_interaction_ratio(&self) -> f64 {
        if self.total_interactions == 0 {
            1.0
        } else {
            self.direct_interactions as f64 / self.total_interactions as f64
        }
    }

    /// Returns the average weighted physical distance per interaction.
    pub fn average_weighted_distance(&self) -> f64 {
        if self.total_interactions == 0 {
            0.0
        } else {
            self.weighted_distance_sum as f64 / self.total_interactions as f64
        }
    }

    /// Returns `true` if all interacting pairs are placed directly adjacent on the coupling graph
    /// with zero routing distance penalty.
    #[inline]
    pub fn is_perfect(&self) -> bool {
        self.disconnected_pairs == 0 && self.max_distance <= 1
    }
}

impl PartialOrd for PlacementScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PlacementScore {
    fn cmp(&self, other: &Self) -> Ordering {
        // Lower is better:
        // 1. Fewer disconnected pairs
        // 2. Lower weighted distance sum
        // 3. Lower unweighted distance sum
        // 4. Lower max distance
        // 5. More direct interactions (higher is better, so invert)
        self.disconnected_pairs
            .cmp(&other.disconnected_pairs)
            .then_with(|| self.weighted_distance_sum.cmp(&other.weighted_distance_sum))
            .then_with(|| {
                self.unweighted_distance_sum
                    .cmp(&other.unweighted_distance_sum)
            })
            .then_with(|| self.max_distance.cmp(&other.max_distance))
            .then_with(|| other.direct_interactions.cmp(&self.direct_interactions))
    }
}
