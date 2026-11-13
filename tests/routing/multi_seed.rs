//! Integration tests for Phase 75: Multi-Run SABRE Candidate Generation Engine.
//!
//! Verifies candidate isolation, deterministic identity preservation, cumulative budget
//! enforcement (swaps, iterations, cooperative timeouts), partial resource accounting on failure,
//! independent validation evidence, fallback policies, and provisional selection.

use std::time::Duration;

use qcc::backend::openqasm::emit_openqasm;
use qcc::builder::CircuitBuilder;
use qcc::compiler::config::{CompilerConfig, RouterChoice};
use qcc::compiler::pipeline::CompilerPipeline;
use qcc::ir::circuit::Circuit;
use qcc::routing::placement::PlacementStrategy;
use qcc::routing::sabre::SabreRouterConfig;
use qcc::routing::search::{
    BudgetExhaustionReason, CandidateExecutionStatus, CheckStatus, FallbackPolicy, LayoutStrategy,
    MultiCandidateRoutingConfig, MultiCandidateSearchRunner, RoutingObjective,
    RoutingSearchExecutionError, SearchBudget, SeedStrategy,
};
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};

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

/// Builds a 5-qubit ring target: 0 - 1 - 2 - 3 - 4 - 0.
fn make_5q_ring_target() -> Target {
    let mut builder = TargetBuilder::new("ring_5q", 5);
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
        (PhysicalQubitId(4), PhysicalQubitId(0)),
    ];
    builder = builder.add_operation(
        TargetOperationBuilder::new("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(couplings))
            .build(),
    );
    builder.build().expect("build 5q ring target")
}

/// Builds an 18-qubit target to test the >16 physical qubit boundary.
fn make_18q_linear_target() -> Target {
    let mut builder = TargetBuilder::new("linear_18q", 18);
    for gate in ["rx", "ry", "rz", "h", "x", "sx"] {
        builder = builder.add_operation(
            TargetOperationBuilder::new(gate, 1)
                .connectivity(OperationConnectivity::all_single_qubits(18))
                .build(),
        );
    }
    let couplings: Vec<(PhysicalQubitId, PhysicalQubitId)> = (0..17)
        .map(|i| (PhysicalQubitId(i), PhysicalQubitId(i + 1)))
        .collect();
    builder = builder.add_operation(
        TargetOperationBuilder::new("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(couplings))
            .build(),
    );
    builder.build().expect("build 18q target")
}

/// Builds a 4-qubit circuit with line-compatible connectivity.
fn make_simple_circuit() -> Circuit {
    let mut builder = CircuitBuilder::new("simple_cx");
    let q = builder.add_quantum_register("q", 4).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[1], q[2]).unwrap();
    builder.cx(q[2], q[3]).unwrap();
    builder.finish().unwrap()
}

/// Builds a 4-qubit complete graph (K4) circuit: every pair interacts.
/// Mathematically impossible to route on a 1D linear target without inserting SWAPs!
fn make_k4_circuit() -> Circuit {
    let mut builder = CircuitBuilder::new("k4_circuit");
    let q = builder.add_quantum_register("q", 4).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[1], q[2]).unwrap();
    builder.cx(q[2], q[3]).unwrap();
    builder.cx(q[3], q[0]).unwrap();
    builder.cx(q[0], q[2]).unwrap();
    builder.cx(q[1], q[3]).unwrap();
    builder.finish().unwrap()
}

#[test]
fn test_single_candidate_compatibility() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let cfg = MultiCandidateRoutingConfig::default();
    assert!(cfg.is_single_candidate());

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner
        .run()
        .expect("Single-candidate search should succeed");

    assert_eq!(report.total_requested, 1);
    assert_eq!(report.total_executed, 1);
    assert_eq!(report.total_succeeded, 1);
    assert_eq!(report.total_validated, 1);
    assert!(report.budget_exhaustion.is_none());

    let candidate = &report.candidates[0];
    assert!(candidate.is_baseline);
    assert!(candidate.status.is_success());
    assert!(candidate.validation.is_fully_validated());
    assert_eq!(candidate.validation.target_legality, CheckStatus::Passed);
    assert_eq!(
        candidate.validation.semantic_equivalence,
        CheckStatus::Passed
    );
    assert_eq!(candidate.validation.ancilla_zero_state, CheckStatus::Passed);

    let provisional = report.select_provisional_candidate().unwrap();
    assert_eq!(provisional.candidate_id, candidate.descriptor.identity.id);
}

#[test]
fn test_multi_seed_identity_uniqueness_and_isolation() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();
    let orig_ops = circuit.num_operations();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 42,
            count: 3,
        })
        .layout_strategy(LayoutStrategy::Single(PlacementStrategy::Weighted))
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner.run().expect("Multi-seed search should succeed");

    assert_eq!(report.total_requested, 3);
    assert_eq!(report.total_executed, 3);
    assert_eq!(report.total_succeeded, 3);

    // Verify candidate isolation: original circuit was not mutated
    assert_eq!(circuit.num_operations(), orig_ops);

    // Verify distinct seeds and distinct candidate IDs
    let ids: Vec<_> = report
        .candidates
        .iter()
        .map(|c| &c.descriptor.identity.id)
        .collect();
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
    assert_ne!(ids[0], ids[2]);

    for c in &report.candidates {
        assert!(c.validation.is_fully_validated());
        assert!(c.total_iterations > 0);
    }
}

#[test]
fn test_multi_layout_generation_and_execution() {
    let target = make_5q_linear_target();
    let circuit = make_simple_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .layout_strategy(LayoutStrategy::Strategies(vec![
            PlacementStrategy::Identity,
            PlacementStrategy::Degree,
            PlacementStrategy::Weighted,
        ]))
        .seed_strategy(SeedStrategy::Single(Some(123)))
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner.run().expect("Multi-layout search should succeed");

    assert_eq!(report.total_executed, 3);
    assert_eq!(
        report.candidates[0].descriptor.identity.placement_strategy,
        PlacementStrategy::Identity
    );
    assert_eq!(
        report.candidates[1].descriptor.identity.placement_strategy,
        PlacementStrategy::Degree
    );
    assert_eq!(
        report.candidates[2].descriptor.identity.placement_strategy,
        PlacementStrategy::Weighted
    );

    for c in &report.candidates {
        assert!(c.status.is_success());
        assert!(c.validation.is_fully_validated());
    }
}

#[test]
fn test_candidate_exceeding_budget_fails_and_not_validated() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let sabre_base = SabreRouterConfig {
        max_swaps: 0,
        ..Default::default()
    }; // Impossible: K4 on a line requires SWAPs

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(1)
        .sabre_base_config(sabre_base)
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let err = runner.run().expect_err("Zero-swap budget must fail");

    match err {
        RoutingSearchExecutionError::AllCandidatesFailed { report, .. } => {
            let candidate = &report.candidates[0];
            assert!(matches!(
                candidate.status,
                CandidateExecutionStatus::Failed { .. }
            ));
            assert!(!candidate.validation.is_fully_validated());
            match &candidate.validation.target_legality {
                CheckStatus::Failed { reason } => {
                    assert!(reason.contains("inserted SWAPs exceeded limit of 0"));
                }
                other => panic!("Expected Failed target legality, got: {other:?}"),
            }
        }
        other => panic!("Expected AllCandidatesFailed, got: {other:?}"),
    }
}

#[test]
fn test_partial_resource_consumption_preserved_on_failure() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let sabre_base = SabreRouterConfig {
        max_swaps: 0,
        ..Default::default()
    }; // Exceeds immediately on first needed SWAP

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(1)
        .sabre_base_config(sabre_base)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let err = runner.run().expect_err("Must fail due to max_swaps = 0");

    let report = err
        .execution_report()
        .expect("Execution report must be preserved");
    assert_eq!(report.candidates.len(), 1);
    let cand = &report.candidates[0];

    match &cand.status {
        CandidateExecutionStatus::Failed {
            error,
            partial_swaps,
            partial_iterations,
        } => {
            assert!(error.contains("inserted SWAPs exceeded limit of 0"));
            assert_eq!(*partial_swaps, 0);
            assert!(*partial_iterations > 0);
        }
        other => panic!("Expected Failed status, got: {other:?}"),
    }
}

#[test]
fn test_cumulative_iteration_budget_enforcement() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    // Set max_total_iterations very low (e.g. 1) so candidate 0 exhausts it
    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(2)
        .budget(SearchBudget {
            max_candidates: 64,
            max_total_swaps: None,
            max_total_iterations: Some(1),
            timeout_ms: None,
        })
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let res = runner.run();

    let report = match &res {
        Ok(r) => r,
        Err(e) => e.execution_report().expect("report preserved"),
    };

    assert_eq!(report.candidates.len(), 2);
    assert!(report.budget_exhaustion.is_some());
    match &report.budget_exhaustion {
        Some(BudgetExhaustionReason::MaxTotalIterationsExceeded { limit, .. }) => {
            assert_eq!(*limit, 1);
        }
        other => panic!("Expected MaxTotalIterationsExceeded, got: {other:?}"),
    }
    assert!(matches!(
        report.candidates[1].status,
        CandidateExecutionStatus::SkippedBudgetExceeded { .. }
    ));
}

#[test]
fn test_cumulative_swap_budget_enforcement() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    // Set max_total_swaps = 1: K4 needs 2+ SWAPs, so candidate 0 consumes it and candidate 1 is skipped
    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(2)
        .budget(SearchBudget {
            max_candidates: 64,
            max_total_swaps: Some(1),
            max_total_iterations: None,
            timeout_ms: None,
        })
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let res = runner.run();

    let report = match &res {
        Ok(r) => r,
        Err(e) => e.execution_report().expect("report preserved"),
    };

    assert_eq!(report.candidates.len(), 2);
    assert!(report.budget_exhaustion.is_some());
    match &report.budget_exhaustion {
        Some(BudgetExhaustionReason::MaxTotalSwapsExceeded { limit, .. }) => {
            assert_eq!(*limit, 1);
        }
        other => panic!("Expected MaxTotalSwapsExceeded, got: {other:?}"),
    }
    assert!(matches!(
        report.candidates[1].status,
        CandidateExecutionStatus::SkippedBudgetExceeded { .. }
    ));
}

#[test]
fn test_timeout_cooperative_deadline_between_candidates() {
    let target = make_5q_linear_target();
    let circuit = make_simple_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(2)
        .budget(SearchBudget {
            max_candidates: 64,
            max_total_swaps: None,
            max_total_iterations: None,
            timeout_ms: Some(1), // 1 millisecond
        })
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .unwrap();

    // Small delay to ensure timeout elapses before subsequent candidate
    std::thread::sleep(Duration::from_millis(5));

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let res = runner.run();

    let report = match &res {
        Ok(r) => r,
        Err(e) => e.execution_report().expect("report preserved"),
    };

    assert_eq!(report.candidates.len(), 2);
    assert!(report.budget_exhaustion.is_some());
    match &report.budget_exhaustion {
        Some(BudgetExhaustionReason::Timeout { timeout_ms, .. }) => {
            assert_eq!(*timeout_ms, 1);
        }
        other => panic!("Expected Timeout exhaustion, got: {other:?}"),
    }
}

#[test]
fn test_fallback_policy_abort() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let sabre_base = SabreRouterConfig {
        max_swaps: 0,
        ..Default::default()
    }; // Candidate will fail

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(1)
        .sabre_base_config(sabre_base)
        .fallback_policy(FallbackPolicy::Abort)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let err = runner
        .run()
        .expect_err("Abort policy must abort on failure");

    match err {
        RoutingSearchExecutionError::Aborted { reason, report } => {
            assert!(reason.contains("failed routing execution"));
            assert_eq!(report.candidates.len(), 1);
            assert!(matches!(
                report.candidates[0].status,
                CandidateExecutionStatus::Failed { .. }
            ));
        }
        other => panic!("Expected Aborted error, got: {other:?}"),
    }
}

#[test]
fn test_fallback_policy_deterministic_baseline_valid() {
    let target = make_5q_linear_target();
    let circuit = make_simple_circuit();

    // Baseline succeeds, but swap budget bounds candidate 1
    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(2)
        .budget(SearchBudget {
            max_candidates: 64,
            max_total_swaps: Some(1),
            max_total_iterations: None,
            timeout_ms: None,
        })
        .fallback_policy(FallbackPolicy::DeterministicBaseline)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner
        .run()
        .expect("Baseline fallback must succeed if baseline is valid");

    assert_eq!(
        report.provisional_candidate_id,
        Some(report.candidates[0].descriptor.identity.id.clone())
    );
    assert!(report.candidates[0].is_baseline);
    assert!(report.candidates[0].validation.is_fully_validated());
}

#[test]
fn test_invalid_baseline_cannot_be_used_as_fallback() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let sabre_base = SabreRouterConfig {
        max_swaps: 0,
        ..Default::default()
    }; // Baseline candidate 0 will fail

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(2)
        .sabre_base_config(sabre_base)
        .fallback_policy(FallbackPolicy::DeterministicBaseline)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let err = runner
        .run()
        .expect_err("Invalid baseline must NOT be used as fallback");

    match err {
        RoutingSearchExecutionError::InvalidBaselineFallback { reason, report } => {
            assert!(reason.contains("baseline candidate routing failed"));
            assert_eq!(report.candidates.len(), 2);
            assert!(matches!(
                report.candidates[0].status,
                CandidateExecutionStatus::Failed { .. }
            ));
        }
        other => panic!("Expected InvalidBaselineFallback, got: {other:?}"),
    }
}

#[test]
fn test_later_valid_candidate_survives_earlier_candidate_failure() {
    let target = make_5q_linear_target();
    let circuit = make_simple_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(2)
        .layout_strategy(LayoutStrategy::Strategies(vec![
            PlacementStrategy::Identity, // Candidate 0
            PlacementStrategy::Weighted, // Candidate 1
        ]))
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner
        .run()
        .expect("BestValidSoFar selects valid candidate");

    assert_eq!(report.total_succeeded, 2);
    assert_eq!(report.total_validated, 2);
    let provisional = report.select_provisional_candidate().unwrap();
    // Prefer baseline if valid
    assert_eq!(
        provisional.candidate_id,
        report.candidates[0].descriptor.identity.id
    );
}

#[test]
fn test_independent_representation_of_three_mandatory_constraints() {
    let target = make_5q_linear_target();
    let circuit = make_simple_circuit();

    let cfg = MultiCandidateRoutingConfig::default();
    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner.run().unwrap();

    let cand = &report.candidates[0];
    assert_eq!(cand.validation.target_legality, CheckStatus::Passed);
    assert_eq!(cand.validation.semantic_equivalence, CheckStatus::Passed);
    assert_eq!(cand.validation.ancilla_zero_state, CheckStatus::Passed);
    assert!(cand.validation.is_fully_validated());
}

#[test]
fn test_no_unvalidated_candidate_is_returned_by_pipeline() {
    let target = make_18q_linear_target(); // > 16 qubits
    let mut builder = CircuitBuilder::new("circ_18q");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().unwrap();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(1)
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let err = runner
        .run()
        .expect_err("Targets > 16 qubits cannot validate semantic equivalence");

    match err {
        RoutingSearchExecutionError::NoValidCandidates {
            total_executed,
            report,
        } => {
            assert_eq!(total_executed, 1);
            let cand = &report.candidates[0];
            assert_eq!(cand.validation.target_legality, CheckStatus::Passed);
            // Must be truthfully reported as Unexecuted
            assert!(matches!(
                cand.validation.semantic_equivalence,
                CheckStatus::Unexecuted { .. }
            ));
            assert!(matches!(
                cand.validation.ancilla_zero_state,
                CheckStatus::Unexecuted { .. }
            ));
            assert!(!cand.validation.is_fully_validated());
        }
        other => panic!("Expected NoValidCandidates, got: {other:?}"),
    }
}

#[test]
fn test_provisional_selection_by_stable_id() {
    let target = make_5q_linear_target();
    let circuit = make_simple_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(2)
        .seed_strategy(SeedStrategy::Range {
            start: 100,
            step: 5,
            count: 2,
        })
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner.run().unwrap();

    let selection = report.select_provisional_candidate().unwrap();
    assert_eq!(
        selection.candidate_id,
        report.candidates[0].descriptor.identity.id
    );
    assert_eq!(
        selection.descriptor.identity.id,
        report.candidates[0].descriptor.identity.id
    );
    assert_eq!(
        selection.routed_circuit.name,
        format!("{}_sabre", circuit.name)
    );
}

#[test]
fn test_deterministic_provisional_selection_is_reproducible() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 777,
            count: 3,
        })
        .build()
        .unwrap();

    let runner1 = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let rep1 = runner1.run().unwrap();

    let runner2 = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let rep2 = runner2.run().unwrap();

    assert_eq!(rep1.candidates.len(), rep2.candidates.len());
    for i in 0..rep1.candidates.len() {
        assert_eq!(
            rep1.candidates[i].descriptor.identity.id,
            rep2.candidates[i].descriptor.identity.id
        );
        assert_eq!(
            rep1.candidates[i].swaps_inserted,
            rep2.candidates[i].swaps_inserted
        );
        assert_eq!(
            rep1.candidates[i].total_iterations,
            rep2.candidates[i].total_iterations
        );
        assert_eq!(rep1.candidates[i].validation, rep2.candidates[i].validation);
    }
    assert_eq!(rep1.provisional_candidate_id, rep2.provisional_candidate_id);
}

#[test]
fn test_pipeline_integration_multi_candidate() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let search_cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(2)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 999,
            count: 2,
        })
        .build()
        .unwrap();

    let compiler_cfg =
        CompilerConfig::o2().with_router(RouterChoice::MultiCandidate(Box::new(search_cfg)));

    let pipeline = CompilerPipeline::new(compiler_cfg);
    let result = pipeline
        .compile(&circuit, &target)
        .expect("Multi-candidate compilation must succeed");

    assert!(result.report.metrics.final_gate_count > 0);
    let stage_rep = &result.report.routing;
    assert!(stage_rep
        .router_name
        .contains("SabreBidirectionalRouter [MultiCandidate Provisional: cand_000_"));
    assert!(stage_rep.swaps_inserted > 0);
}

#[test]
fn test_cumulative_swap_limit_shared_across_refinement_passes() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    // With refinement_rounds: 3, forward pass needs ~3 SWAPs and reverse pass also needs SWAPs.
    // If each pass could independently consume max_swaps, a limit of 4 would allow 4 + 4 = 8 SWAPs.
    // But because max_swaps is shared across passes, reverse pass fails when exceeding remaining budget.
    let sabre_cfg = SabreRouterConfig {
        refinement_rounds: 3,
        max_swaps: 4,
        ..Default::default()
    };

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(1)
        .sabre_base_config(sabre_cfg)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let err = runner
        .run()
        .expect_err("Must fail because cumulative swaps across refinement rounds exceed 4");

    let report = err.execution_report().expect("Execution report preserved");
    let cand = &report.candidates[0];

    match &cand.status {
        CandidateExecutionStatus::Failed {
            error,
            partial_swaps,
            partial_iterations,
        } => {
            assert!(error.contains("limit of 4"));
            // Strict upper bound: under limit N=4, at most 4 SWAPs can ever be executed across all passes
            assert_eq!(
                *partial_swaps, 4,
                "total executed SWAPs must strictly equal limit 4, got {}",
                *partial_swaps
            );
            assert!(*partial_iterations > 0);
            assert_eq!(cand.total_swaps_executed, 4);
            assert_eq!(cand.total_iterations, *partial_iterations);
        }
        other => panic!("Expected Failed status, got: {other:?}"),
    }
}

#[test]
fn test_failed_candidate_preserves_accumulated_work_across_multiple_passes() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let sabre_cfg = SabreRouterConfig {
        refinement_rounds: 3,
        max_swaps: 4,
        ..Default::default()
    };

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(2)
        .sabre_base_config(sabre_cfg)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let err = runner.run().expect_err("Must fail");
    let report = err.execution_report().expect("Report preserved");

    // Both candidates failed; verify each preserved its exact work and search totals accumulated them
    assert_eq!(report.candidates.len(), 2);
    let c0 = &report.candidates[0];
    let c1 = &report.candidates[1];

    assert!(c0.total_swaps_executed >= 3);
    assert!(c0.total_iterations > 0);
    assert!(c1.total_swaps_executed >= 3);
    assert!(c1.total_iterations > 0);

    assert_eq!(
        report.cumulative_swaps,
        c0.total_swaps_executed + c1.total_swaps_executed
    );
    assert_eq!(
        report.cumulative_iterations,
        c0.total_iterations + c1.total_iterations
    );
}

#[test]
fn test_explicit_baseline_identity_preservation_and_selection() {
    let target = make_5q_linear_target();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 42,
            count: 3,
        })
        .layout_strategy(LayoutStrategy::Strategies(vec![
            PlacementStrategy::Weighted,
            PlacementStrategy::Degree,
            PlacementStrategy::Random { seed: 100 },
        ]))
        .build()
        .unwrap();

    let descriptors = cfg.derive_candidate_descriptors(&target).unwrap();
    assert_eq!(descriptors.len(), 3);

    // Candidate 0 is explicitly designated as baseline
    assert!(descriptors[0].is_baseline);
    assert_eq!(
        descriptors[0].config.placement_strategy,
        cfg.sabre_base_config.placement_strategy
    );

    // Candidates 1 and 2 are not baseline
    assert!(!descriptors[1].is_baseline);
    assert!(!descriptors[2].is_baseline);

    // Run search with DeterministicBaseline policy
    let circuit = make_simple_circuit();
    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner.run().expect("Simple circuit routes successfully");

    // Must have chosen the baseline candidate
    assert_eq!(
        report.provisional_candidate_id,
        Some(descriptors[0].identity.id.clone())
    );
}

#[test]
fn test_provisional_selection_policy_no_objective_scoring() {
    let target = make_5q_linear_target();
    let circuit = make_simple_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(2)
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner.run().expect("Search succeeds");

    // In Phase 75 provisional selection:
    // 1. Valid baseline is preferred
    let sel = report.select_provisional_candidate().unwrap();
    assert_eq!(
        sel.candidate_id,
        report.candidates[0].descriptor.identity.id
    );
    assert!(sel.record.is_baseline);
}

#[test]
fn test_baseline_identity_survives_reordering_and_deterministic_selection() {
    let target = make_5q_linear_target();
    let circuit = make_simple_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 42,
            count: 3,
        })
        .layout_strategy(LayoutStrategy::Strategies(vec![
            PlacementStrategy::Weighted,
            PlacementStrategy::Degree,
            PlacementStrategy::Random { seed: 100 },
        ]))
        .build()
        .unwrap();

    let mut descriptors = cfg.derive_candidate_descriptors(&target).unwrap();
    assert!(descriptors[0].is_baseline);
    assert!(!descriptors[1].is_baseline);
    assert!(!descriptors[2].is_baseline);
    let baseline_id = descriptors[0].identity.id.clone();

    // 1. Reorder descriptors: reverse them so the baseline is at the end (index 2)
    descriptors.reverse();
    assert!(!descriptors[0].is_baseline);
    assert!(!descriptors[1].is_baseline);
    assert!(descriptors[2].is_baseline);
    assert_eq!(descriptors[2].identity.id, baseline_id);

    // 2. Execute with reordered descriptors: provisional selection must STILL prefer the baseline
    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner
        .run_with_descriptors(descriptors.clone())
        .expect("Search must succeed");

    let sel = report.select_provisional_candidate().unwrap();
    assert_eq!(sel.candidate_id, baseline_id);
    assert!(sel.record.is_baseline);
    assert_eq!(report.provisional_candidate_id, Some(baseline_id));

    // 3. If baseline candidate is invalid (e.g. failed max_swaps budget),
    // provisional selection must choose the FIRST fully validated candidate in the supplied deterministic execution order
    let mut invalid_baseline_descriptors = descriptors;
    // Set max_swaps = 0 on the baseline candidate (now at index 2) using a circuit needing swaps (k4)
    invalid_baseline_descriptors[2].config.max_swaps = 0;

    let k4_circuit = make_k4_circuit();
    let runner_k4 = MultiCandidateSearchRunner::new(&k4_circuit, &target, &cfg);
    let report_k4 = runner_k4
        .run_with_descriptors(invalid_baseline_descriptors)
        .expect("Search must succeed using other valid candidates");

    // The baseline candidate (index 2) failed routing due to max_swaps = 0
    assert!(matches!(
        report_k4.candidates[2].status,
        CandidateExecutionStatus::Failed { .. }
    ));
    assert!(report_k4.candidates[2].is_baseline);

    // Phase 76: Selection chooses the highest-ranked valid candidate according to the objective (lowest swaps)
    let sel_k4 = report_k4.select_provisional_candidate().unwrap();
    let expected_winner_id = report_k4
        .ranking
        .as_ref()
        .and_then(|r| r.selected_candidate_id.clone())
        .expect("Winner must be selected by Phase 76 ranking");
    assert_eq!(sel_k4.candidate_id, expected_winner_id);
    assert!(!sel_k4.record.is_baseline);
    assert_eq!(report_k4.provisional_candidate_id, Some(expected_winner_id));
}

fn make_k5_circuit() -> Circuit {
    let mut builder = CircuitBuilder::new("k5_circuit");
    let q = builder.add_quantum_register("q", 5).unwrap();
    for i in 0..5 {
        for j in (i + 1)..5 {
            builder.cx(q[i], q[j]).unwrap();
        }
    }
    builder.finish().unwrap()
}

#[test]
fn test_seeded_candidates_produce_distinct_routing_trajectories_on_symmetric_workload() {
    let ring = make_5q_ring_target();
    let circuit = make_k5_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(8)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 42,
            count: 8,
        })
        .objective(RoutingObjective::minimize_swaps())
        .build()
        .expect("config build");

    let runner = MultiCandidateSearchRunner::new(&circuit, &ring, &cfg);
    let report1 = runner.run().expect("run 1");
    assert_eq!(report1.total_succeeded, 8);

    // Collect routed circuits as emitted QASM strings
    let mut distinct_circuits = std::collections::HashSet::new();
    for c in &report1.candidates {
        if let CandidateExecutionStatus::Success { routed_circuit, .. } = &c.status {
            distinct_circuits.insert(emit_openqasm(routed_circuit).unwrap());
        }
    }

    // Assert that candidates produced genuinely distinct routing trajectories
    assert!(
        distinct_circuits.len() > 1,
        "Multi-candidate search with seeded tie-breaking must produce at least 2 distinct routing trajectories across 8 candidates, got {}",
        distinct_circuits.len()
    );

    // Repeated execution with identical seeds reproduces identical complete outputs
    let report2 = runner.run().expect("run 2");
    for i in 0..8 {
        let qasm1 = match &report1.candidates[i].status {
            CandidateExecutionStatus::Success { routed_circuit, .. } => {
                emit_openqasm(routed_circuit).unwrap()
            }
            _ => panic!("candidate must succeed"),
        };
        let qasm2 = match &report2.candidates[i].status {
            CandidateExecutionStatus::Success { routed_circuit, .. } => {
                emit_openqasm(routed_circuit).unwrap()
            }
            _ => panic!("candidate must succeed"),
        };
        assert_eq!(
            qasm1, qasm2,
            "Candidate {i} must reproduce identical output across runs"
        );
    }
}

#[test]
fn test_baseline_compatibility_b1_matches_candidate0_of_b4() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    // 1. Single candidate run (B=1)
    let cfg_b1 = MultiCandidateRoutingConfig::builder()
        .candidate_count(1)
        .build()
        .unwrap();
    let runner_b1 = MultiCandidateSearchRunner::new(&circuit, &target, &cfg_b1);
    let report_b1 = runner_b1.run().expect("B=1 run succeeds");
    let sel_b1 = report_b1
        .select_candidate()
        .expect("B=1 selection succeeds");

    // 2. Multi-candidate run (B=4)
    let cfg_b4 = MultiCandidateRoutingConfig::builder()
        .candidate_count(4)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 42,
            count: 4,
        })
        .build()
        .unwrap();
    let runner_b4 = MultiCandidateSearchRunner::new(&circuit, &target, &cfg_b4);
    let report_b4 = runner_b4.run().expect("B=4 run succeeds");
    let cand0 = &report_b4.candidates[0];

    assert!(cand0.is_baseline, "Candidate 0 must be designated baseline");

    // 3. Compare B=1 with Candidate 0 of B=4
    if let (
        CandidateExecutionStatus::Success {
            routed_circuit: rc1,
            report: rep1,
        },
        CandidateExecutionStatus::Success {
            routed_circuit: rc4,
            report: rep4,
        },
    ) = (&sel_b1.record.status, &cand0.status)
    {
        assert_eq!(rep1.swaps_inserted, rep4.swaps_inserted);
        assert_eq!(rep1.total_iterations, rep4.total_iterations);
        assert_eq!(rep1.layout.initial(), rep4.layout.initial());
        assert_eq!(rep1.layout.final_layout(), rep4.layout.final_layout());
        assert_eq!(rc1.num_operations(), rc4.num_operations());
        assert_eq!(emit_openqasm(rc1).unwrap(), emit_openqasm(rc4).unwrap());
    } else {
        panic!("Both B=1 and Candidate 0 must succeed");
    }
}

#[test]
fn test_best_valid_so_far_never_worse_than_baseline() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(4)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 42,
            count: 4,
        })
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner.run().expect("run succeeds");

    let sel = report.select_candidate().expect("select succeeds");
    let baseline = report
        .candidates
        .iter()
        .find(|c| c.is_baseline)
        .expect("baseline exists");

    // The selected candidate must be valid
    assert!(sel.record.validation.is_fully_validated());

    // The selected candidate's swaps must be <= baseline's swaps
    assert!(
        sel.sabre_report.swaps_inserted <= baseline.swaps_inserted,
        "BestValidSoFar must never select a candidate worse than baseline"
    );
}

#[test]
fn test_fallback_when_baseline_is_invalid() {
    let target = make_5q_linear_target();
    let circuit = make_k4_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(3)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 42,
            count: 3,
        })
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .unwrap();

    let mut descriptors = cfg.derive_candidate_descriptors(&target).unwrap();
    // Invalidate baseline candidate (index 0) by setting max_swaps = 0
    assert!(descriptors[0].is_baseline);
    descriptors[0].config.max_swaps = 0;

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner
        .run_with_descriptors(descriptors)
        .expect("Search must succeed using other valid candidates");

    // Baseline failed
    assert!(matches!(
        report.candidates[0].status,
        CandidateExecutionStatus::Failed { .. }
    ));
    assert!(report.candidates[0].is_baseline);

    // Selected candidate is NOT the invalid baseline, but a valid candidate
    let sel = report
        .select_candidate()
        .expect("select valid non-baseline");
    assert!(!sel.record.is_baseline);
    assert!(sel.record.validation.is_fully_validated());
}
