//! Integration and regression tests for Phase 77: Multi-Candidate Routing Search Validation,
//! Comparative Benchmark Regression Invariants, Reproducibility, Budget Enforcements,
//! and Non-Regression Guarantees.
//!
//! Roadmap v3.1, Milestone M9 (Phase 77).

use qcc::builder::CircuitBuilder;
use qcc::gate_signature::ParameterDomain;
use qcc::ir::circuit::Circuit;
use qcc::routing::sabre::SabreRouterConfig;
use qcc::routing::search::error::{RoutingSearchConfigError, RoutingSearchExecutionError};
use qcc::routing::search::objective::{ObjectiveMetric, RoutingObjective};
use qcc::routing::search::runner::{
    CandidateExecutionStatus, CheckStatus, MultiCandidateSearchRunner,
};
use qcc::routing::search::{
    FallbackPolicy, MultiCandidateRoutingConfig, SearchBudget, SeedStrategy,
};
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};

/// Builds a 5-qubit linear target: 0 - 1 - 2 - 3 - 4.
fn make_5q_linear_target() -> Target {
    let mut builder = TargetBuilder::new("linear_5q", 5);
    for gate in ["h", "x", "sx"] {
        builder = builder.add_operation(
            TargetOperationBuilder::new(gate, 1)
                .connectivity(OperationConnectivity::all_single_qubits(5))
                .build(),
        );
    }
    for gate in ["rx", "ry", "rz"] {
        builder = builder.add_operation(
            TargetOperationBuilder::new(gate, 1)
                .add_parameter_domain(ParameterDomain::Angle4Pi)
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

/// Builds a 5-qubit ring target: 0 - 1 - 2 - 3 - 4 - 0.
fn make_5q_ring_target() -> Target {
    let mut builder = TargetBuilder::new("ring_5q", 5);
    for gate in ["h", "x", "sx"] {
        builder = builder.add_operation(
            TargetOperationBuilder::new(gate, 1)
                .connectivity(OperationConnectivity::all_single_qubits(5))
                .build(),
        );
    }
    for gate in ["rx", "ry", "rz"] {
        builder = builder.add_operation(
            TargetOperationBuilder::new(gate, 1)
                .add_parameter_domain(ParameterDomain::Angle4Pi)
                .connectivity(OperationConnectivity::all_single_qubits(5))
                .build(),
        );
    }
    let couplings = vec![
        (PhysicalQubitId(0), PhysicalQubitId(1)),
        (PhysicalQubitId(1), PhysicalQubitId(2)),
        (PhysicalQubitId(2), PhysicalQubitId(3)),
        (PhysicalQubitId(3), PhysicalQubitId(4)),
        (PhysicalQubitId(4), PhysicalQubitId(0)),
    ];
    builder = builder.add_operation(
        TargetOperationBuilder::new("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(couplings))
            .build(),
    );
    builder.build().expect("build 5q ring target")
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

/// Builds a 5-qubit QFT-like non-adjacent circuit requiring multiple SWAPs.
fn make_qft_like_circuit() -> Circuit {
    let mut builder = CircuitBuilder::new("qft_like");
    let q = builder.add_quantum_register("q", 5).unwrap();
    for i in 0..5 {
        builder.h(q[i]).unwrap();
        for j in (i + 1)..5 {
            builder.rz(q[j], std::f64::consts::FRAC_PI_8).unwrap();
            builder.cx(q[j], q[i]).unwrap();
            builder.rz(q[i], -std::f64::consts::FRAC_PI_8).unwrap();
            builder.cx(q[j], q[i]).unwrap();
        }
    }
    builder.finish().unwrap()
}

// ============================================================================
// 1. Deterministic Multi-Candidate Routing Reproducibility on Fixed Seeds
// ============================================================================

#[test]
fn test_multi_candidate_deterministic_reproducibility_on_fixed_seeds() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(4)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 12345,
            count: 4,
        })
        .objective(RoutingObjective::minimize_swaps())
        .build()
        .expect("config build");

    let runner1 = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report1 = runner1.run().expect("run 1 must succeed");
    let sel1 = report1.select_candidate().expect("select 1");

    let runner2 = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report2 = runner2.run().expect("run 2 must succeed");
    let sel2 = report2.select_candidate().expect("select 2");

    // Exact winner identity reproducibility
    assert_eq!(sel1.candidate_id, sel2.candidate_id);
    assert_eq!(
        sel1.sabre_report.swaps_inserted,
        sel2.sabre_report.swaps_inserted
    );
    assert_eq!(
        sel1.sabre_report.routed_operations,
        sel2.sabre_report.routed_operations
    );
    assert_eq!(sel1.sabre_report.layout, sel2.sabre_report.layout);

    // Exact candidate table reproducibility
    assert_eq!(report1.candidates.len(), report2.candidates.len());
    for (c1, c2) in report1.candidates.iter().zip(report2.candidates.iter()) {
        assert_eq!(c1.descriptor.identity.id, c2.descriptor.identity.id);
        assert_eq!(c1.descriptor.identity.seed, c2.descriptor.identity.seed);
        assert_eq!(c1.is_baseline, c2.is_baseline);
        assert_eq!(
            c1.validation.is_fully_validated(),
            c2.validation.is_fully_validated()
        );
        match (&c1.status, &c2.status) {
            (
                CandidateExecutionStatus::Success { report: r1, .. },
                CandidateExecutionStatus::Success { report: r2, .. },
            ) => {
                assert_eq!(r1.swaps_inserted, r2.swaps_inserted);
                assert_eq!(r1.routed_operations, r2.routed_operations);
            }
            _ => panic!("Expected both runs to have identical success statuses"),
        }
    }

    // Ranking reproducibility
    let rank1 = report1.ranking.as_ref().expect("ranking 1");
    let rank2 = report2.ranking.as_ref().expect("ranking 2");
    assert_eq!(rank1.selected_candidate_id, rank2.selected_candidate_id);
    assert_eq!(rank1.eligible_count, rank2.eligible_count);
    assert_eq!(rank1.candidates.len(), rank2.candidates.len());
    for (rc1, rc2) in rank1.candidates.iter().zip(rank2.candidates.iter()) {
        assert_eq!(rc1.candidate_id, rc2.candidate_id);
        assert_eq!(rc1.overall_rank, rc2.overall_rank);
        assert_eq!(rc1.pareto_front, rc2.pareto_front);
        assert_eq!(rc1.eligibility.is_eligible(), rc2.eligibility.is_eligible());
    }
}

// ============================================================================
// 2. Preservation of Candidate Budgets and Hard Constraints Across Runs
// ============================================================================

#[test]
fn test_preservation_of_candidate_budgets_and_hard_constraints() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    // Budget with strict max candidate count and total swap limit
    let budget = SearchBudget {
        max_candidates: 3,
        timeout_ms: None,
        max_total_swaps: Some(50),
        max_total_iterations: Some(500),
    };

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .budget(budget)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 999,
            count: 3,
        })
        .objective(RoutingObjective::minimize_swaps())
        .build()
        .expect("config build");

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner.run().expect("run must succeed");

    assert_eq!(report.total_requested, 3);
    assert_eq!(report.total_executed, 3);
    assert_eq!(report.total_succeeded, 2);
    assert_eq!(report.total_executed - report.total_succeeded, 1);
    assert_eq!(report.cumulative_swaps, 50); // Exact budget limit preserved!

    // Every succeeded candidate must satisfy all three mandatory hard constraints independently
    for c in &report.candidates {
        if c.status.is_success() {
            assert_eq!(
                c.validation.target_legality,
                CheckStatus::Passed,
                "Target legality must be independently passed"
            );
            assert_eq!(
                c.validation.semantic_equivalence,
                CheckStatus::Passed,
                "Logical equivalence must be independently passed"
            );
            assert_eq!(
                c.validation.ancilla_zero_state,
                CheckStatus::Passed,
                "ADR-0001 clean ancilla policy must be independently passed"
            );
            assert!(c.validation.is_fully_validated());
        }
    }
}

// ============================================================================
// 3. Non-Regression Relative to Baseline Router
// ============================================================================

#[test]
fn test_non_regression_relative_to_baseline_router() {
    let target = make_5q_linear_target();
    let circuit = make_qft_like_circuit();

    // 1. Single-candidate deterministic baseline run
    let base_cfg = MultiCandidateRoutingConfig::single_candidate(SabreRouterConfig::default());
    let base_runner = MultiCandidateSearchRunner::new(&circuit, &target, &base_cfg);
    let base_report = base_runner.run().expect("baseline run");
    let base_sel = base_report.select_candidate().expect("baseline select");
    let baseline_swaps = base_sel.sabre_report.swaps_inserted;

    // 2. Multi-candidate search with candidate budget B=4
    let multi_cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(4)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 42,
            count: 4,
        })
        .objective(RoutingObjective::minimize_swaps())
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .expect("multi config");

    let multi_runner = MultiCandidateSearchRunner::new(&circuit, &target, &multi_cfg);
    let multi_report = multi_runner.run().expect("multi run");
    let multi_sel = multi_report.select_candidate().expect("multi select");
    let multi_swaps = multi_sel.sabre_report.swaps_inserted;

    // Non-regression invariant: multi-candidate mode NEVER selects a candidate
    // strictly worse than baseline under identical evaluation criteria
    assert!(
        multi_swaps <= baseline_swaps,
        "Multi-candidate SWAPs ({multi_swaps}) must be <= baseline SWAPs ({baseline_swaps})"
    );
}

// ============================================================================
// 4. Validated Categorization of Benchmark Outcomes
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BenchmarkOutcomeCategory {
    StrictlyBetter,
    Equivalent,
    StrictlyWorse,
    Invalid,
}

fn categorize_outcome(
    baseline_swaps: usize,
    candidate_swaps: usize,
    is_valid: bool,
) -> BenchmarkOutcomeCategory {
    if !is_valid {
        BenchmarkOutcomeCategory::Invalid
    } else if candidate_swaps < baseline_swaps {
        BenchmarkOutcomeCategory::StrictlyBetter
    } else if candidate_swaps == baseline_swaps {
        BenchmarkOutcomeCategory::Equivalent
    } else {
        BenchmarkOutcomeCategory::StrictlyWorse
    }
}

#[test]
fn test_validated_categorization_of_benchmark_outcomes() {
    let baseline_swaps = 10;

    // 1. Strictly better: fewer swaps and valid
    assert_eq!(
        categorize_outcome(baseline_swaps, 8, true),
        BenchmarkOutcomeCategory::StrictlyBetter
    );

    // 2. Equivalent: same swaps and valid
    assert_eq!(
        categorize_outcome(baseline_swaps, 10, true),
        BenchmarkOutcomeCategory::Equivalent
    );

    // 3. Strictly worse: more swaps and valid
    assert_eq!(
        categorize_outcome(baseline_swaps, 12, true),
        BenchmarkOutcomeCategory::StrictlyWorse
    );

    // 4. Invalid: fails validation regardless of swaps
    assert_eq!(
        categorize_outcome(baseline_swaps, 4, false),
        BenchmarkOutcomeCategory::Invalid
    );
}

// ============================================================================
// 5. Multi-Candidate Routing Across Varied Topologies
// ============================================================================

#[test]
fn test_multi_candidate_routing_across_linear_and_ring_topologies() {
    let circuit = make_k4_circuit();
    let linear = make_5q_linear_target();
    let ring = make_5q_ring_target();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 777,
            count: 3,
        })
        .objective(RoutingObjective::minimize_swaps())
        .build()
        .expect("config build");

    // Linear target
    let runner_linear = MultiCandidateSearchRunner::new(&circuit, &linear, &cfg);
    let report_linear = runner_linear.run().expect("linear run");
    let sel_linear = report_linear.select_candidate().expect("linear select");
    assert!(sel_linear.record.validation.is_fully_validated());

    // Ring target (ring has extra 4-0 coupling edge, so SWAPs should be <= linear)
    let runner_ring = MultiCandidateSearchRunner::new(&circuit, &ring, &cfg);
    let report_ring = runner_ring.run().expect("ring run");
    let sel_ring = report_ring.select_candidate().expect("ring select");
    assert!(sel_ring.record.validation.is_fully_validated());

    assert!(
        sel_ring.sabre_report.swaps_inserted <= sel_linear.sabre_report.swaps_inserted,
        "Ring topology should require <= SWAPs than linear topology"
    );
}

// ============================================================================
// 6. Explicit Unknown Duration Principle in Benchmark Objectives
// ============================================================================

#[test]
fn test_explicit_unknown_duration_fails_closed_in_search_runner() {
    let uncalibrated_target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    // Objective requesting Duration when target has no calibrated timing
    let obj = RoutingObjective::single(ObjectiveMetric::EstimatedDuration);
    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(2)
        .objective(obj)
        .build()
        .expect("config build");

    let runner = MultiCandidateSearchRunner::new(&circuit, &uncalibrated_target, &cfg);
    let err = runner
        .run()
        .expect_err("Runner must fail closed when timing model is missing");

    match err {
        RoutingSearchExecutionError::Config(RoutingSearchConfigError::MissingTimingModel {
            metric,
            target,
        }) => {
            assert_eq!(metric, ObjectiveMetric::EstimatedDuration);
            assert_eq!(target, "linear_5q");
        }
        other => panic!("Expected MissingTimingModel error, got: {other:?}"),
    }
}

// ============================================================================
// 7. Source-Level Audit of Multi-Candidate Search Execution across Budgets
// ============================================================================

#[test]
fn test_audit_candidate_search_execution() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    for budget in [1, 4, 8, 16] {
        let cfg = MultiCandidateRoutingConfig::builder()
            .candidate_count(budget)
            .seed_strategy(SeedStrategy::Derived {
                base_seed: 42,
                count: budget,
            })
            .objective(RoutingObjective::minimize_swaps())
            .build()
            .expect("config build");

        let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
        let report = runner.run().expect("search run");

        assert_eq!(report.total_requested, budget);
        assert_eq!(report.total_executed, budget);
        assert_eq!(report.total_succeeded, budget);
        assert_eq!(report.total_validated, budget);

        let ranking = report.ranking.as_ref().expect("ranking report");
        assert_eq!(ranking.total_candidates, budget);
        assert_eq!(ranking.eligible_count, budget);
        assert_eq!(ranking.ineligible_count, 0);

        // Prove designated baseline candidate is always included at candidate index 0
        let baseline_cand = report
            .candidates
            .iter()
            .find(|c| c.is_baseline)
            .expect("baseline candidate must exist");
        assert_eq!(baseline_cand.descriptor.identity.candidate_index, 0);

        // Baseline config matches single-candidate mode
        let base_sabre = cfg.baseline_sabre_config();
        assert_eq!(
            baseline_cand.descriptor.config.random_seed,
            base_sabre.random_seed
        );
        assert_eq!(
            baseline_cand.descriptor.config.placement_strategy,
            base_sabre.placement_strategy
        );

        // Verify distinct candidate IDs and seeds across all candidates in the batch
        let mut ids = std::collections::HashSet::new();
        let mut seeds = std::collections::HashSet::new();
        for c in &report.candidates {
            assert!(
                ids.insert(c.descriptor.identity.id.clone()),
                "Duplicate candidate ID"
            );
            assert!(seeds.insert(c.descriptor.identity.seed), "Duplicate seed");
            assert!(c.validation.is_fully_validated());
        }
        assert_eq!(ids.len(), budget);
        assert_eq!(seeds.len(), budget);
    }
}
