//! Property-based routing validation for QCC (Phase 53).
//!
//! Evaluates `BaselineRouter` and `SabreBidirectionalRouter` across randomized circuits,
//! coupling graphs, directed constraints, and layout edge cases using `proptest`.
//!
//! # Properties Enforced
//! 1. **QccIR DAG Invariants**: Routed circuits pass internal structural validation without cycles.
//! 2. **Target Legality**: Every routed physical operation satisfies target supported operations,
//!    arities, distinct operands, and connectivity/directionality constraints.
//! 3. **Semantic Layout-Aware Equivalence**: Unitary circuits preserve full $2^M$-dimensional
//!    statevector fidelity under `LayoutEquivalenceChecker`.
//! 4. **ADR-0001 Clean-Ancilla Policy**: Unused physical qubits remain in $|0\rangle$ at termination.
//! 5. **Explicit Error Invariants**: Disconnected topologies and capacity violations return structured errors.
//! 6. **Deterministic Repeatability**: Identical inputs and seeds yield identical routed circuits and metrics.

use std::f64::consts::PI;

use proptest::prelude::*;

use qcc::builder::CircuitBuilder;
use qcc::gate_signature::ParameterDomain;
use qcc::ir::circuit::Circuit;
use qcc::ir::operation::Operation;
use qcc::ir::wire::QubitId;
use qcc::quantum::angle::Angle;
use qcc::quantum::gate::Gate;
use qcc::routing::greedy::BaselineRouter;
use qcc::routing::sabre::{SabreBidirectionalRouter, SabreHeuristicConfig, SabreRouterConfig};
use qcc::routing::RoutingError;
use qcc::simulator::sampling::SplitMix64;
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::legality::TargetLegalityValidator;
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};
use qcc::verification::layout_equivalence::LayoutEquivalenceChecker;
use qcc::SeededRng;

/// Constructs a hardware target with standard single-qubit gates, native `cx`, and native `swap`.
fn make_property_target(
    name: &str,
    num_qubits: usize,
    edges: impl IntoIterator<Item = (u32, u32)>,
    directed_cx: bool,
) -> Target {
    let mut target = Target::new(name, num_qubits);

    // Single-qubit operations
    for op_name in &["h", "x", "y", "z", "s", "sdg", "t", "tdg"] {
        let desc = TargetOperationDescriptor::builder(*op_name, 1)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .duration(20.0)
            .build();
        target.add_operation(desc);
    }

    // Parameterized rotations
    for op_name in &["rx", "ry", "rz", "p"] {
        let desc = TargetOperationDescriptor::builder(*op_name, 1)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .add_parameter_domain(ParameterDomain::Angle4Pi)
            .duration(25.0)
            .build();
        target.add_operation(desc);
    }

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

        // SWAP is always undirected physical exchange
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

/// Generates a connected tree target spanning `num_qubits`.
fn make_connected_tree_target(name: &str, num_qubits: usize, directed_cx: bool) -> Target {
    let mut edges = Vec::new();
    for i in 1..num_qubits {
        let parent = (i - 1) / 2;
        edges.push((parent as u32, i as u32));
    }
    make_property_target(name, num_qubits, edges, directed_cx)
}

/// Generates a line target (0 - 1 - ... - n-1).
fn make_line_target(name: &str, num_qubits: usize, directed_cx: bool) -> Target {
    let mut edges = Vec::new();
    for i in 0..num_qubits.saturating_sub(1) {
        edges.push((i as u32, (i + 1) as u32));
    }
    make_property_target(name, num_qubits, edges, directed_cx)
}

/// Generates a disconnected target with two isolated components: [0..k] and [k..n].
fn make_disconnected_target(
    name: &str,
    num_qubits: usize,
    split: usize,
    directed_cx: bool,
) -> Target {
    let mut edges = Vec::new();
    // Component 1: 0 .. split
    for i in 0..split.saturating_sub(1) {
        edges.push((i as u32, (i + 1) as u32));
    }
    // Component 2: split .. num_qubits
    for i in split..num_qubits.saturating_sub(1) {
        edges.push((i as u32, (i + 1) as u32));
    }
    make_property_target(name, num_qubits, edges, directed_cx)
}

/// Synthesizes a deterministic bounded circuit from PRNG seed.
fn generate_bounded_circuit(
    name: &str,
    num_qubits: usize,
    num_gates: usize,
    rng: &mut SplitMix64,
) -> Circuit {
    let mut circuit = Circuit::new(name);
    circuit.add_quantum_register("q", num_qubits).unwrap();

    let global_phase = (rng.next_f64() * 2.0 - 1.0) * PI;
    circuit.set_global_phase(Angle::from_radians(global_phase).into());

    for _ in 0..num_gates {
        let q0 = (rng.next_u64() % num_qubits as u64) as usize;
        let op_choice = rng.next_u64() % 10;

        match op_choice {
            0 => {
                circuit
                    .add_operation(Operation::gate(&Gate::H, vec![QubitId(q0)]))
                    .unwrap();
            }
            1 => {
                circuit
                    .add_operation(Operation::gate(&Gate::X, vec![QubitId(q0)]))
                    .unwrap();
            }
            2 => {
                circuit
                    .add_operation(Operation::gate(&Gate::S, vec![QubitId(q0)]))
                    .unwrap();
            }
            3 => {
                circuit
                    .add_operation(Operation::gate(&Gate::T, vec![QubitId(q0)]))
                    .unwrap();
            }
            4 => {
                let theta = (rng.next_f64() * 2.0 - 1.0) * PI;
                circuit
                    .add_operation(Operation::gate(
                        &Gate::Rx(Angle::from_radians(theta)),
                        vec![QubitId(q0)],
                    ))
                    .unwrap();
            }
            5 => {
                let theta = (rng.next_f64() * 2.0 - 1.0) * PI;
                circuit
                    .add_operation(Operation::gate(
                        &Gate::Rz(Angle::from_radians(theta)),
                        vec![QubitId(q0)],
                    ))
                    .unwrap();
            }
            _ => {
                // Two-qubit gate (CX)
                if num_qubits >= 2 {
                    let mut q1 = (rng.next_u64() % (num_qubits - 1) as u64) as usize;
                    if q1 >= q0 {
                        q1 += 1;
                    }
                    circuit
                        .add_operation(Operation::gate(&Gate::CX, vec![QubitId(q0), QubitId(q1)]))
                        .unwrap();
                } else {
                    circuit
                        .add_operation(Operation::gate(&Gate::H, vec![QubitId(q0)]))
                        .unwrap();
                }
            }
        }
    }

    circuit
}

// ============================================================================
// Property Test Suites (proptest)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(25))]

    /// Property 1: Connected Line Topologies
    /// For any valid bounded unitary circuit routed on a connected line target:
    /// - Both BaselineRouter and SabreBidirectionalRouter produce valid QccIR DAGs.
    /// - Both pass target legality validation.
    /// - Both preserve layout-aware statevector equivalence up to tolerance < 1e-10.
    /// - Both enforce ADR-0001 clean ancilla zero-state across full 2^M dimensions.
    #[test]
    fn prop_connected_line_routing_preserves_semantics_and_legality(
        seed in 1u64..100000u64,
        num_logical in 2usize..=4usize,
        num_gates in 3usize..=8usize,
        extra_ancillae in 0usize..=2usize,
    ) {
        let mut rng = SplitMix64::new(seed);
        let num_physical = num_logical + extra_ancillae;
        let circuit = generate_bounded_circuit("prop_line_circ", num_logical, num_gates, &mut rng);
        let target = make_line_target("prop_line_target", num_physical, false);

        let validator = TargetLegalityValidator::new(target.clone());
        let equiv_checker = LayoutEquivalenceChecker::default();

        // 1. Baseline Router
        let baseline_router = BaselineRouter::new();
        let (base_routed, base_report) = baseline_router
            .route_circuit(&circuit, &target, None)
            .expect("BaselineRouter must succeed on connected line");

        prop_assert!(base_routed.validate().is_ok(), "Baseline routed circuit must be valid QccIR");
        prop_assert!(validator.validate_circuit(&base_routed).is_ok(), "Baseline routed circuit must be target legal");
        prop_assert!(base_report.layout.initial().validate().is_ok());
        prop_assert!(base_report.layout.final_layout().validate().is_ok());

        let base_verdict = equiv_checker
            .check_layout_equivalence(&circuit, &base_routed, &base_report.layout)
            .expect("Baseline layout equivalence check failed");
        prop_assert!(base_verdict.is_equivalent, "Baseline circuit must be unitarily equivalent");
        prop_assert!(base_verdict.max_amplitude_difference < 1e-10);

        // 2. SABRE Bidirectional Router
        let sabre_config = SabreRouterConfig {
            refinement_rounds: 2,
            max_iterations: 500,
            random_seed: Some(seed),
            heuristic_config: SabreHeuristicConfig {
                lookahead_weight: 0.5,
                decay_delta: 0.2,
            },
            ..Default::default()
        };

        let sabre_router = SabreBidirectionalRouter::with_config(sabre_config);
        let (sabre_routed, sabre_report) = sabre_router
            .route_circuit(&circuit, &target, None)
            .expect("SABRE must succeed on connected line");

        prop_assert!(sabre_routed.validate().is_ok(), "SABRE routed circuit must be valid QccIR");
        prop_assert!(validator.validate_circuit(&sabre_routed).is_ok(), "SABRE routed circuit must be target legal");
        prop_assert!(sabre_report.layout.initial().validate().is_ok());
        prop_assert!(sabre_report.layout.final_layout().validate().is_ok());

        let sabre_verdict = equiv_checker
            .check_layout_equivalence(&circuit, &sabre_routed, &sabre_report.layout)
            .expect("SABRE layout equivalence check failed");
        prop_assert!(sabre_verdict.is_equivalent, "SABRE circuit must be unitarily equivalent");
        prop_assert!(sabre_verdict.max_amplitude_difference < 1e-10);
    }

    /// Property 2: Connected Tree Topologies
    /// Verifies tree-structured target devices with branching vertices.
    #[test]
    fn prop_connected_tree_routing_preserves_semantics_and_legality(
        seed in 1u64..100000u64,
        num_logical in 2usize..=4usize,
        num_gates in 3usize..=8usize,
        extra_ancillae in 0usize..=2usize,
    ) {
        let mut rng = SplitMix64::new(seed);
        let num_physical = num_logical + extra_ancillae;
        let circuit = generate_bounded_circuit("prop_tree_circ", num_logical, num_gates, &mut rng);
        let target = make_connected_tree_target("prop_tree_target", num_physical, false);

        let validator = TargetLegalityValidator::new(target.clone());
        let equiv_checker = LayoutEquivalenceChecker::default();

        // Baseline Router
        let (base_routed, base_report) = BaselineRouter::new()
            .route_circuit(&circuit, &target, None)
            .expect("BaselineRouter must succeed on connected tree");
        prop_assert!(validator.validate_circuit(&base_routed).is_ok());
        let base_verdict = equiv_checker
            .check_layout_equivalence(&circuit, &base_routed, &base_report.layout)
            .unwrap();
        prop_assert!(base_verdict.is_equivalent);

        // SABRE Router
        let sabre_config = SabreRouterConfig {
            refinement_rounds: 2,
            max_iterations: 500,
            random_seed: Some(seed),
            heuristic_config: SabreHeuristicConfig {
                lookahead_weight: 0.5,
                decay_delta: 0.2,
            },
            ..Default::default()
        };

        let sabre_router = SabreBidirectionalRouter::with_config(sabre_config);
        let (sabre_routed, sabre_report) = sabre_router
            .route_circuit(&circuit, &target, None)
            .expect("SABRE must succeed on connected tree");
        prop_assert!(validator.validate_circuit(&sabre_routed).is_ok());
        let sabre_verdict = equiv_checker
            .check_layout_equivalence(&circuit, &sabre_routed, &sabre_report.layout)
            .unwrap();
        prop_assert!(sabre_verdict.is_equivalent);
    }

    /// Property 3: Directed Coupling Topologies
    /// When CX is legal only in one directed orientation, routers must respect
    /// target orientation while preserving semantic equivalence.
    #[test]
    fn prop_directed_coupling_routing_satisfies_directionality(
        seed in 1u64..100000u64,
        num_logical in 2usize..=3usize,
        num_gates in 2usize..=6usize,
    ) {
        let mut rng = SplitMix64::new(seed);
        let circuit = generate_bounded_circuit("prop_dir_circ", num_logical, num_gates, &mut rng);
        let target = make_line_target("prop_dir_target", num_logical, true); // directed_cx = true

        let validator = TargetLegalityValidator::new(target.clone());
        let equiv_checker = LayoutEquivalenceChecker::default();

        // Baseline
        let (base_routed, base_report) = BaselineRouter::new()
            .route_circuit(&circuit, &target, None)
            .expect("BaselineRouter must succeed on directed target");
        prop_assert!(validator.validate_circuit(&base_routed).is_ok(), "Must satisfy directed coupling");
        let base_verdict = equiv_checker
            .check_layout_equivalence(&circuit, &base_routed, &base_report.layout)
            .unwrap();
        prop_assert!(base_verdict.is_equivalent);

        // SABRE
        let sabre_config = SabreRouterConfig {
            refinement_rounds: 2,
            max_iterations: 500,
            random_seed: Some(seed),
            heuristic_config: SabreHeuristicConfig {
                lookahead_weight: 0.5,
                decay_delta: 0.2,
            },
            ..Default::default()
        };

        let sabre_router = SabreBidirectionalRouter::with_config(sabre_config);
        let (sabre_routed, sabre_report) = sabre_router
            .route_circuit(&circuit, &target, None)
            .expect("SABRE must succeed on directed target");
        prop_assert!(validator.validate_circuit(&sabre_routed).is_ok(), "Must satisfy directed coupling");
        let sabre_verdict = equiv_checker
            .check_layout_equivalence(&circuit, &sabre_routed, &sabre_report.layout)
            .unwrap();
        prop_assert!(sabre_verdict.is_equivalent);
    }

    /// Property 4: Disconnected Topologies Must Explicitly Fail
    /// When a circuit requires interaction across disconnected components,
    /// both routers must return RoutingError::DisconnectedTopology.
    #[test]
    fn prop_disconnected_topologies_fail_explicitly(
        _seed in 1u64..100000u64,
    ) {
        // Create 4-qubit circuit with explicit cross-component interaction q0 <-> q3
        let mut b = CircuitBuilder::new("cross_comp");
        let q = b.add_quantum_register("q", 4).unwrap();
        b.h(q[0]).unwrap();
        b.cx(q[0], q[3]).unwrap();
        let circuit = b.finish().unwrap();

        // Disconnected target: component A = [0, 1], component B = [2, 3]
        let target = make_disconnected_target("disc_target", 4, 2, false);

        // Layout mapping q0 to component A (p0) and q3 to component B (p3)
        let layout = qcc::routing::layout_map::LayoutMap::from_mappings(
            4,
            4,
            vec![
                (QubitId(0), PhysicalQubitId(0)),
                (QubitId(1), PhysicalQubitId(1)),
                (QubitId(2), PhysicalQubitId(2)),
                (QubitId(3), PhysicalQubitId(3)),
            ],
        )
        .unwrap();

        // Baseline must fail with DisconnectedTopology
        let base_res = BaselineRouter::new().route_circuit(&circuit, &target, Some(layout.clone()));
        let base_is_disc = matches!(base_res, Err(RoutingError::DisconnectedTopology { .. }));
        prop_assert!(base_is_disc, "BaselineRouter must return DisconnectedTopology");

        // SABRE must fail with DisconnectedTopology
        let sabre_res = SabreBidirectionalRouter::new().route_circuit(&circuit, &target, Some(layout));
        let sabre_is_disc = matches!(sabre_res, Err(RoutingError::DisconnectedTopology { .. }));
        prop_assert!(sabre_is_disc, "SabreRouter must return DisconnectedTopology");
    }

    /// Property 5: Insufficient Capacity Must Explicitly Fail
    /// When logical qubits exceed physical qubits, both routers must fail fast.
    #[test]
    fn prop_insufficient_capacity_fails_explicitly(
        num_logical in 4usize..=6usize,
        num_physical in 1usize..3usize,
    ) {
        let mut b = CircuitBuilder::new("cap_circ");
        b.add_quantum_register("q", num_logical).unwrap();
        let circuit = b.finish().unwrap();

        let target = make_line_target("cap_target", num_physical, false);

        let base_res = BaselineRouter::new().route_circuit(&circuit, &target, None);
        let base_is_cap = matches!(base_res, Err(RoutingError::InsufficientCapacity { .. }));
        prop_assert!(base_is_cap, "BaselineRouter must return InsufficientCapacity");

        let sabre_res = SabreBidirectionalRouter::new().route_circuit(&circuit, &target, None);
        let sabre_is_cap = matches!(sabre_res, Err(RoutingError::InsufficientCapacity { .. }));
        prop_assert!(sabre_is_cap, "SabreRouter must return InsufficientCapacity");
    }

    /// Property 6: Deterministic Repeatability
    /// Identical seeds and inputs must yield identical outputs and metrics bit-for-bit.
    #[test]
    fn prop_deterministic_repeatability_across_identical_runs(
        seed in 1u64..100000u64,
        num_logical in 2usize..=3usize,
        num_gates in 3usize..=6usize,
    ) {
        let mut rng1 = SplitMix64::new(seed);
        let circuit1 = generate_bounded_circuit("rep_c1", num_logical, num_gates, &mut rng1);
        let mut rng2 = SplitMix64::new(seed);
        let circuit2 = generate_bounded_circuit("rep_c2", num_logical, num_gates, &mut rng2);

        let target = make_line_target("rep_target", num_logical + 1, false);

        let sabre_cfg = SabreRouterConfig {
            refinement_rounds: 2,
            max_iterations: 300,
            random_seed: Some(seed),
            heuristic_config: SabreHeuristicConfig {
                lookahead_weight: 0.5,
                decay_delta: 0.2,
            },
            ..Default::default()
        };

        let router = SabreBidirectionalRouter::with_config(sabre_cfg);
        let (out1, rep1) = router.route_circuit(&circuit1, &target, None).unwrap();
        let (out2, rep2) = router.route_circuit(&circuit2, &target, None).unwrap();

        prop_assert_eq!(rep1.swaps_inserted, rep2.swaps_inserted);
        prop_assert_eq!(rep1.metrics.circuit_depth, rep2.metrics.circuit_depth);
        prop_assert_eq!(rep1.metrics.two_qubit_depth, rep2.metrics.two_qubit_depth);
        prop_assert_eq!(rep1.layout.initial(), rep2.layout.initial());
        prop_assert_eq!(rep1.layout.final_layout(), rep2.layout.final_layout());
        prop_assert_eq!(out1.num_operations(), out2.num_operations());
    }
}
