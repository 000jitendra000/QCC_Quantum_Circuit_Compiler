//! Integration and verification tests for Phase 34: DAG-aware Commutation Rewriting.

use proptest::prelude::*;

use qcc::builder::CircuitBuilder;
use qcc::ir::circuit::Circuit;
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::rewrite::CircuitTransaction;
use qcc::ir::wire::ClassicalBitId;
use qcc::opt::context::PassContext;
use qcc::opt::pass::CompilerPass;
use qcc::optimizer::passes::cancellation::InverseCancellationPass;
use qcc::optimizer::passes::commutation_reorder::{CommutationReorderPass, ReorderStrategy};
use qcc::optimizer::passes::rotation_merge::RotationMergePass;
use qcc::verification::{check_equivalence, VerificationError};

// ============================================================================
// 1. Opportunity Exposing Tests (Phase 29 & Phase 30 Enablement)
// ============================================================================

#[test]
fn test_expose_inverse_cancellation_rz_cx() {
    // Circuit: Rz(pi/3, q0) -> CX(q0, q1) -> Rz(-pi/3, q0)
    // op1 and op3 cannot cancel initially because CX(0, 1) is between them on q0.
    // CommutationReorderPass swaps CX(0, 1) and Rz(pi/3, q0) (or Rz(-pi/3, q0)),
    // bringing the two Rz rotations adjacent so InverseCancellationPass cancels them.
    let mut builder = CircuitBuilder::new("expose_rz_cx");
    let q = builder.add_quantum_register("q", 2).unwrap();

    let pi_3 = ParameterExpr::from(std::f64::consts::PI / 3.0);
    let neg_pi_3 = ParameterExpr::from(-std::f64::consts::PI / 3.0);

    builder.rz(q[0], pi_3).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], neg_pi_3).unwrap();

    let mut circuit = builder.finish().unwrap();
    assert_eq!(circuit.num_operations(), 3);

    // Initial inverse cancellation pass CANNOT cancel op1 and op3
    let mut cancel_pass = InverseCancellationPass::new();
    let mut ctx = PassContext::new();
    let res = cancel_pass.run(&mut circuit, &mut ctx).unwrap();
    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);

    // Run CommutationReorderPass
    let mut reorder_pass = CommutationReorderPass::new();
    let res = reorder_pass.run(&mut circuit, &mut ctx).unwrap();
    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 3);
    assert_eq!(reorder_pass.last_stats().unwrap().rewrites_applied, 1);

    // Now InverseCancellationPass CAN cancel the adjacent Rz rotations!
    let res = cancel_pass.run(&mut circuit, &mut ctx).unwrap();
    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    // Verify remaining operation is CX(q0, q1)
    let remaining_op = circuit.graph.ordered_nodes().unwrap()[0].operation.clone();
    assert_eq!(remaining_op.name(), "cx");
    assert_eq!(remaining_op.qubits(), &[q[0].qubit_id(), q[1].qubit_id()]);

    // Verify circuit passes Phase 10 unitary equivalence against reference CX(q0, q1)
    let mut ref_builder = CircuitBuilder::new("ref_cx");
    let rq = ref_builder.add_quantum_register("q", 2).unwrap();
    ref_builder.cx(rq[0], rq[1]).unwrap();
    let ref_circuit = ref_builder.finish().unwrap();

    let eq_res = check_equivalence(&circuit, &ref_circuit).unwrap();
    assert!(eq_res.is_equivalent);
}

#[test]
fn test_expose_inverse_cancellation_shared_control_cx() {
    // Circuit: CX(0, 1) -> CX(0, 2) -> CX(0, 1)
    // CX(0, 1) and CX(0, 2) share control q0 with disjoint targets (q1, q2), so they commute.
    // Commutation reordering brings the two CX(0, 1) operations adjacent so they cancel to identity!
    let mut builder = CircuitBuilder::new("expose_cx_cx");
    let q = builder.add_quantum_register("q", 3).unwrap();

    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[0], q[2]).unwrap();
    builder.cx(q[0], q[1]).unwrap();

    let mut circuit = builder.finish().unwrap();
    assert_eq!(circuit.num_operations(), 3);

    // Inverse cancellation alone does nothing initially
    let mut cancel_pass = InverseCancellationPass::new();
    let mut ctx = PassContext::new();
    let res = cancel_pass.run(&mut circuit, &mut ctx).unwrap();
    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);

    // Run CommutationReorderPass
    let mut reorder_pass = CommutationReorderPass::new();
    let res = reorder_pass.run(&mut circuit, &mut ctx).unwrap();
    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 3);

    // Run InverseCancellationPass
    let res = cancel_pass.run(&mut circuit, &mut ctx).unwrap();
    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    // Only CX(0, 2) remains
    let remaining_op = circuit.graph.ordered_nodes().unwrap()[0].operation.clone();
    assert_eq!(remaining_op.name(), "cx");
    assert_eq!(remaining_op.qubits(), &[q[0].qubit_id(), q[2].qubit_id()]);

    // Verify unitary equivalence
    let mut ref_builder = CircuitBuilder::new("ref_cx02");
    let rq = ref_builder.add_quantum_register("q", 3).unwrap();
    ref_builder.cx(rq[0], rq[2]).unwrap();
    let ref_circuit = ref_builder.finish().unwrap();

    let eq_res = check_equivalence(&circuit, &ref_circuit).unwrap();
    assert!(eq_res.is_equivalent);
}

#[test]
fn test_expose_inverse_cancellation_x_on_cx_target() {
    // Circuit: X(1) -> CX(0, 1) -> X(1)
    // Target is q1. X on target commutes with CX(0, 1).
    // Reordering brings the two X(1) operations adjacent so they cancel.
    let mut builder = CircuitBuilder::new("expose_x_cx");
    let q = builder.add_quantum_register("q", 2).unwrap();

    builder.x(q[1]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.x(q[1]).unwrap();

    let mut circuit = builder.finish().unwrap();
    assert_eq!(circuit.num_operations(), 3);

    let mut ctx = PassContext::new();
    let mut reorder_pass = CommutationReorderPass::new();
    reorder_pass.run(&mut circuit, &mut ctx).unwrap();

    let mut cancel_pass = InverseCancellationPass::new();
    cancel_pass.run(&mut circuit, &mut ctx).unwrap();

    assert_eq!(circuit.num_operations(), 1);
    let nodes = circuit.graph.ordered_nodes().unwrap();
    assert_eq!(nodes[0].operation.name(), "cx");

    let mut ref_builder = CircuitBuilder::new("ref");
    let rq = ref_builder.add_quantum_register("q", 2).unwrap();
    ref_builder.cx(rq[0], rq[1]).unwrap();
    let ref_circuit = ref_builder.finish().unwrap();

    assert!(
        check_equivalence(&circuit, &ref_circuit)
            .unwrap()
            .is_equivalent
    );
}

#[test]
fn test_expose_rotation_merge_rz_cz() {
    // Circuit: Rz(pi/4, 0) -> CZ(0, 1) -> Rz(pi/4, 0)
    // CZ commutes with Z-axis rotations on either wire.
    // Reordering brings Rz(pi/4, 0) and Rz(pi/4, 0) adjacent so RotationMergePass merges to Rz(pi/2, 0).
    let mut builder = CircuitBuilder::new("expose_rz_cz");
    let q = builder.add_quantum_register("q", 2).unwrap();

    let pi_4 = ParameterExpr::from(std::f64::consts::PI / 4.0);
    builder.rz(q[0], pi_4.clone()).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    builder.rz(q[0], pi_4).unwrap();

    let mut circuit = builder.finish().unwrap();
    assert_eq!(circuit.num_operations(), 3);

    let mut merge_pass = RotationMergePass::new();
    let mut ctx = PassContext::new();
    let res = merge_pass.run(&mut circuit, &mut ctx).unwrap();
    assert!(!res.modified);

    let mut reorder_pass = CommutationReorderPass::new();
    let res = reorder_pass.run(&mut circuit, &mut ctx).unwrap();
    assert!(res.modified);

    let res = merge_pass.run(&mut circuit, &mut ctx).unwrap();
    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 2);

    // Reference circuit: CZ(0, 1) -> Rz(pi/2, 0)
    let mut ref_builder = CircuitBuilder::new("ref");
    let rq = ref_builder.add_quantum_register("q", 2).unwrap();
    let pi_2 = ParameterExpr::from(std::f64::consts::PI / 2.0);
    ref_builder.cz(rq[0], rq[1]).unwrap();
    ref_builder.rz(rq[0], pi_2).unwrap();
    let ref_circuit = ref_builder.finish().unwrap();

    let eq_res = check_equivalence(&circuit, &ref_circuit).unwrap();
    assert!(eq_res.is_equivalent);
}

// ============================================================================
// 2. Noncommuting Pairs Never Reordered
// ============================================================================

#[test]
fn test_noncommuting_x_on_cx_control_never_reordered() {
    // X on control does NOT commute with CX(0, 1)
    let mut builder = CircuitBuilder::new("x_ctrl");
    let q = builder.add_quantum_register("q", 2).unwrap();

    builder.x(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();

    let mut circuit = builder.finish().unwrap();
    let mut pass = CommutationReorderPass::new();
    let mut ctx = PassContext::new();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(pass.last_stats().unwrap().rewrites_applied, 0);
    assert_eq!(pass.last_stats().unwrap().commutation_checks_rejected, 1);
}

#[test]
fn test_noncommuting_z_on_cx_target_never_reordered() {
    // Z on target does NOT commute with CX(0, 1)
    let mut builder = CircuitBuilder::new("z_target");
    let q = builder.add_quantum_register("q", 2).unwrap();

    builder.z(q[1]).unwrap();
    builder.cx(q[0], q[1]).unwrap();

    let mut circuit = builder.finish().unwrap();
    let mut pass = CommutationReorderPass::new();
    let mut ctx = PassContext::new();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(pass.last_stats().unwrap().rewrites_applied, 0);
    assert_eq!(pass.last_stats().unwrap().commutation_checks_rejected, 1);
}

#[test]
fn test_noncommuting_reversed_cx_never_reordered() {
    // CX(0, 1) and CX(1, 0) do NOT commute
    let mut builder = CircuitBuilder::new("cx_rev");
    let q = builder.add_quantum_register("q", 2).unwrap();

    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[1], q[0]).unwrap();

    let mut circuit = builder.finish().unwrap();
    let mut pass = CommutationReorderPass::new();
    let mut ctx = PassContext::new();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(pass.last_stats().unwrap().rewrites_applied, 0);
    assert_eq!(pass.last_stats().unwrap().commutation_checks_rejected, 1);
}

#[test]
fn test_noncommuting_chained_cx_never_reordered() {
    // CX(0, 1) and CX(1, 2) share wire q1 where q1 is target in first and control in second.
    // They do NOT commute.
    let mut builder = CircuitBuilder::new("cx_chain");
    let q = builder.add_quantum_register("q", 3).unwrap();

    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[1], q[2]).unwrap();

    let mut circuit = builder.finish().unwrap();
    let mut pass = CommutationReorderPass::new();
    let mut ctx = PassContext::new();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(pass.last_stats().unwrap().rewrites_applied, 0);
    assert_eq!(pass.last_stats().unwrap().commutation_checks_rejected, 1);
}

// ============================================================================
// 3. Side-Effect Boundaries & Non-Unitary Protection
// ============================================================================

#[test]
fn test_barrier_blocks_reordering_across_boundary() {
    // Circuit: Rz(pi/3, 0) -> Barrier([0, 1]) -> CX(0, 1) -> Rz(-pi/3, 0)
    // The barrier prevents Rz(pi/3, 0) from moving across to meet Rz(-pi/3, 0).
    let mut builder = CircuitBuilder::new("barrier_block");
    let q = builder.add_quantum_register("q", 2).unwrap();

    let pi_3 = ParameterExpr::from(std::f64::consts::PI / 3.0);
    let neg_pi_3 = ParameterExpr::from(-std::f64::consts::PI / 3.0);

    builder.rz(q[0], pi_3).unwrap();
    builder.barrier(&[q[0], q[1]]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], neg_pi_3).unwrap();

    let mut circuit = builder.finish().unwrap();
    let mut pass = CommutationReorderPass::new();
    let mut ctx = PassContext::new();
    let _ = pass.run(&mut circuit, &mut ctx).unwrap();

    // Verify operations did not cross the barrier
    let new_nodes = circuit.graph.ordered_nodes().unwrap();
    // Barrier is still at index 1
    assert_eq!(new_nodes[1].operation.name(), "barrier");
    // op 0 is still Rz(pi/3, 0)
    assert_eq!(new_nodes[0].operation.name(), "rz");
}

#[test]
fn test_measurement_boundary_blocks_reordering() {
    // Measurement on q0 -> c0 prevents any movement across it
    let mut builder = CircuitBuilder::new("meas_block");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();

    builder.h(q[0]).unwrap();
    builder.measure(q[0], c[0]).unwrap();

    let mut circuit = builder.finish().unwrap();
    let mut pass = CommutationReorderPass::new();
    let mut ctx = PassContext::new();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    let nodes = circuit.graph.ordered_nodes().unwrap();
    assert_eq!(nodes[0].operation.name(), "h");
    assert_eq!(nodes[1].operation.name(), "measure");
}

#[test]
fn test_classical_raw_war_waw_preservation() {
    let mut circuit = Circuit::new("classical_hazard_block");
    circuit.add_classical_register("c", 2).unwrap();
    let c0 = ClassicalBitId(0);

    // op1 writes c0, op2 reads c0 (RAW conflict)
    let op1 = Operation::classical("write_c0", vec![], vec![c0]);
    let op2 = Operation::classical("read_c0", vec![c0], vec![]);
    circuit.add_operation(op1).unwrap();
    circuit.add_operation(op2).unwrap();

    let mut pass = CommutationReorderPass::new();
    let mut ctx = PassContext::new();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(pass.last_stats().unwrap().rewrites_applied, 0);
}

// ============================================================================
// 4. Anti-Oscillation and Termination Tests
// ============================================================================

#[test]
fn test_anti_oscillation_under_all_commuting_strategy() {
    // Circuit: Rz(pi/3, 0) -> CX(0, 1)
    // Under AllCommuting strategy, these two gates commute.
    // In iteration 1, they swap: [CX(0, 1), Rz(pi/3, 0)].
    // In iteration 2, CX(0, 1) and Rz(pi/3, 0) still commute.
    // The anti-oscillation guard MUST prevent swapping them back!
    let mut builder = CircuitBuilder::new("anti_osc");
    let q = builder.add_quantum_register("q", 2).unwrap();

    let pi_3 = ParameterExpr::from(std::f64::consts::PI / 3.0);
    builder.rz(q[0], pi_3).unwrap();
    builder.cx(q[0], q[1]).unwrap();

    let mut circuit = builder.finish().unwrap();
    let mut pass = CommutationReorderPass::new().with_strategy(ReorderStrategy::AllCommuting);
    let mut ctx = PassContext::new();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.rewrites_applied, 1);
    assert!(stats.rewrites_prevented_by_anti_oscillation >= 1);
    assert!(!stats.budget_exhausted);
}

#[test]
fn test_rewrite_budget_exhaustion() {
    // Circuit with multiple commuting opportunities
    // Set max_rewrites = 1: pass MUST terminate after 1 rewrite with budget_exhausted = true.
    let mut builder = CircuitBuilder::new("budget_exhaust");
    let q = builder.add_quantum_register("q", 3).unwrap();

    // CX(0, 1) and CX(0, 2) commute
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[0], q[2]).unwrap();
    // CX(0, 1) again
    builder.cx(q[0], q[1]).unwrap();

    let mut circuit = builder.finish().unwrap();
    let mut pass = CommutationReorderPass::new().with_max_rewrites(1);
    let mut ctx = PassContext::new();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.rewrites_applied, 1);
    assert!(stats.budget_exhausted);
}

#[test]
fn test_deterministic_output_across_repeated_runs() {
    let create_circuit = || {
        let mut builder = CircuitBuilder::new("det_circ");
        let q = builder.add_quantum_register("q", 3).unwrap();

        let pi_4 = ParameterExpr::from(std::f64::consts::PI / 4.0);
        let neg_pi_4 = ParameterExpr::from(-std::f64::consts::PI / 4.0);

        builder.rz(q[0], pi_4).unwrap();
        builder.cx(q[0], q[1]).unwrap();
        builder.cx(q[0], q[2]).unwrap();
        builder.rz(q[0], neg_pi_4).unwrap();
        builder.finish().unwrap()
    };

    let base_circuit = create_circuit();
    let mut pass = CommutationReorderPass::new();
    let mut ctx = PassContext::new();

    let mut expected_circuit = base_circuit.clone();
    pass.run(&mut expected_circuit, &mut ctx).unwrap();
    let expected_ops: Vec<String> = expected_circuit
        .graph
        .ordered_nodes()
        .unwrap()
        .into_iter()
        .map(|n| n.operation.to_string())
        .collect();
    let expected_stats = pass.last_stats().unwrap();

    for _ in 0..10 {
        let mut test_circ = base_circuit.clone();
        pass.run(&mut test_circ, &mut ctx).unwrap();
        let test_ops: Vec<String> = test_circ
            .graph
            .ordered_nodes()
            .unwrap()
            .into_iter()
            .map(|n| n.operation.to_string())
            .collect();
        assert_eq!(test_ops, expected_ops);
        assert_eq!(pass.last_stats().unwrap(), expected_stats);
    }
}

// ============================================================================
// 5. Transaction Rollback & Invariant Validation
// ============================================================================

#[test]
fn test_transaction_rollback_preserves_circuit() {
    let mut builder = CircuitBuilder::new("rollback_test");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let id1 = builder.h(q[0]).unwrap();
    let id2 = builder.x(q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig_circuit = circuit.clone();

    // Start a transaction, perform a swap, and rollback (drop)
    {
        let mut tx = CircuitTransaction::begin(&mut circuit);
        tx.swap_adjacent_operations(id1, id2).unwrap();
        // Drop tx without calling commit()
    }

    // Circuit should be completely unmodified
    assert_eq!(circuit.num_operations(), orig_circuit.num_operations());
    let nodes = circuit.graph.ordered_nodes().unwrap();
    assert_eq!(nodes[0].id, id1);
    assert_eq!(nodes[1].id, id2);
}

#[test]
fn test_graph_invariants_preserved_after_rewrites() {
    let mut builder = CircuitBuilder::new("invariants_check");
    let q = builder.add_quantum_register("q", 3).unwrap();
    let pi_6 = ParameterExpr::from(std::f64::consts::PI / 6.0);

    builder.rz(q[0], pi_6.clone()).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[0], q[2]).unwrap();
    builder.rz(q[0], pi_6).unwrap();

    let mut circuit = builder.finish().unwrap();
    let mut pass = CommutationReorderPass::new();
    let mut ctx = PassContext::new();
    pass.run(&mut circuit, &mut ctx).unwrap();

    // Invariant checks
    assert!(circuit.validate().is_ok());
    assert!(!circuit.graph.is_cyclic());
    assert_eq!(circuit.graph.node_count(), 4);

    let nodes = circuit.graph.ordered_nodes().unwrap();
    for (i, node) in nodes.iter().enumerate() {
        assert_eq!(node.sequence_number, (i + 1) as u64);
    }
}

#[test]
fn test_non_unitary_circuit_rejected_by_unitary_checker() {
    // Circuits containing measurement must be rejected by Phase 10 unitary checker
    let mut builder = CircuitBuilder::new("meas_non_unitary");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();

    builder.h(q[0]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    let circ1 = builder.finish().unwrap();
    let circ2 = circ1.clone();

    let res = check_equivalence(&circ1, &circ2);
    assert!(matches!(
        res,
        Err(VerificationError::NonUnitaryOperation { .. })
    ));
}

#[test]
fn test_disjoint_wire_operations_reordered_preserve_semantics() {
    // X on q0 and Z on q1 are on disjoint wires and commute.
    // Reordering them under SingleQubitFirst or AllCommuting preserves unitary equivalence.
    let mut builder = CircuitBuilder::new("disjoint_reorder");
    let q = builder.add_quantum_register("q", 2).unwrap();

    builder.x(q[0]).unwrap();
    builder.z(q[1]).unwrap();

    let mut circuit = builder.finish().unwrap();
    let orig = circuit.clone();

    let mut pass = CommutationReorderPass::new().with_strategy(ReorderStrategy::AllCommuting);
    let mut ctx = PassContext::new();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(pass.last_stats().unwrap().rewrites_applied, 1);
    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 6. Property-Based Randomized Tests
// ============================================================================

proptest! {
    #[test]
    fn prop_random_circuit_commutation_reordering_preserves_semantics(
        angles in prop::collection::vec(-std::f64::consts::PI..std::f64::consts::PI, 4),
    ) {
        let mut builder = CircuitBuilder::new("prop_comm_reorder");
        let q = builder.add_quantum_register("q", 2).unwrap();

        // Construct 2-qubit circuit with commuting and noncommuting sections:
        // Rz(a0, q0) -> CX(q0, q1) -> Rz(a1, q0) -> Rx(a2, q1) -> CX(q0, q1) -> Rx(a3, q1)
        builder.rz(q[0], ParameterExpr::from(angles[0])).unwrap();
        builder.cx(q[0], q[1]).unwrap();
        builder.rz(q[0], ParameterExpr::from(angles[1])).unwrap();
        builder.rx(q[1], ParameterExpr::from(angles[2])).unwrap();
        builder.cx(q[0], q[1]).unwrap();
        builder.rx(q[1], ParameterExpr::from(angles[3])).unwrap();

        let mut circuit = builder.finish().unwrap();
        let ref_circuit = circuit.clone();

        let mut pass = CommutationReorderPass::new().with_strategy(ReorderStrategy::ExposeOpportunities);
        let mut ctx = PassContext::new();
        let _ = pass.run(&mut circuit, &mut ctx).unwrap();

        // 1. Graph validation and invariants
        prop_assert!(circuit.validate().is_ok());
        prop_assert!(!circuit.graph.is_cyclic());

        // 2. Unitary equivalence against original circuit with common global phase
        let eq_res = check_equivalence(&circuit, &ref_circuit).unwrap();
        prop_assert!(eq_res.is_equivalent);
    }

    #[test]
    fn prop_all_commuting_reordering_preserves_unitary_equivalence(
        theta in -std::f64::consts::PI..std::f64::consts::PI,
    ) {
        let mut builder = CircuitBuilder::new("prop_all_comm");
        let q = builder.add_quantum_register("q", 2).unwrap();

        // Rz on control commutes with CX
        builder.rz(q[0], ParameterExpr::from(theta)).unwrap();
        builder.cx(q[0], q[1]).unwrap();

        let mut circuit = builder.finish().unwrap();
        let ref_circuit = circuit.clone();

        let mut pass = CommutationReorderPass::new().with_strategy(ReorderStrategy::AllCommuting);
        let mut ctx = PassContext::new();
        let _ = pass.run(&mut circuit, &mut ctx).unwrap();

        prop_assert!(circuit.validate().is_ok());
        let eq_res = check_equivalence(&circuit, &ref_circuit).unwrap();
        prop_assert!(eq_res.is_equivalent);
    }
}
