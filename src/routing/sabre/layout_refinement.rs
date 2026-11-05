//! SABRE Iterative Layout Refinement engine.
//!
//! Implements multi-round bidirectional layout refinement (Li et al., ASPLOS 2019):
//! alternates between forward circuit routing and reverse circuit routing to iteratively
//! discover initial layouts that minimize total inserted SWAPs, critical-path DAG depth,
//! and target duration.
//!
//! # Architecture & Permutation Semantics
//! - **Forward Pass**: Routes logical circuit $C$ starting from initial layout $L_{\text{initial}}^{(r)}$,
//!   yielding routed circuit $C_{\text{routed}}^{(r)}$, final layout $L_{\text{final}}^{(r)}$, and
//!   comprehensive routing metrics $M^{(r)}$.
//! - **Reverse Pass**: Routes the reversed circuit $C_{rev}$ starting from $L_{\text{final}}^{(r)}$,
//!   yielding reverse final layout $L_{\text{rev\_final}}^{(r)}$.
//! - **Candidate Initial Layout**: Because early gates of $C$ are the *final* gates routed in $C_{rev}$,
//!   their operands are placed adjacent in $L_{\text{rev\_final}}^{(r)}$. Therefore, $L_{\text{rev\_final}}^{(r)}$
//!   directly serves as the candidate initial layout $L_{\text{initial}}^{(r+1)}$ for the next forward pass.
//! - **Acceptance Rule**: A forward pass is accepted as the new best candidate if its composite routing cost
//!   (or inserted SWAP count on tie) is strictly lower than the best previously seen forward pass.

use serde::{Deserialize, Serialize};

use crate::routing::layout_map::LayoutMap;
use crate::routing::metrics::RoutingMetrics;

/// Record of a single bidirectional refinement iteration round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefinementRoundRecord {
    /// 1-indexed refinement round number.
    pub round: usize,
    /// Forward initial layout used for this round.
    pub initial_layout: LayoutMap,
    /// Physical SWAP operations inserted during forward routing.
    pub forward_swaps: usize,
    /// Critical-path DAG depth of the forward routed circuit.
    pub forward_depth: usize,
    /// Composite cost score evaluated on the forward routed circuit.
    pub forward_cost: f64,
    /// Physical SWAP operations inserted during reverse traversal.
    pub reverse_swaps: usize,
    /// Composite cost score evaluated on the reverse traversal circuit.
    pub reverse_cost: f64,
    /// The next initial layout derived from reverse traversal's resulting permutation.
    pub next_initial_layout: LayoutMap,
    /// Whether this round's forward routed circuit was accepted as the new global best.
    pub accepted: bool,
}

/// Comprehensive summary of all bidirectional refinement iterations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefinementSummary {
    /// Ordered history of all refinement rounds executed.
    pub rounds: Vec<RefinementRoundRecord>,
    /// The final selected initial layout for forward routing.
    pub selected_initial_layout: LayoutMap,
    /// The final layout produced at circuit termination under the selected routing.
    pub selected_final_layout: LayoutMap,
    /// Index of the refinement round that produced the winning result (1-indexed).
    pub winning_round: usize,
    /// Number of SWAPs inserted in the winning forward pass.
    pub winning_swaps: usize,
    /// Critical-path DAG depth in the winning forward pass.
    pub winning_depth: usize,
    /// Composite cost score of the winning forward pass.
    pub winning_cost: f64,
}

impl RefinementSummary {
    /// Constructs a new refinement summary.
    pub fn new(
        rounds: Vec<RefinementRoundRecord>,
        selected_initial_layout: LayoutMap,
        selected_final_layout: LayoutMap,
        winning_round: usize,
        winning_swaps: usize,
        winning_depth: usize,
        winning_cost: f64,
    ) -> Self {
        Self {
            rounds,
            selected_initial_layout,
            selected_final_layout,
            winning_round,
            winning_swaps,
            winning_depth,
            winning_cost,
        }
    }

    /// Returns the total number of refinement rounds executed.
    #[inline]
    pub fn total_rounds(&self) -> usize {
        self.rounds.len()
    }

    /// Returns `true` if any refinement round improved upon the initial pass (round 1).
    #[inline]
    pub fn improved_upon_initial(&self) -> bool {
        self.winning_round > 1
    }
}

/// Evaluates whether candidate metrics improve upon baseline metrics.
///
/// # Acceptance Semantics & Decision Hierarchy
/// Functions as an **explicit acceptance predicate** (not a sorting comparator) governed by a
/// hierarchical decision rule strictly dominated by the primary composite weighted cost objective:
///
/// 1. **Primary Weighted Cost**:
///    - If either candidate cost or baseline cost is non-finite (NaN or infinite), the candidate
///      is safely rejected (`false`).
///    - If `|cand_cost - base_cost| > 1e-12`, the candidate is accepted if and only if
///      `cand_cost < base_cost`. Under no circumstances can a candidate with worse composite
///      cost be accepted, even if individual secondary metrics (e.g. SWAPs or depth) improve.
///
/// 2. **Secondary Deterministic Tie-Breakers** (when `|cand_cost - base_cost| <= 1e-12`):
///    When composite costs are numerically equivalent within `1e-12` float tolerance:
///    - **Tie-Breaker 1**: Strictly fewer inserted routing SWAPs (`routing_swaps_count`).
///    - **Tie-Breaker 2**: Strictly lower critical-path DAG depth (`circuit_depth`).
///    - **Tie-Breaker 3**: Strictly lower two-qubit DAG depth (`two_qubit_depth`).
///
/// If all four metrics are equal (or candidate is worse in tie-breakers), returns `false`.
/// This strict anti-reflexivity ensures that equal or near-equal rounds never overwrite earlier
/// winning passes.
pub fn is_refinement_improvement(
    candidate_metrics: &RoutingMetrics,
    baseline_metrics: &RoutingMetrics,
) -> bool {
    let cand_cost = candidate_metrics.weighted_cost;
    let base_cost = baseline_metrics.weighted_cost;

    if !cand_cost.is_finite() || !base_cost.is_finite() {
        return false;
    }

    let diff = cand_cost - base_cost;
    if diff.abs() > 1e-12 {
        return diff < 0.0;
    }

    // Tie-breaker 1: Fewer inserted SWAP operations
    if candidate_metrics.routing_swaps_count != baseline_metrics.routing_swaps_count {
        return candidate_metrics.routing_swaps_count < baseline_metrics.routing_swaps_count;
    }

    // Tie-breaker 2: Lower critical-path DAG depth
    if candidate_metrics.circuit_depth != baseline_metrics.circuit_depth {
        return candidate_metrics.circuit_depth < baseline_metrics.circuit_depth;
    }

    // Tie-breaker 3: Lower two-qubit critical-path depth
    candidate_metrics.two_qubit_depth < baseline_metrics.two_qubit_depth
}
