//! Integration tests and hand-calculated fixtures for SABRE Bidirectional Routing and Layout Refinement (Phase 52).
//!
//! Validates:
//! - Reverse circuit traversal inverting dependency order without mutating original circuit.
//! - Correct permutation and inverse-permutation conventions at forward/reverse boundaries.
//! - Derivation of improved initial layouts reducing inserted SWAPs from hand-calculated fixtures.
//! - Forward routing after reverse traversal.
//! - Layout refinement acceptance and rejection behavior.
//! - Initial, intermediate, and final layout consistency.
//! - Layout-aware semantic equivalence verification.
//! - Partially occupied physical devices and ADR-0001 strict ancilla zero-state restoration.
//! - Disconnected topology and capacity failure paths.
//! - 100% deterministic repeatability across identical runs.
//! - Regression tests preventing confusion between forward layouts and inverse permutations.

use qcc::builder::CircuitBuilder;
use qcc::ir::wire::QubitId;
use qcc::quantum::Angle;
use qcc::routing::layout_map::LayoutMap;
use qcc::routing::pathfinding::RoutingError;
use qcc::routing::sabre::{reverse_circuit, SabreBidirectionalRouter, SabreRouterConfig};
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};
use qcc::verification::layout_equivalence::LayoutEquivalenceChecker;

/// Helper constructing a standard linear coupling target (0 - 1 - ... - n-1).
fn make_linear_target(name: &str, num_qubits: usize) -> Target {
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
        .duration(20.0)
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

        let swap_desc = TargetOperationDescriptor::builder("swap", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(edges))
            .duration(300.0)
            .build();
        target.add_operation(swap_desc);
    }

    target.validate().unwrap();
    target
}

/// Helper constructing a ring coupling target (0 - 1 - ... - n-1 - 0).
fn make_ring_target(name: &str, num_qubits: usize) -> Target {
    let mut target = Target::new(name, num_qubits);

    let h_desc = TargetOperationDescriptor::builder("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(20.0)
        .build();
    target.add_operation(h_desc);

    let mut edges = Vec::new();
    for i in 0..num_qubits {
        let next = (i + 1) % num_qubits;
        edges.push((
            PhysicalQubitId::new(i as u32),
            PhysicalQubitId::new(next as u32),
        ));
        edges.push((
            PhysicalQubitId::new(next as u32),
            PhysicalQubitId::new(i as u32),
        ));
    }
    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit(edges.clone()))
        .duration(150.0)
        .build();
    target.add_operation(cx_desc);

    let swap_desc = TargetOperationDescriptor::builder("swap", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit(edges))
        .duration(300.0)
        .build();
    target.add_operation(swap_desc);

    target.validate().unwrap();
    target
}

// ============================================================================
// 1. Reverse Traversal on Hand-Calculated Fixtures
// ============================================================================

#[test]
fn test_reverse_circuit_traversal_inverts_dependencies_and_phase() {
    let mut b = CircuitBuilder::new("forward_test");
    let q = b.add_quantum_register("q", 3).unwrap();
    let c = b.add_classical_register("c", 2).unwrap();
    b.set_global_phase(Angle::from_radians(std::f64::consts::FRAC_PI_4));

    b.h(q[0]).unwrap(); // op 1
    b.cx(q[0], q[1]).unwrap(); // op 2
    b.x(q[2]).unwrap(); // op 3
    b.cx(q[1], q[2]).unwrap(); // op 4
    b.measure(q[0], c[0]).unwrap(); // op 5

    let forward = b.finish().unwrap();
    let fwd_ops_count = forward.num_operations();
    let fwd_phase = forward.global_phase().clone();

    // Reverse circuit
    let reversed = reverse_circuit(&forward).unwrap();

    // Invariants:
    // 1. Original forward circuit is completely unmutated
    assert_eq!(forward.num_operations(), fwd_ops_count);
    assert_eq!(forward.global_phase(), &fwd_phase);

    // 2. Reversed circuit preserves register structure
    assert_eq!(reversed.quantum_registers.len(), 1);
    assert_eq!(reversed.quantum_registers[0].size, 3);
    assert_eq!(reversed.classical_registers.len(), 1);
    assert_eq!(reversed.classical_registers[0].size, 2);

    // 3. Reversed global phase is negated: -pi/4
    let expected_rev_phase = -fwd_phase;
    assert_eq!(reversed.global_phase(), &expected_rev_phase);

    // 4. Reversed circuit operation order
    let rev_topo = reversed.graph.deterministic_topological_order().unwrap();
    assert_eq!(rev_topo.len(), 5);

    // First op in reversed is measurement on q0, second is CX(q1, q2)
    let first_op = reversed.graph.get_operation(rev_topo[0]).unwrap();
    assert!(matches!(
        first_op,
        qcc::ir::operation::Operation::Measure { .. }
    ));

    let second_op = reversed.graph.get_operation(rev_topo[1]).unwrap();
    assert!(matches!(
        second_op,
        qcc::ir::operation::Operation::Gate { gate_name, .. } if gate_name == "cx"
    ));
}

// ============================================================================
// 2. Improved Initial Layout Derivation (Hand-Calculated Fixture)
// ============================================================================

#[test]
fn test_bidirectional_sabre_derives_improved_initial_layout() {
    // 3-qubit line: p0 - p1 - p2
    let target = make_linear_target("line3", 3);

    // Circuit:
    // g1: CX(q0, q2)
    // g2: CX(q1, q2)
    let mut b = CircuitBuilder::new("hand_calc_swap_reduction");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[2]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    // Baseline: forward SABRE from identity layout [q0->p0, q1->p1, q2->p2]
    // Under identity: q2 is placed at endpoint p2, requiring 1 SWAP to move to center p1.
    let single_config = SabreRouterConfig {
        refinement_rounds: 1, // forward only
        ..Default::default()
    };
    let single_router = SabreBidirectionalRouter::with_config(single_config);

    let identity_layout = LayoutMap::identity(3, 3).unwrap();
    let (_routed_1, report_1) = single_router
        .route_circuit(&circuit, &target, Some(identity_layout.clone()))
        .unwrap();
    assert_eq!(report_1.swaps_inserted, 1);

    // Bidirectional SABRE with 3 refinement rounds:
    // Reverse traversal routes g2 then g1 from the end of the circuit,
    // placing q0 and q2 adjacent at the initial position!
    let bidi_config = SabreRouterConfig {
        refinement_rounds: 3,
        ..Default::default()
    };
    let bidi_router = SabreBidirectionalRouter::with_config(bidi_config);

    let (routed_bidi, report_bidi) = bidi_router
        .route_circuit(&circuit, &target, Some(identity_layout))
        .unwrap();

    // Bidirectional SABRE must find an initial layout that strictly reduces SWAPs
    assert!(
        report_bidi.swaps_inserted < report_1.swaps_inserted,
        "Bidirectional SABRE swaps ({}) should be strictly less than unrefined forward pass ({})",
        report_bidi.swaps_inserted,
        report_1.swaps_inserted
    );

    // With the optimal derived initial layout, 0 SWAPs are needed!
    assert_eq!(report_bidi.swaps_inserted, 0);
    assert_eq!(routed_bidi.num_operations(), 2);
    assert!(report_bidi.refinement.improved_upon_initial());

    // Verify layout-aware semantic equivalence
    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&circuit, &routed_bidi, &report_bidi.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

// ============================================================================
// 3. Multi-Round Refinement Acceptance & Rejection
// ============================================================================

#[test]
fn test_refinement_acceptance_and_summary_reporting() {
    let target = make_linear_target("line4", 4);

    let mut b = CircuitBuilder::new("refinement_flow");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.cx(q[0], q[3]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    b.cx(q[0], q[2]).unwrap();
    b.cx(q[1], q[3]).unwrap();
    let circuit = b.finish().unwrap();

    let config = SabreRouterConfig {
        refinement_rounds: 4,
        ..Default::default()
    };
    let router = SabreBidirectionalRouter::with_config(config);

    let (routed, report) = router.route_circuit(&circuit, &target, None).unwrap();

    // Summary must accurately record refinement progression
    let summary = &report.refinement;
    assert!(summary.total_rounds() >= 1);
    assert!(summary.winning_round <= summary.total_rounds());
    assert_eq!(summary.winning_swaps, report.swaps_inserted);
    assert_eq!(summary.winning_depth, report.metrics.circuit_depth);
    assert!((summary.winning_cost - report.metrics.weighted_cost).abs() < 1e-12);

    // Check round records consistency
    for round in &summary.rounds {
        assert!(round.round >= 1);
        assert_eq!(round.initial_layout.num_logical(), 4);
        assert_eq!(round.initial_layout.num_physical(), 4);
        assert_eq!(round.next_initial_layout.num_logical(), 4);
        assert_eq!(round.next_initial_layout.num_physical(), 4);
    }

    // Equivalence check
    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&circuit, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 4. ADR-0001 Strict Ancilla Zero-State Preservation
// ============================================================================

#[test]
fn test_partially_occupied_device_and_strict_ancilla_zero_state() {
    // 2 logical qubits on a 5-physical-qubit line device (0 - 1 - 2 - 3 - 4)
    // Physical qubits 2, 3, 4 are idle ancillae starting in |0>
    let target = make_linear_target("line5", 5);

    let mut b = CircuitBuilder::new("bell_ancillae");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    // Start with non-adjacent placement: q0 at p0, q1 at p2
    let initial_layout =
        LayoutMap::from_slice(5, &[PhysicalQubitId::new(0), PhysicalQubitId::new(2)]).unwrap();

    let config = SabreRouterConfig {
        refinement_rounds: 2,
        ..Default::default()
    };
    let router = SabreBidirectionalRouter::with_config(config);

    let (routed, report) = router
        .route_circuit(&circuit, &target, Some(initial_layout))
        .unwrap();

    // The routed circuit must satisfy ADR-0001 strict ancilla policy:
    // Physical qubits not occupied by logical qubits must remain/return to |0>
    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&circuit, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

// ============================================================================
// 5. Permutation Inversion Regression Test
// ============================================================================

#[test]
fn test_permutation_inversion_convention_preservation() {
    // Verify that asymmetric layout permutations are never confused with their inverse
    // Consider asymmetric bijection:
    // q0 -> p1
    // q1 -> p2
    // q2 -> p0
    // L = [1, 2, 0]. Its inverse is L^{-1} = [2, 0, 1].
    let perm = vec![1, 2, 0];
    let layout = LayoutMap::from_permutation(&perm).unwrap();

    assert_eq!(layout.get_physical(QubitId(0)), Some(PhysicalQubitId(1)));
    assert_eq!(layout.get_physical(QubitId(1)), Some(PhysicalQubitId(2)));
    assert_eq!(layout.get_physical(QubitId(2)), Some(PhysicalQubitId(0)));

    // Inverse check
    assert_eq!(layout.get_logical(PhysicalQubitId(0)), Some(QubitId(2)));
    assert_eq!(layout.get_logical(PhysicalQubitId(1)), Some(QubitId(0)));
    assert_eq!(layout.get_logical(PhysicalQubitId(2)), Some(QubitId(1)));

    // A router confusing L with L^{-1} would map q0 to p2 instead of p1.
    // Verify SABRE bidirectional router preserves L precisely:
    let target = make_ring_target("ring3", 3);
    let mut b = CircuitBuilder::new("asym_perm");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    let router = SabreBidirectionalRouter::new();
    let (routed, report) = router
        .route_circuit(&circuit, &target, Some(layout.clone()))
        .unwrap();

    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&circuit, &routed, &report.layout)
        .unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 6. Error & Failure Paths
// ============================================================================

#[test]
fn test_disconnected_topology_fails_explicitly() {
    // Disconnected target: {0, 1} and {2, 3}
    let mut target = Target::new("disconnected", 4);
    let h_desc = TargetOperationDescriptor::builder("h", 1)
        .connectivity(OperationConnectivity::all_single_qubits(4))
        .build();
    target.add_operation(h_desc);

    let cx_desc = TargetOperationDescriptor::builder("cx", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit(vec![
            (PhysicalQubitId::new(0), PhysicalQubitId::new(1)),
            (PhysicalQubitId::new(1), PhysicalQubitId::new(0)),
            (PhysicalQubitId::new(2), PhysicalQubitId::new(3)),
            (PhysicalQubitId::new(3), PhysicalQubitId::new(2)),
        ]))
        .build();
    target.add_operation(cx_desc);
    target.validate().unwrap();

    let mut b = CircuitBuilder::new("cross_component");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.cx(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    // Map q0 to p0, q1 to p2 (disconnected components)
    let layout =
        LayoutMap::from_slice(4, &[PhysicalQubitId::new(0), PhysicalQubitId::new(2)]).unwrap();

    let router = SabreBidirectionalRouter::new();
    let err = router
        .route_circuit(&circuit, &target, Some(layout))
        .unwrap_err();
    eprintln!("Actual error: {err:?}");

    assert!(matches!(err, RoutingError::DisconnectedTopology { .. }));
}

#[test]
fn test_insufficient_capacity_fails_explicitly() {
    let target = make_linear_target("line2", 2);

    let mut b = CircuitBuilder::new("needs_3_qubits");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let router = SabreBidirectionalRouter::new();
    let err = router.route_circuit(&circuit, &target, None).unwrap_err();

    assert_eq!(
        err,
        RoutingError::InsufficientCapacity {
            required: 3,
            capacity: 2,
        }
    );
}

// ============================================================================
// 7. Deterministic Repeatability
// ============================================================================

#[test]
fn test_deterministic_repeatability_across_identical_runs() {
    let target = make_linear_target("line5", 5);

    let mut b = CircuitBuilder::new("repeatability");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[3]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    b.cx(q[2], q[3]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    let config = SabreRouterConfig {
        refinement_rounds: 3,
        random_seed: Some(12345),
        ..Default::default()
    };
    let router = SabreBidirectionalRouter::with_config(config);

    let (baseline_circuit, baseline_report) =
        router.route_circuit(&circuit, &target, None).unwrap();

    for _ in 0..5 {
        let (run_circuit, run_report) = router.route_circuit(&circuit, &target, None).unwrap();

        assert_eq!(run_report.swaps_inserted, baseline_report.swaps_inserted);
        assert_eq!(
            run_report.metrics.circuit_depth,
            baseline_report.metrics.circuit_depth
        );
        assert_eq!(run_report.layout, baseline_report.layout);
        assert_eq!(
            run_circuit.num_operations(),
            baseline_circuit.num_operations()
        );
        assert_eq!(run_report.refinement, baseline_report.refinement);
    }
}
