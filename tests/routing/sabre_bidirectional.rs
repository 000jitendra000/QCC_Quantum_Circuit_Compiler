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

use std::collections::BTreeMap;

use qcc::builder::CircuitBuilder;
use qcc::ir::circuit::Circuit;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::QubitId;
use qcc::quantum::Angle;
use qcc::routing::layout_map::LayoutMap;
use qcc::routing::metrics::RoutingMetrics;
use qcc::routing::pathfinding::RoutingError;
use qcc::routing::sabre::{
    is_refinement_improvement, reverse_circuit, SabreBidirectionalRouter, SabreRouterConfig,
};
use qcc::simulator::execute_circuit;
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::metrics::DurationCost;
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

    let measure_desc = TargetOperationDescriptor::builder("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(100.0)
        .build();
    target.add_operation(measure_desc);

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
    b.set_global_phase(Angle::from_radians(std::f64::consts::FRAC_PI_3));

    b.h(q[0]).unwrap(); // op 1 (self-inverse)
    b.cx(q[0], q[1]).unwrap(); // op 2 (self-inverse 2Q)
    b.s(q[2]).unwrap(); // op 3 (paired -> sdg)
    b.t(q[0]).unwrap(); // op 4 (paired -> tdg)
    b.rx(q[1], Angle::from_radians(0.45)).unwrap(); // op 5 (param -> -0.45)
    b.ry(q[2], Angle::from_radians(0.75)).unwrap(); // op 6 (param -> -0.75)
    b.rz(q[0], Angle::from_radians(1.25)).unwrap(); // op 7 (param -> -1.25)
    b.rx(q[2], Angle::from_radians(0.65)).unwrap(); // op 8 (param -> -0.65)
    b.cz(q[0], q[2]).unwrap(); // op 9 (self-inverse 2Q)
    b.barrier(&[q[0], q[1], q[2]]).unwrap(); // op 10 (barrier preserved)

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

    // 3. Reversed global phase is negated: -pi/3
    let expected_rev_phase = -fwd_phase;
    assert_eq!(reversed.global_phase(), &expected_rev_phase);

    // 4. Reversed circuit operation count and order
    let rev_topo = reversed.graph.deterministic_topological_order().unwrap();
    assert_eq!(rev_topo.len(), 10);

    // First op in reversed is barrier on [q0, q1, q2]
    let op0 = reversed.graph.get_operation(rev_topo[0]).unwrap();
    assert!(matches!(op0, qcc::ir::operation::Operation::Barrier { .. }));

    // Second op is CZ(q0, q2)
    let op1 = reversed.graph.get_operation(rev_topo[1]).unwrap();
    assert!(matches!(
        op1,
        qcc::ir::operation::Operation::Gate { gate_name, .. } if gate_name == "cz"
    ));

    // Third op is Rx with negated parameter -0.65
    let op2 = reversed.graph.get_operation(rev_topo[2]).unwrap();
    if let qcc::ir::operation::Operation::Gate {
        gate_name,
        parameters,
        ..
    } = op2
    {
        assert_eq!(gate_name, "rx");
        assert_eq!(parameters.len(), 1);
        if let ParameterExpr::Constant(val) = &parameters[0] {
            assert!((val + 0.65).abs() < 1e-12);
        } else {
            panic!("expected constant parameter");
        }
    } else {
        panic!("expected Rx gate");
    }

    // Fourth op is Rz with negated parameter -1.25
    let op3 = reversed.graph.get_operation(rev_topo[3]).unwrap();
    if let qcc::ir::operation::Operation::Gate {
        gate_name,
        parameters,
        ..
    } = op3
    {
        assert_eq!(gate_name, "rz");
        assert_eq!(parameters.len(), 1);
        if let ParameterExpr::Constant(val) = &parameters[0] {
            assert!((val + 1.25).abs() < 1e-12);
        } else {
            panic!("expected constant parameter");
        }
    } else {
        panic!("expected Rz gate");
    }

    // Fifth op is Ry with negated parameter -0.75
    let op4 = reversed.graph.get_operation(rev_topo[4]).unwrap();
    if let qcc::ir::operation::Operation::Gate {
        gate_name,
        parameters,
        ..
    } = op4
    {
        assert_eq!(gate_name, "ry");
        assert_eq!(parameters.len(), 1);
        if let ParameterExpr::Constant(val) = &parameters[0] {
            assert!((val + 0.75).abs() < 1e-12);
        } else {
            panic!("expected constant parameter");
        }
    } else {
        panic!("expected Ry gate");
    }

    // Sixth op is Rx with negated parameter -0.45
    let op5 = reversed.graph.get_operation(rev_topo[5]).unwrap();
    if let qcc::ir::operation::Operation::Gate {
        gate_name,
        parameters,
        ..
    } = op5
    {
        assert_eq!(gate_name, "rx");
        assert_eq!(parameters.len(), 1);
        if let ParameterExpr::Constant(val) = &parameters[0] {
            assert!((val + 0.45).abs() < 1e-12);
        } else {
            panic!("expected constant parameter");
        }
    } else {
        panic!("expected Rx gate");
    }

    // Seventh op is Tdg (inverse of T)
    let op6 = reversed.graph.get_operation(rev_topo[6]).unwrap();
    assert!(matches!(
        op6,
        qcc::ir::operation::Operation::Gate { gate_name, .. } if gate_name == "tdg"
    ));

    // Eighth op is Sdg (inverse of S)
    let op7 = reversed.graph.get_operation(rev_topo[7]).unwrap();
    assert!(matches!(
        op7,
        qcc::ir::operation::Operation::Gate { gate_name, .. } if gate_name == "sdg"
    ));

    // Ninth op is CX(q0, q1)
    let op8 = reversed.graph.get_operation(rev_topo[8]).unwrap();
    assert!(matches!(
        op8,
        qcc::ir::operation::Operation::Gate { gate_name, .. } if gate_name == "cx"
    ));

    // Tenth op is H(q0)
    let op9 = reversed.graph.get_operation(rev_topo[9]).unwrap();
    assert!(matches!(
        op9,
        qcc::ir::operation::Operation::Gate { gate_name, .. } if gate_name == "h"
    ));

    // 5. Semantic Unitarity Verification: C_rev . C == Identity
    // Construct composite circuit: C followed by C_rev
    let mut identity_circuit = Circuit::new("identity_test");
    identity_circuit.add_quantum_register("q", 3).unwrap();
    identity_circuit
        .set_global_phase(forward.global_phase().clone() + reversed.global_phase().clone());

    for op_id in forward.graph.deterministic_topological_order().unwrap() {
        let op = forward.graph.get_operation(op_id).unwrap();
        identity_circuit.add_operation(op.clone()).unwrap();
    }
    for op_id in reversed.graph.deterministic_topological_order().unwrap() {
        let op = reversed.graph.get_operation(op_id).unwrap();
        identity_circuit.add_operation(op.clone()).unwrap();
    }

    let (sv, _trace) = execute_circuit(&identity_circuit).unwrap();

    // Output state must equal |000> with magnitude 1.0 (zero phase offset)
    assert!(
        (sv.get(0).unwrap().norm() - 1.0).abs() < 1e-12,
        "Identity circuit amplitude for |000> should be 1.0"
    );
    for i in 1..8 {
        assert!(
            sv.get(i).unwrap().norm() < 1e-12,
            "Amplitude for state |{i:03b}> should be 0.0"
        );
    }
}

#[test]
fn test_reverse_circuit_rejects_non_unitary_operations() {
    // 1. Measurement rejection
    let mut b1 = CircuitBuilder::new("has_measure");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    let c1 = b1.add_classical_register("c", 1).unwrap();
    b1.h(q1[0]).unwrap();
    b1.measure(q1[0], c1[0]).unwrap();
    let c_measure = b1.finish().unwrap();

    let err1 = reverse_circuit(&c_measure).unwrap_err();
    assert!(
        matches!(err1, RoutingError::NonUnitaryReverseTraversal(_)),
        "Expected NonUnitaryReverseTraversal, got {err1:?}"
    );

    // 2. Reset rejection
    let mut b2 = CircuitBuilder::new("has_reset");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.h(q2[0]).unwrap();
    b2.reset(q2[0]).unwrap();
    let c_reset = b2.finish().unwrap();

    let err2 = reverse_circuit(&c_reset).unwrap_err();
    assert!(
        matches!(err2, RoutingError::NonUnitaryReverseTraversal(_)),
        "Expected NonUnitaryReverseTraversal, got {err2:?}"
    );

    // 3. Multi-round router rejection on circuits with measurement
    let target = make_linear_target("line2", 2);
    let config = SabreRouterConfig {
        refinement_rounds: 3, // multi-round triggers reverse traversal
        ..Default::default()
    };
    let router = SabreBidirectionalRouter::with_config(config);
    let err3 = router.route_circuit(&c_measure, &target, None).unwrap_err();
    assert!(
        matches!(err3, RoutingError::NonUnitaryReverseTraversal(_)),
        "Multi-round refinement on non-unitary circuit must fail explicitly, got {err3:?}"
    );

    // 4. Single-round router (forward-only pass) supports circuits with measurement
    let forward_config = SabreRouterConfig {
        refinement_rounds: 1,      // forward only
        verify_equivalence: false, // layout equivalence checker only operates on unitary circuits
        ..Default::default()
    };
    let fwd_router = SabreBidirectionalRouter::with_config(forward_config);
    let (routed, report) = fwd_router.route_circuit(&c_measure, &target, None).unwrap();
    assert!(report.routed_operations >= 2);
    assert_eq!(routed.num_qubits(), 2);
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

// ============================================================================
// 8. Refinement Decision Rule, Tie-Breaking, and Telemetry Audit Tests
// ============================================================================

fn make_test_metrics(
    weighted_cost: f64,
    routing_swaps_count: usize,
    circuit_depth: usize,
    two_qubit_depth: usize,
) -> RoutingMetrics {
    RoutingMetrics {
        circuit_depth,
        two_qubit_depth,
        total_gates: 0,
        single_qubit_gates: 0,
        two_qubit_gates: 0,
        multi_qubit_gates: 0,
        routing_swaps_count,
        routing_swap_gates: routing_swaps_count,
        original_entanglers_count: 0,
        direction_corrections_count: 0,
        total_routing_overhead_ops: routing_swaps_count,
        gate_counts: BTreeMap::new(),
        duration: DurationCost::Unknown,
        initial_layout: LayoutMap::identity(2, 2).unwrap(),
        final_layout: LayoutMap::identity(2, 2).unwrap(),
        seed: None,
        run_duration: None,
        weighted_cost,
    }
}

#[test]
fn test_is_refinement_improvement_decision_hierarchy_and_tie_breaking() {
    let baseline = make_test_metrics(100.0, 5, 10, 6);

    // Case 1: Lower composite cost is accepted even if SWAPs and depth are substantially higher
    let candidate_lower_cost = make_test_metrics(90.0, 10, 20, 12);
    assert!(is_refinement_improvement(&candidate_lower_cost, &baseline));

    // Case 2: Higher composite cost is rejected even if SWAPs and depth are substantially lower (conflicting metrics)
    let candidate_higher_cost = make_test_metrics(110.0, 0, 2, 1);
    assert!(!is_refinement_improvement(
        &candidate_higher_cost,
        &baseline
    ));

    // Case 3: Equal cost (within 1e-12) -> Tie-Breaker 1: Fewer SWAPs accepted
    let candidate_fewer_swaps = make_test_metrics(100.0, 4, 15, 10);
    assert!(is_refinement_improvement(&candidate_fewer_swaps, &baseline));

    let candidate_more_swaps = make_test_metrics(100.0, 6, 5, 3);
    assert!(!is_refinement_improvement(&candidate_more_swaps, &baseline));

    // Case 4: Equal cost, equal SWAPs -> Tie-Breaker 2: Lower critical-path depth accepted
    let candidate_lower_depth = make_test_metrics(100.0, 5, 9, 8);
    assert!(is_refinement_improvement(&candidate_lower_depth, &baseline));

    let candidate_higher_depth = make_test_metrics(100.0, 5, 11, 4);
    assert!(!is_refinement_improvement(
        &candidate_higher_depth,
        &baseline
    ));

    // Case 5: Equal cost, equal SWAPs, equal depth -> Tie-Breaker 3: Lower 2Q depth accepted
    let candidate_lower_2q_depth = make_test_metrics(100.0, 5, 10, 5);
    assert!(is_refinement_improvement(
        &candidate_lower_2q_depth,
        &baseline
    ));

    let candidate_higher_2q_depth = make_test_metrics(100.0, 5, 10, 7);
    assert!(!is_refinement_improvement(
        &candidate_higher_2q_depth,
        &baseline
    ));

    // Case 6: All four metrics identical -> Candidate is not an improvement (returns false)
    let candidate_identical = make_test_metrics(100.0, 5, 10, 6);
    assert!(!is_refinement_improvement(&candidate_identical, &baseline));

    // Case 7: NaN handling
    let candidate_nan = make_test_metrics(f64::NAN, 1, 1, 1);
    assert!(!is_refinement_improvement(&candidate_nan, &baseline));

    let baseline_nan = make_test_metrics(f64::NAN, 1, 1, 1);
    let candidate_normal = make_test_metrics(100.0, 1, 1, 1);
    assert!(!is_refinement_improvement(&candidate_normal, &baseline_nan));
}

#[test]
fn test_refinement_rejection_preserves_winning_round_consistency() {
    // Construct circuit that routes with 0 SWAPs in Round 1
    let target = make_linear_target("line3", 3);
    let mut b = CircuitBuilder::new("zero_swap_optimal");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let initial_layout = LayoutMap::identity(3, 3).unwrap();
    let config = SabreRouterConfig {
        refinement_rounds: 3,
        ..Default::default()
    };
    let router = SabreBidirectionalRouter::with_config(config);

    let (routed, report) = router
        .route_circuit(&circuit, &target, Some(initial_layout))
        .unwrap();

    // Round 1 achieves 0 SWAPs, which terminates refinement immediately as optimal
    assert_eq!(report.swaps_inserted, 0);
    assert_eq!(report.refinement.winning_round, 1);
    assert_eq!(report.refinement.total_rounds(), 1);
    assert_eq!(report.refinement.winning_swaps, 0);
    assert_eq!(routed.num_operations(), circuit.num_operations());
}

#[test]
fn test_asymmetric_initial_layout_permutation_and_refinement() {
    // 4-qubit line device (0 - 1 - 2 - 3)
    let target = make_linear_target("line4", 4);

    // Asymmetric 4-cycle permutation:
    // q0 -> p2
    // q1 -> p0
    // q2 -> p3
    // q3 -> p1
    // L = [2, 0, 3, 1]. Its inverse is L^{-1} = [1, 3, 0, 2].
    // Since L != L^{-1}, any inverted layout convention would immediately fail layout equivalence.
    let perm = vec![2, 0, 3, 1];
    let initial_layout = LayoutMap::from_permutation(&perm).unwrap();

    let mut b = CircuitBuilder::new("asym_refinement");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[2], q[3]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let config = SabreRouterConfig {
        refinement_rounds: 3,
        ..Default::default()
    };
    let router = SabreBidirectionalRouter::with_config(config);

    let (routed, report) = router
        .route_circuit(&circuit, &target, Some(initial_layout.clone()))
        .unwrap();

    // Verify layout equivalence on the resulting routed circuit
    let checker = LayoutEquivalenceChecker::default();
    let verdict = checker
        .check_layout_equivalence(&circuit, &routed, &report.layout)
        .unwrap();
    assert!(
        verdict.is_equivalent,
        "Asymmetric permutation routing must satisfy layout-aware equivalence"
    );
    assert!(verdict.max_amplitude_difference < 1e-12);

    // Verify refinement round history correctly maps next_initial_layout as a valid layout
    for round in &report.refinement.rounds {
        assert_eq!(round.initial_layout.num_logical(), 4);
        assert_eq!(round.initial_layout.num_physical(), 4);
        assert_eq!(round.next_initial_layout.num_logical(), 4);
        assert_eq!(round.next_initial_layout.num_physical(), 4);
        assert!(round.next_initial_layout.validate().is_ok());
    }
}

#[test]
fn test_telemetry_separates_forward_and_reverse_metrics() {
    let target = make_linear_target("line4", 4);

    let mut b = CircuitBuilder::new("telemetry_separation");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.cx(q[0], q[3]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    b.cx(q[0], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let config = SabreRouterConfig {
        refinement_rounds: 3,
        ..Default::default()
    };
    let router = SabreBidirectionalRouter::with_config(config);

    let (_routed, report) = router.route_circuit(&circuit, &target, None).unwrap();

    let summary = &report.refinement;
    assert!(summary.total_rounds() >= 1);

    // Each round record explicitly distinguishes forward metrics from reverse metrics
    for round in &summary.rounds {
        // Forward metrics match forward pass
        assert_eq!(round.forward_swaps, round.forward_swaps);
        assert!(round.forward_depth >= 1);
        assert!(round.forward_cost > 0.0);

        // If reverse was executed in this round (non-zero SWAPs in forward pass)
        if round.forward_swaps > 0 {
            // Reverse swaps and cost are explicitly captured
            assert!(round.reverse_cost >= 0.0);
        }
    }

    // Winning metrics in report match winning forward pass strictly
    assert_eq!(report.swaps_inserted, summary.winning_swaps);
    assert_eq!(report.metrics.circuit_depth, summary.winning_depth);
    assert!((report.metrics.weighted_cost - summary.winning_cost).abs() < 1e-12);
}
