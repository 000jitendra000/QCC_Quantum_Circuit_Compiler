//! Integration tests for Phase 48: SWAP Insertion and Dependency Maintenance.
//!
//! Validates:
//! 1. Atomic insertion of physical SWAP nodes touching both physical wires.
//! 2. Accurate dependency ordering across both affected wires in QccIR DAGs.
//! 3. Synchronous logical-to-physical layout mutation tracking ($L_{\text{initial}} \to L_{\text{final}}$).
//! 4. Preservation of original logical operation identity and traceable provenance.
//! 5. Routed Bell circuit verification under ADR-0001 strict ancilla zero-state policies.
//! 6. Incremental QccIR graph validation after every rewrite step.
//! 7. Regressions: inconsistent layout updates fail equivalence; disconnected swaps are rejected.

use std::collections::BTreeSet;

use qcc::builder::CircuitBuilder;
use qcc::ir::node::OperationId;
use qcc::ir::operation::Operation;
use qcc::ir::validation::validate_circuit;
use qcc::ir::wire::{QubitId, WireId};
use qcc::routing::layout_map::LayoutMap;
use qcc::routing::pathfinding::RoutingError;
use qcc::routing::provenance::RouteOperationKind;
use qcc::routing::route_state::RouteState;
use qcc::target::connectivity::OperationConnectivity;
use qcc::target::model::Target;
use qcc::target::operation::{PhysicalQubitId, TargetOperationDescriptor};

/// Helper constructing a target device with native `swap`, `cx`, and single-qubit gates.
fn make_line_target(name: &str, num_qubits: usize) -> Target {
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
    for i in 0..(num_qubits.saturating_sub(1)) {
        edges.push((
            PhysicalQubitId::new(i as u32),
            PhysicalQubitId::new((i + 1) as u32),
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
// 1. Single and Repeated SWAP Insertion Tests
// ============================================================================

#[test]
fn test_single_inserted_swap_updates_graph_and_layout() {
    let target = make_line_target("line_3q", 3);
    let initial_layout = LayoutMap::identity(3, 3).unwrap();

    let mut b = CircuitBuilder::new("empty_logical");
    let _q = b.add_quantum_register("q", 3).unwrap();
    let logical = b.finish().unwrap();

    let mut state = RouteState::from_logical(&logical, target, initial_layout).unwrap();

    // Insert SWAP between p0 and p1
    let p0 = PhysicalQubitId(0);
    let p1 = PhysicalQubitId(1);
    let res = state.insert_swap(p0, p1, None).unwrap();

    assert_eq!(res.u, p0);
    assert_eq!(res.v, p1);
    assert_eq!(res.inserted_node_ids.len(), 1);

    // Graph checks: 1 node in circuit touching both p0 and p1
    let circuit = state.circuit();
    assert_eq!(circuit.num_operations(), 1);
    let node = circuit.graph.get_node(res.inserted_node_ids[0]).unwrap();
    match &node.operation {
        Operation::Gate {
            gate_name, qubits, ..
        } => {
            assert_eq!(gate_name, "swap");
            assert_eq!(qubits, &[QubitId(0), QubitId(1)]);
        }
        other => panic!("expected SWAP gate, got {other:?}"),
    }

    // Layout checks: q0 is now at p1, q1 is at p0, q2 at p2
    assert_eq!(state.current_layout().get_physical(QubitId(0)), Some(p1));
    assert_eq!(state.current_layout().get_physical(QubitId(1)), Some(p0));
    assert_eq!(
        state.current_layout().get_physical(QubitId(2)),
        Some(PhysicalQubitId(2))
    );

    // Reverse lookup consistency
    assert_eq!(state.current_layout().get_logical(p0), Some(QubitId(1)));
    assert_eq!(state.current_layout().get_logical(p1), Some(QubitId(0)));

    // Graph validation
    state.validate_all().unwrap();
}

#[test]
fn test_repeated_swaps_on_same_wires_restore_layout() {
    let target = make_line_target("line_3q", 3);
    let initial_layout = LayoutMap::identity(3, 3).unwrap();

    let mut b = CircuitBuilder::new("empty_logical");
    let _q = b.add_quantum_register("q", 3).unwrap();
    let logical = b.finish().unwrap();

    let mut state = RouteState::from_logical(&logical, target, initial_layout).unwrap();
    let p0 = PhysicalQubitId(0);
    let p1 = PhysicalQubitId(1);

    // First SWAP(p0, p1)
    let res1 = state.insert_swap(p0, p1, None).unwrap();
    assert_eq!(state.current_layout().get_physical(QubitId(0)), Some(p1));

    // Second SWAP(p0, p1) on same wires
    let res2 = state.insert_swap(p0, p1, None).unwrap();
    // Layout restored to identity!
    assert_eq!(state.current_layout().get_physical(QubitId(0)), Some(p0));
    assert_eq!(state.current_layout().get_physical(QubitId(1)), Some(p1));

    // Dependency check: second SWAP must depend on first SWAP on BOTH wires p0 and p1
    let id1 = res1.inserted_node_ids[0];
    let id2 = res2.inserted_node_ids[0];
    let preds = state.circuit().graph.predecessors(id2);

    let pred_wires: BTreeSet<WireId> = preds
        .into_iter()
        .filter(|(pred_id, _)| *pred_id == id1)
        .map(|(_, dep)| dep.wire)
        .collect();

    assert!(pred_wires.contains(&WireId::Quantum(QubitId(0))));
    assert!(pred_wires.contains(&WireId::Quantum(QubitId(1))));

    state.validate_all().unwrap();
}

#[test]
fn test_multi_wire_repeated_swaps_dependency_chain() {
    let target = make_line_target("line_3q", 3);
    let initial_layout = LayoutMap::identity(3, 3).unwrap();

    let mut b = CircuitBuilder::new("empty_logical");
    let _q = b.add_quantum_register("q", 3).unwrap();
    let logical = b.finish().unwrap();

    let mut state = RouteState::from_logical(&logical, target, initial_layout).unwrap();
    let p0 = PhysicalQubitId(0);
    let p1 = PhysicalQubitId(1);
    let p2 = PhysicalQubitId(2);

    // SWAP(p0, p1), then SWAP(p1, p2)
    let res1 = state.insert_swap(p0, p1, None).unwrap();
    let res2 = state.insert_swap(p1, p2, None).unwrap();

    let id1 = res1.inserted_node_ids[0];
    let id2 = res2.inserted_node_ids[0];

    // res2 touches p1 and p2. On wire p1, it must depend on res1!
    let preds = state.circuit().graph.predecessors(id2);
    let has_dep_on_p1 = preds
        .iter()
        .any(|(p, dep)| *p == id1 && dep.wire == WireId::Quantum(QubitId(1)));
    assert!(
        has_dep_on_p1,
        "SWAP(p1, p2) must depend on SWAP(p0, p1) on wire p1"
    );

    // Layout check:
    // q0 -> p0 -> (swap 0,1) -> p1 -> (swap 1,2) -> p2
    // q1 -> p1 -> (swap 0,1) -> p0 -> p0
    // q2 -> p2 -> p2 -> (swap 1,2) -> p1
    assert_eq!(state.current_layout().get_physical(QubitId(0)), Some(p2));
    assert_eq!(state.current_layout().get_physical(QubitId(1)), Some(p0));
    assert_eq!(state.current_layout().get_physical(QubitId(2)), Some(p1));

    state.validate_all().unwrap();
}

// ============================================================================
// 2. Dependency Ordering on Both Affected Wires Tests
// ============================================================================

#[test]
fn test_swap_dependency_order_on_both_affected_wires() {
    let target = make_line_target("line_3q", 3);
    let initial_layout = LayoutMap::identity(3, 3).unwrap();

    let mut b = CircuitBuilder::new("dep_test");
    let q = b.add_quantum_register("q", 3).unwrap();
    // Pre-SWAP operations: H on q0, X on q1, Z on q2 (disjoint)
    b.h(q[0]).unwrap(); // op#1
    b.x(q[1]).unwrap(); // op#2
    b.z(q[2]).unwrap(); // op#3
    let logical = b.finish().unwrap();

    let mut state = RouteState::from_logical(&logical, target, initial_layout).unwrap();

    // Apply pre-operations
    let h_phys = state
        .apply_logical_operation(
            OperationId(1),
            &Operation::custom_gate("h", vec![QubitId(0)], vec![]),
        )
        .unwrap();
    let x_phys = state
        .apply_logical_operation(
            OperationId(2),
            &Operation::custom_gate("x", vec![QubitId(1)], vec![]),
        )
        .unwrap();
    let z_phys = state
        .apply_logical_operation(
            OperationId(3),
            &Operation::custom_gate("z", vec![QubitId(2)], vec![]),
        )
        .unwrap();

    // Insert SWAP between p0 and p1
    let p0 = PhysicalQubitId(0);
    let p1 = PhysicalQubitId(1);
    let swap_res = state.insert_swap(p0, p1, None).unwrap();
    let swap_id = swap_res.inserted_node_ids[0];

    // Post-SWAP operations: H on physical p0, X on physical p1, Z on physical p2
    let post_h = state
        .circuit_mut()
        .add_operation(Operation::custom_gate("h", vec![QubitId(0)], vec![]))
        .unwrap();
    let post_x = state
        .circuit_mut()
        .add_operation(Operation::custom_gate("x", vec![QubitId(1)], vec![]))
        .unwrap();
    let post_z = state
        .circuit_mut()
        .add_operation(Operation::custom_gate("z", vec![QubitId(2)], vec![]))
        .unwrap();

    let g = &state.circuit().graph;

    // 1. SWAP must depend on h_phys (wire p0) and x_phys (wire p1)
    let swap_preds: Vec<OperationId> = g
        .predecessors(swap_id)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert!(
        swap_preds.contains(&h_phys),
        "SWAP must depend on predecessor on wire p0"
    );
    assert!(
        swap_preds.contains(&x_phys),
        "SWAP must depend on predecessor on wire p1"
    );
    assert!(
        !swap_preds.contains(&z_phys),
        "SWAP must NOT depend on disjoint wire p2"
    );

    // 2. Post-operations on p0 and p1 must depend on SWAP
    let post_h_preds: Vec<OperationId> = g
        .predecessors(post_h)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let post_x_preds: Vec<OperationId> = g
        .predecessors(post_x)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let post_z_preds: Vec<OperationId> = g
        .predecessors(post_z)
        .into_iter()
        .map(|(id, _)| id)
        .collect();

    assert!(
        post_h_preds.contains(&swap_id),
        "Post-gate on p0 must depend on SWAP"
    );
    assert!(
        post_x_preds.contains(&swap_id),
        "Post-gate on p1 must depend on SWAP"
    );
    assert!(
        !post_z_preds.contains(&swap_id),
        "Post-gate on disjoint wire p2 must NOT depend on SWAP"
    );

    state.validate_all().unwrap();
}

// ============================================================================
// 3. Provenance and Original Operation Identity Preservation Tests
// ============================================================================

#[test]
fn test_original_logical_operation_identity_and_provenance_preserved() {
    let target = make_line_target("line_3q", 3);
    let initial_layout = LayoutMap::identity(3, 3).unwrap();

    let mut b = CircuitBuilder::new("prov_test");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[0]).unwrap(); // op#1
    b.cx(q[0], q[2]).unwrap(); // op#2 (distance 2, requires routing SWAP)
    let logical = b.finish().unwrap();

    let mut state = RouteState::from_logical(&logical, target, initial_layout).unwrap();

    // 1. Apply logical H(q0) with provenance
    let h_phys = state
        .apply_logical_operation(
            OperationId(1),
            &Operation::custom_gate("h", vec![QubitId(0)], vec![]),
        )
        .unwrap();

    // Provenance verification for H
    let h_node = state.circuit().graph.get_node(h_phys).unwrap();
    assert_eq!(h_node.provenance, vec![OperationId(1)]);
    assert_eq!(
        state.provenance().get_kind(h_phys),
        Some(&RouteOperationKind::Logical {
            original_id: OperationId(1)
        })
    );

    // 2. Route CX(q0, q2): shortest path [p0, p1, p2]. Needs SWAP(p0, p1)
    let p0 = PhysicalQubitId(0);
    let p1 = PhysicalQubitId(1);
    let swap_res = state.insert_swap(p0, p1, Some(OperationId(2))).unwrap();
    let swap_id = swap_res.inserted_node_ids[0];

    // Provenance verification for SWAP
    let swap_node = state.circuit().graph.get_node(swap_id).unwrap();
    assert_eq!(swap_node.provenance, vec![OperationId(2)]);
    assert_eq!(
        state.provenance().get_kind(swap_id),
        Some(&RouteOperationKind::RoutingSwap {
            caused_by: Some(OperationId(2)),
            step: 0
        })
    );

    // 3. Apply logical CX(q0, q2): mapped through layout (q0 at p1, q2 at p2)
    let cx_phys = state
        .apply_logical_operation(
            OperationId(2),
            &Operation::custom_gate("cx", vec![QubitId(0), QubitId(2)], vec![]),
        )
        .unwrap();

    let cx_node = state.circuit().graph.get_node(cx_phys).unwrap();
    assert_eq!(cx_node.provenance, vec![OperationId(2)]);

    // Query provenance tracker forward and reverse
    let ops_for_op2 = state.provenance().physical_operations_for(OperationId(2));
    assert!(ops_for_op2.contains(&swap_id));
    assert!(ops_for_op2.contains(&cx_phys));

    let swaps_for_op2 = state.provenance().swaps_for(OperationId(2));
    assert_eq!(swaps_for_op2, vec![swap_id]);

    // Validate coverage across the full circuit
    state
        .provenance()
        .validate_coverage(state.circuit())
        .unwrap();
    state.validate_all().unwrap();
}

// ============================================================================
// 4. Routed Bell Circuit Test under ADR-0001
// ============================================================================

#[test]
fn test_routed_bell_circuit_with_swap_passes_validation_and_equivalence() {
    // 3 physical qubits in a line: 0 - 1 - 2
    let target = make_line_target("bell_target_3q", 3);

    // Logical Bell circuit on 2 qubits:
    // q0: --H--*--
    //          |
    // q1: -----X--
    let mut b = CircuitBuilder::new("bell_logical");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap(); // op#1
    b.cx(q[0], q[1]).unwrap(); // op#2
    let logical = b.finish().unwrap();

    // Initial mapping separates the qubits: q0 -> p0, q1 -> p2 (p1 is an idle ancilla |0>)
    let initial_layout = LayoutMap::from_mappings(
        2,
        3,
        [
            (QubitId(0), PhysicalQubitId(0)),
            (QubitId(1), PhysicalQubitId(2)),
        ],
    )
    .unwrap();

    let mut state = RouteState::from_logical(&logical, target, initial_layout).unwrap();

    // 1. Apply H(q0): mapped to physical p0
    state
        .apply_logical_operation(
            OperationId(1),
            &Operation::custom_gate("h", vec![QubitId(0)], vec![]),
        )
        .unwrap();

    // 2. Insert SWAP(p0, p1): q0 moves from p0 to p1
    let p0 = PhysicalQubitId(0);
    let p1 = PhysicalQubitId(1);
    state.insert_swap(p0, p1, Some(OperationId(2))).unwrap();

    // 3. Apply CX(q0, q1): q0 is at p1, q1 is at p2 -> physically CX(p1, p2)
    state
        .apply_logical_operation(
            OperationId(2),
            &Operation::custom_gate("cx", vec![QubitId(0), QubitId(1)], vec![]),
        )
        .unwrap();

    state.finalize_layout();

    // Final layout check: q0 at p1, q1 at p2 (p0 is idle ancilla!)
    assert_eq!(state.final_layout().get_physical(QubitId(0)), Some(p1));
    assert_eq!(
        state.final_layout().get_physical(QubitId(1)),
        Some(PhysicalQubitId(2))
    );
    assert_eq!(state.final_layout().unused_physical_qubits(), vec![p0]);

    // Graph validation
    state.validate_all().unwrap();

    // Full 3-qubit physical statevector equivalence under ADR-0001
    let verdict = state.validate_equivalence(&logical).unwrap();
    assert!(verdict.is_equivalent);
    assert_eq!(verdict.qubits, 3);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

// ============================================================================
// 5. Incremental QccIR Graph Validation after Each Rewrite Step
// ============================================================================

#[test]
fn test_qccir_validation_after_each_swap_rewrite() {
    let target = make_line_target("line_5q", 5);
    let initial_layout = LayoutMap::identity(5, 5).unwrap();

    let mut b = CircuitBuilder::new("incremental_test");
    let _q = b.add_quantum_register("q", 5).unwrap();
    let logical = b.finish().unwrap();

    let mut state = RouteState::from_logical(&logical, target, initial_layout).unwrap();

    // Sequential chain of 4 SWAPs across 5 physical wires
    let swaps = [(0, 1), (1, 2), (2, 3), (3, 4), (2, 3)];

    for (step, &(u, v)) in swaps.iter().enumerate() {
        let pu = PhysicalQubitId(u);
        let pv = PhysicalQubitId(v);

        let res = state.insert_swap(pu, pv, None).unwrap();
        assert_eq!(res.step, step);

        // Invariant: Graph remains acyclic and valid at every step
        validate_circuit(state.circuit()).unwrap();
        assert!(!state.circuit().graph.is_cyclic());
        assert_eq!(state.circuit().num_operations(), step + 1);

        // Layout remains valid and injective
        state.layout().validate().unwrap();
    }
}

// ============================================================================
// 6. Regression Tests: Inconsistent Layouts and Invalid Rewrites
// ============================================================================

#[test]
fn test_regression_inconsistent_layout_update_fails_equivalence() {
    let target = make_line_target("tamper_target", 3);
    let initial_layout = LayoutMap::identity(3, 3).unwrap();

    let mut b = CircuitBuilder::new("tamper_circ");
    let q = b.add_quantum_register("q", 3).unwrap();
    b.h(q[0]).unwrap();
    b.x(q[1]).unwrap();
    let logical = b.finish().unwrap();

    let mut state = RouteState::from_logical(&logical, target, initial_layout).unwrap();

    state
        .apply_logical_operation(
            OperationId(1),
            &Operation::custom_gate("h", vec![QubitId(0)], vec![]),
        )
        .unwrap();
    state
        .apply_logical_operation(
            OperationId(2),
            &Operation::custom_gate("x", vec![QubitId(1)], vec![]),
        )
        .unwrap();

    // Tamper: Insert a physical SWAP into the circuit graph WITHOUT updating the layout!
    state
        .circuit_mut()
        .add_operation(Operation::custom_gate(
            "swap",
            vec![QubitId(0), QubitId(1)],
            vec![],
        ))
        .unwrap();

    // Equivalence checker must detect the unmapped permutation and fail
    let err = state.validate_equivalence(&logical).unwrap_err();
    match err {
        RoutingError::TargetLegality(msg) => {
            assert!(msg.contains("failed layout-aware equivalence"));
        }
        other => panic!("expected TargetLegality error from equivalence failure, got {other:?}"),
    }
}

#[test]
fn test_regression_disconnected_qubit_swap_rejected() {
    // Disconnected target: {0, 1} and {2, 3}
    let mut target = Target::new("disconnected", 4);
    let swap_desc = TargetOperationDescriptor::builder("swap", 2)
        .connectivity(OperationConnectivity::undirected_two_qubit([
            (PhysicalQubitId::new(0), PhysicalQubitId::new(1)),
            (PhysicalQubitId::new(2), PhysicalQubitId::new(3)),
        ]))
        .build();
    target.add_operation(swap_desc);
    target.validate().unwrap();

    let initial_layout = LayoutMap::identity(4, 4).unwrap();
    let mut b = CircuitBuilder::new("disc");
    let _q = b.add_quantum_register("q", 4).unwrap();
    let logical = b.finish().unwrap();

    let mut state = RouteState::from_logical(&logical, target, initial_layout).unwrap();

    // Attempting to swap across disconnected components must return DisconnectedTopology
    let err = state
        .insert_swap(PhysicalQubitId(0), PhysicalQubitId(3), None)
        .unwrap_err();
    assert!(matches!(err, RoutingError::DisconnectedTopology { .. }));

    // Circuit remains clean (0 operations)
    assert_eq!(state.circuit().num_operations(), 0);
}

#[test]
fn test_regression_duplicate_operand_swap_rejected() {
    let target = make_line_target("line_3q", 3);
    let initial_layout = LayoutMap::identity(3, 3).unwrap();
    let mut b = CircuitBuilder::new("dup");
    let _q = b.add_quantum_register("q", 3).unwrap();
    let logical = b.finish().unwrap();

    let mut state = RouteState::from_logical(&logical, target, initial_layout).unwrap();

    // SWAP(p0, p0) must be rejected
    let err = state
        .insert_swap(PhysicalQubitId(0), PhysicalQubitId(0), None)
        .unwrap_err();
    assert!(matches!(err, RoutingError::DuplicateOperand { .. }));
}
