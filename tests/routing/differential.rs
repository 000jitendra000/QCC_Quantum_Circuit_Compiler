//! Differential routing validation and regression reproduction (Phase 53).
//!
//! Provides differential comparison between `BaselineRouter` and `SabreBidirectionalRouter`
//! on identical circuit and hardware targets. Includes minimized regression fixtures with
//! preserved failure reproduction seeds.

use qcc::builder::CircuitBuilder;
use qcc::gate_signature::ParameterDomain;
use qcc::ir::wire::QubitId;
use qcc::quantum::angle::Angle;
use qcc::routing::greedy::BaselineRouter;
use qcc::routing::layout_map::LayoutMap;
use qcc::routing::sabre::{SabreBidirectionalRouter, SabreHeuristicConfig, SabreRouterConfig};
use qcc::routing::RoutingError;
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::legality::TargetLegalityValidator;
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};
use qcc::verification::layout_equivalence::LayoutEquivalenceChecker;

// ============================================================================
// Preserved Regression Seeds
// ============================================================================

/// Recorded seed for disconnected graph validation.
pub const REGRESSION_SEED_DISCONNECTED: u64 = 0xDEAD_BEEF;

/// Recorded seed for star graph hub contention.
pub const REGRESSION_SEED_STAR_CONTENTION: u64 = 0xCAFE_BABE;

/// Recorded seed for asymmetric initial layout tracking ($L \neq L^{-1}$).
pub const REGRESSION_SEED_ASYMMETRIC_LAYOUT: u64 = 0xFEED_FACE;

/// Recorded seed for directed coupling constraint validation.
pub const REGRESSION_SEED_DIRECTED_LINE: u64 = 0x1234_5678;

/// Recorded seed for mixed rotation parameters and global phase.
pub const REGRESSION_SEED_MIXED_ROTATIONS: u64 = 0x9ABC_DEF0;

// ============================================================================
// Target Helpers
// ============================================================================

fn make_diff_target(
    name: &str,
    num_qubits: usize,
    edges: impl IntoIterator<Item = (u32, u32)>,
    directed_cx: bool,
) -> Target {
    let mut target = Target::new(name, num_qubits);

    for op_name in &["h", "x", "y", "z", "s", "sdg", "t", "tdg"] {
        let desc = TargetOperationDescriptor::builder(*op_name, 1)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .duration(20.0)
            .build();
        target.add_operation(desc);
    }

    for op_name in &["rx", "ry", "rz", "p"] {
        let desc = TargetOperationDescriptor::builder(*op_name, 1)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .add_parameter_domain(ParameterDomain::Angle4Pi)
            .duration(25.0)
            .build();
        target.add_operation(desc);
    }

    let desc_measure = TargetOperationDescriptor::builder("measure", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(100.0)
        .build();
    target.add_operation(desc_measure);

    let desc_reset = TargetOperationDescriptor::builder("reset", 1)
        .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
        .duration(50.0)
        .build();
    target.add_operation(desc_reset);

    let edge_list: Vec<(u32, u32)> = edges.into_iter().collect();
    if !edge_list.is_empty() {
        if directed_cx {
            let directed_edges: Vec<(PhysicalQubitId, PhysicalQubitId)> = edge_list
                .iter()
                .map(|(u, v)| (PhysicalQubitId::new(*u), PhysicalQubitId::new(*v)))
                .collect();
            let cx_desc = TargetOperationDescriptor::builder("cx", 2)
                .connectivity(OperationConnectivity::directed_two_qubit(directed_edges))
                .duration(150.0)
                .build();
            target.add_operation(cx_desc);
        } else {
            let undirected_edges: Vec<(PhysicalQubitId, PhysicalQubitId)> = edge_list
                .iter()
                .map(|(u, v)| (PhysicalQubitId::new(*u), PhysicalQubitId::new(*v)))
                .collect();
            let cx_desc = TargetOperationDescriptor::builder("cx", 2)
                .connectivity(OperationConnectivity::undirected_two_qubit(
                    undirected_edges.clone(),
                ))
                .duration(150.0)
                .build();
            target.add_operation(cx_desc);
        }

        let mut swap_edges = Vec::new();
        for &(u, v) in &edge_list {
            swap_edges.push((PhysicalQubitId::new(u), PhysicalQubitId::new(v)));
            swap_edges.push((PhysicalQubitId::new(v), PhysicalQubitId::new(u)));
        }
        let swap_desc = TargetOperationDescriptor::builder("swap", 2)
            .connectivity(OperationConnectivity::undirected_two_qubit(swap_edges))
            .duration(300.0)
            .build();
        target.add_operation(swap_desc);
    }

    target.validate().unwrap();
    target
}

fn make_line_target(name: &str, num_qubits: usize) -> Target {
    let mut edges = Vec::new();
    for i in 0..num_qubits.saturating_sub(1) {
        edges.push((i as u32, (i + 1) as u32));
    }
    make_diff_target(name, num_qubits, edges, false)
}

fn make_star_target(name: &str, num_qubits: usize) -> Target {
    let mut edges = Vec::new();
    for i in 1..num_qubits {
        edges.push((0u32, i as u32));
    }
    make_diff_target(name, num_qubits, edges, false)
}

fn make_grid_target(name: &str, rows: usize, cols: usize) -> Target {
    let num_qubits = rows * cols;
    let mut edges = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let u = (r * cols + c) as u32;
            if c + 1 < cols {
                let v = (r * cols + (c + 1)) as u32;
                edges.push((u, v));
            }
            if r + 1 < rows {
                let v = ((r + 1) * cols + c) as u32;
                edges.push((u, v));
            }
        }
    }
    make_diff_target(name, num_qubits, edges, false)
}

// ============================================================================
// 1. Differential Comparison Tests
// ============================================================================

#[test]
fn test_differential_routing_line_topology_comparison() {
    // 4-qubit circuit with non-adjacent interactions on a 4-qubit line
    let mut b = CircuitBuilder::new("diff_line_4q");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[3]).unwrap(); // distance 3 (requires SWAPs)
    b.cx(q[1], q[2]).unwrap(); // distance 1 (adjacent)
    b.cx(q[0], q[2]).unwrap(); // distance 2
    b.cx(q[1], q[3]).unwrap(); // distance 2
    let circuit = b.finish().unwrap();

    let target = make_line_target("line_4q", 4);
    let validator = TargetLegalityValidator::new(target.clone());
    let equiv_checker = LayoutEquivalenceChecker::default();

    // Baseline Router
    let baseline_router = BaselineRouter::new();
    let (base_routed, base_report) = baseline_router
        .route_circuit(&circuit, &target, None)
        .unwrap();

    // SABRE Bidirectional Router (3 refinement rounds)
    let sabre_config = SabreRouterConfig {
        refinement_rounds: 3,
        max_iterations: 500,
        random_seed: Some(42),
        heuristic_config: SabreHeuristicConfig {
            lookahead_weight: 0.5,
            decay_delta: 0.2,
        },
        ..Default::default()
    };

    let sabre_router = SabreBidirectionalRouter::with_config(sabre_config);
    let (sabre_routed, sabre_report) = sabre_router.route_circuit(&circuit, &target, None).unwrap();

    // 1. Both produce valid QccIR
    assert!(base_routed.validate().is_ok());
    assert!(sabre_routed.validate().is_ok());

    // 2. Both satisfy target legality
    assert!(validator.validate_circuit(&base_routed).is_ok());
    assert!(validator.validate_circuit(&sabre_routed).is_ok());

    // 3. Both satisfy layout-aware equivalence
    let base_verdict = equiv_checker
        .check_layout_equivalence(&circuit, &base_routed, &base_report.layout)
        .unwrap();
    assert!(base_verdict.is_equivalent);
    assert!(base_verdict.max_amplitude_difference < 1e-12);

    let sabre_verdict = equiv_checker
        .check_layout_equivalence(&circuit, &sabre_routed, &sabre_report.layout)
        .unwrap();
    assert!(sabre_verdict.is_equivalent);
    assert!(sabre_verdict.max_amplitude_difference < 1e-12);

    // 4. Characterize relative quality: SABRE with lookahead & refinement should achieve <= SWAPs
    assert!(
        sabre_report.swaps_inserted <= base_report.swaps_inserted,
        "SABRE SWAP count ({}) must not exceed Baseline SWAP count ({}) on line_4q",
        sabre_report.swaps_inserted,
        base_report.swaps_inserted
    );
}

#[test]
fn test_differential_routing_grid_2x3_topology() {
    // 6-qubit circuit with a 3-qubit clique (q0, q1, q2) and additional cross-grid interactions.
    // In a 2x3 grid (bipartite graph with no triangles), a 3-qubit clique cannot all be
    // mutually adjacent, mathematically necessitating at least one routing SWAP.
    let mut b = CircuitBuilder::new("diff_grid_6q");
    let q = b.add_quantum_register("q", 6).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.cx(q[1], q[2]).unwrap();
    b.cx(q[0], q[2]).unwrap(); // completes triangle on bipartite grid -> requires SWAP
    b.cx(q[3], q[4]).unwrap();
    b.cx(q[4], q[5]).unwrap();
    b.cx(q[0], q[5]).unwrap();
    let circuit = b.finish().unwrap();

    let target = make_grid_target("grid_2x3", 2, 3);
    let validator = TargetLegalityValidator::new(target.clone());
    let equiv_checker = LayoutEquivalenceChecker::default();

    let (base_routed, base_report) = BaselineRouter::new()
        .route_circuit(&circuit, &target, None)
        .unwrap();

    let sabre_config = SabreRouterConfig {
        refinement_rounds: 3,
        max_iterations: 500,
        random_seed: Some(12345),
        heuristic_config: SabreHeuristicConfig {
            lookahead_weight: 0.5,
            decay_delta: 0.2,
        },
        ..Default::default()
    };

    let (sabre_routed, sabre_report) = SabreBidirectionalRouter::with_config(sabre_config)
        .route_circuit(&circuit, &target, None)
        .unwrap();

    assert!(validator.validate_circuit(&base_routed).is_ok());
    assert!(validator.validate_circuit(&sabre_routed).is_ok());

    let base_verdict = equiv_checker
        .check_layout_equivalence(&circuit, &base_routed, &base_report.layout)
        .unwrap();
    assert!(base_verdict.is_equivalent);

    let sabre_verdict = equiv_checker
        .check_layout_equivalence(&circuit, &sabre_routed, &sabre_report.layout)
        .unwrap();
    assert!(sabre_verdict.is_equivalent);

    // Both successfully route the grid topology with SWAPs inserted for triangle
    assert!(base_report.swaps_inserted > 0);
    assert!(sabre_report.swaps_inserted > 0);
}

// ============================================================================
// 2. Preserved Regression Reproduction Fixtures
// ============================================================================

#[test]
fn test_regression_seed_disconnected_graph_failure() {
    // Seed REGRESSION_SEED_DISCONNECTED: Disconnected 4-qubit target
    let _seed = REGRESSION_SEED_DISCONNECTED;
    let mut b = CircuitBuilder::new("disc_regr");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.cx(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    // Target with two disjoint components: (0-1) and (2-3)
    let target = make_diff_target("disc_regr_target", 4, [(0, 1), (2, 3)], false);

    // Initial layout explicitly mapping q0 into component 1 (p0) and q1 into component 2 (p2)
    let layout = LayoutMap::from_mappings(
        2,
        4,
        vec![
            (QubitId(0), PhysicalQubitId(0)),
            (QubitId(1), PhysicalQubitId(2)),
        ],
    )
    .unwrap();

    // Both routers must reliably return DisconnectedTopology
    let base_err = BaselineRouter::new()
        .route_circuit(&circuit, &target, Some(layout.clone()))
        .unwrap_err();
    assert!(
        matches!(base_err, RoutingError::DisconnectedTopology { .. }),
        "Expected DisconnectedTopology error from BaselineRouter, got: {base_err:?}"
    );

    let sabre_err = SabreBidirectionalRouter::new()
        .route_circuit(&circuit, &target, Some(layout))
        .unwrap_err();
    assert!(
        matches!(sabre_err, RoutingError::DisconnectedTopology { .. }),
        "Expected DisconnectedTopology error from SabreRouter, got: {sabre_err:?}"
    );
}

#[test]
fn test_regression_seed_star_topology_hub_contention() {
    // Seed REGRESSION_SEED_STAR_CONTENTION: 5-qubit star topology where hub 0 is heavily contended
    let target = make_star_target("star_5q", 5);
    let validator = TargetLegalityValidator::new(target.clone());
    let equiv_checker = LayoutEquivalenceChecker::default();

    let mut b = CircuitBuilder::new("star_contention");
    let q = b.add_quantum_register("q", 5).unwrap();
    // Inter-leaf interactions: (1, 2), (2, 3), (3, 4), (4, 1)
    b.cx(q[1], q[2]).unwrap();
    b.cx(q[2], q[3]).unwrap();
    b.cx(q[3], q[4]).unwrap();
    b.cx(q[4], q[1]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    // Baseline router
    let (base_routed, base_report) = BaselineRouter::new()
        .route_circuit(&circuit, &target, None)
        .unwrap();
    assert!(validator.validate_circuit(&base_routed).is_ok());
    assert!(
        equiv_checker
            .check_layout_equivalence(&circuit, &base_routed, &base_report.layout)
            .unwrap()
            .is_equivalent
    );

    // SABRE router with decay to penalize frequent hub swaps
    let sabre_cfg = SabreRouterConfig {
        refinement_rounds: 3,
        max_iterations: 500,
        random_seed: Some(REGRESSION_SEED_STAR_CONTENTION),
        heuristic_config: SabreHeuristicConfig {
            lookahead_weight: 0.5,
            decay_delta: 0.3,
        },
        ..Default::default()
    };

    let (sabre_routed, sabre_report) = SabreBidirectionalRouter::with_config(sabre_cfg)
        .route_circuit(&circuit, &target, None)
        .unwrap();
    assert!(validator.validate_circuit(&sabre_routed).is_ok());
    assert!(
        equiv_checker
            .check_layout_equivalence(&circuit, &sabre_routed, &sabre_report.layout)
            .unwrap()
            .is_equivalent
    );
}

#[test]
fn test_regression_seed_asymmetric_initial_layout() {
    // Seed REGRESSION_SEED_ASYMMETRIC_LAYOUT: 4-cycle permutation L != L^-1
    // Initial mapping: q0->p1, q1->p2, q2->p3, q3->p0
    let target = make_line_target("line_4q", 4);
    let equiv_checker = LayoutEquivalenceChecker::default();
    let validator = TargetLegalityValidator::new(target.clone());

    let forward_map = vec![
        (QubitId(0), PhysicalQubitId(1)),
        (QubitId(1), PhysicalQubitId(2)),
        (QubitId(2), PhysicalQubitId(3)),
        (QubitId(3), PhysicalQubitId(0)),
    ];
    let initial_layout = LayoutMap::from_mappings(4, 4, forward_map).unwrap();

    let mut b = CircuitBuilder::new("asym_circ");
    let q = b.add_quantum_register("q", 4).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[1]).unwrap(); // initially at (p1, p2) -> adjacent on line!
    b.cx(q[2], q[3]).unwrap(); // initially at (p3, p0) -> distance 3 on line!
    let circuit = b.finish().unwrap();

    // Verify Baseline with asymmetric initial layout
    let (base_routed, base_report) = BaselineRouter::new()
        .route_circuit(&circuit, &target, Some(initial_layout.clone()))
        .unwrap();
    assert_eq!(*base_report.layout.initial(), initial_layout);
    assert!(validator.validate_circuit(&base_routed).is_ok());
    assert!(
        equiv_checker
            .check_layout_equivalence(&circuit, &base_routed, &base_report.layout)
            .unwrap()
            .is_equivalent
    );

    // Verify SABRE with asymmetric initial layout
    let sabre_cfg = SabreRouterConfig {
        refinement_rounds: 2,
        max_iterations: 300,
        random_seed: Some(REGRESSION_SEED_ASYMMETRIC_LAYOUT),
        heuristic_config: SabreHeuristicConfig {
            lookahead_weight: 0.5,
            decay_delta: 0.2,
        },
        ..Default::default()
    };

    let (sabre_routed, sabre_report) = SabreBidirectionalRouter::with_config(sabre_cfg)
        .route_circuit(&circuit, &target, Some(initial_layout.clone()))
        .unwrap();
    assert_eq!(*sabre_report.layout.initial(), initial_layout);
    assert!(validator.validate_circuit(&sabre_routed).is_ok());
    assert!(
        equiv_checker
            .check_layout_equivalence(&circuit, &sabre_routed, &sabre_report.layout)
            .unwrap()
            .is_equivalent
    );
}

#[test]
fn test_regression_seed_directed_line_coupling() {
    // Seed REGRESSION_SEED_DIRECTED_LINE: Strictly directed 3-qubit line: 0 -> 1 -> 2
    let target = make_diff_target("dir_line_3q", 3, [(0, 1), (1, 2)], true); // directed_cx = true
    let validator = TargetLegalityValidator::new(target.clone());
    let equiv_checker = LayoutEquivalenceChecker::default();

    // Circuit has CX in reverse orientation: CX(q1, q0) and CX(q2, q1)
    let mut b = CircuitBuilder::new("dir_reversal");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[1], q[0]).unwrap(); // needs orientation swap or reversal
    b.cx(q[2], q[1]).unwrap(); // needs orientation swap or reversal
    let circuit = b.finish().unwrap();

    let (base_routed, base_report) = BaselineRouter::new()
        .route_circuit(&circuit, &target, None)
        .unwrap();
    assert!(
        validator.validate_circuit(&base_routed).is_ok(),
        "Baseline output must satisfy directed constraints"
    );
    assert!(
        equiv_checker
            .check_layout_equivalence(&circuit, &base_routed, &base_report.layout)
            .unwrap()
            .is_equivalent
    );

    let sabre_cfg = SabreRouterConfig {
        refinement_rounds: 2,
        max_iterations: 300,
        random_seed: Some(REGRESSION_SEED_DIRECTED_LINE),
        heuristic_config: SabreHeuristicConfig {
            lookahead_weight: 0.5,
            decay_delta: 0.2,
        },
        ..Default::default()
    };

    let (sabre_routed, sabre_report) = SabreBidirectionalRouter::with_config(sabre_cfg)
        .route_circuit(&circuit, &target, None)
        .unwrap();
    assert!(
        validator.validate_circuit(&sabre_routed).is_ok(),
        "SABRE output must satisfy directed constraints"
    );
    assert!(
        equiv_checker
            .check_layout_equivalence(&circuit, &sabre_routed, &sabre_report.layout)
            .unwrap()
            .is_equivalent
    );
}

#[test]
fn test_regression_seed_mixed_rotations_and_global_phase() {
    // Seed REGRESSION_SEED_MIXED_ROTATIONS: Circuit with continuous rotations and global phase
    let target = make_line_target("line_3q", 3);
    let validator = TargetLegalityValidator::new(target.clone());
    let equiv_checker = LayoutEquivalenceChecker::default();

    let mut b = CircuitBuilder::new("rot_phase_circ");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.set_global_phase(Angle::from_radians(0.785));
    b.rx(q[0], Angle::from_radians(-0.5)).unwrap();
    b.ry(q[1], Angle::from_radians(1.2)).unwrap();
    b.rz(q[2], Angle::from_radians(-2.1)).unwrap();
    b.cx(q[0], q[2]).unwrap();
    b.rz(q[1], Angle::from_radians(0.3)).unwrap();
    b.cx(q[1], q[2]).unwrap();
    let circuit = b.finish().unwrap();

    let (base_routed, base_report) = BaselineRouter::new()
        .route_circuit(&circuit, &target, None)
        .unwrap();
    assert!(validator.validate_circuit(&base_routed).is_ok());
    assert!(
        equiv_checker
            .check_layout_equivalence(&circuit, &base_routed, &base_report.layout)
            .unwrap()
            .is_equivalent
    );

    let sabre_cfg = SabreRouterConfig {
        refinement_rounds: 2,
        max_iterations: 300,
        random_seed: Some(REGRESSION_SEED_MIXED_ROTATIONS),
        heuristic_config: SabreHeuristicConfig {
            lookahead_weight: 0.5,
            decay_delta: 0.2,
        },
        ..Default::default()
    };

    let (sabre_routed, sabre_report) = SabreBidirectionalRouter::with_config(sabre_cfg)
        .route_circuit(&circuit, &target, None)
        .unwrap();
    assert!(validator.validate_circuit(&sabre_routed).is_ok());
    assert!(
        equiv_checker
            .check_layout_equivalence(&circuit, &sabre_routed, &sabre_report.layout)
            .unwrap()
            .is_equivalent
    );
}

// ============================================================================
// 3. Non-Unitary Routing & Refinement Policy
// ============================================================================

#[test]
fn test_non_unitary_circuits_routing_behavior() {
    let target = make_line_target("line_3q", 3);

    // Circuit containing terminal measurement and reset
    let mut b = CircuitBuilder::new("non_unitary");
    let q = b.add_quantum_register("q", 2).unwrap();
    let c = b.add_classical_register("c", 2).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    b.measure(q[0], c[0]).unwrap();
    b.reset(q[1]).unwrap();
    let circuit = b.finish().unwrap();

    // 1. BaselineRouter forwards non-unitary circuits cleanly
    let base_cfg = qcc::routing::greedy::BaselineRouterConfig {
        verify_equivalence: false, // non-unitary circuits cannot be checked with unitary statevector simulation
        ..Default::default()
    };
    let (base_routed, base_report) = BaselineRouter::with_config(base_cfg)
        .route_circuit(&circuit, &target, None)
        .unwrap();
    assert_eq!(base_report.original_operations, 4);
    assert_eq!(base_routed.num_operations(), 4);

    // 2. SABRE with refinement_rounds: 1 (forward only) succeeds cleanly
    let sabre_cfg = SabreRouterConfig {
        refinement_rounds: 1,
        verify_equivalence: false, // non-unitary circuits cannot be checked with unitary statevector simulation
        max_iterations: 200,
        random_seed: Some(42),
        heuristic_config: SabreHeuristicConfig {
            lookahead_weight: 0.5,
            decay_delta: 0.2,
        },
        ..Default::default()
    };

    let (sabre_routed, sabre_report) = SabreBidirectionalRouter::with_config(sabre_cfg)
        .route_circuit(&circuit, &target, None)
        .unwrap();
    assert_eq!(sabre_report.original_operations, 4);
    assert_eq!(sabre_routed.num_operations(), 4);

    // 3. Multi-round SABRE (refinement_rounds > 1) rejects non-unitary operations explicitly
    let multi_cfg = SabreRouterConfig {
        refinement_rounds: 2,
        verify_equivalence: false,
        max_iterations: 200,
        random_seed: Some(42),
        heuristic_config: SabreHeuristicConfig {
            lookahead_weight: 0.5,
            decay_delta: 0.2,
        },
        ..Default::default()
    };

    let multi_round_err = SabreBidirectionalRouter::with_config(multi_cfg)
        .route_circuit(&circuit, &target, None)
        .unwrap_err();

    assert!(
        matches!(multi_round_err, RoutingError::NonUnitaryReverseTraversal(_)),
        "Multi-round SABRE must explicitly reject non-unitary reverse traversal, got: {multi_round_err:?}"
    );
}

// ============================================================================
// 4. ADR-0001 Clean Ancilla Zero-State Preservation
// ============================================================================

#[test]
fn test_adr0001_clean_ancilla_preservation_under_both_routers() {
    // 2 logical qubits placed on a 5-qubit target device (3 clean ancillae)
    let target = make_line_target("line_5q", 5);
    let equiv_checker = LayoutEquivalenceChecker::default();

    let mut b = CircuitBuilder::new("bell_2q_5p");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    b.cx(q[0], q[1]).unwrap();
    let circuit = b.finish().unwrap();

    // Baseline route
    let (base_routed, base_report) = BaselineRouter::new()
        .route_circuit(&circuit, &target, None)
        .unwrap();
    let base_verdict = equiv_checker
        .check_layout_equivalence(&circuit, &base_routed, &base_report.layout)
        .unwrap();
    assert!(
        base_verdict.is_equivalent,
        "Baseline must preserve clean ancillae under ADR-0001"
    );
    assert!(base_verdict.max_amplitude_difference < 1e-12);

    // SABRE route
    let sabre_cfg = SabreRouterConfig {
        refinement_rounds: 2,
        max_iterations: 300,
        random_seed: Some(42),
        heuristic_config: SabreHeuristicConfig {
            lookahead_weight: 0.5,
            decay_delta: 0.2,
        },
        ..Default::default()
    };

    let (sabre_routed, sabre_report) = SabreBidirectionalRouter::with_config(sabre_cfg)
        .route_circuit(&circuit, &target, None)
        .unwrap();
    let sabre_verdict = equiv_checker
        .check_layout_equivalence(&circuit, &sabre_routed, &sabre_report.layout)
        .unwrap();
    assert!(
        sabre_verdict.is_equivalent,
        "SABRE must preserve clean ancillae under ADR-0001"
    );
    assert!(sabre_verdict.max_amplitude_difference < 1e-12);
}
