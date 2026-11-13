//! Deterministic candidate ranking, Pareto dominance analysis, tie-breaking, and explainability reporting.
//!
//! Roadmap v3.1, Milestone M9 (Phase 76).
//!
//! Evaluates candidate routes against the declared objective function, enforces mandatory hard
//! constraints (TargetLegality, SemanticEquivalence, AncillaZeroState) as strict filters,
//! performs deterministic Pareto dominance analysis and non-dominated sorting, applies a
//! multi-tier deterministic tie-breaking hierarchy ending in canonical candidate IDs, and generates
//! structured explainability reports.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::routing::placement::PlacementStrategy;
use crate::routing::search::evaluator::{
    CandidateMetricsSnapshot, CandidateObjectiveScore, ObjectiveEvaluator,
};
use crate::routing::search::objective::{ObjectiveFunction, ObjectiveMetric, RoutingObjective};
use crate::routing::search::runner::{
    CandidateExecutionRecord, CandidateExecutionStatus, CheckStatus,
};
use crate::target::model::Target;

/// Eligibility status of a candidate for objective ranking and selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateRankEligibility {
    /// Candidate passed routing execution, all mandatory hard constraints, and objective evaluation.
    Eligible,
    /// Candidate was disqualified from ranking.
    Ineligible {
        /// Reason for disqualification.
        reason: String,
    },
}

impl CandidateRankEligibility {
    /// Returns `true` if the candidate is fully eligible for ranking and selection.
    #[inline]
    pub fn is_eligible(&self) -> bool {
        matches!(self, Self::Eligible)
    }
}

/// Comprehensive evaluation and ranking record for a single candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CandidateEvaluationRecord {
    /// Canonical collision-resistant candidate ID.
    pub candidate_id: String,
    /// Evaluation sequence index within the search batch.
    pub candidate_index: usize,
    /// Whether this candidate is the designated deterministic baseline candidate.
    pub is_baseline: bool,
    /// Randomization seed used, if applicable.
    pub seed: Option<u64>,
    /// Initial placement heuristic strategy.
    pub placement_strategy: PlacementStrategy,
    /// Captured quantitative metrics snapshot (if routing succeeded).
    pub metrics: Option<CandidateMetricsSnapshot>,
    /// Eligibility status (Eligible vs Ineligible).
    pub eligibility: CandidateRankEligibility,
    /// Evaluated objective score (if eligible).
    pub score: Option<CandidateObjectiveScore>,
    /// Pareto non-dominated front index (0 is best/non-dominated; None if ineligible).
    pub pareto_front: Option<usize>,
    /// Overall 1-based rank among all evaluated candidates (None if ineligible).
    pub overall_rank: Option<usize>,
    /// Human-readable explanation of score, rank, selection, or rejection reason.
    pub explanation: String,
}

/// Complete candidate ranking and explainability report for a multi-candidate search batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RankingReport {
    /// Objective contract that governed this evaluation.
    pub objective: RoutingObjective,
    /// All candidate evaluation records in sorted rank order (eligible first, then ineligible).
    pub candidates: Vec<CandidateEvaluationRecord>,
    /// Canonical ID of the selected winner candidate, if one was chosen.
    pub selected_candidate_id: Option<String>,
    /// High-level justification for why the winner was selected or why selection failed.
    pub selection_reason: Option<String>,
    /// Total number of candidates evaluated.
    pub total_candidates: usize,
    /// Count of candidates that passed all mandatory constraints and were eligible for ranking.
    pub eligible_count: usize,
    /// Count of candidates disqualified from ranking.
    pub ineligible_count: usize,
    /// Number of distinct Pareto fronts formed by eligible candidates.
    pub pareto_fronts_count: usize,
}

impl RankingReport {
    /// Returns a reference to the selected winning candidate, if any.
    pub fn winner(&self) -> Option<&CandidateEvaluationRecord> {
        if let Some(id) = &self.selected_candidate_id {
            self.candidates.iter().find(|c| &c.candidate_id == id)
        } else {
            self.candidates.iter().find(|c| c.overall_rank == Some(1))
        }
    }

    /// Finds a candidate evaluation record by its canonical candidate ID.
    pub fn find_candidate(&self, id: &str) -> Option<&CandidateEvaluationRecord> {
        self.candidates.iter().find(|c| c.candidate_id == id)
    }

    /// Formats a human-readable ASCII summary table of candidate rankings.
    pub fn format_table(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "=== Multi-Candidate Ranking Report ({}/{} eligible, {} fronts) ===\n",
            self.eligible_count, self.total_candidates, self.pareto_fronts_count
        ));
        s.push_str(&format!("Objective: {:?}\n", self.objective.function));
        if let Some(winner) = &self.selected_candidate_id {
            s.push_str(&format!(
                "Winner: {} ({})\n\n",
                winner,
                self.selection_reason.as_deref().unwrap_or("")
            ));
        } else {
            s.push_str("Winner: None (no valid candidates)\n\n");
        }

        s.push_str(&format!(
            "{:<5} {:<36} {:<8} {:<7} {:<16} {:<8} {:<6} {:<6} {:<8} {}\n",
            "Rank",
            "Candidate ID",
            "Base",
            "Front",
            "Score",
            "Swaps",
            "CX",
            "Depth",
            "Duration",
            "Status / Notes"
        ));
        s.push_str(&format!("{}\n", "-".repeat(120)));

        for cand in &self.candidates {
            let rank_str = cand
                .overall_rank
                .map(|r| format!("#{r}"))
                .unwrap_or_else(|| "-".to_string());
            let base_str = if cand.is_baseline { "YES" } else { "no" };
            let front_str = cand
                .pareto_front
                .map(|f| format!("F{f}"))
                .unwrap_or_else(|| "-".to_string());
            let score_str = cand
                .score
                .as_ref()
                .map(|sc| format!("{sc}"))
                .unwrap_or_else(|| "-".to_string());

            let (swaps_str, cx_str, depth_str, dur_str) = if let Some(m) = &cand.metrics {
                (
                    m.swap_count.to_string(),
                    m.cx_count.to_string(),
                    m.circuit_depth.to_string(),
                    m.estimated_duration_ns
                        .map(|d| format!("{d:.1}ns"))
                        .unwrap_or_else(|| "uncal".to_string()),
                )
            } else {
                ("-".into(), "-".into(), "-".into(), "-".into())
            };

            s.push_str(&format!(
                "{:<5} {:<36} {:<8} {:<7} {:<16} {:<8} {:<6} {:<6} {:<8} {}\n",
                rank_str,
                cand.candidate_id,
                base_str,
                front_str,
                score_str,
                swaps_str,
                cx_str,
                depth_str,
                dur_str,
                cand.explanation
            ));
        }

        s
    }
}

impl fmt::Display for RankingReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_table())
    }
}

/// Deterministic candidate ranking and evaluation engine.
pub struct RankingEngine;

impl RankingEngine {
    /// Evaluates, ranks, and analyzes Pareto dominance for all candidate records under the given objective.
    pub fn evaluate_and_rank(
        candidates: &[CandidateExecutionRecord],
        objective: &RoutingObjective,
        target: &Target,
    ) -> RankingReport {
        let total_candidates = candidates.len();

        // 1. Initial evaluation and eligibility determination
        let mut initial_evaluations: Vec<(CandidateEvaluationRecord, Option<Vec<f64>>)> =
            Vec::with_capacity(total_candidates);

        // Determine active metrics for Pareto dominance
        let active_metrics = Self::resolve_active_metrics(&objective.function);

        for cand in candidates {
            let candidate_id = cand.descriptor.identity.id.clone();
            let candidate_index = cand.descriptor.identity.candidate_index;
            let is_baseline = cand.is_baseline;
            let seed = cand.descriptor.identity.seed;
            let placement_strategy = cand.descriptor.identity.placement_strategy;

            // Check execution status
            match &cand.status {
                CandidateExecutionStatus::Success {
                    routed_circuit,
                    report,
                } => {
                    let metrics = CandidateMetricsSnapshot::from_circuit_and_report(
                        routed_circuit,
                        report,
                        target,
                    );

                    // Check mandatory hard constraints
                    let hard_constraint_disqualification =
                        Self::check_hard_constraints(&cand.validation);

                    if let Some(disq_reason) = hard_constraint_disqualification {
                        initial_evaluations.push((
                            CandidateEvaluationRecord {
                                candidate_id,
                                candidate_index,
                                is_baseline,
                                seed,
                                placement_strategy,
                                metrics: Some(metrics),
                                eligibility: CandidateRankEligibility::Ineligible {
                                    reason: disq_reason.clone(),
                                },
                                score: None,
                                pareto_front: None,
                                overall_rank: None,
                                explanation: format!("Disqualified: {disq_reason}"),
                            },
                            None,
                        ));
                    } else {
                        // Candidate passed hard constraints; evaluate objective score
                        match ObjectiveEvaluator::evaluate(&metrics, &objective.function) {
                            Ok(score) => {
                                // Extract metric vector for Pareto dominance
                                let mut metric_vector = Vec::with_capacity(active_metrics.len());
                                let mut vector_err = None;
                                for m in &active_metrics {
                                    match metrics.get_metric(m) {
                                        Ok(v) => metric_vector.push(v),
                                        Err(e) => {
                                            vector_err = Some(e);
                                            break;
                                        }
                                    }
                                }

                                if let Some(e) = vector_err {
                                    let reason = format!("Objective metric unavailable: {e}");
                                    initial_evaluations.push((
                                        CandidateEvaluationRecord {
                                            candidate_id,
                                            candidate_index,
                                            is_baseline,
                                            seed,
                                            placement_strategy,
                                            metrics: Some(metrics),
                                            eligibility: CandidateRankEligibility::Ineligible {
                                                reason: reason.clone(),
                                            },
                                            score: None,
                                            pareto_front: None,
                                            overall_rank: None,
                                            explanation: format!("Disqualified: {reason}"),
                                        },
                                        None,
                                    ));
                                } else {
                                    initial_evaluations.push((
                                        CandidateEvaluationRecord {
                                            candidate_id,
                                            candidate_index,
                                            is_baseline,
                                            seed,
                                            placement_strategy,
                                            metrics: Some(metrics),
                                            eligibility: CandidateRankEligibility::Eligible,
                                            score: Some(score),
                                            pareto_front: None,
                                            overall_rank: None,
                                            explanation: String::new(),
                                        },
                                        Some(metric_vector),
                                    ));
                                }
                            }
                            Err(e) => {
                                let reason = format!("Objective evaluation failed: {e}");
                                initial_evaluations.push((
                                    CandidateEvaluationRecord {
                                        candidate_id,
                                        candidate_index,
                                        is_baseline,
                                        seed,
                                        placement_strategy,
                                        metrics: Some(metrics),
                                        eligibility: CandidateRankEligibility::Ineligible {
                                            reason: reason.clone(),
                                        },
                                        score: None,
                                        pareto_front: None,
                                        overall_rank: None,
                                        explanation: format!("Disqualified: {reason}"),
                                    },
                                    None,
                                ));
                            }
                        }
                    }
                }
                CandidateExecutionStatus::Failed { error, .. } => {
                    let reason = format!("Routing execution failed: {error}");
                    initial_evaluations.push((
                        CandidateEvaluationRecord {
                            candidate_id,
                            candidate_index,
                            is_baseline,
                            seed,
                            placement_strategy,
                            metrics: None,
                            eligibility: CandidateRankEligibility::Ineligible {
                                reason: reason.clone(),
                            },
                            score: None,
                            pareto_front: None,
                            overall_rank: None,
                            explanation: format!("Disqualified: {reason}"),
                        },
                        None,
                    ));
                }
                CandidateExecutionStatus::SkippedBudgetExceeded { reason } => {
                    let reason_str = format!("Skipped due to budget exhaustion: {reason:?}");
                    initial_evaluations.push((
                        CandidateEvaluationRecord {
                            candidate_id,
                            candidate_index,
                            is_baseline,
                            seed,
                            placement_strategy,
                            metrics: None,
                            eligibility: CandidateRankEligibility::Ineligible {
                                reason: reason_str.clone(),
                            },
                            score: None,
                            pareto_front: None,
                            overall_rank: None,
                            explanation: format!("Disqualified: {reason_str}"),
                        },
                        None,
                    ));
                }
            }
        }

        // 2. Separate eligible from ineligible candidates
        let mut eligible_records: Vec<(CandidateEvaluationRecord, Vec<f64>)> = Vec::new();
        let mut ineligible_records: Vec<CandidateEvaluationRecord> = Vec::new();

        for (record, metric_vector) in initial_evaluations {
            if record.eligibility.is_eligible() {
                eligible_records.push((record, metric_vector.unwrap()));
            } else {
                ineligible_records.push(record);
            }
        }

        let eligible_count = eligible_records.len();
        let ineligible_count = ineligible_records.len();

        // 3. Compute Pareto dominance and fronts over eligible candidates
        let mut pareto_fronts_count = 0;
        if !eligible_records.is_empty() {
            let fronts = Self::compute_pareto_fronts(&eligible_records);
            pareto_fronts_count = fronts.len();

            for (front_idx, front_members) in fronts.iter().enumerate() {
                for &member_idx in front_members {
                    eligible_records[member_idx].0.pareto_front = Some(front_idx);
                }
            }
        }

        // 4. Deterministic total ordering among eligible candidates:
        //    Tier 1: Score (lower is better)
        //    Tier 2: Pareto front (lower front is better)
        //    Tier 3: Designated baseline preference (is_baseline == true first)
        //    Tier 4: Canonical candidate_id alphabetical comparison
        eligible_records.sort_by(|(a, _), (b, _)| {
            let score_ord = match (&a.score, &b.score) {
                (Some(sa), Some(sb)) => sa.cmp_score(sb),
                _ => std::cmp::Ordering::Equal,
            };
            if score_ord != std::cmp::Ordering::Equal {
                return score_ord;
            }

            let front_ord = a.pareto_front.cmp(&b.pareto_front);
            if front_ord != std::cmp::Ordering::Equal {
                return front_ord;
            }

            // Prefer baseline candidate on exact score and front tie
            let base_ord = (b.is_baseline).cmp(&(a.is_baseline));
            if base_ord != std::cmp::Ordering::Equal {
                return base_ord;
            }

            // Final deterministic tie-breaker: canonical candidate ID
            a.candidate_id.cmp(&b.candidate_id)
        });

        // Assign overall 1-based ranks and explanations for eligible candidates
        let mut sorted_candidates: Vec<CandidateEvaluationRecord> =
            Vec::with_capacity(total_candidates);

        for (rank_idx, (mut record, _)) in eligible_records.into_iter().enumerate() {
            let rank = rank_idx + 1;
            record.overall_rank = Some(rank);

            let front = record.pareto_front.unwrap_or(0);
            let score_str = record
                .score
                .as_ref()
                .map(|sc| format!("{sc}"))
                .unwrap_or_else(|| "unknown".to_string());

            if rank == 1 {
                let tie_note = if record.is_baseline {
                    " [designated baseline]"
                } else {
                    ""
                };
                record.explanation = format!(
                    "Selected: Rank #1 (Pareto Front {front}, score {score_str}){tie_note}"
                );
            } else {
                record.explanation =
                    format!("Rank #{rank} (Pareto Front {front}, score {score_str})");
            }

            sorted_candidates.push(record);
        }

        // Sort ineligible candidates deterministically by original candidate_index
        ineligible_records.sort_by_key(|c| c.candidate_index);
        sorted_candidates.extend(ineligible_records);

        // 5. Determine overall winner and selection reason
        let (selected_candidate_id, selection_reason) = if let Some(winner) =
            sorted_candidates.first()
        {
            if winner.overall_rank == Some(1) {
                (
                    Some(winner.candidate_id.clone()),
                    Some(winner.explanation.clone()),
                )
            } else {
                (
                    None,
                    Some(
                        "No candidates passed all mandatory validation checks and objective requirements"
                            .to_string(),
                    ),
                )
            }
        } else {
            (
                None,
                Some("Zero candidates generated in search batch".to_string()),
            )
        };

        RankingReport {
            objective: objective.clone(),
            candidates: sorted_candidates,
            selected_candidate_id,
            selection_reason,
            total_candidates,
            eligible_count,
            ineligible_count,
            pareto_fronts_count,
        }
    }

    /// Resolves the active metrics evaluated for Pareto dominance.
    fn resolve_active_metrics(function: &ObjectiveFunction) -> Vec<ObjectiveMetric> {
        match function {
            ObjectiveFunction::Single(m) => vec![m.clone()],
            ObjectiveFunction::Lexicographic(ms) => ms.clone(),
            ObjectiveFunction::Weighted(pairs) => {
                let positive_weight_metrics: Vec<ObjectiveMetric> = pairs
                    .iter()
                    .filter(|(_, w)| *w > 0.0)
                    .map(|(m, _)| m.clone())
                    .collect();
                if positive_weight_metrics.is_empty() {
                    pairs.iter().map(|(m, _)| m.clone()).collect()
                } else {
                    positive_weight_metrics
                }
            }
        }
    }

    /// Verifies all three mandatory hard constraints against the candidate validation record.
    ///
    /// Returns `Some(reason)` if any constraint failed or was unexecuted, or `None` if all passed.
    fn check_hard_constraints(
        validation: &crate::routing::search::runner::CandidateValidationRecord,
    ) -> Option<String> {
        match &validation.target_legality {
            CheckStatus::Failed { reason } => {
                return Some(format!(
                    "Mandatory hard constraint 'TargetLegality' failed: {reason}"
                ));
            }
            CheckStatus::Unexecuted { reason } => {
                return Some(format!(
                    "Mandatory hard constraint 'TargetLegality' unexecuted: {reason}"
                ));
            }
            CheckStatus::Passed => {}
        }

        match &validation.semantic_equivalence {
            CheckStatus::Failed { reason } => {
                return Some(format!(
                    "Mandatory hard constraint 'SemanticEquivalence' failed: {reason}"
                ));
            }
            CheckStatus::Unexecuted { reason } => {
                return Some(format!(
                    "Mandatory hard constraint 'SemanticEquivalence' unexecuted: {reason}"
                ));
            }
            CheckStatus::Passed => {}
        }

        match &validation.ancilla_zero_state {
            CheckStatus::Failed { reason } => {
                return Some(format!(
                    "Mandatory hard constraint 'AncillaZeroState' (ADR-0001) failed: {reason}"
                ));
            }
            CheckStatus::Unexecuted { reason } => {
                return Some(format!(
                    "Mandatory hard constraint 'AncillaZeroState' (ADR-0001) unexecuted: {reason}"
                ));
            }
            CheckStatus::Passed => {}
        }

        None
    }

    /// Computes Pareto non-dominated fronts using the standard fast non-dominated sorting algorithm.
    ///
    /// Minimization direction: candidate $A$ dominates candidate $B$ ($A \succ B$) iff:
    /// $\forall i, A_i \le B_i$ AND $\exists j, A_j < B_j$.
    fn compute_pareto_fronts(
        eligible: &[(CandidateEvaluationRecord, Vec<f64>)],
    ) -> Vec<Vec<usize>> {
        let n = eligible.len();
        if n == 0 {
            return Vec::new();
        }

        let mut dominated_by: Vec<usize> = vec![0; n];
        let mut dominates_list: Vec<Vec<usize>> = vec![Vec::new(); n];

        for (i, (_, vec_i)) in eligible.iter().enumerate() {
            for (j, (_, vec_j)) in eligible.iter().enumerate() {
                if i == j {
                    continue;
                }

                if Self::dominates(vec_i, vec_j) {
                    dominates_list[i].push(j);
                } else if Self::dominates(vec_j, vec_i) {
                    dominated_by[i] += 1;
                }
            }
        }

        let mut fronts: Vec<Vec<usize>> = Vec::new();
        let mut current_front: Vec<usize> = Vec::new();

        for (i, &count) in dominated_by.iter().enumerate() {
            if count == 0 {
                current_front.push(i);
            }
        }

        while !current_front.is_empty() {
            let mut next_front = Vec::new();
            for &p in &current_front {
                for &q in &dominates_list[p] {
                    dominated_by[q] = dominated_by[q].saturating_sub(1);
                    if dominated_by[q] == 0 {
                        next_front.push(q);
                    }
                }
            }
            fronts.push(current_front);
            current_front = next_front;
        }

        fronts
    }

    /// Returns `true` if vector $a$ strictly dominates vector $b$ (minimization).
    #[inline]
    fn dominates(a: &[f64], b: &[f64]) -> bool {
        let mut strictly_better = false;
        for (va, vb) in a.iter().zip(b.iter()) {
            match va.total_cmp(vb) {
                std::cmp::Ordering::Greater => return false,
                std::cmp::Ordering::Less => strictly_better = true,
                std::cmp::Ordering::Equal => {}
            }
        }
        strictly_better
    }
}
