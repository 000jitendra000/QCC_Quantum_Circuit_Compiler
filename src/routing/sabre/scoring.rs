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

/// Floating-point tolerance threshold for numerical equivalence checks in assertions and diagnostics.
///
/// Note: To preserve strict ordering transitivity and satisfy the [`Ord`] contract,
/// the sorting comparator [`ScoreComponents::cmp_deterministic`] uses exact IEEE 754-2008
/// [`f64::total_cmp`] rather than epsilon-based equality.
pub const SCORE_EPSILON: f64 = 1e-12;

/// Deterministically hashes a candidate physical SWAP pair and random seed using SplitMix64.
///
/// Ensures symmetry: candidate (u, v) and (v, u) hash to identical values,
/// while different seeds produce distinct pseudo-random permutations of tied candidate SWAPs.
#[inline]
pub fn candidate_seed_hash(candidate: &CandidateSwap, seed: u64) -> u64 {
    let u = candidate.u.index() as u64;
    let v = candidate.v.index() as u64;
    let min = u.min(v);
    let max = u.max(v);
    // Combine seed, min, max using golden ratio and SplitMix64 mixing constants
    let mut x = seed
        .wrapping_add(min.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(max.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

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

    /// Deterministically compares two scored candidates using a 5-tier total ordering hierarchy,
    /// optionally breaking tier-5 ties using a deterministic pseudo-random hash seeded by `seed`.
    ///
    /// # Transitivity and Total Order
    /// Floating-point comparisons use IEEE 754-2008 [`f64::total_cmp`] rather than epsilon-based
    /// thresholding. Epsilon thresholding violates the transitive law of equivalence
    /// ($A \approx B \land B \approx C \not\implies A \approx C$) and can cause cyclic comparisons
    /// or sorting panics. `total_cmp` provides a strict, mathematically sound total order across
    /// all floating-point values.
    ///
    /// # 5-Tier Deterministic Comparison Hierarchy
    /// 1. `total_score` ascending (primary objective: smaller score preferred).
    /// 2. `front_cost` ascending (secondary tie-breaker: prefer immediate front-layer distance reduction).
    /// 3. `lookahead_cost` ascending (tertiary tie-breaker: prefer superior lookahead distance positioning).
    /// 4. `decay_factor` ascending (quaternary tie-breaker: prefer less frequently swapped physical qubits).
    /// 5. Tie-breaker:
    ///    - If `seed` is `None`: candidate physical operands `(u, v)` lexicographically ascending.
    ///    - If `seed` is `Some(s)`: candidate physical operands hashed with `s` using SplitMix64,
    ///      falling back to lexicographical `(u, v)` on exact hash collision.
    pub fn cmp_deterministic_with_seed(&self, other: &Self, seed: Option<u64>) -> Ordering {
        self.total_score
            .total_cmp(&other.total_score)
            .then_with(|| self.front_cost.total_cmp(&other.front_cost))
            .then_with(|| self.lookahead_cost.total_cmp(&other.lookahead_cost))
            .then_with(|| self.decay_factor.total_cmp(&other.decay_factor))
            .then_with(|| match seed {
                None => self.candidate.cmp(&other.candidate),
                Some(s) => {
                    let hash_self = candidate_seed_hash(&self.candidate, s);
                    let hash_other = candidate_seed_hash(&other.candidate, s);
                    hash_self
                        .cmp(&hash_other)
                        .then_with(|| self.candidate.cmp(&other.candidate))
                }
            })
    }

    /// Deterministically compares two scored candidates using a 5-tier total ordering hierarchy.
    ///
    /// Delegates to [`Self::cmp_deterministic_with_seed`] with `seed: None`.
    pub fn cmp_deterministic(&self, other: &Self) -> Ordering {
        self.cmp_deterministic_with_seed(other, None)
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
