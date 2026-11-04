//! Integration tests for Phase 47: Baseline Shortest-Path Router.
//!
//! Validates:
//! 1. Dependency-respecting DAG traversal scheduling executable nodes.
//! 2. Shortest-path routing across Line, Ring, Grid, Star, Disconnected, and Directed topologies.
//! 3. Target operand tuple and orientation legality under declared basis capabilities.
//! 4. Synchronous layout mutation tracking ($L_{\text{initial}} \to L_{\text{final}}$).
//! 5. ADR-0001 strict ancilla zero-state preservation on partially occupied devices.
//! 6. Unitary semantic equivalence verified against the Phase 45 layout-aware checker.
//! 7. Configurable path SWAP strategies (SourceToTarget, TargetToSource, Bidirectional).
//! 8. Bounded iteration and SWAP limits preventing infinite loops.
//! 9. Explicit failure on unreachable/disconnected components or insufficient capacity.

use std::f64::consts::PI;

use qcc::builder::CircuitBuilder;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::QubitId;
use qcc::routing::greedy::{BaselineRouter, BaselineRouterConfig};
use qcc::routing::layout_map::LayoutMap;
use qcc::routing::pathfinding::RoutingError;
use qcc::routing::swap_insertion::PathSwapStrategy;
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};
use qcc::verification::layout_equivalence::LayoutEquivalenceChecker;

// ============================================================================
// Helper Target Builders
// ============================================================================

/// Helper constructing a target device with native `swap`, `cx`, and single-qubit gates.
fn make_target_with_native_swap(
    name: &str,
    num_qubits: usize,
    edges: impl IntoIterator<Item = (u32, u32)>,
    directed: bool,
) -> Target {
    let mut target = Target::new(name, num_qubits);

    // Single-qubit gates: h, x, rz, sx
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

    let rz_desc = TargetOperationDescriptor::builder("rz", 1)
        .add_parameter_domain(qcc::gate_signature::ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(0.0)
        .build();
    target.add_operation(rz_desc);

    let sx_desc = TargetOperationDescriptor::builder("sx", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(20.0)
        .build();
    target.add_operation(sx_desc);

    let edge_tuples: Vec<(PhysicalQubitId, PhysicalQubitId)> = edges
        .into_iter()
        .map(|(u, v)| (PhysicalQubitId::new(u), PhysicalQubitId::new(v)))
        .collect();

    let cx_conn = if directed {
        OperationConnectivity::directed_two_qubit(edge_tuples.clone())
    } else {
        OperationConnectivity::undirected_two_qubit(edge_tuples.clone())
    };
    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(cx_conn)
        .duration(150.0)
        .build();
    target.add_operation(cx_desc);

    // Native SWAP is symmetric undirected on the coupling edges
    let swap_desc = TargetOperationDescriptor::builder("swap", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit(edge_tuples))
        .duration(300.0)
        .build();
    target.add_operation(swap_desc);

    target.validate().unwrap();
    target
}

/// Helper constructing a target device supporting ONLY `cx` as its 2-qubit gate (no native `swap`).
fn make_target_cx_only(
    name: &str,
    num_qubits: usize,
    edges: impl IntoIterator<Item = (u32, u32)>,
    directed: bool,
) -> Target {
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

    let rz_desc = TargetOperationDescriptor::builder("rz", 1)
        .add_parameter_domain(qcc::gate_signature::ParameterDomain::Angle4Pi)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(0.0)
        .build();
    target.add_operation(rz_desc);

    let edge_tuples: Vec<(PhysicalQubitId, PhysicalQubitId)> = edges
        .into_iter()
        .map(|(u, v)| (PhysicalQubitId::new(u), PhysicalQubitId::new(v)))
        .collect();

    let cx_conn = if directed {
        OperationConnectivity::directed_two_qubit(edge_tuples)
    } else {
        OperationConnectivity::undirected_two_qubit(edge_tuples)
    };
    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(cx_conn)
        .duration(150.0)
        .build();
    target.add_operation(cx_desc);

    target.validate().unwrap();
    target
}

// ============================================================================
// 1. Line Topology Tests
// ============================================================================

#[test]
fn test_baseline_router_line_topology_already_legal() {
    // 3 physical qubits: 0 - 1 - 2
    let target = make_target_with_native_swap("line_3q", 3, [(0, 1), (1, 2)], false);

    // Logical circuit with gates already adjacent under identity layout:
    // CX(q0, q1) and CX(q1, q2)
    let mut b = CircuitBuilder::new("line_already_legal");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    b.x(q[2]).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let id_layout = LayoutMap::identity(3, 3).unwrap();
    let (routed, report) = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap();

    assert_eq!(report.swaps_inserted, 0);
    assert_eq!(report.original_operations, 4);
    assert_eq!(report.routed_operations, 4);
    assert_eq!(report.layout.initial(), report.layout.final_layout());

    // Verify semantic layout-aware equivalence
    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&logical, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_baseline_router_line_topology_single_hop_swap() {
    // 3 physical qubits: 0 - 1 - 2
    let target = make_target_with_native_swap("line_3q", 3, [(0, 1), (1, 2)], false);

    // Logical circuit with CX(q0, q2): distance is 2 hops on line
    let mut b = CircuitBuilder::new("line_single_swap");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[2]).unwrap();
    b.x(q[2]).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let id_layout = LayoutMap::identity(3, 3).unwrap();
    let (routed, report) = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap();

    assert_eq!(report.swaps_inserted, 1);
    // 3 original ops + 1 SWAP = 4 routed ops
    assert_eq!(report.routed_operations, 4);

    // Final layout: q[0] was swapped from p[0] to p[1], so q[0] is at p[1], q[1] is at p[0]
    assert_eq!(
        report.layout.final_layout().get_physical(QubitId(0)),
        Some(PhysicalQubitId(1))
    );
    assert_eq!(
        report.layout.final_layout().get_physical(QubitId(1)),
        Some(PhysicalQubitId(0))
    );
    assert_eq!(
        report.layout.final_layout().get_physical(QubitId(2)),
        Some(PhysicalQubitId(2))
    );

    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&logical, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_baseline_router_line_topology_multi_hop_swaps() {
    // 5 physical qubits: 0 - 1 - 2 - 3 - 4
    let target =
        make_target_with_native_swap("line_5q", 5, [(0, 1), (1, 2), (2, 3), (3, 4)], false);

    // CX(q0, q4): distance is 4 hops -> requires 3 SWAPs
    let mut b = CircuitBuilder::new("line_multi_hop");
    let q = b.add_quantum_register("q", 5).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[4]).unwrap();
    b.x(q[4]).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let id_layout = LayoutMap::identity(5, 5).unwrap();
    let (routed, report) = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap();

    assert_eq!(report.swaps_inserted, 3);
    assert_eq!(report.routed_operations, 3 + 3);

    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&logical, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

// ============================================================================
// 2. Ring Topology Tests
// ============================================================================

#[test]
fn test_baseline_router_ring_topology_wraparound() {
    // 6 physical qubits in a ring: 0-1, 1-2, 2-3, 3-4, 4-5, 5-0
    let target = make_target_with_native_swap(
        "ring_6q",
        6,
        [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0)],
        false,
    );

    // CX(q0, q5): direct edge exists via wraparound (distance 1)!
    let mut b = CircuitBuilder::new("ring_wraparound_direct");
    let q = b.add_quantum_register("q", 6).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[5]).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let id_layout = LayoutMap::identity(6, 6).unwrap();
    let (routed, report) = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap();

    // Directly legal across wraparound edge, 0 SWAPs!
    assert_eq!(report.swaps_inserted, 0);

    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&logical, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_baseline_router_ring_topology_shortest_wraparound_path() {
    let target = make_target_with_native_swap(
        "ring_6q",
        6,
        [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0)],
        false,
    );

    // CX(q0, q4): on line distance is 4 (0-1-2-3-4).
    // On ring, distance is 2 hops (0-5-4)!
    // BFS chooses path [0, 5, 4], requiring only 1 SWAP (0 <-> 5)!
    let mut b = CircuitBuilder::new("ring_shortest_path");
    let q = b.add_quantum_register("q", 6).unwrap();
    b.cx(q[0], q[4]).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let id_layout = LayoutMap::identity(6, 6).unwrap();
    let (routed, report) = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap();

    assert_eq!(report.swaps_inserted, 1);

    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&logical, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 3. Grid Topology Tests
// ============================================================================

#[test]
fn test_baseline_router_grid_topology_2d() {
    // 2x3 Grid:
    // 0 - 1 - 2
    // |   |   |
    // 3 - 4 - 5
    let target = make_target_with_native_swap(
        "grid_2x3",
        6,
        [(0, 1), (1, 2), (3, 4), (4, 5), (0, 3), (1, 4), (2, 5)],
        false,
    );

    // CX(q0, q5): diagonally opposite corners.
    // Shortest path: e.g. [0, 1, 2, 5] or [0, 3, 4, 5] (3 hops -> 2 SWAPs)
    let mut b = CircuitBuilder::new("grid_diag");
    let q = b.add_quantum_register("q", 6).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[5]).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let id_layout = LayoutMap::identity(6, 6).unwrap();
    let (routed, report) = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap();

    assert_eq!(report.swaps_inserted, 2);

    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&logical, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

// ============================================================================
// 4. Star Topology Tests
// ============================================================================

#[test]
fn test_baseline_router_star_topology_hub() {
    // Star topology: central hub 0 connected to leaves 1, 2, 3, 4
    let target =
        make_target_with_native_swap("star_5q", 5, [(0, 1), (0, 2), (0, 3), (0, 4)], false);

    // CX between leaf 1 and leaf 2: distance 2 hops (1 -> 0 -> 2)
    let mut b = CircuitBuilder::new("star_leaves");
    let q = b.add_quantum_register("q", 5).unwrap();
    b.h(q[1]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let id_layout = LayoutMap::identity(5, 5).unwrap();
    let (routed, report) = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap();

    assert_eq!(report.swaps_inserted, 1);

    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&logical, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 5. Disconnected Topology Tests
// ============================================================================

#[test]
fn test_baseline_router_disconnected_topology_fails_explicitly() {
    // Target with two disconnected components: {0, 1} and {2, 3}
    let target = make_target_with_native_swap("disconnected_target", 4, [(0, 1), (2, 3)], false);

    let mut b = CircuitBuilder::new("disconnected_circuit");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.cx(q[0], q[3]).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let id_layout = LayoutMap::identity(4, 4).unwrap();
    let err = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap_err();

    match err {
        RoutingError::DisconnectedTopology { src, dst } => {
            assert_eq!(src, PhysicalQubitId(0));
            assert_eq!(dst, PhysicalQubitId(3));
        }
        other => panic!("expected DisconnectedTopology, got {other:?}"),
    }
}

// ============================================================================
// 6. Directed Coupling and Orientation Tests
// ============================================================================

#[test]
fn test_baseline_router_directed_coupling_orientation() {
    // Directed chain: only 0 -> 1 and 1 -> 2 permitted for CX
    let target = make_target_with_native_swap("directed_chain", 3, [(0, 1), (1, 2)], true);

    // CX(q1, q0): control at q1, target at q0.
    // Under identity layout, this requires CX(1, 0), but only (0, 1) is allowed!
    let mut b = CircuitBuilder::new("directed_reverse_cx");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[1]).unwrap();
    b.cx(q[1], q[0]).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let id_layout = LayoutMap::identity(3, 3).unwrap();
    let (routed, report) = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap();

    // An orientation SWAP between p0 and p1 was inserted
    assert_eq!(report.swaps_inserted, 1);

    // Output circuit is legal on target
    let validator = qcc::target::legality::TargetLegalityValidator::new(target.clone());
    let leg_report = validator.check_circuit(&routed).unwrap();
    assert!(leg_report.is_valid());

    // Verified semantically equivalent
    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&logical, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 7. ADR-0001 Strict Ancilla Zero-State Tests
// ============================================================================

#[test]
fn test_baseline_router_adr0001_ancilla_zero_state_preserved() {
    // Target has 4 physical qubits in a line: 0 - 1 - 2 - 3
    let target =
        make_target_with_native_swap("ancilla_line_4q", 4, [(0, 1), (1, 2), (2, 3)], false);

    // Logical circuit has only 2 qubits: q0, q1
    // Initial mapping puts them at ends: q0 -> p0, q1 -> p3 (p1 and p2 are idle ancillae!)
    let initial_layout = LayoutMap::from_mappings(
        2,
        4,
        [
            (QubitId(0), PhysicalQubitId(0)),
            (QubitId(1), PhysicalQubitId(3)),
        ],
    )
    .unwrap();

    let mut b = CircuitBuilder::new("adr0001_circuit");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.rz(q[1], ParameterExpr::constant(PI / 4.0)).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let (routed, report) = router
        .route_circuit(&logical, &target, Some(initial_layout))
        .unwrap();

    // Logical qubits routed through the ancillae p1, p2
    assert!(report.swaps_inserted > 0);

    // Full 4-qubit layout-aware check verifying ancillae remain strictly in |0>
    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&logical, &routed, &report.layout)
        .unwrap();

    assert!(verdict.is_equivalent);
    assert_eq!(verdict.qubits, 4); // Verified across all 4 physical qubits
    assert!(verdict.max_amplitude_difference < 1e-12);
}

// ============================================================================
// 8. Dependency Preservation and Sequential Ordering Tests
// ============================================================================

#[test]
fn test_baseline_router_dependency_preservation() {
    // 3 physical qubits: 0 - 1 - 2
    let target = make_target_with_native_swap("dep_target", 3, [(0, 1), (1, 2)], false);

    // Interleaved dependent operations:
    // 1. CX(q0, q2) -> requires SWAP
    // 2. CX(q1, q2) -> depends on previous operations on q2
    // 3. CX(q0, q1) -> depends on previous operations on q0, q1
    let mut b = CircuitBuilder::new("dep_circuit");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[2]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let id_layout = LayoutMap::identity(3, 3).unwrap();
    let (routed, report) = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap();

    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&logical, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

// ============================================================================
// 9. SWAP Strategies Tests
// ============================================================================

#[test]
fn test_baseline_router_swap_strategies() {
    let target = make_target_with_native_swap("strategy_line", 4, [(0, 1), (1, 2), (2, 3)], false);

    let mut b = CircuitBuilder::new("strategy_circuit");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[3]).unwrap();
    let logical = b.finish().unwrap();

    let strategies = [
        PathSwapStrategy::SourceToTarget,
        PathSwapStrategy::TargetToSource,
        PathSwapStrategy::Bidirectional,
    ];

    let checker = LayoutEquivalenceChecker::default();

    for strategy in strategies {
        let config = BaselineRouterConfig {
            swap_strategy: strategy,
            ..BaselineRouterConfig::default()
        };
        let router = BaselineRouter::with_config(config);
        let id_layout = LayoutMap::identity(4, 4).unwrap();
        let (routed, report) = router
            .route_circuit(&logical, &target, Some(id_layout))
            .unwrap();

        assert_eq!(report.swaps_inserted, 2);
        let verdict = checker
            .check_layout_equivalence(&logical, &routed, &report.layout)
            .unwrap();
        assert!(
            verdict.is_equivalent,
            "failed equivalence under strategy {:?}",
            strategy
        );
    }
}

// ============================================================================
// 10. Global Phase and Parameter Preservation Tests
// ============================================================================

#[test]
fn test_baseline_router_preserves_parameters_and_global_phase() {
    let target = make_target_with_native_swap("phase_target", 3, [(0, 1), (1, 2)], false);

    let mut b = CircuitBuilder::new("phase_circuit");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[0]).unwrap();
    b.rz(q[0], ParameterExpr::constant(PI / 3.0)).unwrap();
    b.cx(q[0], q[2]).unwrap();
    let mut logical = b.finish().unwrap();
    logical.set_global_phase(ParameterExpr::constant(PI / 4.0));

    let router = BaselineRouter::new();
    let id_layout = LayoutMap::identity(3, 3).unwrap();
    let (routed, report) = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap();

    // Check circuit-level global phase preserved
    assert_eq!(routed.global_phase(), logical.global_phase());

    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&logical, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

// ============================================================================
// 11. Bounded Iteration and Error Handling Tests
// ============================================================================

#[test]
fn test_baseline_router_swap_limit_exceeded() {
    let target = make_target_with_native_swap("limit_target", 4, [(0, 1), (1, 2), (2, 3)], false);

    // CX(q0, q3) requires 2 SWAPs, but limit is set to 1
    let mut b = CircuitBuilder::new("limit_circuit");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.cx(q[0], q[3]).unwrap();
    let logical = b.finish().unwrap();

    let config = BaselineRouterConfig {
        max_swaps: 1,
        ..BaselineRouterConfig::default()
    };
    let router = BaselineRouter::with_config(config);
    let id_layout = LayoutMap::identity(4, 4).unwrap();
    let err = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap_err();

    assert!(matches!(err, RoutingError::IterationLimitExceeded { .. }));
}

#[test]
fn test_baseline_router_insufficient_capacity() {
    let target = make_target_with_native_swap("small_target", 3, [(0, 1), (1, 2)], false);

    // 4 logical qubits on a 3-qubit device
    let mut b = CircuitBuilder::new("too_large");
    let _q = b.add_quantum_register("q", 4).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let err = router.route_circuit(&logical, &target, None).unwrap_err();

    match err {
        RoutingError::InsufficientCapacity { required, capacity } => {
            assert_eq!(required, 4);
            assert_eq!(capacity, 3);
        }
        other => panic!("expected InsufficientCapacity, got {other:?}"),
    }
}

// ============================================================================
// 12. Decomposed SWAP Target Tests (No Native SWAP)
// ============================================================================

#[test]
fn test_baseline_router_decomposed_swap_target() {
    // Target supports ONLY CX (no native SWAP)
    let target = make_target_cx_only("cx_only_target", 3, [(0, 1), (1, 2)], false);

    let mut b = CircuitBuilder::new("decomposed_swap_circ");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[2]).unwrap();
    let logical = b.finish().unwrap();

    let router = BaselineRouter::new();
    let id_layout = LayoutMap::identity(3, 3).unwrap();
    let (routed, report) = router
        .route_circuit(&logical, &target, Some(id_layout))
        .unwrap();

    // 1 physical routing SWAP was needed, decomposed into 3 CXs
    assert_eq!(report.swaps_inserted, 1);

    // Target legality validator passes with 0 violations
    let validator = qcc::target::legality::TargetLegalityValidator::new(target.clone());
    let leg_report = validator.check_circuit(&routed).unwrap();
    assert!(leg_report.is_valid());

    // Verified semantically equivalent
    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&logical, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}
