//! Comprehensive integration, unit, and property tests for Same-Axis Rotation Merging (Phase 30).
//!
//! Validates:
//! 1. Numeric single-qubit rotation composition: Rx, Ry, Rz.
//! 2. Random numeric angles verified with Phase 10 UnitaryEquivalenceChecker.
//! 3. 4π periodicity (exact zero identity elimination with 0 global phase).
//! 4. 2π global-phase behavior (phase zero identity elimination with +π global phase).
//! 5. Negative angles, boundary periodicity, and non-zero angle normalization.
//! 6. Chains of sequential rotations merging across multiple fixed-point iterations.
//! 7. Symbolic parameter expressions (algebraic opposites cancellation, bound symbol_env, and CombineSymbolic).
//! 8. Unresolved symbolic cases remaining unchanged under PreserveUnresolved.
//! 9. Intervening operations on the same wire blocking merge (H, CX, different rotation axes).
//! 10. Operations on unrelated wires commuting in the DAG without blocking merge.
//! 11. Non-unitary boundaries blocking merge (measurements, resets, barriers, classical operations).
//! 12. DAG invariants, provenance tracking, stable operation IDs, and transactional rollback.
//! 13. Telemetry statistics accuracy.
//! 14. Proptest property-based tests and randomized circuit verification (exit-gate proof).

use std::collections::HashMap;
use std::f64::consts::PI;

use proptest::prelude::*;
use qcc::builder::CircuitBuilder;
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::rewrite::CircuitTransaction;
use qcc::ir::wire::QubitId;
use qcc::opt::context::PassContext;
use qcc::opt::pass::CompilerPass;
use qcc::opt::pass_manager::PassManager;
use qcc::optimizer::passes::rotation_merge::*;
use qcc::quantum::angle::{normalize_rotation_angle, RotationNormalizationMode, FOUR_PI, TWO_PI};
use qcc::quantum::gate::Gate;
use qcc::quantum::parameter::Angle;
use qcc::simulator::sampling::SplitMix64;
use qcc::testkit::random_circuit::generate_random_unitary_circuit;
use qcc::verification::check_equivalence;

// ============================================================================
// 1. Numeric Rx, Ry, Rz Angle Combinations
// ============================================================================

#[test]
fn test_numeric_rx_simple_merge() {
    let mut builder = CircuitBuilder::new("rx_merge");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], Angle::from_radians(0.3)).unwrap();
    builder.rx(q[0], Angle::from_radians(0.4)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 2);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    let node = circuit.graph.nodes().next().unwrap();
    assert_eq!(node.operation.name(), "rx");
    assert_eq!(node.operation.qubits(), &[QubitId(0)]);
    let param = node.operation.parameters()[0].as_constant().unwrap();
    assert!((param - 0.7).abs() < 1e-12);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.rotations_merged, 1);
    assert_eq!(stats.non_zero_rotations_fused, 1);
    assert_eq!(stats.operations_removed, 2);
    assert_eq!(stats.operations_added, 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_numeric_ry_simple_merge() {
    let mut builder = CircuitBuilder::new("ry_merge");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.ry(q[0], Angle::from_radians(0.5)).unwrap();
    builder.ry(q[0], Angle::from_radians(0.6)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 2);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    let node = circuit.graph.nodes().next().unwrap();
    assert_eq!(node.operation.name(), "ry");
    let param = node.operation.parameters()[0].as_constant().unwrap();
    assert!((param - 1.1).abs() < 1e-12);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_numeric_rz_simple_merge() {
    let mut builder = CircuitBuilder::new("rz_merge");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rz(q[0], Angle::from_radians(PI / 4.0)).unwrap();
    builder.rz(q[0], Angle::from_radians(PI / 4.0)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 2);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    let node = circuit.graph.nodes().next().unwrap();
    assert_eq!(node.operation.name(), "rz");
    let param = node.operation.parameters()[0].as_constant().unwrap();
    assert!((param - PI / 2.0).abs() < 1e-12);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_chain_of_three_rotations_merges_to_single() {
    let mut builder = CircuitBuilder::new("rx_chain_3");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], Angle::from_radians(0.1)).unwrap();
    builder.rx(q[0], Angle::from_radians(0.2)).unwrap();
    builder.rx(q[0], Angle::from_radians(0.3)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 3);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    let node = circuit.graph.nodes().next().unwrap();
    assert_eq!(node.operation.name(), "rx");
    let param = node.operation.parameters()[0].as_constant().unwrap();
    assert!((param - 0.6).abs() < 1e-12);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.rotations_merged, 2);
    assert_eq!(stats.non_zero_rotations_fused, 2);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 2. 4π Periodicity & 2π Global Phase Behavior
// ============================================================================

#[test]
fn test_four_pi_periodicity_exact_zero_eliminated() {
    let mut builder = CircuitBuilder::new("four_pi_exact");
    let q = builder.add_quantum_register("q", 1).unwrap();
    // Sum is 4π: exact identity with zero global phase
    builder.rx(q[0], Angle::from_radians(2.5 * PI)).unwrap();
    builder.rx(q[0], Angle::from_radians(1.5 * PI)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 2);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.rotations_merged, 1);
    assert_eq!(stats.exact_zero_identities_eliminated, 1);
    assert_eq!(stats.global_phase_adjustments, 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_two_pi_periodicity_phase_zero_eliminated_with_phase() {
    let mut builder = CircuitBuilder::new("two_pi_phase");
    let q = builder.add_quantum_register("q", 1).unwrap();
    // Sum is 2π: evaluates to -I = exp(i*π) * I
    builder.ry(q[0], Angle::from_radians(PI)).unwrap();
    builder.ry(q[0], Angle::from_radians(PI)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 2);
    assert_eq!(circuit.global_phase().as_constant().unwrap(), 0.0);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    // Global phase must have been adjusted by +π
    let gp = circuit.global_phase().as_constant().unwrap();
    assert!((gp - PI).abs() < 1e-12);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.rotations_merged, 1);
    assert_eq!(stats.phase_zero_identities_eliminated, 1);
    assert_eq!(stats.global_phase_adjustments, 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_nonzero_rotation_normalized_with_two_pi_shift_adds_phase() {
    let mut builder = CircuitBuilder::new("two_pi_shift_nonzero");
    let q = builder.add_quantum_register("q", 1).unwrap();
    // Sum is 2.5π: normalized into 0.5π with +π global phase
    builder.rz(q[0], Angle::from_radians(1.5 * PI)).unwrap();
    builder.rz(q[0], Angle::from_radians(PI)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    let node = circuit.graph.nodes().next().unwrap();
    let param = node.operation.parameters()[0].as_constant().unwrap();
    assert!((param - 0.5 * PI).abs() < 1e-12);

    let gp = circuit.global_phase().as_constant().unwrap();
    assert!((gp - PI).abs() < 1e-12);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.global_phase_adjustments, 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 3. Negative Angles and Boundaries
// ============================================================================

#[test]
fn test_negative_angles_merge_correctly() {
    let mut builder = CircuitBuilder::new("neg_angles");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], Angle::from_radians(-0.4)).unwrap();
    builder.rx(q[0], Angle::from_radians(-0.6)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_negative_two_pi_phase_zero() {
    let mut builder = CircuitBuilder::new("neg_two_pi");
    let q = builder.add_quantum_register("q", 1).unwrap();
    // Sum is -2π: -2π ≡ 2π mod 4π -> phase zero with +π phase adjustment
    builder.ry(q[0], Angle::from_radians(-PI)).unwrap();
    builder.ry(q[0], Angle::from_radians(-PI)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let gp = circuit.global_phase().as_constant().unwrap();
    assert!((gp - PI).abs() < 1e-12);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_negative_four_pi_exact_zero() {
    let mut builder = CircuitBuilder::new("neg_four_pi");
    let q = builder.add_quantum_register("q", 1).unwrap();
    // Sum is -4π: -4π ≡ 0 mod 4π -> exact zero with 0 global phase
    builder.rz(q[0], Angle::from_radians(-2.0 * PI)).unwrap();
    builder.rz(q[0], Angle::from_radians(-2.0 * PI)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let gp = circuit.global_phase().as_constant().unwrap();
    assert_eq!(gp, 0.0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_opposite_angles_cancel_to_exact_zero() {
    let mut builder = CircuitBuilder::new("opposite_angles");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], Angle::from_radians(1.2345)).unwrap();
    builder.rx(q[0], Angle::from_radians(-1.2345)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.exact_zero_identities_eliminated, 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 4. Symbolic Parameter Handling
// ============================================================================

#[test]
fn test_symbolic_algebraic_opposites_cancel_to_identity() {
    let mut builder = CircuitBuilder::new("sym_opposites");
    builder.add_quantum_register("q", 1).unwrap();
    let mut circuit = builder.finish().unwrap();
    let theta = ParameterExpr::symbol("theta");
    let neg_theta = ParameterExpr::Neg(Box::new(theta.clone()));

    circuit
        .add_operation(Operation::custom_gate("rx", vec![QubitId(0)], vec![theta]))
        .unwrap();
    circuit
        .add_operation(Operation::custom_gate(
            "rx",
            vec![QubitId(0)],
            vec![neg_theta],
        ))
        .unwrap();

    assert_eq!(circuit.num_operations(), 2);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.rotations_merged, 1);
    assert_eq!(stats.exact_zero_identities_eliminated, 1);
}

#[test]
fn test_unresolved_symbolic_parameters_remain_unchanged_under_preserve_policy() {
    let mut builder = CircuitBuilder::new("sym_unresolved");
    builder.add_quantum_register("q", 1).unwrap();
    let mut circuit = builder.finish().unwrap();
    let alpha = ParameterExpr::symbol("alpha");
    let beta = ParameterExpr::symbol("beta");

    circuit
        .add_operation(Operation::custom_gate("rx", vec![QubitId(0)], vec![alpha]))
        .unwrap();
    circuit
        .add_operation(Operation::custom_gate("rx", vec![QubitId(0)], vec![beta]))
        .unwrap();

    assert_eq!(circuit.num_operations(), 2);

    // Default policy: PreserveUnresolved
    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 2);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.rotations_merged, 0);
    assert_eq!(stats.candidate_pairs_rejected, 1);
}

#[test]
fn test_symbolic_combine_under_combine_policy() {
    let mut builder = CircuitBuilder::new("sym_combine");
    builder.add_quantum_register("q", 1).unwrap();
    let mut circuit = builder.finish().unwrap();
    let alpha = ParameterExpr::symbol("alpha");
    let beta = ParameterExpr::symbol("beta");

    circuit
        .add_operation(Operation::custom_gate(
            "rx",
            vec![QubitId(0)],
            vec![alpha.clone()],
        ))
        .unwrap();
    circuit
        .add_operation(Operation::custom_gate(
            "rx",
            vec![QubitId(0)],
            vec![beta.clone()],
        ))
        .unwrap();

    assert_eq!(circuit.num_operations(), 2);

    let mut pass =
        RotationMergePass::new().with_symbolic_policy(SymbolicMergePolicy::CombineSymbolic);
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    let node = circuit.graph.nodes().next().unwrap();
    assert_eq!(node.operation.name(), "rx");
    assert_eq!(
        node.operation.parameters()[0],
        ParameterExpr::Add(Box::new(alpha), Box::new(beta))
    );

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.rotations_merged, 1);
    assert_eq!(stats.symbolic_pairs_merged, 1);
    assert_eq!(stats.non_zero_rotations_fused, 1);
}

#[test]
fn test_bound_symbol_env_evaluates_and_merges() {
    let mut builder = CircuitBuilder::new("sym_bound");
    builder.add_quantum_register("q", 1).unwrap();
    let mut circuit = builder.finish().unwrap();
    let alpha = ParameterExpr::symbol("alpha");
    let beta = ParameterExpr::symbol("beta");

    circuit
        .add_operation(Operation::custom_gate("ry", vec![QubitId(0)], vec![alpha]))
        .unwrap();
    circuit
        .add_operation(Operation::custom_gate("ry", vec![QubitId(0)], vec![beta]))
        .unwrap();

    let mut env = HashMap::new();
    env.insert("alpha".to_string(), 0.4);
    env.insert("beta".to_string(), 0.6);

    let mut pass = RotationMergePass::new().with_symbol_env(env);
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    let node = circuit.graph.nodes().next().unwrap();
    let param = node.operation.parameters()[0].as_constant().unwrap();
    assert!((param - 1.0).abs() < 1e-12);
}

// ============================================================================
// 5. Intervening Wire Operations & Different Axes Prevent Merging
// ============================================================================

#[test]
fn test_intervening_single_qubit_gate_blocks_merge() {
    let mut builder = CircuitBuilder::new("intervening_h");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], Angle::from_radians(0.5)).unwrap();
    builder.h(q[0]).unwrap();
    builder.rx(q[0], Angle::from_radians(0.5)).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 3);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_intervening_two_qubit_gate_blocks_merge() {
    let mut builder = CircuitBuilder::new("intervening_cx");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.rz(q[0], Angle::from_radians(0.5)).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], Angle::from_radians(0.5)).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 3);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_different_rotation_axes_do_not_merge() {
    let mut builder = CircuitBuilder::new("diff_axes");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], Angle::from_radians(0.5)).unwrap();
    builder.rz(q[0], Angle::from_radians(0.5)).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 2);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 2);
}

// ============================================================================
// 6. Non-Unitary and Synchronization Boundaries
// ============================================================================

#[test]
fn test_barrier_blocks_merge() {
    let mut builder = CircuitBuilder::new("barrier_block");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], Angle::from_radians(0.5)).unwrap();
    builder.barrier(&[q[0]]).unwrap();
    builder.rx(q[0], Angle::from_radians(0.5)).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 3);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_measurement_blocks_merge() {
    let mut builder = CircuitBuilder::new("measure_block");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();
    builder.ry(q[0], Angle::from_radians(0.5)).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    builder.ry(q[0], Angle::from_radians(0.5)).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 3);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_reset_blocks_merge() {
    let mut builder = CircuitBuilder::new("reset_block");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rz(q[0], Angle::from_radians(0.5)).unwrap();
    builder.reset(q[0]).unwrap();
    builder.rz(q[0], Angle::from_radians(0.5)).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 3);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);
}

// ============================================================================
// 7. Operations on Unrelated Wires Commute in DAG
// ============================================================================

#[test]
fn test_disjoint_wire_operations_do_not_block_merge() {
    let mut builder = CircuitBuilder::new("disjoint_wire");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.rx(q[0], Angle::from_radians(0.3)).unwrap();
    builder.h(q[1]).unwrap(); // on q[1], not q[0]
    builder.rx(q[0], Angle::from_radians(0.4)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 3);

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 2); // Rx(0.7) on q0, H on q1

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 8. DAG Invariants, Provenance & Transactional Rollback
// ============================================================================

#[test]
fn test_provenance_and_id_stability() {
    let mut builder = CircuitBuilder::new("provenance_test");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let id_rx1 = builder.rx(q[0], Angle::from_radians(0.2)).unwrap();
    let id_h = builder.h(q[1]).unwrap();
    let id_rx2 = builder.rx(q[0], Angle::from_radians(0.3)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    pass.run(&mut circuit, &mut ctx).unwrap();

    // H on q1 must retain its exact stable OperationId
    let h_node = circuit.graph.get_node(id_h).expect("H node must exist");
    assert_eq!(h_node.operation.name(), "h");

    // The fused node must record [id_rx1, id_rx2] in its provenance
    let fused_node = circuit
        .graph
        .nodes()
        .find(|n| n.operation.name() == "rx")
        .expect("Fused Rx node must exist");
    assert_eq!(fused_node.provenance, vec![id_rx1, id_rx2]);
}

#[test]
fn test_transactional_rollback_preserves_circuit() {
    let mut builder = CircuitBuilder::new("rollback_test");
    let _q = builder.add_quantum_register("q", 1).unwrap();
    let mut circuit = builder.finish().unwrap();
    circuit
        .add_operation(Operation::custom_gate(
            "rx",
            vec![QubitId(0)],
            vec![ParameterExpr::Constant(0.2)],
        ))
        .unwrap();
    circuit
        .add_operation(Operation::custom_gate(
            "rx",
            vec![QubitId(0)],
            vec![ParameterExpr::Constant(0.3)],
        ))
        .unwrap();

    let orig = circuit.clone();

    // Staged transaction that rolls back without commit
    {
        let mut tx = CircuitTransaction::begin(&mut circuit);
        let id_a = tx.circuit().graph.ordered_nodes().unwrap()[0].id;
        let id_b = tx.circuit().graph.ordered_nodes().unwrap()[1].id;
        tx.fuse_operations(
            &[id_a, id_b],
            Operation::custom_gate("rx", vec![QubitId(0)], vec![ParameterExpr::Constant(0.5)]),
        )
        .unwrap();
        tx.rollback();
    }

    assert_eq!(circuit.num_operations(), orig.num_operations());
    assert_eq!(circuit.num_qubits(), orig.num_qubits());
}

// ============================================================================
// 9. PassManager Integration & Telemetry Statistics
// ============================================================================

#[test]
fn test_pass_manager_pipeline_integration() {
    let mut builder = CircuitBuilder::new("pm_pipeline");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.rx(q[0], Angle::from_radians(0.25)).unwrap();
    builder.rx(q[0], Angle::from_radians(0.75)).unwrap();
    builder.rz(q[1], Angle::from_radians(PI)).unwrap();
    builder.rz(q[1], Angle::from_radians(PI)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 4);

    let mut pm = PassManager::new();
    pm.add_pass(RotationMergePass::new());

    let report = pm.run(&mut circuit).unwrap();
    assert!(report.is_success());
    assert_eq!(report.total_passes_executed, 1);
    assert_eq!(report.total_passes_modified, 1);
    // Rx merged into 1 (reduction of 1), Rz cancelled into 0 (reduction of 2) -> net delta = -3
    assert_eq!(report.total_delta_operations, -3);
    assert_eq!(circuit.num_operations(), 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_telemetry_statistics_reporting() {
    let mut builder = CircuitBuilder::new("stats_test");
    let q = builder.add_quantum_register("q", 2).unwrap();
    // 1 fuse: Rx(0.2) + Rx(0.3) -> Rx(0.5)
    builder.rx(q[0], Angle::from_radians(0.2)).unwrap();
    builder.rx(q[0], Angle::from_radians(0.3)).unwrap();
    // 1 exact zero: Ry(2.5pi) + Ry(1.5pi) -> 0 mod 4pi
    builder.ry(q[1], Angle::from_radians(2.5 * PI)).unwrap();
    builder.ry(q[1], Angle::from_radians(1.5 * PI)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = RotationMergePass::new();
    let mut ctx = PassContext::default();
    pass.run(&mut circuit, &mut ctx).unwrap();

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.rotations_merged, 2);
    assert_eq!(stats.non_zero_rotations_fused, 1);
    assert_eq!(stats.exact_zero_identities_eliminated, 1);
    assert_eq!(stats.operations_removed, 4);
    assert_eq!(stats.operations_added, 1);
    assert_eq!(stats.net_gate_reduction(), 3);
}

// ============================================================================
// 10. Property-Based Testing with Proptest
// ============================================================================

proptest! {
    #[test]
    fn prop_rotation_angle_four_pi_exact(k in -50i64..50i64, offset in -1e-13f64..1e-13f64) {
        let angle = (k as f64) * FOUR_PI + offset;
        let norm = normalize_rotation_angle(angle, 1e-12, RotationNormalizationMode::TwoPiWithPhase);
        prop_assert!(norm.is_exact_zero());
        prop_assert_eq!(norm.global_phase_correction, 0.0);
    }

    #[test]
    fn prop_rotation_angle_two_pi_phase(k in -50i64..50i64, offset in -1e-13f64..1e-13f64) {
        let angle = ((2 * k + 1) as f64) * TWO_PI + offset;
        let norm = normalize_rotation_angle(angle, 1e-12, RotationNormalizationMode::TwoPiWithPhase);
        prop_assert!(norm.is_phase_zero());
        prop_assert!((norm.global_phase_correction - PI).abs() < 1e-12);
    }

    #[test]
    fn prop_rotation_merge_random_numeric_rx(t1 in -6.0f64..6.0f64, t2 in -6.0f64..6.0f64) {
        let mut builder = CircuitBuilder::new("prop_rx");
        let q = builder.add_quantum_register("q", 1).unwrap();
        builder.rx(q[0], Angle::from_radians(t1)).unwrap();
        builder.rx(q[0], Angle::from_radians(t2)).unwrap();
        let mut circuit = builder.finish().unwrap();

        let orig = circuit.clone();

        let mut pass = RotationMergePass::new();
        let mut ctx = PassContext::default();
        let _ = pass.run(&mut circuit, &mut ctx).unwrap();

        let verdict = check_equivalence(&orig, &circuit).unwrap();
        prop_assert!(verdict.is_equivalent);
    }

    #[test]
    fn prop_rotation_merge_random_numeric_ry(t1 in -6.0f64..6.0f64, t2 in -6.0f64..6.0f64) {
        let mut builder = CircuitBuilder::new("prop_ry");
        let q = builder.add_quantum_register("q", 1).unwrap();
        builder.ry(q[0], Angle::from_radians(t1)).unwrap();
        builder.ry(q[0], Angle::from_radians(t2)).unwrap();
        let mut circuit = builder.finish().unwrap();

        let orig = circuit.clone();

        let mut pass = RotationMergePass::new();
        let mut ctx = PassContext::default();
        let _ = pass.run(&mut circuit, &mut ctx).unwrap();

        let verdict = check_equivalence(&orig, &circuit).unwrap();
        prop_assert!(verdict.is_equivalent);
    }

    #[test]
    fn prop_rotation_merge_random_numeric_rz(t1 in -6.0f64..6.0f64, t2 in -6.0f64..6.0f64) {
        let mut builder = CircuitBuilder::new("prop_rz");
        let q = builder.add_quantum_register("q", 1).unwrap();
        builder.rz(q[0], Angle::from_radians(t1)).unwrap();
        builder.rz(q[0], Angle::from_radians(t2)).unwrap();
        let mut circuit = builder.finish().unwrap();

        let orig = circuit.clone();

        let mut pass = RotationMergePass::new();
        let mut ctx = PassContext::default();
        let _ = pass.run(&mut circuit, &mut ctx).unwrap();

        let verdict = check_equivalence(&orig, &circuit).unwrap();
        prop_assert!(verdict.is_equivalent);
    }
}

// ============================================================================
// 11. Randomized Circuit Equivalence Testing (Exit Gate Proof)
// ============================================================================

#[test]
fn test_randomized_circuits_with_injected_rotations_pass_unitary_equivalence() {
    let num_trials = 30;

    for trial in 0..num_trials {
        let num_qubits = 2 + (trial % 3); // 2, 3, or 4 qubits
        let base_depth = 4 + (trial % 4);
        let seed = 888_000 + trial as u64;

        let mut rng = SplitMix64::new(seed);
        let base_circuit = generate_random_unitary_circuit(
            format!("rand_rot_{trial}"),
            num_qubits,
            base_depth,
            &mut rng,
        )
        .unwrap();

        // Build augmented circuit with injected adjacent same-axis rotations
        let mut aug_circuit = base_circuit.clone();

        // Inject Rx pair on q[0]
        let angle1 = (trial as f64 * 0.7).sin() * 2.0;
        let angle2 = (trial as f64 * 1.3).cos() * 2.0;
        aug_circuit
            .add_operation(Operation::gate(
                &Gate::Rx(Angle::from_radians(angle1)),
                vec![QubitId(0)],
            ))
            .unwrap();
        aug_circuit
            .add_operation(Operation::gate(
                &Gate::Rx(Angle::from_radians(angle2)),
                vec![QubitId(0)],
            ))
            .unwrap();

        // Inject Ry pair on q[1]
        let angle3 = (trial as f64 * 0.4).cos() * 3.0;
        let angle4 = (trial as f64 * 0.9).sin() * 3.0;
        aug_circuit
            .add_operation(Operation::gate(
                &Gate::Ry(Angle::from_radians(angle3)),
                vec![QubitId(1)],
            ))
            .unwrap();
        aug_circuit
            .add_operation(Operation::gate(
                &Gate::Ry(Angle::from_radians(angle4)),
                vec![QubitId(1)],
            ))
            .unwrap();

        let orig_aug = aug_circuit.clone();

        let mut pass = RotationMergePass::new();
        let mut ctx = PassContext::default();
        let res = pass.run(&mut aug_circuit, &mut ctx).unwrap();

        assert!(res.modified);
        let stats = pass.last_stats().unwrap();
        assert!(stats.rotations_merged >= 2);

        // Unitary equivalence check against original unoptimized circuit
        let verdict = check_equivalence(&orig_aug, &aug_circuit).unwrap();
        assert!(
            verdict.is_equivalent,
            "Trial {trial} failed unitary equivalence check: {verdict:?}"
        );
    }
}
