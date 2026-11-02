//! Comprehensive integration, unit, and property tests for Identity-Gate Elimination (Phase 28).
//!
//! Validates:
//! 1. Detection and elimination of explicit identity gates (`id`, `i`).
//! 2. Proven zero rotations ($R_x(0), R_y(0), R_z(0)$) eliminated with zero global phase adjustment.
//! 3. Continuous rotation $4\pi$ periodicity ($\pm 4\pi, 8\pi$) eliminated with zero global phase adjustment.
//! 4. Continuous rotation $2\pi$ periodicity ($\pm 2\pi, 6\pi$) eliminated with explicit $+\pi$ global phase correction.
//! 5. Configurable refusal of $2\pi$ rotations when `eliminate_two_pi_rotations` is disabled.
//! 6. Strict preservation of unresolved symbolic rotations ($R_x(\theta)$).
//! 7. Evaluation of bound symbolic rotations in `symbol_env`.
//! 8. Strict preservation of non-removable operations: barriers, resets, measurements, classical compute.
//! 9. Automatic wire dependency reconnection and QccIR DAG invariant validation.
//! 10. PassManager integration, execution tracing, and telemetry statistics.
//! 11. Property testing with Proptest for rotation angle classification and tolerance bounds.
//! 12. Randomized circuit equivalence testing: injecting identities into Haar-random circuits, eliminating them,
//!     and proving unitary equivalence using [`check_equivalence`].
//! 13. Behavioral sampling equivalence for non-unitary circuits containing identities.

use std::collections::HashMap;
use std::f64::consts::PI;

use proptest::prelude::*;
use qcc::builder::CircuitBuilder;
use qcc::ir::circuit::Circuit;
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::QubitId;
use qcc::opt::context::PassContext;
use qcc::opt::pass::CompilerPass;
use qcc::opt::pass_manager::PassManager;
use qcc::optimizer::passes::identity::*;
use qcc::quantum::gate::Gate;
use qcc::quantum::parameter::Angle;
use qcc::simulator::sample_circuit;
use qcc::simulator::sampling::SplitMix64;
use qcc::testkit::random_circuit::generate_random_unitary_circuit;
use qcc::verification::check_equivalence;

// ============================================================================
// 1. Explicit Identity Gates (`id`, `i`)
// ============================================================================

#[test]
fn test_explicit_id_gate_elimination_single_qubit() {
    let mut builder = CircuitBuilder::new("explicit_id_single");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let id_op = builder.id(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 1);
    assert!(circuit.graph.contains_operation(id_op));

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert!(!circuit.graph.contains_operation(id_op));
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.operations_examined, 1);
    assert_eq!(stats.explicit_identities_removed, 1);
    assert_eq!(stats.operations_preserved, 0);
    assert_eq!(stats.global_phase_adjustments, 0);
}

#[test]
fn test_explicit_id_gate_between_hadamards() {
    // Circuit: H(0) -> id(0) -> H(0)
    // After elimination: H(0) -> H(0), which is unitarily identity overall
    let mut builder = CircuitBuilder::new("h_id_h");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let h1 = builder.h(q[0]).unwrap();
    let id_op = builder.id(q[0]).unwrap();
    let h2 = builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 3);

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 2);
    assert!(!circuit.graph.contains_operation(id_op));
    assert!(circuit.graph.contains_operation(h1));
    assert!(circuit.graph.contains_operation(h2));
    assert!(circuit.validate().is_ok());

    // Verify wire dependency connects h1 directly to h2
    let preds = circuit.graph.predecessors(h2);
    assert!(preds.iter().any(|(op, _)| *op == h1));
}

#[test]
fn test_explicit_id_gate_alternate_name_i() {
    let mut builder = CircuitBuilder::new("id_alias_i");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.x(q[0]).unwrap();
    builder.custom_gate("i", &[q[0]], Vec::new()).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 2);

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.explicit_identities_removed, 1);
    assert_eq!(stats.operations_preserved, 1);
}

#[test]
fn test_multiple_id_gates_across_multiple_qubits() {
    let mut builder = CircuitBuilder::new("multi_qubit_id");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.id(q[0]).unwrap();
    builder.h(q[1]).unwrap();
    builder.id(q[1]).unwrap();
    builder.cx(q[1], q[2]).unwrap();
    builder.id(q[2]).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 5);

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 2); // only H and CX remain
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.explicit_identities_removed, 3);
    assert_eq!(stats.operations_preserved, 2);
}

// ============================================================================
// 2. Proven Zero Rotations ($R_x(0), R_y(0), R_z(0)$)
// ============================================================================

#[test]
fn test_exact_zero_rotations_rx_ry_rz() {
    let mut builder = CircuitBuilder::new("zero_rotations");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.rx(q[0], 0.0).unwrap();
    builder.ry(q[1], 0.0).unwrap();
    builder.rz(q[0], 0.0).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 3);

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert_eq!(circuit.global_phase(), &ParameterExpr::constant(0.0));
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.operations_examined, 3);
    assert_eq!(stats.exact_zero_rotations_removed, 3);
    assert_eq!(stats.global_phase_adjustments, 0);
}

#[test]
fn test_near_zero_rotation_within_tolerance() {
    let mut builder = CircuitBuilder::new("near_zero");
    let q = builder.add_quantum_register("q", 1).unwrap();
    // 1e-14 is well within default tolerance 1e-12
    builder.rx(q[0], 1.0e-14).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
}

#[test]
fn test_rotation_four_pi_periodicity_exact() {
    // R(4π) = I, R(-4π) = I, R(8π) = I (all exact identities, 0 global phase difference)
    let mut builder = CircuitBuilder::new("four_pi_rotations");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], 4.0 * PI).unwrap();
    builder.ry(q[0], -4.0 * PI).unwrap();
    builder.rz(q[0], 8.0 * PI).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 3);

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert_eq!(circuit.global_phase(), &ParameterExpr::constant(0.0));

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.exact_zero_rotations_removed, 3);
    assert_eq!(stats.global_phase_adjustments, 0);
}

#[test]
fn test_rotation_two_pi_periodicity_with_global_phase() {
    // R(2π) = -I = e^{iπ} I
    // Removing R(2π) requires adding π to the circuit global phase
    let mut builder = CircuitBuilder::new("two_pi_rotation");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], 2.0 * PI).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.global_phase(), &ParameterExpr::constant(0.0));

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    // Global phase should now be π
    let phase_eval = circuit.global_phase().eval(&HashMap::new()).unwrap();
    assert!((phase_eval - PI).abs() < 1e-12);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.phase_zero_rotations_removed, 1);
    assert_eq!(stats.global_phase_adjustments, 1);
}

#[test]
fn test_rotation_negative_two_pi_periodicity_with_global_phase() {
    // R(-2π) = -I = e^{iπ} I
    let mut builder = CircuitBuilder::new("neg_two_pi_rotation");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.ry(q[0], -2.0 * PI).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let phase_eval = circuit.global_phase().eval(&HashMap::new()).unwrap();
    assert!((phase_eval - PI).abs() < 1e-12);
}

#[test]
fn test_rotation_six_pi_periodicity_with_global_phase() {
    // 6π = 4π + 2π ≡ 2π (mod 4π) => carries phase π
    let mut builder = CircuitBuilder::new("six_pi_rotation");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rz(q[0], 6.0 * PI).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let phase_eval = circuit.global_phase().eval(&HashMap::new()).unwrap();
    assert!((phase_eval - PI).abs() < 1e-12);
}

#[test]
fn test_two_pi_rotation_elimination_disabled_in_exact_mode() {
    let mut builder = CircuitBuilder::new("exact_only_two_pi");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], 2.0 * PI).unwrap();
    builder.ry(q[0], 0.0).unwrap(); // should be removed
    let mut circuit = builder.finish().unwrap();

    let mut pass = IdentityEliminationPass::exact_only();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    // Ry(0) was removed, Rx(2π) was preserved
    assert_eq!(circuit.num_operations(), 1);
    assert_eq!(circuit.global_phase(), &ParameterExpr::constant(0.0));

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.exact_zero_rotations_removed, 1);
    assert_eq!(stats.phase_zero_rotations_removed, 0);
    assert_eq!(stats.operations_preserved, 1);
}

#[test]
fn test_phase_gate_p_zero_elimination() {
    let mut builder = CircuitBuilder::new("p_gate_zero");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder
        .custom_gate("p", &[q[0]], vec![ParameterExpr::constant(0.0)])
        .unwrap();
    builder
        .custom_gate("u1", &[q[0]], vec![ParameterExpr::constant(2.0 * PI)])
        .unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert_eq!(circuit.global_phase(), &ParameterExpr::constant(0.0));
}

// ============================================================================
// 3. Preservation of Non-Identities and Unresolved Symbols
// ============================================================================

#[test]
fn test_preserve_non_zero_rotations() {
    let mut builder = CircuitBuilder::new("non_zero_rotations");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], 0.1).unwrap();
    builder.ry(q[0], PI / 2.0).unwrap();
    builder.rz(q[0], PI).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.operations_examined, 3);
    assert_eq!(stats.total_removed(), 0);
    assert_eq!(stats.operations_preserved, 3);
}

#[test]
fn test_preserve_unresolved_symbolic_rotations() {
    let mut builder = CircuitBuilder::new("unresolved_symbols");
    let q = builder.add_quantum_register("q", 1).unwrap();
    // Symbolic angle theta: NEVER treat as zero
    builder.rx(q[0], ParameterExpr::symbol("theta")).unwrap();
    builder
        .ry(
            q[0],
            ParameterExpr::Add(
                Box::new(ParameterExpr::symbol("alpha")),
                Box::new(ParameterExpr::constant(0.0)),
            ),
        )
        .unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 2);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.operations_examined, 2);
    assert_eq!(stats.total_removed(), 0);
    assert_eq!(stats.operations_preserved, 2);
}

#[test]
fn test_eliminate_bound_symbolic_rotations_via_symbol_env() {
    let mut builder = CircuitBuilder::new("bound_symbols");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], ParameterExpr::symbol("theta")).unwrap();
    builder.ry(q[0], ParameterExpr::symbol("phi")).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut env = HashMap::new();
    env.insert("theta".to_string(), 0.0);
    env.insert("phi".to_string(), 0.5); // non-zero, should be preserved

    let mut pass = IdentityEliminationPass::new().with_symbol_env(env);
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1); // only Ry(phi=0.5) preserved

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.exact_zero_rotations_removed, 1);
    assert_eq!(stats.operations_preserved, 1);
}

#[test]
fn test_preserve_standard_fixed_gates() {
    let mut builder = CircuitBuilder::new("fixed_gates");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    builder.y(q[0]).unwrap();
    builder.z(q[0]).unwrap();
    builder.s(q[0]).unwrap();
    builder.sdg(q[0]).unwrap();
    builder.t(q[0]).unwrap();
    builder.tdg(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    builder.swap(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig_ops = circuit.num_operations();
    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), orig_ops);
    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.total_removed(), 0);
    assert_eq!(stats.operations_preserved, orig_ops);
}

// ============================================================================
// 4. Preservation of Non-Removable Operations (Barriers, Resets, Measurements)
// ============================================================================

#[test]
fn test_preserve_barrier_between_identities() {
    let mut builder = CircuitBuilder::new("barrier_preservation");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.id(q[0]).unwrap();
    let barrier_id = builder.barrier(&[q[0], q[1]]).unwrap();
    builder.rx(q[1], 0.0).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 3);

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);
    assert!(circuit.graph.contains_operation(barrier_id));
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.total_removed(), 2);
    assert_eq!(stats.operations_preserved, 1);
}

#[test]
fn test_preserve_reset_and_measurement() {
    let mut builder = CircuitBuilder::new("non_unitary_preservation");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();

    builder.id(q[0]).unwrap();
    let reset_id = builder.reset(q[0]).unwrap();
    builder.rx(q[0], 0.0).unwrap();
    builder.h(q[0]).unwrap();
    let meas_id = builder.measure(q[0], c[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 5);

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 3); // Reset, H, Measure remain
    assert!(circuit.graph.contains_operation(reset_id));
    assert!(circuit.graph.contains_operation(meas_id));
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.total_removed(), 2);
    assert_eq!(stats.operations_preserved, 3);
}

#[test]
fn test_preserve_classical_computation() {
    let mut builder = CircuitBuilder::new("classical_preservation");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();

    builder.id(q[0]).unwrap();
    let class_id = builder.classical("parity_check", &[c[0]], &[c[1]]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);
    assert!(circuit.graph.contains_operation(class_id));
    assert!(circuit.validate().is_ok());
}

// ============================================================================
// 5. Wire Dependency Repair & DAG Invariants
// ============================================================================

#[test]
fn test_wire_dependency_reconnection_after_removal() {
    // Sequence on qubit 0: X -> id -> Y -> id -> Z
    // Expected after: X -> Y -> Z with direct causal dependency edges
    let mut builder = CircuitBuilder::new("wire_reconnect");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let x_id = builder.x(q[0]).unwrap();
    let id1 = builder.id(q[0]).unwrap();
    let y_id = builder.y(q[0]).unwrap();
    let id2 = builder.id(q[0]).unwrap();
    let z_id = builder.z(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 3);
    assert!(!circuit.graph.contains_operation(id1));
    assert!(!circuit.graph.contains_operation(id2));

    // Check DAG edges
    let preds_y = circuit.graph.predecessors(y_id);
    assert!(preds_y.iter().any(|(op, _)| *op == x_id));

    let preds_z = circuit.graph.predecessors(z_id);
    assert!(preds_z.iter().any(|(op, _)| *op == y_id));

    assert!(circuit.validate().is_ok());
}

#[test]
fn test_all_identities_reduces_to_empty_circuit_preserving_registers() {
    let mut builder = CircuitBuilder::new("all_identities");
    let q = builder.add_quantum_register("alice", 2).unwrap();
    let _c = builder.add_classical_register("bob", 3).unwrap();
    builder.set_metadata("author", "qcc");
    builder.id(q[0]).unwrap();
    builder.id(q[1]).unwrap();
    builder.rx(q[0], 0.0).unwrap();
    builder.ry(q[1], 4.0 * PI).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert_eq!(circuit.num_qubits(), 2);
    assert_eq!(circuit.num_classical_bits(), 3);
    assert_eq!(circuit.quantum_registers[0].name, "alice");
    assert_eq!(circuit.classical_registers[0].name, "bob");
    assert_eq!(
        circuit.metadata.get("author").map(|s| s.as_str()),
        Some("qcc")
    );
    assert!(circuit.validate().is_ok());
}

// ============================================================================
// 6. PassManager Integration
// ============================================================================

#[test]
fn test_pass_manager_pipeline_execution() {
    let mut builder = CircuitBuilder::new("pm_pipeline");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.id(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    builder.rz(q[0], 0.0).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pm = PassManager::new();
    pm.add_pass(IdentityEliminationPass::new());

    let report = pm
        .run(&mut circuit)
        .expect("pass manager run should succeed");
    assert!(report.is_success());
    assert_eq!(report.total_passes_executed, 1);
    assert_eq!(report.total_passes_modified, 1);
    assert_eq!(report.total_delta_operations, -2);
    assert_eq!(circuit.num_operations(), 2);
}

// ============================================================================
// 7. Property-Based Testing with Proptest
// ============================================================================

proptest! {
    #[test]
    fn prop_classify_rotation_angle_four_pi_exact(k in -100i64..100i64, offset in -1e-13f64..1e-13f64) {
        let angle = (k as f64) * 4.0 * PI + offset;
        let kind = classify_rotation_angle(angle, DEFAULT_ANGLE_TOLERANCE);
        prop_assert_eq!(kind, RotationIdentityKind::ExactZero);
    }

    #[test]
    fn prop_classify_rotation_angle_two_pi_phase(k in -100i64..100i64, offset in -1e-13f64..1e-13f64) {
        // Odd multiples of 2pi: (2k + 1) * 2pi
        let angle = ((2 * k + 1) as f64) * 2.0 * PI + offset;
        let kind = classify_rotation_angle(angle, DEFAULT_ANGLE_TOLERANCE);
        prop_assert_eq!(kind, RotationIdentityKind::PhaseZero { phase_adjustment: PI });
    }

    #[test]
    fn prop_classify_rotation_angle_non_identity(angle in 0.05f64..6.2f64) {
        // Exclude neighborhood of 2pi
        let dist_2pi = (angle - 2.0 * PI).abs();
        if dist_2pi > 0.05 {
            let kind = classify_rotation_angle(angle, DEFAULT_ANGLE_TOLERANCE);
            prop_assert_eq!(kind, RotationIdentityKind::NotIdentity);
        }
    }
}

// ============================================================================
// 8. Randomized Circuit Equivalence Testing (Exit Gate Proof)
// ============================================================================

#[test]
fn test_randomized_circuits_with_identity_insertions_pass_unitary_equivalence() {
    let mut rng = SplitMix64::new(0xDEADBEEFCAFE1234);

    for trial in 0..10 {
        let num_qubits = 2 + (trial % 2); // 2 or 3 qubits
        let depth = 6 + trial * 2;

        // Generate base random unitary circuit
        let base_circuit = generate_random_unitary_circuit(
            format!("base_trial_{trial}"),
            num_qubits,
            depth,
            &mut rng,
        )
        .expect("random circuit generation should succeed");

        let orig_ops = base_circuit.num_operations();

        // Build a mutated copy with injected identities
        let mut injected_circuit = Circuit::new(format!("injected_trial_{trial}"));
        injected_circuit
            .add_quantum_register("q", num_qubits)
            .unwrap();

        // Inject identities at various stages
        let nodes = base_circuit.graph.ordered_nodes().unwrap();
        let mut injected_count = 0;

        for (i, node) in nodes.iter().enumerate() {
            injected_circuit
                .add_operation(node.operation.clone())
                .unwrap();

            // Periodically inject identity operations
            if i % 2 == 0 {
                let target_q = i % num_qubits;
                match i % 4 {
                    0 => {
                        // Explicit id gate
                        injected_circuit
                            .add_operation(Operation::custom_gate(
                                "id",
                                vec![QubitId(target_q)],
                                Vec::new(),
                            ))
                            .unwrap();
                        injected_count += 1;
                    }
                    1 => {
                        // Rx(0)
                        injected_circuit
                            .add_operation(Operation::gate(
                                &Gate::Rx(Angle::from_radians(0.0)),
                                vec![QubitId(target_q)],
                            ))
                            .unwrap();
                        injected_count += 1;
                    }
                    2 => {
                        // Ry(4π)
                        injected_circuit
                            .add_operation(Operation::gate(
                                &Gate::Ry(Angle::from_radians(4.0 * PI)),
                                vec![QubitId(target_q)],
                            ))
                            .unwrap();
                        injected_count += 1;
                    }
                    3 => {
                        // Rz(2π) - with global phase compensation
                        injected_circuit
                            .add_operation(Operation::gate(
                                &Gate::Rz(Angle::from_radians(2.0 * PI)),
                                vec![QubitId(target_q)],
                            ))
                            .unwrap();
                        injected_count += 1;
                    }
                    _ => unreachable!(),
                }
            }
        }

        assert_eq!(injected_circuit.num_operations(), orig_ops + injected_count);

        // Run IdentityEliminationPass on the injected circuit
        let mut optimized_circuit = injected_circuit.clone();
        let mut pass = IdentityEliminationPass::new();
        let mut ctx = PassContext::default();
        let pass_res = pass
            .run(&mut optimized_circuit, &mut ctx)
            .expect("identity pass should succeed");

        assert!(pass_res.modified);
        assert_eq!(optimized_circuit.num_operations(), orig_ops);

        let stats = pass.last_stats().unwrap();
        assert_eq!(stats.total_removed(), injected_count);

        // Assert Unitary Equivalence between base and optimized circuit
        let verdict = check_equivalence(&base_circuit, &optimized_circuit)
            .expect("unitary equivalence check should succeed");

        assert!(
            verdict.is_equivalent,
            "trial {trial}: optimized circuit is not unitarily equivalent to base circuit! max diff = {:.2e}",
            verdict.max_amplitude_difference
        );
        assert!(verdict.max_amplitude_difference < 1e-10);

        // Also assert equivalence between injected (unoptimized) and optimized circuit
        let verdict_injected = check_equivalence(&injected_circuit, &optimized_circuit)
            .expect("unitary equivalence check should succeed");

        assert!(
            verdict_injected.is_equivalent,
            "trial {trial}: optimized circuit is not equivalent to injected circuit! max diff = {:.2e}",
            verdict_injected.max_amplitude_difference
        );
    }
}

// ============================================================================
// 9. Behavioral Sampling Equivalence on Non-Unitary Circuit
// ============================================================================

#[test]
fn test_non_unitary_bell_circuit_with_identities_sampling_equivalence() {
    // Circuit: Prepare Bell state (|00> + |11>) / sqrt(2) with injected id gates and measurements
    let mut builder = CircuitBuilder::new("bell_sampling");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();

    builder.h(q[0]).unwrap();
    builder.id(q[0]).unwrap(); // injected id
    builder.cx(q[0], q[1]).unwrap();
    builder.rx(q[1], 0.0).unwrap(); // injected zero rotation
    builder.measure(q[0], c[0]).unwrap();
    builder.measure(q[1], c[1]).unwrap();

    let mut circuit = builder.finish().unwrap();
    assert_eq!(circuit.num_operations(), 6);

    // Sample before optimization
    let mut rng = SplitMix64::new(42);
    let (exec_before, _) = sample_circuit(&circuit, 1000, &mut rng).unwrap();
    let counts_before = exec_before.counts(qcc::quantum::ClassicalBitOrder::MsbFirst);

    // Optimize
    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 4); // H, CX, Measure, Measure
    assert!(circuit.validate().is_ok());

    // Sample after optimization with same seed
    let mut rng_after = SplitMix64::new(42);
    let (exec_after, _) = sample_circuit(&circuit, 1000, &mut rng_after).unwrap();
    let counts_after = exec_after.counts(qcc::quantum::ClassicalBitOrder::MsbFirst);

    // Verify deterministic reproducibility and Bell state statistics
    assert_eq!(counts_before, counts_after);

    // Only "00" and "11" should appear
    assert!(counts_after.get("00").copied().unwrap_or(0) > 400);
    assert!(counts_after.get("11").copied().unwrap_or(0) > 400);
    assert_eq!(counts_after.get("01").copied().unwrap_or(0), 0);
    assert_eq!(counts_after.get("10").copied().unwrap_or(0), 0);
}

// ============================================================================
// 10. OpenQASM Lowering and Pass Integration
// ============================================================================

#[test]
fn test_openqasm_id_gate_lowering_and_optimization() {
    let qasm = r#"
        OPENQASM 3.0;
        include "stdgates.inc";
        qubit[2] q;
        h q[0];
        id q[0];
        cx q[0], q[1];
        id q[1];
    "#;
    let res = qcc::frontend::lowering::lower_qasm_str(qasm, Some("qasm_id"), None);
    let mut circuit = res.into_result().expect("lowering should succeed");
    assert_eq!(circuit.num_operations(), 4);

    let mut pass = IdentityEliminationPass::new();
    let mut ctx = PassContext::default();
    let opt_res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(opt_res.modified);
    assert_eq!(circuit.num_operations(), 2);
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.explicit_identities_removed, 2);
    assert_eq!(stats.operations_preserved, 2);
}
