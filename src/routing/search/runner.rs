//! Multi-run SABRE candidate generation and execution engine (Phase 75).
//!
//! Orchestrates bounded, deterministic multi-candidate routing search over SABRE seeds
//! and initial layout configurations with candidate isolation, cumulative budget enforcement,
//! independent validation evidence, and fallback handling.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::routing::sabre::{SabreBidirectionalRouter, SabreRouterReport};
use crate::routing::search::candidate::CandidateDescriptor;
use crate::routing::search::config::{FallbackPolicy, MultiCandidateRoutingConfig};
use crate::routing::search::error::RoutingSearchExecutionError;
use crate::target::legality::TargetLegalityValidator;
use crate::target::model::Target;
use crate::verification::layout_equivalence::LayoutEquivalenceChecker;

/// Detailed outcome of a single validation check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    /// Check was executed and passed with full verification evidence.
    Passed,
    /// Check was executed and explicitly failed.
    Failed {
        /// Reason or diagnostic message for the failure.
        reason: String,
    },
    /// Check was not executed / skipped due to documented hardware or boundary limits.
    Unexecuted {
        /// Explanation why the check was unexecuted (e.g. target width exceeds statevector limit).
        reason: String,
    },
}

impl CheckStatus {
    /// Returns `true` if this check explicitly passed with verification evidence.
    #[inline]
    pub fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

/// Independent validation record for all three mandatory hard constraints established in Phase 74.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateValidationRecord {
    /// Target physical gate legality and coupling directionality (`TargetLegalityValidator`).
    pub target_legality: CheckStatus,
    /// Layout-aware statevector unitary equivalence under a common global phase.
    pub semantic_equivalence: CheckStatus,
    /// ADR-0001 clean ancilla zero-state preservation on all unused physical qubits.
    pub ancilla_zero_state: CheckStatus,
}

impl CandidateValidationRecord {
    /// A candidate is eligible for selection ONLY when all three mandatory checks are genuinely `Passed`.
    ///
    /// If any check is `Failed` or `Unexecuted`, returns `false`.
    #[inline]
    pub fn is_fully_validated(&self) -> bool {
        matches!(
            (
                &self.target_legality,
                &self.semantic_equivalence,
                &self.ancilla_zero_state
            ),
            (
                CheckStatus::Passed,
                CheckStatus::Passed,
                CheckStatus::Passed
            )
        )
    }

    /// Creates an empty validation record where all checks are unexecuted.
    pub fn unexecuted(reason: impl Into<String>) -> Self {
        let r = reason.into();
        Self {
            target_legality: CheckStatus::Unexecuted { reason: r.clone() },
            semantic_equivalence: CheckStatus::Unexecuted { reason: r.clone() },
            ancilla_zero_state: CheckStatus::Unexecuted { reason: r },
        }
    }

    /// Creates a validation record where all checks are failed.
    pub fn failed(reason: impl Into<String>) -> Self {
        let r = reason.into();
        Self {
            target_legality: CheckStatus::Failed { reason: r.clone() },
            semantic_equivalence: CheckStatus::Failed { reason: r.clone() },
            ancilla_zero_state: CheckStatus::Failed { reason: r },
        }
    }
}

/// Structured exhaustion reasons for multi-candidate search budgets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetExhaustionReason {
    /// Reached the maximum configured candidate count.
    MaxCandidatesReached(usize),
    /// Cumulative physical SWAP insertions exceeded `max_total_swaps`.
    MaxTotalSwapsExceeded {
        /// Configured limit.
        limit: usize,
        /// Actual cumulative swaps attempted or reached.
        actual: usize,
    },
    /// Cumulative scheduling iterations exceeded `max_total_iterations`.
    MaxTotalIterationsExceeded {
        /// Configured limit.
        limit: usize,
        /// Actual cumulative iterations reached.
        actual: usize,
    },
    /// Cooperative wall-clock timeout elapsed between candidate executions.
    Timeout {
        /// Configured timeout in milliseconds.
        timeout_ms: u64,
        /// Actual elapsed wall-clock milliseconds.
        elapsed_ms: u64,
    },
}

/// Execution outcome of an individual routing candidate.
#[derive(Clone, Debug, PartialEq)]
pub enum CandidateExecutionStatus {
    /// Candidate executed successfully to circuit completion.
    Success {
        /// Resulting routed physical circuit.
        routed_circuit: Box<Circuit>,
        /// Detailed SABRE report from this run.
        report: Box<SabreRouterReport>,
    },
    /// Candidate routing failed (e.g. swap limit, iteration limit, disconnected topology).
    Failed {
        /// Diagnostic error description.
        error: String,
        /// Partial physical SWAP operations emitted before failure occurred.
        partial_swaps: usize,
        /// Partial scheduling iterations executed before failure occurred.
        partial_iterations: usize,
    },
    /// Candidate execution was skipped because a search budget or timeout was exhausted before starting.
    SkippedBudgetExceeded {
        /// Reason why this candidate was skipped.
        reason: BudgetExhaustionReason,
    },
}

impl CandidateExecutionStatus {
    /// Returns `true` if this candidate completed successfully.
    #[inline]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }
}

/// Full execution record and provenance for a single routing candidate.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateExecutionRecord {
    /// Candidate descriptor and identity from Phase 74.
    pub descriptor: CandidateDescriptor,
    /// Whether this candidate is the designated deterministic baseline candidate.
    pub is_baseline: bool,
    /// Execution status (Success, Failed, Skipped).
    pub status: CandidateExecutionStatus,
    /// Independent validation outcomes for the three mandatory hard constraints.
    pub validation: CandidateValidationRecord,
    /// Wall-clock time spent executing this candidate.
    pub duration: Duration,
    /// Swaps inserted in the winning forward pass (0 if failed/skipped).
    pub swaps_inserted: usize,
    /// Total swaps executed during this candidate's run (including reverse passes).
    pub total_swaps_executed: usize,
    /// Cumulative swaps across all candidates completed up to and including this one.
    pub cumulative_swaps: usize,
    /// Total scheduling iterations executed during this candidate's run.
    pub total_iterations: usize,
    /// Cumulative scheduling iterations across all candidates up to and including this one.
    pub cumulative_iterations: usize,
}

/// Aggregated report of the entire multi-candidate execution batch.
#[derive(Clone, Debug, PartialEq)]
pub struct MultiCandidateExecutionReport {
    /// All candidate records in deterministic execution order.
    pub candidates: Vec<CandidateExecutionRecord>,
    /// Total candidate count requested.
    pub total_requested: usize,
    /// Count of candidates actually executed (successful or failed).
    pub total_executed: usize,
    /// Count of candidates that successfully completed routing.
    pub total_succeeded: usize,
    /// Count of candidates that passed all mandatory validation checks.
    pub total_validated: usize,
    /// Cumulative physical SWAP gates executed across all candidate passes combined.
    pub cumulative_swaps: usize,
    /// Cumulative scheduling loop iterations executed across all candidate passes combined.
    pub cumulative_iterations: usize,
    /// Total wall-clock time spent across the search batch.
    pub total_duration: Duration,
    /// Budget exhaustion details if the search was curtailed early.
    pub budget_exhaustion: Option<BudgetExhaustionReason>,
    /// Fallback policy that governed this search.
    pub fallback_policy: FallbackPolicy,
    /// Candidate ID of the provisional candidate selected for pipeline compilation, if any.
    pub provisional_candidate_id: Option<String>,
}

/// Provisional candidate selection for compilation pipelines.
#[derive(Clone, Debug, PartialEq)]
pub struct ProvisionalCandidateSelection<'a> {
    /// Canonical stable candidate ID of the selected candidate.
    pub candidate_id: &'a str,
    /// Reference to candidate descriptor and configuration.
    pub descriptor: &'a CandidateDescriptor,
    /// Reference to the routed physical circuit.
    pub routed_circuit: &'a Circuit,
    /// Reference to the SABRE router report.
    pub sabre_report: &'a SabreRouterReport,
    /// Full candidate execution record.
    pub record: &'a CandidateExecutionRecord,
}

impl MultiCandidateExecutionReport {
    /// Evaluates the Phase 75 deterministic provisional selection policy:
    ///
    /// 1. Prefer the designated baseline candidate (`is_baseline: true`), provided
    ///    `candidate.validation.is_fully_validated() == true`.
    /// 2. Otherwise, select the first candidate in deterministic execution order
    ///    that satisfies `candidate.validation.is_fully_validated() == true`.
    /// 3. If no candidate is fully validated, returns `RoutingSearchExecutionError::NoValidCandidates`.
    ///
    /// Returns the selected candidate reference explicitly keyed by stable `candidate_id`.
    pub fn select_provisional_candidate(
        &self,
    ) -> Result<ProvisionalCandidateSelection<'_>, RoutingSearchExecutionError> {
        let selected_record = if let Some(baseline) = self
            .candidates
            .iter()
            .find(|c| c.is_baseline && c.validation.is_fully_validated())
        {
            baseline
        } else if let Some(first_valid) = self
            .candidates
            .iter()
            .find(|c| c.validation.is_fully_validated())
        {
            first_valid
        } else {
            return Err(RoutingSearchExecutionError::NoValidCandidates {
                total_executed: self.total_executed,
                report: Box::new(self.clone()),
            });
        };

        match &selected_record.status {
            CandidateExecutionStatus::Success {
                routed_circuit,
                report,
            } => Ok(ProvisionalCandidateSelection {
                candidate_id: &selected_record.descriptor.identity.id,
                descriptor: &selected_record.descriptor,
                routed_circuit,
                sabre_report: report,
                record: selected_record,
            }),
            _ => unreachable!("fully validated candidate must have Success status"),
        }
    }
}

/// Execution engine for multi-candidate routing search.
pub struct MultiCandidateSearchRunner<'a> {
    circuit: &'a Circuit,
    target: &'a Target,
    config: &'a MultiCandidateRoutingConfig,
}

impl<'a> MultiCandidateSearchRunner<'a> {
    /// Constructs a new runner for the given circuit, target, and configuration.
    pub fn new(
        circuit: &'a Circuit,
        target: &'a Target,
        config: &'a MultiCandidateRoutingConfig,
    ) -> Self {
        Self {
            circuit,
            target,
            config,
        }
    }

    /// Executes multi-candidate routing search, returning the complete batch report.
    ///
    /// # Errors
    /// Returns [`RoutingSearchExecutionError`] on budget exhaustion with `Abort`, invalid baseline
    /// fallback, or when zero candidates can be validated, preserving the full batch execution report.
    pub fn run(&self) -> Result<MultiCandidateExecutionReport, RoutingSearchExecutionError> {
        self.config
            .validate_with_target(self.target)
            .map_err(RoutingSearchExecutionError::Config)?;

        let descriptors = self
            .config
            .derive_candidate_descriptors(self.target)
            .map_err(RoutingSearchExecutionError::Config)?;

        let total_requested = descriptors.len();
        let start_time = Instant::now();

        let mut cumulative_swaps = 0usize;
        let mut cumulative_iterations = 0usize;
        let mut candidates = Vec::with_capacity(total_requested);
        let mut budget_exhaustion: Option<BudgetExhaustionReason> = None;

        for desc in descriptors {
            let is_baseline = desc.is_baseline;

            // 1. Cooperative between-candidate timeout check
            if let Some(timeout_ms) = self.config.budget.timeout_ms {
                let elapsed_ms = start_time.elapsed().as_millis() as u64;
                if elapsed_ms >= timeout_ms {
                    budget_exhaustion = Some(BudgetExhaustionReason::Timeout {
                        timeout_ms,
                        elapsed_ms,
                    });
                }
            }

            // 2. Cumulative swap budget check
            if budget_exhaustion.is_none() {
                if let Some(max_swaps) = self.config.budget.max_total_swaps {
                    if cumulative_swaps >= max_swaps {
                        budget_exhaustion = Some(BudgetExhaustionReason::MaxTotalSwapsExceeded {
                            limit: max_swaps,
                            actual: cumulative_swaps,
                        });
                    }
                }
            }

            // 3. Cumulative iteration budget check
            if budget_exhaustion.is_none() {
                if let Some(max_iters) = self.config.budget.max_total_iterations {
                    if cumulative_iterations >= max_iters {
                        budget_exhaustion =
                            Some(BudgetExhaustionReason::MaxTotalIterationsExceeded {
                                limit: max_iters,
                                actual: cumulative_iterations,
                            });
                    }
                }
            }

            // If budget was exhausted before this candidate started, record as skipped
            if let Some(reason) = &budget_exhaustion {
                candidates.push(CandidateExecutionRecord {
                    descriptor: desc,
                    is_baseline,
                    status: CandidateExecutionStatus::SkippedBudgetExceeded {
                        reason: reason.clone(),
                    },
                    validation: CandidateValidationRecord::unexecuted(format!(
                        "skipped due to budget exhaustion: {reason:?}"
                    )),
                    duration: Duration::ZERO,
                    swaps_inserted: 0,
                    total_swaps_executed: 0,
                    cumulative_swaps,
                    total_iterations: 0,
                    cumulative_iterations,
                });
                continue;
            }

            // Prepare specialized SABRE configuration clamped to remaining cumulative budgets
            let mut cand_cfg = desc.config.clone();
            if let Some(max_swaps) = self.config.budget.max_total_swaps {
                let rem_swaps = max_swaps.saturating_sub(cumulative_swaps);
                cand_cfg.max_swaps = cand_cfg.max_swaps.min(rem_swaps);
            }
            if let Some(max_iters) = self.config.budget.max_total_iterations {
                let rem_iters = max_iters.saturating_sub(cumulative_iterations);
                cand_cfg.max_iterations = cand_cfg.max_iterations.min(rem_iters);
            }
            cand_cfg.validate_target_legality = false;
            cand_cfg.verify_equivalence = false;

            // Execute candidate on isolated copy of circuit & target
            let cand_start = Instant::now();
            let router = SabreBidirectionalRouter::with_config(cand_cfg.clone());
            let route_res = router.route_circuit(
                self.circuit,
                self.target,
                desc.explicit_initial_layout.clone(),
            );
            let cand_duration = cand_start.elapsed();

            match route_res {
                Ok((routed, rep)) => {
                    let cand_swaps_inserted = rep.swaps_inserted;
                    let cand_swaps_executed = rep.total_swaps_executed;
                    let cand_iters = rep.total_iterations;

                    cumulative_swaps += cand_swaps_executed;
                    cumulative_iterations += cand_iters;

                    // Independent verification of the three mandatory constraints
                    let validation = self.validate_candidate(&routed, &rep);

                    candidates.push(CandidateExecutionRecord {
                        descriptor: desc,
                        is_baseline,
                        status: CandidateExecutionStatus::Success {
                            routed_circuit: Box::new(routed),
                            report: Box::new(rep),
                        },
                        validation,
                        duration: cand_duration,
                        swaps_inserted: cand_swaps_inserted,
                        total_swaps_executed: cand_swaps_executed,
                        cumulative_swaps,
                        total_iterations: cand_iters,
                        cumulative_iterations,
                    });
                }
                Err(err) => {
                    // Extract exact partial work from error
                    let (p_swaps, p_iters) = err.partial_work();

                    cumulative_swaps += p_swaps;
                    cumulative_iterations += p_iters;

                    candidates.push(CandidateExecutionRecord {
                        descriptor: desc,
                        is_baseline,
                        status: CandidateExecutionStatus::Failed {
                            error: err.to_string(),
                            partial_swaps: p_swaps,
                            partial_iterations: p_iters,
                        },
                        validation: CandidateValidationRecord::failed(err.to_string()),
                        duration: cand_duration,
                        swaps_inserted: 0,
                        total_swaps_executed: p_swaps,
                        cumulative_swaps,
                        total_iterations: p_iters,
                        cumulative_iterations,
                    });
                }
            }

            // Check if limits were breached by this candidate's execution
            if let Some(max_swaps) = self.config.budget.max_total_swaps {
                if cumulative_swaps >= max_swaps && budget_exhaustion.is_none() {
                    budget_exhaustion = Some(BudgetExhaustionReason::MaxTotalSwapsExceeded {
                        limit: max_swaps,
                        actual: cumulative_swaps,
                    });
                }
            }
            if let Some(max_iters) = self.config.budget.max_total_iterations {
                if cumulative_iterations >= max_iters && budget_exhaustion.is_none() {
                    budget_exhaustion = Some(BudgetExhaustionReason::MaxTotalIterationsExceeded {
                        limit: max_iters,
                        actual: cumulative_iterations,
                    });
                }
            }
        }

        let total_executed = candidates
            .iter()
            .filter(|c| {
                !matches!(
                    c.status,
                    CandidateExecutionStatus::SkippedBudgetExceeded { .. }
                )
            })
            .count();
        let total_succeeded = candidates.iter().filter(|c| c.status.is_success()).count();
        let total_validated = candidates
            .iter()
            .filter(|c| c.validation.is_fully_validated())
            .count();
        let total_duration = start_time.elapsed();

        let mut report = MultiCandidateExecutionReport {
            candidates,
            total_requested,
            total_executed,
            total_succeeded,
            total_validated,
            cumulative_swaps,
            cumulative_iterations,
            total_duration,
            budget_exhaustion,
            fallback_policy: self.config.fallback_policy,
            provisional_candidate_id: None,
        };

        // Fallback Resolution
        match self.config.fallback_policy {
            FallbackPolicy::Abort => {
                if report.budget_exhaustion.is_some()
                    || report.total_succeeded < report.total_requested
                {
                    let reason = if let Some(b) = &report.budget_exhaustion {
                        format!("budget exhausted: {b:?}")
                    } else {
                        format!(
                            "{}/{} candidates failed routing execution",
                            report.total_requested - report.total_succeeded,
                            report.total_requested
                        )
                    };
                    return Err(RoutingSearchExecutionError::Aborted {
                        reason,
                        report: Box::new(report),
                    });
                }
                // All succeeded: set provisional candidate
                let sel = report.select_provisional_candidate()?;
                report.provisional_candidate_id = Some(sel.candidate_id.to_string());
                Ok(report)
            }
            FallbackPolicy::DeterministicBaseline => {
                // Must find and validate the designated baseline candidate
                let baseline = report.candidates.iter().find(|c| c.is_baseline);
                match baseline {
                    Some(b) if b.validation.is_fully_validated() => {
                        report.provisional_candidate_id = Some(b.descriptor.identity.id.clone());
                        Ok(report)
                    }
                    Some(b) => Err(RoutingSearchExecutionError::InvalidBaselineFallback {
                        reason: match &b.status {
                            CandidateExecutionStatus::Success { .. } => {
                                "baseline candidate completed routing but failed mandatory validation constraints"
                                    .to_string()
                            }
                            CandidateExecutionStatus::Failed { error, .. } => {
                                format!("baseline candidate routing failed: {error}")
                            }
                            CandidateExecutionStatus::SkippedBudgetExceeded { reason } => {
                                format!("baseline candidate was skipped due to budget exhaustion: {reason:?}")
                            }
                        },
                        report: Box::new(report),
                    }),
                    None => Err(RoutingSearchExecutionError::InvalidBaselineFallback {
                        reason: "no deterministic baseline candidate was generated in candidate batch"
                            .to_string(),
                        report: Box::new(report),
                    }),
                }
            }
            FallbackPolicy::BestValidSoFar => {
                if report.total_validated > 0 {
                    let sel = report.select_provisional_candidate()?;
                    report.provisional_candidate_id = Some(sel.candidate_id.to_string());
                    Ok(report)
                } else if report.total_succeeded == 0 {
                    Err(RoutingSearchExecutionError::AllCandidatesFailed {
                        total: report.total_executed,
                        details: format!("all {total_executed} executed candidates failed routing"),
                        report: Box::new(report),
                    })
                } else {
                    Err(RoutingSearchExecutionError::NoValidCandidates {
                        total_executed: report.total_executed,
                        report: Box::new(report),
                    })
                }
            }
        }
    }

    /// Validates a routed physical circuit against all three mandatory constraints independently.
    fn validate_candidate(
        &self,
        routed_circuit: &Circuit,
        report: &SabreRouterReport,
    ) -> CandidateValidationRecord {
        // 1. Target Legality
        let validator = TargetLegalityValidator::new(self.target.clone());
        let target_legality = match validator.check_circuit(routed_circuit) {
            Ok(leg_rep) => {
                if leg_rep.is_valid() {
                    CheckStatus::Passed
                } else {
                    CheckStatus::Failed {
                        reason: leg_rep.summary(),
                    }
                }
            }
            Err(e) => CheckStatus::Failed {
                reason: e.to_string(),
            },
        };

        // 2 & 3. Semantic Equivalence and ADR-0001 Clean Ancillae
        let (semantic_equivalence, ancilla_zero_state) = if self.target.num_qubits() <= 16 {
            let checker = LayoutEquivalenceChecker::default();
            match checker.check_layout_equivalence_detailed(
                self.circuit,
                routed_circuit,
                &report.layout,
            ) {
                Ok(eq_rep) => {
                    let sem_stat = if eq_rep.logical_equivalence_passed {
                        CheckStatus::Passed
                    } else {
                        CheckStatus::Failed {
                            reason: format!(
                                "logical statevector mismatch: max diff = {}",
                                eq_rep.verdict.max_amplitude_difference
                            ),
                        }
                    };
                    let anc_stat = if eq_rep.ancilla_zero_state_passed {
                        CheckStatus::Passed
                    } else {
                        CheckStatus::Failed {
                            reason: format!(
                                "ADR-0001 ancilla zero-state violation: max leakage = {:.3e}",
                                eq_rep.max_ancilla_leakage
                            ),
                        }
                    };
                    (sem_stat, anc_stat)
                }
                Err(e) => {
                    let r = format!("equivalence check error: {e}");
                    (
                        CheckStatus::Failed { reason: r.clone() },
                        CheckStatus::Failed { reason: r },
                    )
                }
            }
        } else {
            let r = "statevector simulation limited to <= 16 physical qubits".to_string();
            (
                CheckStatus::Unexecuted { reason: r.clone() },
                CheckStatus::Unexecuted { reason: r },
            )
        };

        CandidateValidationRecord {
            target_legality,
            semantic_equivalence,
            ancilla_zero_state,
        }
    }
}
