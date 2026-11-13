//! Integration tests for Phase 76: Deterministic candidate ranking, objective evaluation,
//! Pareto dominance analysis, tie-breaking, and explainability reporting.
//!
//! Roadmap v3.1, Milestone M9 (Phase 76).

use std::time::Duration;

use qcc::builder::CircuitBuilder;
use qcc::ir::circuit::Circuit;
use qcc::routing::placement::PlacementStrategy;
use qcc::routing::sabre::{SabreBidirectionalRouter, SabreRouterConfig};
use qcc::routing::search::candidate::{
    compute_sabre_config_hash, derive_canonical_candidate_id, CandidateDescriptor,
    CandidateIdentity,
};
use qcc::routing::search::evaluator::{
    CandidateMetricsSnapshot, CandidateObjectiveScore, MetricEvaluationError, ObjectiveEvaluator,
};
use qcc::routing::search::objective::{
    HardConstraint, ObjectiveFunction, ObjectiveMetric, RoutingObjective,
};
use qcc::routing::search::ranking::RankingEngine;
use qcc::routing::search::runner::{
    CandidateExecutionRecord, CandidateExecutionStatus, CandidateValidationRecord, CheckStatus,
    MultiCandidateSearchRunner,
};
use qcc::routing::search::{FallbackPolicy, MultiCandidateRoutingConfig, SearchBudget};
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::metrics::DurationCost;
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};
use qcc::target::timing::{OperationDuration, TimingModel};

/// Builds a 5-qubit linear target: 0 - 1 - 2 - 3 - 4.
fn make_5q_linear_target() -> Target {
    let mut builder = TargetBuilder::new("linear_5q", 5);
    for gate in ["rx", "ry", "rz", "h", "x", "sx"] {
        builder = builder.add_operation(
            TargetOperationBuilder::new(gate, 1)
                .connectivity(OperationConnectivity::all_single_qubits(5))
                .build(),
        );
    }
    let couplings = vec![
        (PhysicalQubitId(0), PhysicalQubitId(1)),
        (PhysicalQubitId(1), PhysicalQubitId(2)),
        (PhysicalQubitId(2), PhysicalQubitId(3)),
        (PhysicalQubitId(3), PhysicalQubitId(4)),
    ];
    builder = builder.add_operation(
        TargetOperationBuilder::new("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(couplings))
            .build(),
    );
    builder.build().expect("build 5q target")
}

/// Builds a 5-qubit linear target with fully calibrated hardware timing.
fn make_5q_timed_target() -> Target {
    let mut target = make_5q_linear_target();
    let mut timing = TimingModel::new().with_clock_period_ns(1.0);
    timing.set_default_duration("rz", OperationDuration::known_ns(0.0));
    timing.set_default_duration("sx", OperationDuration::known_ns(20.0));
    timing.set_default_duration("x", OperationDuration::known_ns(20.0));
    timing.set_default_duration("rx", OperationDuration::known_ns(20.0));
    timing.set_default_duration("ry", OperationDuration::known_ns(20.0));
    timing.set_default_duration("h", OperationDuration::known_ns(20.0));
    timing.set_default_duration("cx", OperationDuration::known_ns(100.0));
    target.timing = Some(timing);
    target
}

/// Builds a 4-qubit complete graph (K4) circuit requiring routing SWAPs on a line.
fn make_k4_circuit() -> Circuit {
    let mut builder = CircuitBuilder::new("k4");
    let q = builder.add_quantum_register("q", 4).unwrap();
    for i in 0..4 {
        for j in (i + 1)..4 {
            builder.cx(q[i], q[j]).unwrap();
        }
    }
    builder.finish().unwrap()
}

/// Builds a synthetic CandidateExecutionRecord for unit testing ranking logic.
fn make_synthetic_candidate(
    index: usize,
    is_baseline: bool,
    swaps: usize,
    cx: usize,
    depth: usize,
    duration_ns: Option<f64>,
    fully_valid: bool,
) -> CandidateExecutionRecord {
    let target = make_5q_linear_target();
    let router_cfg = SabreRouterConfig::default();
    let r_hash = compute_sabre_config_hash(&router_cfg);
    let t_hash = "0".repeat(64);
    let id = derive_canonical_candidate_id(
        index,
        Some(42 + index as u64),
        PlacementStrategy::Degree,
        None,
        &r_hash,
        &t_hash,
    );

    let descriptor = CandidateDescriptor {
        identity: CandidateIdentity {
            candidate_index: index,
            seed: Some(42 + index as u64),
            placement_strategy: PlacementStrategy::Degree,
            explicit_layout_index: None,
            router_options_hash: r_hash,
            target_hash: t_hash,
            id: id.clone(),
        },
        config: router_cfg,
        explicit_initial_layout: None,
        is_baseline,
    };

    let validation = if fully_valid {
        CandidateValidationRecord {
            target_legality: CheckStatus::Passed,
            semantic_equivalence: CheckStatus::Passed,
            ancilla_zero_state: CheckStatus::Passed,
        }
    } else {
        CandidateValidationRecord {
            target_legality: CheckStatus::Passed,
            semantic_equivalence: CheckStatus::Failed {
                reason: "injected statevector mismatch".to_string(),
            },
            ancilla_zero_state: CheckStatus::Passed,
        }
    };

    // Construct mock circuit & report
    let mut builder = CircuitBuilder::new(format!("cand_{index}"));
    let q = builder.add_quantum_register("q", 4).unwrap();
    for _ in 0..cx {
        builder.cx(q[0], q[1]).unwrap();
    }
    let circ = builder.finish().unwrap();

    let router = SabreBidirectionalRouter::default();
    let (routed, mut report) = router.route_circuit(&circ, &target, None).unwrap();

    report.swaps_inserted = swaps;
    report.metrics.routing_swaps_count = swaps;
    report.metrics.circuit_depth = depth;
    if let Some(dur) = duration_ns {
        report.metrics.duration = DurationCost::Exact {
            total_ns: dur,
            is_measured_only: true,
        };
    } else {
        report.metrics.duration = DurationCost::Unknown;
    }

    CandidateExecutionRecord {
        descriptor,
        is_baseline,
        status: CandidateExecutionStatus::Success {
            routed_circuit: Box::new(routed),
            report: Box::new(report),
        },
        validation,
        duration: Duration::from_millis(10),
        swaps_inserted: swaps,
        total_swaps_executed: swaps,
        cumulative_swaps: swaps,
        total_iterations: 10,
        cumulative_iterations: 10,
    }
}

// ============================================================================
// 1. Objective Metric Extraction & Single Metric Minimization Tests
// ============================================================================

#[test]
fn test_objective_swap_count_minimization() {
    let target = make_5q_linear_target();
    let candidates = vec![
        make_synthetic_candidate(0, false, 5, 10, 15, None, true),
        make_synthetic_candidate(1, false, 2, 10, 12, None, true), // Lowest swaps: 2
        make_synthetic_candidate(2, false, 4, 10, 10, None, true),
    ];

    let objective = RoutingObjective::minimize_swaps();
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    assert_eq!(ranking.eligible_count, 3);
    assert_eq!(ranking.ineligible_count, 0);
    assert_eq!(
        ranking.selected_candidate_id,
        Some(candidates[1].descriptor.identity.id.clone())
    );
    assert_eq!(ranking.candidates[0].overall_rank, Some(1));
    assert_eq!(
        ranking.candidates[0].candidate_id,
        candidates[1].descriptor.identity.id
    );
    assert_eq!(
        ranking.candidates[0].score,
        Some(CandidateObjectiveScore::Single {
            metric: ObjectiveMetric::SwapCount,
            value: 2.0,
        })
    );
}

#[test]
fn test_objective_cx_count_minimization() {
    let target = make_5q_linear_target();
    let candidates = vec![
        make_synthetic_candidate(0, false, 4, 12, 20, None, true),
        make_synthetic_candidate(1, false, 4, 6, 18, None, true), // Lowest CX: 6
        make_synthetic_candidate(2, false, 4, 8, 15, None, true),
    ];

    let objective = RoutingObjective {
        function: ObjectiveFunction::Single(ObjectiveMetric::CxCount),
        hard_constraints: vec![
            HardConstraint::TargetLegality,
            HardConstraint::SemanticEquivalence,
            HardConstraint::AncillaZeroState,
        ],
    };
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    assert_eq!(
        ranking.selected_candidate_id,
        Some(candidates[1].descriptor.identity.id.clone())
    );
    assert_eq!(ranking.candidates[0].overall_rank, Some(1));
    assert_eq!(
        ranking.candidates[0].candidate_id,
        candidates[1].descriptor.identity.id
    );
}

#[test]
fn test_objective_two_qubit_gate_count_minimization() {
    let target = make_5q_linear_target();
    let candidates = vec![
        make_synthetic_candidate(0, false, 3, 10, 15, None, true),
        make_synthetic_candidate(1, false, 3, 4, 10, None, true), // Lowest 2Q: 4
    ];

    let objective = RoutingObjective {
        function: ObjectiveFunction::Single(ObjectiveMetric::TwoQubitGateCount),
        hard_constraints: vec![
            HardConstraint::TargetLegality,
            HardConstraint::SemanticEquivalence,
            HardConstraint::AncillaZeroState,
        ],
    };
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    assert_eq!(
        ranking.selected_candidate_id,
        Some(candidates[1].descriptor.identity.id.clone())
    );
}

#[test]
fn test_objective_total_gate_count_minimization() {
    let target = make_5q_linear_target();
    let candidates = vec![
        make_synthetic_candidate(0, false, 3, 15, 20, None, true),
        make_synthetic_candidate(1, false, 3, 5, 12, None, true), // Lowest total gates
    ];

    let objective = RoutingObjective {
        function: ObjectiveFunction::Single(ObjectiveMetric::TotalGateCount),
        hard_constraints: vec![
            HardConstraint::TargetLegality,
            HardConstraint::SemanticEquivalence,
            HardConstraint::AncillaZeroState,
        ],
    };
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    assert_eq!(
        ranking.selected_candidate_id,
        Some(candidates[1].descriptor.identity.id.clone())
    );
}

#[test]
fn test_objective_circuit_depth_minimization() {
    let target = make_5q_linear_target();
    let candidates = vec![
        make_synthetic_candidate(0, false, 2, 8, 25, None, true),
        make_synthetic_candidate(1, false, 5, 8, 11, None, true), // Lowest depth: 11
        make_synthetic_candidate(2, false, 3, 8, 19, None, true),
    ];

    let objective = RoutingObjective::minimize_depth();
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    assert_eq!(
        ranking.selected_candidate_id,
        Some(candidates[1].descriptor.identity.id.clone())
    );
    assert_eq!(
        ranking.candidates[0].score,
        Some(CandidateObjectiveScore::Single {
            metric: ObjectiveMetric::CircuitDepth,
            value: 11.0,
        })
    );
}

// ============================================================================
// 2. Hardware Timing & EstimatedDuration Tests (Explicit Unknown Principle)
// ============================================================================

#[test]
fn test_objective_estimated_duration_with_calibrated_timing_model() {
    let target = make_5q_timed_target();
    let candidates = vec![
        make_synthetic_candidate(0, false, 2, 8, 20, Some(450.0), true),
        make_synthetic_candidate(1, false, 3, 8, 15, Some(280.0), true), // Lowest duration: 280.0 ns
        make_synthetic_candidate(2, false, 4, 8, 18, Some(350.0), true),
    ];

    let objective = RoutingObjective {
        function: ObjectiveFunction::Single(ObjectiveMetric::EstimatedDuration),
        hard_constraints: vec![
            HardConstraint::TargetLegality,
            HardConstraint::SemanticEquivalence,
            HardConstraint::AncillaZeroState,
        ],
    };
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    assert_eq!(
        ranking.selected_candidate_id,
        Some(candidates[1].descriptor.identity.id.clone())
    );
    assert_eq!(
        ranking.candidates[0].score,
        Some(CandidateObjectiveScore::Single {
            metric: ObjectiveMetric::EstimatedDuration,
            value: 280.0,
        })
    );
}

#[test]
fn test_objective_estimated_duration_fails_closed_when_timing_is_unavailable() {
    // Target lacks timing model
    let target = make_5q_linear_target();
    let candidates = vec![
        make_synthetic_candidate(0, false, 2, 8, 20, None, true), // duration_ns is None
        make_synthetic_candidate(1, false, 3, 8, 15, None, true),
    ];

    let objective = RoutingObjective {
        function: ObjectiveFunction::Single(ObjectiveMetric::EstimatedDuration),
        hard_constraints: vec![
            HardConstraint::TargetLegality,
            HardConstraint::SemanticEquivalence,
            HardConstraint::AncillaZeroState,
        ],
    };
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    // Candidates cannot be scored under an unavailable duration metric
    assert_eq!(ranking.eligible_count, 0);
    assert_eq!(ranking.ineligible_count, 2);
    assert_eq!(ranking.selected_candidate_id, None);
    assert!(ranking.candidates[0]
        .explanation
        .contains("estimated duration unavailable"));
}

// ============================================================================
// 3. Multi-Objective Lexicographic and Weighted Scoring Tests
// ============================================================================

#[test]
fn test_lexicographic_objective_ranking_primary_and_secondary() {
    let target = make_5q_linear_target();
    let candidates = vec![
        // Candidate 0: 3 swaps, 20 depth
        make_synthetic_candidate(0, false, 3, 10, 20, None, true),
        // Candidate 1: 2 swaps, 25 depth (Wins on primary metric swaps)
        make_synthetic_candidate(1, false, 2, 10, 25, None, true),
        // Candidate 2: 2 swaps, 15 depth (Wins primary tie by lower secondary depth)
        make_synthetic_candidate(2, false, 2, 10, 15, None, true),
    ];

    let objective = RoutingObjective::lexicographic(vec![
        ObjectiveMetric::SwapCount,
        ObjectiveMetric::CircuitDepth,
    ]);
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    // Candidate 2 should be ranked #1 (2 swaps, 15 depth)
    assert_eq!(
        ranking.selected_candidate_id,
        Some(candidates[2].descriptor.identity.id.clone())
    );
    // Candidate 1 should be ranked #2 (2 swaps, 25 depth)
    assert_eq!(
        ranking.candidates[1].candidate_id,
        candidates[1].descriptor.identity.id
    );
    // Candidate 0 should be ranked #3 (3 swaps, 20 depth)
    assert_eq!(
        ranking.candidates[2].candidate_id,
        candidates[0].descriptor.identity.id
    );
}

#[test]
fn test_weighted_objective_linear_combination() {
    let target = make_5q_linear_target();
    let candidates = vec![
        // Candidate 0: swaps = 4, depth = 10 -> score = 10.0*4 + 1.0*10 = 50.0
        make_synthetic_candidate(0, false, 4, 10, 10, None, true),
        // Candidate 1: swaps = 2, depth = 25 -> score = 10.0*2 + 1.0*25 = 45.0 (Winner)
        make_synthetic_candidate(1, false, 2, 10, 25, None, true),
        // Candidate 2: swaps = 3, depth = 30 -> score = 10.0*3 + 1.0*30 = 60.0
        make_synthetic_candidate(2, false, 3, 10, 30, None, true),
    ];

    let objective = RoutingObjective::weighted(vec![
        (ObjectiveMetric::SwapCount, 10.0),
        (ObjectiveMetric::CircuitDepth, 1.0),
    ]);
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    assert_eq!(
        ranking.selected_candidate_id,
        Some(candidates[1].descriptor.identity.id.clone())
    );
    assert_eq!(
        ranking.candidates[0].score,
        Some(CandidateObjectiveScore::Weighted {
            composite_score: 45.0,
            components: vec![
                (ObjectiveMetric::SwapCount, 10.0, 2.0),
                (ObjectiveMetric::CircuitDepth, 1.0, 25.0),
            ],
        })
    );
}

// ============================================================================
// 4. Pareto Dominance and Non-Dominated Sorting Tests
// ============================================================================

#[test]
fn test_pareto_dominance_and_front_assignment() {
    let target = make_5q_linear_target();
    let candidates = vec![
        // Candidate 0: swaps = 1, depth = 10 (Non-dominated: best swaps) -> Front 0
        make_synthetic_candidate(0, false, 1, 5, 10, None, true),
        // Candidate 1: swaps = 5, depth = 2 (Non-dominated: best depth) -> Front 0
        make_synthetic_candidate(1, false, 5, 5, 2, None, true),
        // Candidate 2: swaps = 3, depth = 5 (Non-dominated between (1,10) and (5,2)) -> Front 0
        make_synthetic_candidate(2, false, 3, 5, 5, None, true),
        // Candidate 3: swaps = 4, depth = 6 (Dominated by Candidate 2: 3 <= 4 and 5 <= 6) -> Front 1
        make_synthetic_candidate(3, false, 4, 5, 6, None, true),
    ];

    let objective = RoutingObjective::lexicographic(vec![
        ObjectiveMetric::SwapCount,
        ObjectiveMetric::CircuitDepth,
    ]);
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    assert_eq!(ranking.pareto_fronts_count, 2);

    let rec0 = ranking
        .find_candidate(&candidates[0].descriptor.identity.id)
        .unwrap();
    let rec1 = ranking
        .find_candidate(&candidates[1].descriptor.identity.id)
        .unwrap();
    let rec2 = ranking
        .find_candidate(&candidates[2].descriptor.identity.id)
        .unwrap();
    let rec3 = ranking
        .find_candidate(&candidates[3].descriptor.identity.id)
        .unwrap();

    assert_eq!(rec0.pareto_front, Some(0));
    assert_eq!(rec1.pareto_front, Some(0));
    assert_eq!(rec2.pareto_front, Some(0));
    assert_eq!(rec3.pareto_front, Some(1)); // Dominated by candidate 2
}

// ============================================================================
// 5. Deterministic Tie-Breaking Hierarchy Tests
// ============================================================================

#[test]
fn test_tie_breaking_designated_baseline_preference() {
    let target = make_5q_linear_target();
    let candidates = vec![
        // Candidate 0: non-baseline, score = 2 swaps
        make_synthetic_candidate(0, false, 2, 5, 10, None, true),
        // Candidate 1: designated baseline, identical score = 2 swaps
        make_synthetic_candidate(1, true, 2, 5, 10, None, true),
    ];

    let objective = RoutingObjective::minimize_swaps();
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    // Designated baseline (Candidate 1) must win the tie-break
    assert_eq!(
        ranking.selected_candidate_id,
        Some(candidates[1].descriptor.identity.id.clone())
    );
    assert_eq!(
        ranking.candidates[0].candidate_id,
        candidates[1].descriptor.identity.id
    );
    assert!(ranking.candidates[0].is_baseline);
    assert!(ranking.candidates[0]
        .explanation
        .contains("[designated baseline]"));
}

#[test]
fn test_tie_breaking_canonical_candidate_id_alphabetical() {
    let target = make_5q_linear_target();
    // Two non-baseline candidates with identical scores and metrics
    let candidates = vec![
        make_synthetic_candidate(1, false, 3, 5, 10, None, true),
        make_synthetic_candidate(0, false, 3, 5, 10, None, true),
    ];

    let objective = RoutingObjective::minimize_swaps();
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    // Candidate with lexicographically smaller ID wins
    let expected_winner =
        if candidates[0].descriptor.identity.id < candidates[1].descriptor.identity.id {
            &candidates[0].descriptor.identity.id
        } else {
            &candidates[1].descriptor.identity.id
        };

    assert_eq!(ranking.selected_candidate_id, Some(expected_winner.clone()));
}

// ============================================================================
// 6. Mandatory Hard Constraint Disqualification Tests
// ============================================================================

#[test]
fn test_hard_constraint_semantic_equivalence_disqualification() {
    let target = make_5q_linear_target();
    let candidates = vec![
        // Candidate 0: 1 swap (very low), but invalid equivalence!
        make_synthetic_candidate(0, false, 1, 5, 10, None, false),
        // Candidate 1: 5 swaps, but fully valid!
        make_synthetic_candidate(1, false, 5, 5, 10, None, true),
    ];

    let objective = RoutingObjective::minimize_swaps();
    let ranking = RankingEngine::evaluate_and_rank(&candidates, &objective, &target);

    assert_eq!(ranking.eligible_count, 1);
    assert_eq!(ranking.ineligible_count, 1);

    // Candidate 0 must be disqualified regardless of its superior swap count
    let rec0 = ranking
        .find_candidate(&candidates[0].descriptor.identity.id)
        .unwrap();
    assert!(!rec0.eligibility.is_eligible());
    assert_eq!(rec0.overall_rank, None);
    assert!(rec0
        .explanation
        .contains("Disqualified: Mandatory hard constraint 'SemanticEquivalence' failed"));

    // Candidate 1 must be selected
    assert_eq!(
        ranking.selected_candidate_id,
        Some(candidates[1].descriptor.identity.id.clone())
    );
    assert_eq!(
        ranking.candidates[0].candidate_id,
        candidates[1].descriptor.identity.id
    );
}

#[test]
fn test_hard_constraint_unexecuted_validation_disqualification() {
    let target = make_5q_linear_target();
    let mut cand = make_synthetic_candidate(0, false, 2, 5, 10, None, true);
    cand.validation.semantic_equivalence = CheckStatus::Unexecuted {
        reason: "statevector limit exceeded".to_string(),
    };

    let objective = RoutingObjective::minimize_swaps();
    let ranking = RankingEngine::evaluate_and_rank(&[cand], &objective, &target);

    assert_eq!(ranking.eligible_count, 0);
    assert_eq!(ranking.ineligible_count, 1);
    assert_eq!(ranking.selected_candidate_id, None);
    assert!(ranking.candidates[0]
        .explanation
        .contains("Disqualified: Mandatory hard constraint 'SemanticEquivalence' unexecuted"));
}

// ============================================================================
// 7. Full Search Runner Integration & Explainability Reporting Tests
// ============================================================================

#[test]
fn test_multi_candidate_runner_ranking_integration_and_explainability() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .objective(RoutingObjective::lexicographic(vec![
            ObjectiveMetric::SwapCount,
            ObjectiveMetric::CircuitDepth,
        ]))
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner.run().expect("Runner search must succeed");

    let ranking = report
        .ranking
        .as_ref()
        .expect("Ranking report must be populated");
    assert_eq!(ranking.total_candidates, 3);
    assert!(ranking.eligible_count > 0);
    assert!(ranking.selected_candidate_id.is_some());

    // Verify winner selection API
    let selection = report.select_candidate().expect("Selection must succeed");
    assert_eq!(
        selection.candidate_id,
        ranking.selected_candidate_id.as_ref().unwrap()
    );
    assert!(selection
        .selection_reason()
        .starts_with("Selected: Rank #1"));

    // Format human-readable ASCII table
    let table = ranking.format_table();
    assert!(table.contains("=== Multi-Candidate Ranking Report"));
    assert!(table.contains("Winner: cand_"));
    assert!(table.contains("Front"));
    assert!(table.contains("Rank"));
}

#[test]
fn test_repeat_run_determinism() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .objective(RoutingObjective::weighted(vec![
            (ObjectiveMetric::SwapCount, 2.0),
            (ObjectiveMetric::CircuitDepth, 1.0),
        ]))
        .build()
        .unwrap();

    let runner1 = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let rep1 = runner1.run().unwrap();

    let runner2 = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let rep2 = runner2.run().unwrap();

    let rank1 = rep1.ranking.as_ref().unwrap();
    let rank2 = rep2.ranking.as_ref().unwrap();

    assert_eq!(rank1.selected_candidate_id, rank2.selected_candidate_id);
    assert_eq!(rank1.selection_reason, rank2.selection_reason);
    assert_eq!(rank1.candidates.len(), rank2.candidates.len());

    for (c1, c2) in rank1.candidates.iter().zip(rank2.candidates.iter()) {
        assert_eq!(c1.candidate_id, c2.candidate_id);
        assert_eq!(c1.overall_rank, c2.overall_rank);
        assert_eq!(c1.pareto_front, c2.pareto_front);
        assert_eq!(c1.score, c2.score);
        assert_eq!(c1.explanation, c2.explanation);
    }
}

#[test]
fn test_hard_constraint_target_legality_disqualification() {
    let target = make_5q_linear_target();
    let mut cand = make_synthetic_candidate(0, false, 1, 5, 10, None, true);
    cand.validation.target_legality = CheckStatus::Failed {
        reason: "coupling edge (0, 3) is not adjacent on linear topology".to_string(),
    };

    let objective = RoutingObjective::minimize_swaps();
    let ranking = RankingEngine::evaluate_and_rank(&[cand], &objective, &target);

    assert_eq!(ranking.eligible_count, 0);
    assert_eq!(ranking.ineligible_count, 1);
    assert_eq!(ranking.selected_candidate_id, None);
    assert!(ranking.candidates[0]
        .explanation
        .contains("Disqualified: Mandatory hard constraint 'TargetLegality' failed"));
}

#[test]
fn test_hard_constraint_ancilla_zero_state_disqualification() {
    let target = make_5q_linear_target();
    let mut cand = make_synthetic_candidate(0, false, 1, 5, 10, None, true);
    cand.validation.ancilla_zero_state = CheckStatus::Failed {
        reason: "residual population 0.05 on spare ancilla qubit 4 (ADR-0001)".to_string(),
    };

    let objective = RoutingObjective::minimize_swaps();
    let ranking = RankingEngine::evaluate_and_rank(&[cand], &objective, &target);

    assert_eq!(ranking.eligible_count, 0);
    assert_eq!(ranking.ineligible_count, 1);
    assert_eq!(ranking.selected_candidate_id, None);
    assert!(ranking.candidates[0]
        .explanation
        .contains("Disqualified: Mandatory hard constraint 'AncillaZeroState' (ADR-0001) failed"));
}

#[test]
fn test_fallback_policies_with_ranking() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    // 1. FallbackPolicy::DeterministicBaseline
    let cfg_base = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .fallback_policy(FallbackPolicy::DeterministicBaseline)
        .build()
        .unwrap();

    let runner_base = MultiCandidateSearchRunner::new(&circuit, &target, &cfg_base);
    let report_base = runner_base.run().unwrap();
    let sel_base = report_base.select_candidate().unwrap();
    assert!(sel_base.record.is_baseline);

    // 2. FallbackPolicy::BestValidSoFar
    let cfg_best = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .unwrap();

    let runner_best = MultiCandidateSearchRunner::new(&circuit, &target, &cfg_best);
    let report_best = runner_best.run().unwrap();
    let sel_best = report_best.select_candidate().unwrap();
    let rank_rep = report_best.ranking.as_ref().unwrap();
    assert_eq!(
        sel_best.candidate_id,
        rank_rep.selected_candidate_id.as_ref().unwrap()
    );
}

#[test]
fn test_compiler_pipeline_multi_candidate_ranking_integration() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let search_cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .objective(RoutingObjective::lexicographic(vec![
            ObjectiveMetric::SwapCount,
            ObjectiveMetric::CircuitDepth,
        ]))
        .build()
        .unwrap();

    let compiler_cfg = qcc::compiler::config::CompilerConfig {
        router: qcc::compiler::config::RouterChoice::MultiCandidate(Box::new(search_cfg)),
        ..Default::default()
    };

    let pipeline = qcc::compiler::pipeline::CompilerPipeline::new(compiler_cfg);
    let result = pipeline
        .compile(&circuit, &target)
        .expect("Pipeline compile succeeds");

    assert!(result.report.metrics.final_gate_count > 0);
    assert!(result
        .report
        .routing
        .router_name
        .contains("SabreBidirectionalRouter [MultiCandidate Provisional: cand_"));
    assert!(result.report.routing.swaps_inserted > 0);
}

// ============================================================================
// 8. Audit Verification & Regression Tests
// ============================================================================

#[test]
fn test_invalid_weights_and_normalization_edge_cases() {
    // 1. NaN weight rejected
    let nan_weight = RoutingObjective::weighted(vec![(ObjectiveMetric::SwapCount, f64::NAN)]);
    assert!(nan_weight.validate().is_err());

    // 2. Infinity weight rejected
    let inf_weight = RoutingObjective::weighted(vec![(ObjectiveMetric::SwapCount, f64::INFINITY)]);
    assert!(inf_weight.validate().is_err());

    // 3. Zero weight with at least one strictly positive weight is valid
    let zero_and_pos = RoutingObjective::weighted(vec![
        (ObjectiveMetric::SwapCount, 0.0),
        (ObjectiveMetric::CircuitDepth, 1.5),
    ]);
    assert!(zero_and_pos.validate().is_ok());

    // 4. Raw linear composite score preserves exact values without hidden batch scaling
    let snapshot = CandidateMetricsSnapshot {
        swap_count: 2,
        cx_count: 4,
        two_qubit_gate_count: 4,
        total_gate_count: 10,
        circuit_depth: 6,
        estimated_duration_ns: Some(120.0),
    };
    let score = ObjectiveEvaluator::evaluate(&snapshot, &zero_and_pos.function).unwrap();
    assert_eq!(
        score,
        CandidateObjectiveScore::Weighted {
            composite_score: 9.0, // 0.0 * 2 + 1.5 * 6 = 9.0
            components: vec![
                (ObjectiveMetric::SwapCount, 0.0, 2.0),
                (ObjectiveMetric::CircuitDepth, 1.5, 6.0),
            ],
        }
    );
}

#[test]
fn test_pareto_dominance_comparator_strictly_prefers_non_dominated_over_dominated() {
    let target = make_5q_linear_target();

    // Candidate G: swaps = 30, depth = 1 -> Non-dominated (best depth) -> Front 0
    let cand_g = make_synthetic_candidate(0, false, 30, 10, 1, None, true);

    // Candidate D: swaps = 1, depth = 20 -> Non-dominated (best swaps) -> Front 0 (Incomparable to G)
    let cand_d = make_synthetic_candidate(1, false, 1, 10, 20, None, true);

    // Candidate H: swaps = 30, depth = 2 -> Strictly dominated by Candidate G (30 <= 30 and 1 < 2) -> Front 1
    let cand_h = make_synthetic_candidate(2, false, 30, 10, 2, None, true);

    // Objective: Weighted with heavy weight on depth: score = 1.0 * swaps + 10.0 * depth
    // Score(G) = 30 + 10 = 40.0
    // Score(D) = 1 + 200 = 201.0
    // Score(H) = 30 + 20 = 50.0
    //
    // Notice: Score(H) [50.0] < Score(D) [201.0]!
    // If the comparator ranked strictly by score, dominated candidate H would beat non-dominated candidate D!
    // But under Phase 76 Pareto-first ranking (Tier 1: Front, Tier 2: Score):
    // Candidate D (Front 0) MUST be ranked ahead of Candidate H (Front 1).
    let objective = RoutingObjective::weighted(vec![
        (ObjectiveMetric::SwapCount, 1.0),
        (ObjectiveMetric::CircuitDepth, 10.0),
    ]);

    let ranking = RankingEngine::evaluate_and_rank(&[cand_g, cand_d, cand_h], &objective, &target);

    assert_eq!(ranking.pareto_fronts_count, 2);
    assert_eq!(ranking.candidates.len(), 3);

    // Rank #1: Candidate G (Front 0, score 40.0)
    assert_eq!(ranking.candidates[0].overall_rank, Some(1));
    assert_eq!(ranking.candidates[0].candidate_index, 0); // Candidate G
    assert_eq!(ranking.candidates[0].pareto_front, Some(0));

    // Rank #2: Candidate D (Front 0, score 201.0)
    assert_eq!(ranking.candidates[1].overall_rank, Some(2));
    assert_eq!(ranking.candidates[1].candidate_index, 1); // Candidate D
    assert_eq!(ranking.candidates[1].pareto_front, Some(0));

    // Rank #3: Candidate H (Front 1, score 50.0) - DOMINATED candidate placed in Front 1
    assert_eq!(ranking.candidates[2].overall_rank, Some(3));
    assert_eq!(ranking.candidates[2].candidate_index, 2); // Candidate H
    assert_eq!(ranking.candidates[2].pareto_front, Some(1));
}

#[test]
fn test_timing_concurrency_makespan_not_sum() {
    let target = make_5q_timed_target();

    // Circuit with two parallel disjoint CX operations: (0, 1) and (2, 3)
    let mut builder = CircuitBuilder::new("parallel_cx");
    let q = builder.add_quantum_register("q", 4).unwrap();
    builder.cx(q[0], q[1]).unwrap(); // duration = 100 ns
    builder.cx(q[2], q[3]).unwrap(); // duration = 100 ns
    let circuit = builder.finish().unwrap();

    let router = SabreBidirectionalRouter::default();
    let (routed, report) = router.route_circuit(&circuit, &target, None).unwrap();

    let snapshot = CandidateMetricsSnapshot::from_circuit_and_report(&routed, &report, &target);
    assert!(snapshot.estimated_duration_ns.is_some());

    // Both CX gates are independent and execute concurrently: makespan is 100 ns, not 200 ns!
    let makespan = snapshot.estimated_duration_ns.unwrap();
    assert_eq!(makespan, 100.0);
}

#[test]
fn test_timing_mixed_known_and_unknown_durations_fail_closed() {
    let mut target = make_5q_linear_target();
    let mut timing = TimingModel::new();
    // Only 'cx' has calibrated timing; single-qubit gates ('h') remain uncalibrated
    timing.set_default_duration("cx", OperationDuration::known_ns(100.0));
    target.timing = Some(timing);

    // Circuit with both 'h' (uncalibrated) and 'cx' (calibrated)
    let mut builder = CircuitBuilder::new("mixed_timing");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let router = SabreBidirectionalRouter::default();
    let (routed, report) = router.route_circuit(&circuit, &target, None).unwrap();

    let snapshot = CandidateMetricsSnapshot::from_circuit_and_report(&routed, &report, &target);

    // Incomplete timing yields None (Explicit Unknown Principle)
    assert!(snapshot.estimated_duration_ns.is_none());

    // Objective requiring duration fails closed
    let obj = RoutingObjective {
        function: ObjectiveFunction::Single(ObjectiveMetric::EstimatedDuration),
        hard_constraints: vec![
            HardConstraint::TargetLegality,
            HardConstraint::SemanticEquivalence,
            HardConstraint::AncillaZeroState,
        ],
    };
    let err = ObjectiveEvaluator::evaluate(&snapshot, &obj.function);
    assert!(matches!(
        err,
        Err(MetricEvaluationError::DurationUnavailable { .. })
    ));
}

#[test]
fn test_no_valid_candidates_preserves_full_report() {
    let target = make_5q_linear_target();

    // Candidate fails validation (e.g. semantic equivalence mismatch)
    let invalid_cand = make_synthetic_candidate(0, true, 2, 5, 10, None, false);

    let objective = RoutingObjective::minimize_swaps();
    let ranking = RankingEngine::evaluate_and_rank(&[invalid_cand], &objective, &target);

    assert_eq!(ranking.eligible_count, 0);
    assert_eq!(ranking.ineligible_count, 1);
    assert_eq!(ranking.selected_candidate_id, None);
    assert!(ranking.selection_reason.is_some());
    assert!(!ranking.candidates[0].eligibility.is_eligible());
    assert!(ranking.candidates[0].explanation.contains("Disqualified"));
}

#[test]
fn test_abort_budget_exhausted_preserves_accumulated_report() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    // Budget of 1 total swap on K4 circuit (needing ~3 SWAPs) with Abort policy
    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .budget(SearchBudget {
            max_total_swaps: Some(1),
            ..Default::default()
        })
        .fallback_policy(FallbackPolicy::Abort)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let err = runner.run().unwrap_err();

    match err {
        qcc::routing::search::RoutingSearchExecutionError::Aborted { reason, report } => {
            assert!(reason.contains("budget exhausted") || reason.contains("failed routing"));
            assert_eq!(report.candidates.len(), 3);
            assert!(report.ranking.is_some());
        }
        other => panic!("Expected Aborted error, got: {other:?}"),
    }
}

#[test]
fn test_weighted_score_numerical_overflow_safety() {
    let target = make_5q_linear_target();

    // 1. Weight validation rejects sum of weights overflow
    let overflowing_weights = RoutingObjective::weighted(vec![
        (ObjectiveMetric::SwapCount, 1e308),
        (ObjectiveMetric::CircuitDepth, 1e308),
    ]);
    assert!(overflowing_weights.validate().is_err());
    let err_msg = overflowing_weights.validate().unwrap_err().to_string();
    assert!(err_msg.contains("overflowed to non-finite"));

    // 2. Near-f64::MAX finite weight (e.g. 1e308) with metric = 10 overflows multiplication
    let large_weight_obj =
        RoutingObjective::weighted(vec![(ObjectiveMetric::SwapCount, 1e308)]).function;

    let snapshot = CandidateMetricsSnapshot {
        swap_count: 10,
        cx_count: 10,
        two_qubit_gate_count: 10,
        total_gate_count: 20,
        circuit_depth: 10,
        estimated_duration_ns: None,
    };

    let eval_err = ObjectiveEvaluator::evaluate(&snapshot, &large_weight_obj).unwrap_err();
    assert!(matches!(
        eval_err,
        MetricEvaluationError::NonFiniteScore { .. }
    ));
    assert!(eval_err.to_string().contains("overflowed to non-finite"));

    // 3. Overflowing candidate evaluated under RankingEngine is disqualified fail-closed
    let candidate = make_synthetic_candidate(0, false, 10, 10, 10, None, true);
    let obj = RoutingObjective::weighted(vec![(ObjectiveMetric::SwapCount, 1e308)]);
    let ranking = RankingEngine::evaluate_and_rank(&[candidate], &obj, &target);

    assert_eq!(ranking.eligible_count, 0);
    assert_eq!(ranking.ineligible_count, 1);
    assert!(!ranking.candidates[0].eligibility.is_eligible());
    assert!(ranking.candidates[0]
        .explanation
        .contains("Disqualified: Objective evaluation failed: non-finite score"));
}

#[test]
fn test_pareto_incomparable_candidates_and_front_tie_breaking() {
    let target = make_5q_linear_target();

    // Three candidates with trade-offs:
    // Candidate A: swaps = 2, depth = 10
    // Candidate B: swaps = 5, depth = 4
    // Candidate C: swaps = 6, depth = 12
    //
    // Dominance analysis:
    // - A vs B: A has fewer swaps (2 < 5), B has lower depth (4 < 10) => MUTUALLY INCOMPARABLE
    // - A vs C: 2 < 6 and 10 < 12 => A strictly dominates C (A > C)
    // - B vs C: 5 < 6 and 4 < 12 => B strictly dominates C (B > C)
    //
    // Therefore:
    // - Pareto Front 0: {A, B} (both non-dominated)
    // - Pareto Front 1: {C} (dominated by both A and B)
    let cand_a = make_synthetic_candidate(0, false, 2, 10, 10, None, true);
    let cand_b = make_synthetic_candidate(1, false, 5, 10, 4, None, true);
    let cand_c = make_synthetic_candidate(2, false, 6, 10, 12, None, true);

    // Case 1: Objective weights equal: score = 1.0 * swaps + 1.0 * depth
    // Score(A) = 2 + 10 = 12.0
    // Score(B) = 5 + 4 = 9.0
    // Score(C) = 6 + 12 = 18.0
    //
    // Ranking expectations:
    // - Incomparable candidates A and B BOTH receive Pareto Front 0
    // - Dominated candidate C receives Pareto Front 1
    // - Within Front 0, tie-breaking orders by score: Score(B) = 9.0 < Score(A) = 12.0
    // - Rank #1: Candidate B (Front 0, score 9.0) -> WINNER
    // - Rank #2: Candidate A (Front 0, score 12.0)
    // - Rank #3: Candidate C (Front 1, score 18.0)
    let objective_equal = RoutingObjective::weighted(vec![
        (ObjectiveMetric::SwapCount, 1.0),
        (ObjectiveMetric::CircuitDepth, 1.0),
    ]);

    let ranking_1 = RankingEngine::evaluate_and_rank(
        &[cand_a.clone(), cand_b.clone(), cand_c.clone()],
        &objective_equal,
        &target,
    );

    assert_eq!(ranking_1.pareto_fronts_count, 2);
    assert_eq!(ranking_1.eligible_count, 3);

    // Front checks
    assert_eq!(ranking_1.candidates[0].candidate_index, 1); // Candidate B
    assert_eq!(ranking_1.candidates[0].pareto_front, Some(0));
    assert_eq!(ranking_1.candidates[0].overall_rank, Some(1));
    assert_eq!(
        ranking_1.selected_candidate_id,
        Some(cand_b.descriptor.identity.id.clone())
    );

    assert_eq!(ranking_1.candidates[1].candidate_index, 0); // Candidate A
    assert_eq!(ranking_1.candidates[1].pareto_front, Some(0));
    assert_eq!(ranking_1.candidates[1].overall_rank, Some(2));

    assert_eq!(ranking_1.candidates[2].candidate_index, 2); // Candidate C (dominated)
    assert_eq!(ranking_1.candidates[2].pareto_front, Some(1));
    assert_eq!(ranking_1.candidates[2].overall_rank, Some(3));

    // Case 2: Objective weights swap-heavy: score = 10.0 * swaps + 1.0 * depth
    // Score(A) = 20 + 10 = 30.0
    // Score(B) = 50 + 4 = 54.0
    // Score(C) = 60 + 12 = 72.0
    //
    // Ranking expectations:
    // - Within Front 0, tie-breaking now prefers Candidate A: Score(A) = 30.0 < Score(B) = 54.0
    // - Rank #1: Candidate A (Front 0, score 30.0) -> WINNER
    // - Rank #2: Candidate B (Front 0, score 54.0)
    // - Rank #3: Candidate C (Front 1, score 72.0)
    let objective_swaps = RoutingObjective::weighted(vec![
        (ObjectiveMetric::SwapCount, 10.0),
        (ObjectiveMetric::CircuitDepth, 1.0),
    ]);

    let ranking_2 =
        RankingEngine::evaluate_and_rank(&[cand_a, cand_b, cand_c], &objective_swaps, &target);

    assert_eq!(ranking_2.candidates[0].candidate_index, 0); // Candidate A
    assert_eq!(ranking_2.candidates[0].pareto_front, Some(0));
    assert_eq!(ranking_2.candidates[0].overall_rank, Some(1));

    assert_eq!(ranking_2.candidates[1].candidate_index, 1); // Candidate B
    assert_eq!(ranking_2.candidates[1].pareto_front, Some(0));
    assert_eq!(ranking_2.candidates[1].overall_rank, Some(2));

    assert_eq!(ranking_2.candidates[2].candidate_index, 2); // Candidate C
    assert_eq!(ranking_2.candidates[2].pareto_front, Some(1));
    assert_eq!(ranking_2.candidates[2].overall_rank, Some(3));
}
