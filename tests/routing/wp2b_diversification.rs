//! WP2b Integration Tests: Multi-Candidate Diversification (Roadmap v3.1, Milestone M9).
//!
//! Verifies:
//! 1. B=1 matches the deterministic baseline 100% bit-for-bit.
//! 2. B >= 4 derives distinct initial placements (Weighted, Identity, Random{seed}) and varied SABRE hyperparameters.
//! 3. B=16 derivation is 100% deterministically reproducible across independent runs.
//! 4. Multi-candidate execution satisfies all mandatory hard constraints (TargetLegality, SemanticEquivalence, AncillaZeroState).

use qcc::builder::CircuitBuilder;
use qcc::ir::circuit::Circuit;
use qcc::routing::placement::PlacementStrategy;
use qcc::routing::search::config::{
    MULTI_CANDIDATE_DECAY_DELTAS, MULTI_CANDIDATE_LOOKAHEAD_SIZES,
    MULTI_CANDIDATE_LOOKAHEAD_WEIGHTS,
};
use qcc::routing::search::{
    FallbackPolicy, MultiCandidateRoutingConfig, MultiCandidateSearchRunner, RoutingObjective,
    SeedStrategy,
};
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};
use qcc::target::OperationConnectivity;
use qcc::ParameterDomain;

fn make_test_target(n: usize) -> Target {
    let mut builder = TargetBuilder::new("linear_target", n);
    let sx = TargetOperationBuilder::new("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(n))
        .duration(30.0)
        .build();
    let rz = TargetOperationBuilder::new("rz", 1)
        .add_parameter_domain(ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(n))
        .duration(0.0)
        .build();
    let meas = TargetOperationBuilder::new("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(n))
        .duration(500.0)
        .build();
    let reset = TargetOperationBuilder::new("reset", 1)
        .connectivity(OperationConnectivity::all_single_qubits(n))
        .duration(1000.0)
        .build();

    builder = builder
        .add_operation(sx)
        .add_operation(rz)
        .add_operation(meas)
        .add_operation(reset);

    let mut edges = Vec::new();
    for i in 0..n.saturating_sub(1) {
        edges.push((PhysicalQubitId(i as u32), PhysicalQubitId((i + 1) as u32)));
    }
    if !edges.is_empty() {
        let cx = TargetOperationBuilder::new("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges))
            .duration(150.0)
            .build();
        builder = builder.add_operation(cx);
    }
    builder.build().unwrap()
}

fn make_test_circuit() -> Circuit {
    let mut builder = CircuitBuilder::new("test_circuit");
    let q = builder.add_quantum_register("q", 4).unwrap();
    // Non-adjacent CX gates that require SWAP routing on a line
    builder.cx(q[0], q[2]).unwrap();
    builder.cx(q[1], q[3]).unwrap();
    builder.cx(q[0], q[3]).unwrap();
    builder.cx(q[1], q[2]).unwrap();
    builder.finish().unwrap()
}

#[test]
fn test_b1_matches_deterministic_baseline_identically() {
    let target = make_test_target(4);
    let circuit = make_test_circuit();

    let cfg_b1 = MultiCandidateRoutingConfig::builder()
        .candidate_count(1)
        .build()
        .unwrap();

    let desc = cfg_b1.derive_candidate_descriptors(&target).unwrap();
    assert_eq!(desc.len(), 1);
    assert!(desc[0].is_baseline);
    assert_eq!(desc[0].identity.candidate_index, 0);
    assert_eq!(
        desc[0].config.placement_strategy,
        PlacementStrategy::Weighted
    );
    assert_eq!(desc[0].config.random_seed, None);
    assert_eq!(desc[0].config.lookahead_config.max_size, 20);
    assert_eq!(desc[0].config.heuristic_config.lookahead_weight, 0.5);
    assert_eq!(desc[0].config.heuristic_config.decay_delta, 0.001);

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg_b1);
    let report = runner.run().expect("B=1 routing succeeds");
    assert_eq!(report.total_executed, 1);
    assert_eq!(report.total_succeeded, 1);
    assert_eq!(report.total_validated, 1);
    assert!(report.candidates[0].is_baseline);
    assert!(report.candidates[0].validation.is_fully_validated());
}

#[test]
fn test_b4_diversified_initial_layouts_and_hyperparameters() {
    let target = make_test_target(5);

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(4)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 42,
            count: 4,
        })
        .build()
        .unwrap();

    let desc = cfg.derive_candidate_descriptors(&target).unwrap();
    assert_eq!(desc.len(), 4);

    // Candidate 0: Baseline
    assert!(desc[0].is_baseline);
    assert_eq!(desc[0].identity.candidate_index, 0);
    assert_eq!(
        desc[0].config.placement_strategy,
        PlacementStrategy::Weighted
    );
    assert_eq!(desc[0].config.random_seed, None);
    assert_eq!(desc[0].config.lookahead_config.max_size, 20);
    assert_eq!(desc[0].config.heuristic_config.lookahead_weight, 0.5);
    assert_eq!(desc[0].config.heuristic_config.decay_delta, 0.001);

    // Candidate 1: Trivial (Identity) placement + varied hyperparameters
    assert!(!desc[1].is_baseline);
    assert_eq!(desc[1].identity.candidate_index, 1);
    assert_eq!(
        desc[1].config.placement_strategy,
        PlacementStrategy::Identity
    );
    assert!(desc[1].config.random_seed.is_some());
    assert_eq!(
        desc[1].config.lookahead_config.max_size,
        MULTI_CANDIDATE_LOOKAHEAD_SIZES[1]
    );
    assert_eq!(
        desc[1].config.heuristic_config.lookahead_weight,
        MULTI_CANDIDATE_LOOKAHEAD_WEIGHTS[1]
    );
    assert_eq!(
        desc[1].config.heuristic_config.decay_delta,
        MULTI_CANDIDATE_DECAY_DELTAS[1]
    );

    // Candidate 2: Seeded Random placement + varied hyperparameters
    assert!(!desc[2].is_baseline);
    assert_eq!(desc[2].identity.candidate_index, 2);
    assert!(matches!(
        desc[2].config.placement_strategy,
        PlacementStrategy::Random { .. }
    ));
    assert!(desc[2].config.random_seed.is_some());
    assert_eq!(
        desc[2].config.lookahead_config.max_size,
        MULTI_CANDIDATE_LOOKAHEAD_SIZES[2]
    );
    assert_eq!(
        desc[2].config.heuristic_config.lookahead_weight,
        MULTI_CANDIDATE_LOOKAHEAD_WEIGHTS[2]
    );
    assert_eq!(
        desc[2].config.heuristic_config.decay_delta,
        MULTI_CANDIDATE_DECAY_DELTAS[2]
    );

    // Candidate 3: Seeded Random placement + varied hyperparameters
    assert!(!desc[3].is_baseline);
    assert_eq!(desc[3].identity.candidate_index, 3);
    assert!(matches!(
        desc[3].config.placement_strategy,
        PlacementStrategy::Random { .. }
    ));
    assert!(desc[3].config.random_seed.is_some());
    assert_eq!(
        desc[3].config.lookahead_config.max_size,
        MULTI_CANDIDATE_LOOKAHEAD_SIZES[3 % MULTI_CANDIDATE_LOOKAHEAD_SIZES.len()]
    );
    assert_eq!(
        desc[3].config.heuristic_config.lookahead_weight,
        MULTI_CANDIDATE_LOOKAHEAD_WEIGHTS[3 % MULTI_CANDIDATE_LOOKAHEAD_WEIGHTS.len()]
    );
    assert_eq!(
        desc[3].config.heuristic_config.decay_delta,
        MULTI_CANDIDATE_DECAY_DELTAS[3 % MULTI_CANDIDATE_DECAY_DELTAS.len()]
    );

    // Verify all candidate IDs are mutually distinct
    let ids: Vec<_> = desc.iter().map(|d| &d.identity.id).collect();
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert_ne!(ids[i], ids[j], "Candidate IDs must be distinct");
        }
    }
}

#[test]
fn test_b16_deterministic_reproducibility() {
    let target = make_test_target(6);

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(16)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 42,
            count: 16,
        })
        .build()
        .unwrap();

    let run1 = cfg.derive_candidate_descriptors(&target).unwrap();
    let run2 = cfg.derive_candidate_descriptors(&target).unwrap();

    assert_eq!(run1.len(), 16);
    assert_eq!(run2.len(), 16);

    for i in 0..16 {
        assert_eq!(run1[i].identity.id, run2[i].identity.id);
        assert_eq!(run1[i].identity.seed, run2[i].identity.seed);
        assert_eq!(
            run1[i].identity.placement_strategy,
            run2[i].identity.placement_strategy
        );
        assert_eq!(run1[i].config, run2[i].config);
        assert_eq!(run1[i].is_baseline, run2[i].is_baseline);
    }
}

#[test]
fn test_b4_search_execution_passes_all_hard_constraints() {
    let target = make_test_target(5);
    let circuit = make_test_circuit();

    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(4)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 42,
            count: 4,
        })
        .objective(RoutingObjective::minimize_swaps())
        .fallback_policy(FallbackPolicy::BestValidSoFar)
        .build()
        .unwrap();

    let runner = MultiCandidateSearchRunner::new(&circuit, &target, &cfg);
    let report = runner.run().expect("B=4 multi-candidate search succeeds");

    assert_eq!(report.total_executed, 4);
    assert_eq!(report.total_succeeded, 4);
    assert_eq!(report.total_validated, 4);

    for cand in &report.candidates {
        assert!(
            cand.validation.is_fully_validated(),
            "Every candidate must pass legality, equivalence, and clean ancilla"
        );
        assert!(cand.total_iterations > 0);
    }

    let winner = report
        .select_candidate()
        .expect("Winner candidate must be selected");
    assert!(winner.record.validation.is_fully_validated());
    let baseline = &report.candidates[0];
    assert!(
        winner.sabre_report.swaps_inserted <= baseline.swaps_inserted,
        "Winner swaps ({}) must not exceed baseline swaps ({})",
        winner.sabre_report.swaps_inserted,
        baseline.swaps_inserted
    );
}
