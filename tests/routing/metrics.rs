//! Hand-calculated and integration tests for routing objectives and metrics (Phase 49).
//!
//! Validates:
//! - DAG depth for sequential, independent, and mixed-layer circuits.
//! - SWAP and native entangler counts under native SWAP and CX-decomposed targets.
//! - Exact critical path duration on calibrated targets.
//! - Incomplete and unknown duration handling under the Explicit Unknown Principle.
//! - Multi-objective weighted cost calculation with configurable weights.
//! - Determinism across runs and preservation of seed and multi-stage layout information.

use std::collections::BTreeMap;

use qcc::builder::CircuitBuilder;
use qcc::routing::cost::{RoutingCostError, RoutingCostModel, RoutingCostWeights};
use qcc::routing::greedy::{BaselineRouter, BaselineRouterConfig};
use qcc::routing::layout::CircuitLayout;
use qcc::routing::layout_map::LayoutMap;
use qcc::routing::metrics::RoutingMetrics;
use qcc::routing::placement::PlacementStrategy;
use qcc::routing::provenance::RouteProvenanceTracker;
use qcc::routing::swap_insertion::PathSwapStrategy;
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::metrics::DurationCost;
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};

// ============================================================================
// Helper Fixture Builders
// ============================================================================

fn make_linear_target(name: &str, num_qubits: usize, native_swap: bool) -> Target {
    let mut target = Target::new(name, num_qubits);

    let h_desc = TargetOperationDescriptor::builder("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(20.0)
        .build();
    target.add_operation(h_desc);

    let x_desc = TargetOperationDescriptor::builder("x", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(20.0)
        .build();
    target.add_operation(x_desc);

    let z_desc = TargetOperationDescriptor::builder("z", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(10.0)
        .build();
    target.add_operation(z_desc);

    let mut edges = Vec::new();
    for i in 0..num_qubits.saturating_sub(1) {
        edges.push((
            PhysicalQubitId::new(i as u32),
            PhysicalQubitId::new((i + 1) as u32),
        ));
        edges.push((
            PhysicalQubitId::new((i + 1) as u32),
            PhysicalQubitId::new(i as u32),
        ));
    }

    if !edges.is_empty() {
        let cx_desc = TargetOperationDescriptor::builder("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges.clone()))
            .duration(150.0)
            .build();
        target.add_operation(cx_desc);

        if native_swap {
            let swap_desc = TargetOperationDescriptor::builder("swap", 2)
                .connectivity(OperationConnectivity::undirected_two_qubit(edges))
                .duration(300.0)
                .build();
            target.add_operation(swap_desc);
        }
    }

    target.validate().unwrap();
    target
}

fn make_calibrated_target(name: &str) -> Target {
    let mut target = Target::new(name, 2);

    let h_desc = TargetOperationDescriptor::builder("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(20.0)
        .build();
    target.add_operation(h_desc);

    let z_desc = TargetOperationDescriptor::builder("z", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(10.0)
        .build();
    target.add_operation(z_desc);

    let edges = vec![
        (PhysicalQubitId::new(0), PhysicalQubitId::new(1)),
        (PhysicalQubitId::new(1), PhysicalQubitId::new(0)),
    ];
    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit(edges))
        .duration(150.0)
        .build();
    target.add_operation(cx_desc);

    target.validate().unwrap();
    target
}

fn make_partially_calibrated_target(name: &str) -> Target {
    let mut target = Target::new(name, 2);

    let h_desc = TargetOperationDescriptor::builder("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(2))
        .duration(20.0)
        .build();
    target.add_operation(h_desc);

    let edges = vec![
        (PhysicalQubitId::new(0), PhysicalQubitId::new(1)),
        (PhysicalQubitId::new(1), PhysicalQubitId::new(0)),
    ];
    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::directed_two_qubit(edges))
        .build(); // no duration configured => Absent!
    target.add_operation(cx_desc);

    target.validate().unwrap();
    target
}

fn make_uncalibrated_target(name: &str, num_qubits: usize) -> Target {
    let mut target = Target::new(name, num_qubits);

    let h_desc = TargetOperationDescriptor::builder("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .build(); // no duration
    target.add_operation(h_desc);

    let mut edges = Vec::new();
    for i in 0..num_qubits.saturating_sub(1) {
        edges.push((
            PhysicalQubitId::new(i as u32),
            PhysicalQubitId::new((i + 1) as u32),
        ));
        edges.push((
            PhysicalQubitId::new((i + 1) as u32),
            PhysicalQubitId::new(i as u32),
        ));
    }
    if !edges.is_empty() {
        let cx_desc = TargetOperationDescriptor::builder("cx", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges))
            .build(); // no duration
        target.add_operation(cx_desc);
    }

    target.validate().unwrap();
    target
}

// ============================================================================
// 1. Hand-Calculated DAG Depth Tests
// ============================================================================

#[test]
fn test_hand_calculated_sequential_dag_depth() {
    let mut b = CircuitBuilder::new("seq_depth");
    let q = b.add_quantum_register("q", 1).unwrap();
    b.h(q[0]).unwrap();
    b.x(q[0]).unwrap();
    b.z(q[0]).unwrap();
    let circuit = b.finish().unwrap();

    let target = make_linear_target("target1", 1, false);
    let layout = CircuitLayout::new(LayoutMap::identity(1, 1).unwrap());
    let provenance = RouteProvenanceTracker::new();

    let metrics = RoutingMetrics::evaluate(&circuit, &target, &layout, &provenance, None, None);

    // Hand-calculated:
    // Sequential dependency chain: H(0) [depth 1] -> X(0) [depth 2] -> Z(0) [depth 3]
    assert_eq!(metrics.circuit_depth, 3);
    assert_eq!(metrics.two_qubit_depth, 0);
    assert_eq!(metrics.total_gates, 3);
    assert_eq!(metrics.single_qubit_gates, 3);
    assert_eq!(metrics.two_qubit_gates, 0);
}

#[test]
fn test_hand_calculated_independent_parallel_dag_depth() {
    let mut b = CircuitBuilder::new("par_depth");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.h(q[0]).unwrap();
    b.h(q[1]).unwrap();
    b.x(q[2]).unwrap();
    b.z(q[3]).unwrap();
    let circuit = b.finish().unwrap();

    let target = make_linear_target("target4", 4, false);
    let layout = CircuitLayout::new(LayoutMap::identity(4, 4).unwrap());
    let provenance = RouteProvenanceTracker::new();

    let metrics = RoutingMetrics::evaluate(&circuit, &target, &layout, &provenance, None, None);

    // Hand-calculated:
    // All 4 operations are independent (in-degree 0) => max depth = 1
    assert_eq!(metrics.circuit_depth, 1);
    assert_eq!(metrics.two_qubit_depth, 0);
    assert_eq!(metrics.total_gates, 4);
    assert_eq!(metrics.single_qubit_gates, 4);
    assert_eq!(metrics.two_qubit_gates, 0);
}

#[test]
fn test_hand_calculated_mixed_dependencies_dag_depth() {
    // Mixed layers on 4 qubits:
    // Layer 1: H(q0) [d:1, 2Q:0], H(q1) [d:1, 2Q:0], H(q2) [d:1, 2Q:0], X(q3) [d:1, 2Q:0]
    // Layer 2: CX(q0, q1) [depends on H(0), H(1) => d:2, 2Q:1]
    // Layer 3: CX(q1, q2) [depends on CX(0,1), H(2) => d:3, 2Q:2]
    // Layer 4: CX(q2, q3) [depends on CX(1,2), X(3) => d:4, 2Q:3]
    let mut b = CircuitBuilder::new("mixed_depth");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.h(q[0]).unwrap();
    b.h(q[1]).unwrap();
    b.h(q[2]).unwrap();
    b.x(q[3]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    b.cx(q[2], q[3]).unwrap();
    let circuit = b.finish().unwrap();

    let target = make_linear_target("target4", 4, false);
    let layout = CircuitLayout::new(LayoutMap::identity(4, 4).unwrap());
    let provenance = RouteProvenanceTracker::new();

    let metrics = RoutingMetrics::evaluate(&circuit, &target, &layout, &provenance, None, None);

    // Hand-calculated:
    // Max critical path length = 4 (H(0)->CX(0,1)->CX(1,2)->CX(2,3))
    // Max 2Q critical path length = 3 (CX(0,1)->CX(1,2)->CX(2,3))
    assert_eq!(metrics.circuit_depth, 4);
    assert_eq!(metrics.two_qubit_depth, 3);
    assert_eq!(metrics.total_gates, 7);
    assert_eq!(metrics.single_qubit_gates, 4);
    assert_eq!(metrics.two_qubit_gates, 3);
}

// ============================================================================
// 2. SWAP and Native Entangler Counts Tests
// ============================================================================

#[test]
fn test_hand_calculated_swap_and_entangler_counts_native_swap() {
    let target = make_linear_target("line3_native_swap", 3, true);

    let mut b = CircuitBuilder::new("cx_dist2");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let router = BaselineRouter::with_config(BaselineRouterConfig {
        placement_strategy: PlacementStrategy::Identity,
        swap_strategy: PathSwapStrategy::SourceToTarget,
        ..BaselineRouterConfig::default()
    });

    let id_layout = LayoutMap::identity(3, 3).unwrap();
    let (_routed, report) = router
        .route_circuit(&circuit, &target, Some(id_layout))
        .unwrap();

    // Hand-calculated:
    // Source q0 moved to p1 via SWAP(p0, p1) => 1 inserted SWAP
    // CX executed on (p1, p2)
    assert_eq!(report.swaps_inserted, 1);
    assert_eq!(report.metrics.routing_swaps_count, 1);
    assert_eq!(report.metrics.routing_swap_gates, 1); // 1 native SWAP gate emitted
    assert_eq!(report.metrics.original_entanglers_count, 1); // the 1 original CX
    assert_eq!(report.metrics.two_qubit_gates, 2); // 1 SWAP + 1 CX = 2 physical 2Q gates
    assert_eq!(report.metrics.total_routing_overhead_ops, 1);
    assert_eq!(report.metrics.direction_corrections_count, 0);
    assert_eq!(report.metrics.total_gates, 2);
}

#[test]
fn test_hand_calculated_swap_and_entangler_counts_decomposed_cx_swap() {
    // Target supports CX but does NOT support native SWAP => each SWAP decomposes to 3 CX gates
    let target = make_linear_target("line3_decomposed", 3, false);

    let mut b = CircuitBuilder::new("cx_dist2_decomp");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let router = BaselineRouter::with_config(BaselineRouterConfig {
        placement_strategy: PlacementStrategy::Identity,
        swap_strategy: PathSwapStrategy::SourceToTarget,
        ..BaselineRouterConfig::default()
    });

    let id_layout = LayoutMap::identity(3, 3).unwrap();
    let (_routed, report) = router
        .route_circuit(&circuit, &target, Some(id_layout))
        .unwrap();

    // Hand-calculated:
    // 1 logical SWAP inserted
    // Decomposed into 3 CX gates on (p0, p1)
    // Then original CX on (p1, p2)
    // Total physical 2Q gates = 3 + 1 = 4 CX gates
    assert_eq!(report.swaps_inserted, 1);
    assert_eq!(report.metrics.routing_swaps_count, 1);
    assert_eq!(report.metrics.routing_swap_gates, 3);
    assert_eq!(report.metrics.original_entanglers_count, 1);
    assert_eq!(report.metrics.two_qubit_gates, 4);
    assert_eq!(report.metrics.total_routing_overhead_ops, 3);
    assert_eq!(report.metrics.total_gates, 4);
}

// ============================================================================
// 3. Execution Duration Tests (Calibrated vs Explicit Unknown)
// ============================================================================

#[test]
fn test_hand_calculated_exact_duration_calibrated() {
    let target = make_calibrated_target("calibrated_device");

    // Circuit on 2 qubits:
    // H(q0) -> dur = 20.0 ns => ends at 20.0 ns
    // H(q1) -> dur = 20.0 ns => ends at 20.0 ns
    // CX(q0, q1) -> starts at max(20.0, 20.0) = 20.0 ns, dur = 150.0 ns => ends at 170.0 ns
    // Z(q1) -> starts at 170.0 ns, dur = 10.0 ns => ends at 180.0 ns
    let mut b = CircuitBuilder::new("timed_circuit");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.h(q[1]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.z(q[1]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = CircuitLayout::new(LayoutMap::identity(2, 2).unwrap());
    let provenance = RouteProvenanceTracker::new();

    let metrics = RoutingMetrics::evaluate(&circuit, &target, &layout, &provenance, None, None);

    // Hand-calculated: Critical path execution time = 180.0 ns
    match metrics.duration {
        DurationCost::Exact { total_ns, .. } => {
            assert!(
                (total_ns - 180.0).abs() < 1e-9,
                "expected total_ns = 180.0, got {total_ns}"
            );
        }
        other => panic!("expected DurationCost::Exact, got {other:?}"),
    }
}

#[test]
fn test_missing_and_incomplete_duration_explicit_unknown() {
    let target = make_partially_calibrated_target("partial_target");

    let mut b = CircuitBuilder::new("partial_circuit");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    let layout = CircuitLayout::new(LayoutMap::identity(2, 2).unwrap());
    let provenance = RouteProvenanceTracker::new();

    let metrics = RoutingMetrics::evaluate(&circuit, &target, &layout, &provenance, None, None);

    // Under the Explicit Unknown Principle:
    // Missing CX duration must NOT be assumed to be 0.0 ns.
    // It must yield DurationCost::Incomplete with 1 uncalibrated operation!
    match metrics.duration {
        DurationCost::Incomplete {
            accumulated_known_ns,
            uncalibrated_operations,
        } => {
            assert!((accumulated_known_ns - 20.0).abs() < 1e-9);
            assert_eq!(uncalibrated_operations, 1);
        }
        other => panic!("expected DurationCost::Incomplete, got {other:?}"),
    }

    // Case 2: Target with completely absent timing data
    let uncalibrated_target = make_uncalibrated_target("zero_calib", 2);
    let metrics_unknown = RoutingMetrics::evaluate(
        &circuit,
        &uncalibrated_target,
        &layout,
        &provenance,
        None,
        None,
    );

    assert_eq!(metrics_unknown.duration, DurationCost::Unknown);
}

// ============================================================================
// 4. Weighted Cost Calculation and Configurable Weights Tests
// ============================================================================

#[test]
fn test_hand_calculated_weighted_cost() {
    let mut metrics = RoutingMetrics {
        circuit_depth: 5,
        two_qubit_depth: 3,
        total_gates: 7,
        single_qubit_gates: 4,
        two_qubit_gates: 3,
        multi_qubit_gates: 0,
        routing_swaps_count: 2,
        routing_swap_gates: 2,
        original_entanglers_count: 1,
        direction_corrections_count: 1,
        total_routing_overhead_ops: 3,
        gate_counts: BTreeMap::new(),
        duration: DurationCost::Exact {
            total_ns: 200.0,
            is_measured_only: true,
        },
        initial_layout: LayoutMap::identity(2, 2).unwrap(),
        final_layout: LayoutMap::identity(2, 2).unwrap(),
        seed: None,
        run_duration: None,
        weighted_cost: 0.0,
    };

    let weights = RoutingCostWeights {
        swap_count: 10.0,
        two_qubit_gates: 5.0,
        total_gates: 1.0,
        depth: 2.0,
        two_qubit_depth: 3.0,
        duration_ns: 0.5,
        direction_corrections: 4.0,
    };

    let model = RoutingCostModel::new(weights.clone());
    let cost = model.evaluate(&metrics);

    // Hand-calculated:
    // 2 swaps * 10.0 = 20.0
    // 3 two_qubit_gates * 5.0 = 15.0
    // 7 total_gates * 1.0 = 7.0
    // 5 depth * 2.0 = 10.0
    // 3 two_qubit_depth * 3.0 = 9.0
    // 1 direction_corrections * 4.0 = 4.0
    // 200.0 ns * 0.5 = 100.0
    // Sum = 20 + 15 + 7 + 10 + 9 + 4 + 100 = 165.0
    assert!(
        (cost - 165.0).abs() < 1e-9,
        "expected cost = 165.0, got {cost}"
    );

    // Checked evaluation passes
    let checked_cost = model.evaluate_checked(&metrics).unwrap();
    assert!((checked_cost - 165.0).abs() < 1e-9);

    // If duration is incomplete and exact duration is required, evaluate_checked must fail
    metrics.duration = DurationCost::Incomplete {
        accumulated_known_ns: 100.0,
        uncalibrated_operations: 2,
    };
    let strict_model = RoutingCostModel::new(weights).with_exact_duration_required(true);
    let err = strict_model.evaluate_checked(&metrics).unwrap_err();
    assert_eq!(
        err,
        RoutingCostError::MissingTiming {
            uncalibrated_operations: 2
        }
    );
}

#[test]
fn test_negative_weight_validation() {
    let invalid_weights = RoutingCostWeights {
        swap_count: -1.0,
        ..RoutingCostWeights::default()
    };
    let model = RoutingCostModel::new(invalid_weights);
    let dummy_metrics = RoutingMetrics {
        circuit_depth: 0,
        two_qubit_depth: 0,
        total_gates: 0,
        single_qubit_gates: 0,
        two_qubit_gates: 0,
        multi_qubit_gates: 0,
        routing_swaps_count: 0,
        routing_swap_gates: 0,
        original_entanglers_count: 0,
        direction_corrections_count: 0,
        total_routing_overhead_ops: 0,
        gate_counts: BTreeMap::new(),
        duration: DurationCost::Unknown,
        initial_layout: LayoutMap::identity(1, 1).unwrap(),
        final_layout: LayoutMap::identity(1, 1).unwrap(),
        seed: None,
        run_duration: None,
        weighted_cost: 0.0,
    };

    let res = model.evaluate_checked(&dummy_metrics);
    assert!(matches!(
        res,
        Err(RoutingCostError::NegativeWeight {
            component: "swap_count",
            ..
        })
    ));
}

// ============================================================================
// 5. Determinism and Layout/Seed Preservation Tests
// ============================================================================

#[test]
fn test_metrics_determinism_across_repeated_runs() {
    let target = make_linear_target("line4", 4, true);

    let mut b = CircuitBuilder::new("determ_circuit");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[3]).unwrap();
    b.h(q[2]).unwrap();
    b.cx(q[2], q[3]).unwrap();
    let circuit = b.finish().unwrap();

    let router = BaselineRouter::with_config(BaselineRouterConfig {
        placement_strategy: PlacementStrategy::Weighted,
        swap_strategy: PathSwapStrategy::SourceToTarget,
        ..BaselineRouterConfig::default()
    });

    let (_, report1) = router.route_circuit(&circuit, &target, None).unwrap();
    let (_, report2) = router.route_circuit(&circuit, &target, None).unwrap();

    // Verify deterministic equality across runs
    assert!(
        report1.metrics.is_deterministic_match(&report2.metrics),
        "metrics should be strictly deterministic across repeated runs"
    );

    assert_eq!(report1.metrics.circuit_depth, report2.metrics.circuit_depth);
    assert_eq!(
        report1.metrics.two_qubit_depth,
        report2.metrics.two_qubit_depth
    );
    assert_eq!(
        report1.metrics.routing_swaps_count,
        report2.metrics.routing_swaps_count
    );
    assert_eq!(report1.metrics.total_gates, report2.metrics.total_gates);
    assert_eq!(
        report1.metrics.initial_layout,
        report2.metrics.initial_layout
    );
    assert_eq!(report1.metrics.final_layout, report2.metrics.final_layout);
}

#[test]
fn test_seed_and_layout_preservation() {
    let target = make_linear_target("line4_seed", 4, true);

    let mut b = CircuitBuilder::new("seed_circuit");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let seed = 424_242_u64;
    let router = BaselineRouter::with_config(BaselineRouterConfig {
        placement_strategy: PlacementStrategy::Random { seed },
        swap_strategy: PathSwapStrategy::SourceToTarget,
        ..BaselineRouterConfig::default()
    });

    let (_, report) = router.route_circuit(&circuit, &target, None).unwrap();

    // Verify seed is recorded
    assert_eq!(report.metrics.seed, Some(seed));

    // Verify initial and final layouts match report layout snapshots
    assert_eq!(report.metrics.initial_layout, *report.layout.initial());
    assert_eq!(report.metrics.final_layout, *report.layout.current());

    // Verify wall-clock run duration is captured
    assert!(report.metrics.run_duration.is_some());
}
