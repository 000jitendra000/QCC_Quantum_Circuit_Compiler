//! Explainable score breakdown and deterministic tie-breaking for SABRE candidates.
//!
//! Exposes individual score components for candidate SWAP evaluation:
//! front-layer distance contribution ($H_F$), extended lookahead distance
//! contribution ($H_E$), lookahead weight ($W$), decay factor, and total score.
//!
//! # Mathematical Formulation
//! - Front-layer distance:
//!   $$H_F(\pi') = \frac{1}{|F|} \sum_{g \in F} D(\pi'(g_1), \pi'(g_2))$$
//! - Extended-lookahead distance:
//!   $$H_E(\pi') = \begin{cases} \frac{1}{|E|} \sum_{g \in E} D(\pi'(g_1), \pi'(g_2)) & |E| > 0 \\ 0.0 & |E| = 0 \end{cases}$$
//! - Basic distance cost:
//!   $$\text{Cost}(\pi') = H_F(\pi') + W \cdot H_E(\pi')$$
//! - Final candidate score:
//!   $$\text{Score}(\text{SWAP}) = \max(\text{decay}(u), \text{decay}(v)) \times \text{Cost}(\pi')$$

use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::routing::sabre::candidate::CandidateSwap;

/// Floating-point tolerance threshold for deterministic score comparison.
pub const SCORE_EPSILON: f64 = 1e-12;

/// Detailed, explainable breakdown of a candidate SWAP's evaluation score.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScoreComponents {
    /// Candidate physical SWAP evaluated.
    pub candidate: CandidateSwap,
    /// Unweighted normalized front-layer distance:
    /// $$H_F = \frac{1}{|F|} \sum_{g \in F} D(\pi'(g_1), \pi'(g_2))$$
    pub front_cost: f64,
    /// Unweighted normalized extended-lookahead distance:
    /// $$H_E = \frac{1}{|E|} \sum_{g \in E} D(\pi'(g_1), \pi'(g_2))$$
    pub lookahead_cost: f64,
    /// Configured lookahead weight multiplier ($W$).
    pub lookahead_weight: f64,
    /// Weighted lookahead contribution ($W \cdot H_E$).
    pub weighted_lookahead_cost: f64,
    /// Basic distance sum before decay ($H_F + W \cdot H_E$).
    pub basic_cost: f64,
    /// Decay factor multiplier: $\max(\text{decay}(u), \text{decay}(v))$.
    pub decay_factor: f64,
    /// Final candidate score: $\text{decay\_factor} \times \text{basic\_cost}$.
    pub total_score: f64,
}

impl ScoreComponents {
    /// Constructs a new score breakdown with explicit components.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        candidate: CandidateSwap,
        front_cost: f64,
        lookahead_cost: f64,
        lookahead_weight: f64,
        weighted_lookahead_cost: f64,
        basic_cost: f64,
        decay_factor: f64,
        total_score: f64,
    ) -> Self {
        Self {
            candidate,
            front_cost,
            lookahead_cost,
            lookahead_weight,
            weighted_lookahead_cost,
            basic_cost,
            decay_factor,
            total_score,
        }
    }

    /// Deterministically compares two scored candidates.
    ///
    /// Hierarchical tie-breaking order:
    /// 1. `total_score` ascending (smaller score is preferred).
    /// 2. `front_cost` ascending (prefer immediate front-layer progress).
    /// 3. `lookahead_cost` ascending (prefer better lookahead positioning).
    /// 4. `decay_factor` ascending (prefer less frequently swapped qubits).
    /// 5. Candidate physical operands `(u, v)` lexicographically ascending.
    pub fn cmp_deterministic(&self, other: &Self) -> Ordering {
        let diff = self.total_score - other.total_score;
        if diff.abs() > SCORE_EPSILON {
            return if diff < 0.0 {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        let front_diff = self.front_cost - other.front_cost;
        if front_diff.abs() > SCORE_EPSILON {
            return if front_diff < 0.0 {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        let la_diff = self.lookahead_cost - other.lookahead_cost;
        if la_diff.abs() > SCORE_EPSILON {
            return if la_diff < 0.0 {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        let decay_diff = self.decay_factor - other.decay_factor;
        if decay_diff.abs() > SCORE_EPSILON {
            return if decay_diff < 0.0 {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        self.candidate.cmp(&other.candidate)
    }
}

impl Eq for ScoreComponents {}

impl PartialOrd for ScoreComponents {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScoreComponents {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_deterministic(other)
    }
}

impl fmt::Display for ScoreComponents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SWAP({}, {}) score={:.6} [H_F={:.4}, H_E={:.4} (W={:.2}), decay={:.4}]",
            self.candidate.u,
            self.candidate.v,
            self.total_score,
            self.front_cost,
            self.lookahead_cost,
            self.lookahead_weight,
            self.decay_factor
        )
    }
}
