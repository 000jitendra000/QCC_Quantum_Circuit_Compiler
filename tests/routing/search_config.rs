//! Unit and integration tests for multi-candidate routing configuration and objectives (Phase 74).

use qcc::builder::CircuitBuilder;
use qcc::compiler::config::{CompilerConfig, RouterChoice};
use qcc::compiler::pipeline::CompilerPipeline;
use qcc::routing::layout_map::LayoutMap;
use qcc::routing::placement::PlacementStrategy;
use qcc::routing::sabre::SabreRouterConfig;
use qcc::routing::search::{
    FallbackPolicy, HardConstraint, LayoutStrategy, MultiCandidateRoutingConfig, ObjectiveFunction,
    ObjectiveMetric, RoutingObjective, RoutingSearchConfigError, SearchBudget, SeedStrategy,
    HARD_MAX_CANDIDATE_LIMIT,
};
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::model::{Target, TargetBuilder};
use qcc::target::operation::{PhysicalQubitId, TargetOperationBuilder};
use qcc::target::timing::TimingModel;

fn make_test_target(has_timing: bool) -> Target {
    let mut builder = TargetBuilder::new("test_linear_5q", 5);

    // 1Q rotations
    for gate in ["rx", "ry", "rz", "h", "x", "sx"] {
        builder = builder.add_operation(
            TargetOperationBuilder::new(gate, 1)
                .connectivity(OperationConnectivity::all_single_qubits(5))
                .build(),
        );
    }

    // Linear CX coupling: (0, 1), (1, 2), (2, 3), (3, 4)
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

    let mut target = builder.build().expect("Build test target");
    if has_timing {
        target.timing = Some(TimingModel::new());
    }
    target
}

#[test]
fn test_default_multi_candidate_config_is_single_candidate() {
    let cfg = MultiCandidateRoutingConfig::default();
    assert_eq!(cfg.candidate_count, 1);
    assert!(cfg.is_single_candidate());
    assert_eq!(cfg.seed_strategy, SeedStrategy::Single(None));
    assert_eq!(
        cfg.layout_strategy,
        LayoutStrategy::Single(PlacementStrategy::Weighted)
    );
    assert_eq!(cfg.fallback_policy, FallbackPolicy::BestValidSoFar);
    assert!(cfg.validate().is_ok());

    let baseline = cfg.baseline_sabre_config();
    assert_eq!(baseline, SabreRouterConfig::default());
}

#[test]
fn test_builder_fluency_and_custom_configuration() {
    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(8)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 42,
            count: 8,
        })
        .layout_strategy(LayoutStrategy::Strategies(vec![
            PlacementStrategy::Weighted,
            PlacementStrategy::Degree,
        ]))
        .objective(RoutingObjective::lexicographic(vec![
            ObjectiveMetric::SwapCount,
            ObjectiveMetric::CircuitDepth,
        ]))
        .max_candidates(16)
        .fallback_policy(FallbackPolicy::DeterministicBaseline)
        .build()
        .expect("Build valid custom configuration");

    assert_eq!(cfg.candidate_count, 8);
    assert!(!cfg.is_single_candidate());
    assert_eq!(cfg.budget.max_candidates, 16);
    assert_eq!(cfg.fallback_policy, FallbackPolicy::DeterministicBaseline);
}

#[test]
fn test_seed_strategies() {
    // 1. Single seed
    let single = SeedStrategy::Single(Some(123));
    assert!(single.validate().is_ok());
    let s = single.generate_seeds(4).unwrap();
    assert_eq!(s, vec![Some(123), Some(123), Some(123), Some(123)]);

    // 2. Explicit seeds
    let explicit = SeedStrategy::Explicit(vec![10, 20, 30]);
    assert!(explicit.validate().is_ok());
    let s = explicit.generate_seeds(5).unwrap();
    assert_eq!(s, vec![Some(10), Some(20), Some(30), Some(10), Some(20)]);

    // 3. Derived seeds
    let derived = SeedStrategy::Derived {
        base_seed: 999,
        count: 4,
    };
    assert!(derived.validate().is_ok());
    let s = derived.generate_seeds(4).unwrap();
    assert_eq!(s.len(), 4);
    assert!(s[0] != s[1]);
    assert!(s[1] != s[2]);

    // 4. Arithmetic Range
    let range = SeedStrategy::Range {
        start: 100,
        step: 5,
        count: 4,
    };
    assert!(range.validate().is_ok());
    let s = range.generate_seeds(4).unwrap();
    assert_eq!(s, vec![Some(100), Some(105), Some(110), Some(115)]);

    // 5. Validation failures
    assert!(SeedStrategy::Explicit(vec![]).validate().is_err());
    assert!(SeedStrategy::Derived {
        base_seed: 1,
        count: 0
    }
    .validate()
    .is_err());
    assert!(SeedStrategy::Range {
        start: 0,
        step: 0,
        count: 5
    }
    .validate()
    .is_err());
}

#[test]
fn test_layout_strategies() {
    let single = LayoutStrategy::Single(PlacementStrategy::Degree);
    assert!(single.validate().is_ok());

    let multi = LayoutStrategy::Strategies(vec![
        PlacementStrategy::Weighted,
        PlacementStrategy::Random { seed: 1 },
    ]);
    assert!(multi.validate().is_ok());

    let empty = LayoutStrategy::Strategies(vec![]);
    assert!(empty.validate().is_err());

    let explicit = LayoutStrategy::Explicit(vec![LayoutMap::identity(2, 2).unwrap()]);
    assert!(explicit.validate().is_ok());

    let empty_explicit = LayoutStrategy::Explicit(vec![]);
    assert!(empty_explicit.validate().is_err());
}

#[test]
fn test_budget_bounds_validation() {
    // 0 candidates is invalid
    let zero_cand = MultiCandidateRoutingConfig::builder()
        .candidate_count(0)
        .build();
    assert!(matches!(
        zero_cand,
        Err(RoutingSearchConfigError::InvalidCandidateCount { count: 0, .. })
    ));

    // Exceeding budget max is invalid
    let exceed = MultiCandidateRoutingConfig::builder()
        .candidate_count(100)
        .max_candidates(64)
        .build();
    assert!(matches!(
        exceed,
        Err(RoutingSearchConfigError::InvalidCandidateCount {
            count: 100,
            max: 64,
            ..
        })
    ));

    // Exceeding hard limit is invalid
    let hard_exceed = SearchBudget {
        max_candidates: HARD_MAX_CANDIDATE_LIMIT + 1,
        ..Default::default()
    };
    assert!(hard_exceed.validate().is_err());

    // Zero budget values are invalid
    let zero_swaps = SearchBudget {
        max_total_swaps: Some(0),
        ..Default::default()
    };
    assert!(zero_swaps.validate().is_err());

    let zero_timeout = SearchBudget {
        timeout_ms: Some(0),
        ..Default::default()
    };
    assert!(zero_timeout.validate().is_err());
}

#[test]
fn test_objective_validation() {
    // 1. Valid objectives
    assert!(RoutingObjective::minimize_swaps().validate().is_ok());
    assert!(RoutingObjective::minimize_depth().validate().is_ok());
    assert!(RoutingObjective::lexicographic(vec![
        ObjectiveMetric::SwapCount,
        ObjectiveMetric::CircuitDepth,
    ])
    .validate()
    .is_ok());
    assert!(RoutingObjective::weighted(vec![
        (ObjectiveMetric::SwapCount, 1.0),
        (ObjectiveMetric::CircuitDepth, 0.5),
    ])
    .validate()
    .is_ok());

    // 2. Empty lexicographic list
    let empty_lex = RoutingObjective {
        function: ObjectiveFunction::Lexicographic(vec![]),
        hard_constraints: vec![HardConstraint::TargetLegality],
    };
    assert!(empty_lex.validate().is_err());

    // 3. Empty weighted list
    let empty_weight = RoutingObjective {
        function: ObjectiveFunction::Weighted(vec![]),
        hard_constraints: vec![HardConstraint::TargetLegality],
    };
    assert!(empty_weight.validate().is_err());

    // 4. Negative weight
    let neg_weight = RoutingObjective::weighted(vec![(ObjectiveMetric::SwapCount, -1.0)]);
    assert!(neg_weight.validate().is_err());

    // 5. All zero weights
    let zero_weight = RoutingObjective::weighted(vec![(ObjectiveMetric::SwapCount, 0.0)]);
    assert!(zero_weight.validate().is_err());

    // 6. Unsupported metric
    let unsupp = RoutingObjective {
        function: ObjectiveFunction::Single(ObjectiveMetric::Unsupported(
            "noise_fidelity".to_string(),
        )),
        hard_constraints: vec![HardConstraint::TargetLegality],
    };
    assert!(unsupp.validate().is_err());

    // 7. Empty hard constraints
    let no_constraints = RoutingObjective {
        function: ObjectiveFunction::Single(ObjectiveMetric::SwapCount),
        hard_constraints: vec![],
    };
    assert!(no_constraints.validate().is_err());
}

#[test]
fn test_target_timing_compatibility_and_explicit_unknown() {
    let target_no_timing = make_test_target(false);
    let target_with_timing = make_test_target(true);

    // Topological metric validates on both
    let top_config = MultiCandidateRoutingConfig::builder()
        .objective(RoutingObjective::minimize_depth())
        .build()
        .unwrap();
    assert!(top_config.validate_with_target(&target_no_timing).is_ok());
    assert!(top_config.validate_with_target(&target_with_timing).is_ok());

    // Duration metric validates on target with timing
    let duration_config = MultiCandidateRoutingConfig::builder()
        .objective(RoutingObjective {
            function: ObjectiveFunction::Single(ObjectiveMetric::EstimatedDuration),
            hard_constraints: vec![HardConstraint::TargetLegality],
        })
        .build()
        .unwrap();
    assert!(duration_config
        .validate_with_target(&target_with_timing)
        .is_ok());

    // Duration metric FAILS on target without timing (Explicit Unknown Principle)
    let err = duration_config.validate_with_target(&target_no_timing);
    assert!(matches!(
        err,
        Err(RoutingSearchConfigError::MissingTimingModel {
            metric: ObjectiveMetric::EstimatedDuration,
            ..
        })
    ));
}

#[test]
fn test_candidate_descriptor_derivation_determinism() {
    let target = make_test_target(false);
    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(4)
        .seed_strategy(SeedStrategy::Derived {
            base_seed: 777,
            count: 4,
        })
        .layout_strategy(LayoutStrategy::Strategies(vec![
            PlacementStrategy::Weighted,
            PlacementStrategy::Degree,
        ]))
        .build()
        .unwrap();

    let run1 = cfg.derive_candidate_descriptors(&target).unwrap();
    let run2 = cfg.derive_candidate_descriptors(&target).unwrap();

    assert_eq!(run1.len(), 4);
    assert_eq!(run2.len(), 4);

    // Bit-for-bit identical derivation across runs
    for i in 0..4 {
        assert_eq!(run1[i].identity.id, run2[i].identity.id);
        assert_eq!(run1[i].identity.seed, run2[i].identity.seed);
        assert_eq!(
            run1[i].identity.placement_strategy,
            run2[i].identity.placement_strategy
        );
        assert_eq!(run1[i].config, run2[i].config);
    }

    // Alternating placement strategies as configured
    assert_eq!(
        run1[0].identity.placement_strategy,
        PlacementStrategy::Weighted
    );
    assert_eq!(
        run1[1].identity.placement_strategy,
        PlacementStrategy::Degree
    );
    assert_eq!(
        run1[2].identity.placement_strategy,
        PlacementStrategy::Weighted
    );
    assert_eq!(
        run1[3].identity.placement_strategy,
        PlacementStrategy::Degree
    );
}

#[test]
fn test_compiler_pipeline_single_candidate_backward_compatibility() {
    let mut builder = CircuitBuilder::new("multi_candidate_compat_test");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[2]).unwrap(); // Requires SWAP on linear topology (0-1-2)
    builder.cx(q[1], q[2]).unwrap();
    let circuit = builder.finish().unwrap();

    let target = make_test_target(false);

    // 1. Compile with standard SabreRouterConfig
    let sabre_config = CompilerConfig::o2();
    let res_sabre = CompilerPipeline::new(sabre_config)
        .compile(&circuit, &target)
        .expect("Compile with standard Sabre");

    // 2. Compile with RouterChoice::MultiCandidate in default single-candidate mode
    let multi_config =
        CompilerConfig::o2().with_router(MultiCandidateRoutingConfig::default().into());
    assert_eq!(
        multi_config.router,
        RouterChoice::MultiCandidate(Box::default())
    );
    let res_multi = CompilerPipeline::new(multi_config)
        .compile(&circuit, &target)
        .expect("Compile with MultiCandidate default");

    // Exact metric equality
    assert_eq!(
        res_sabre.report.metrics.final_gate_count,
        res_multi.report.metrics.final_gate_count
    );
    assert_eq!(
        res_sabre.report.metrics.final_depth,
        res_multi.report.metrics.final_depth
    );
    assert_eq!(
        res_sabre.report.metrics.router_swaps_inserted,
        res_multi.report.metrics.router_swaps_inserted
    );
}

#[test]
fn test_serde_roundtrip_config() {
    let cfg = MultiCandidateRoutingConfig::builder()
        .candidate_count(6)
        .seed_strategy(SeedStrategy::Range {
            start: 10,
            step: 2,
            count: 6,
        })
        .objective(RoutingObjective::weighted(vec![
            (ObjectiveMetric::SwapCount, 0.7),
            (ObjectiveMetric::CircuitDepth, 0.3),
        ]))
        .max_candidates(32)
        .fallback_policy(FallbackPolicy::DeterministicBaseline)
        .build()
        .unwrap();

    let json = serde_json::to_string_pretty(&cfg).expect("Serialize MultiCandidateRoutingConfig");
    let deserialized: MultiCandidateRoutingConfig =
        serde_json::from_str(&json).expect("Deserialize MultiCandidateRoutingConfig");

    assert_eq!(cfg, deserialized);
}
