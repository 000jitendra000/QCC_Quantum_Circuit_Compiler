//! Integration test suite for Phase 32: One-qubit gate fusion.
//!
//! Verifies:
//! - Consecutive single-qubit operations on the same wire fuse correctly.
//! - Discrete gates fuse and cancel identities ($H \to H = I$, $S \to S = Z$, $H \to X \to H = Z$).
//! - Matrix multiplication order sensitivity (left-multiplication $B \cdot A$).
//! - Global phase extraction and accumulation across single and multiple wires.
//! - Boundary conditions: `Barrier`, `Measure`, `Reset`, `Classical`, and multi-qubit gates ($CX$).
//! - Disjoint wire commutativity and independent fusion.
//! - Provenance tracking and transactional rollback.
//! - Phase 10 unitary equivalence check across all fused circuits using one common global phase.
//! - Exit Gate: Randomized one-qubit blocks pass common-phase equivalence.

use std::collections::HashMap;
use std::f64::consts::PI;

use qcc::builder::CircuitBuilder;
use qcc::ir::parameter::ParameterExpr;
use qcc::opt::context::PassContext;
use qcc::opt::pass::CompilerPass;
use qcc::optimizer::passes::one_qubit_fusion::OneQubitFusionPass;
use qcc::simulator::sampling::{SeededRng, SplitMix64};
use qcc::verification::check_equivalence;

// ============================================================================
// 1. Discrete Gate Sequences & Identity Cancellation
// ============================================================================

#[test]
fn test_fusion_h_h_eliminates_to_identity() {
    let mut builder = CircuitBuilder::new("h_h");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 2);

    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.windows_fused, 1);
    assert_eq!(stats.operations_removed, 2);
    assert_eq!(stats.operations_added, 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_fusion_x_x_eliminates_to_identity() {
    let mut builder = CircuitBuilder::new("x_x");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.x(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_fusion_s_sdg_eliminates_to_identity() {
    let mut builder = CircuitBuilder::new("s_sdg");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.s(q[0]).unwrap();
    builder.sdg(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_fusion_s_s_fuses_to_z() {
    let mut builder = CircuitBuilder::new("s_s");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.s(q[0]).unwrap();
    builder.s(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    // S * S = Z. In Euler ZYZ, Z = exp(i*pi/2) * Rz(pi), which simplifies to 1 gate (Rz(pi))
    assert_eq!(circuit.num_operations(), 1);
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.windows_fused, 1);
    assert_eq!(stats.operations_removed, 2);
    assert_eq!(stats.operations_added, 1);
    assert_eq!(stats.global_phase_adjustments, 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_fusion_t_four_times_fuses_to_z() {
    let mut builder = CircuitBuilder::new("t4");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.t(q[0]).unwrap();
    builder.t(q[0]).unwrap();
    builder.t(q[0]).unwrap();
    builder.t(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 4);

    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.windows_fused, 1);
    assert_eq!(stats.operations_removed, 4);
    assert_eq!(stats.operations_added, 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_fusion_h_x_h_fuses_to_z() {
    let mut builder = CircuitBuilder::new("h_x_h");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 3);

    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    // H X H = Z, which decomposes into 1 rotation (Rz(pi)) and +pi/2 phase
    assert_eq!(circuit.num_operations(), 1);
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.windows_fused, 1);
    assert_eq!(stats.operations_removed, 3);
    assert_eq!(stats.operations_added, 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_fusion_h_z_h_fuses_to_x() {
    let mut builder = CircuitBuilder::new("h_z_h");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.z(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 3);

    // In default ZYZ basis, X decomposes into 2 rotations: Ry(pi) and Rz(pi)
    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 2);
    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);

    // In ZXZ basis, X decomposes into a single rotation: Rx(pi)
    let mut c_zxz = orig.clone();
    let mut pass_zxz =
        OneQubitFusionPass::new().with_basis(qcc::decomposition::euler::EulerBasis::Zxz);
    let res_zxz = pass_zxz.run(&mut c_zxz, &mut ctx).unwrap();
    assert!(res_zxz.modified);
    assert_eq!(c_zxz.num_operations(), 1);
    let v_zxz = check_equivalence(&orig, &c_zxz).unwrap();
    assert!(v_zxz.is_equivalent);
}

#[test]
fn test_fusion_pauli_xyz_sequence() {
    // X * Y * Z = i * I
    let mut builder = CircuitBuilder::new("xyz");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.z(q[0]).unwrap();
    builder.y(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    // Total matrix is i * I = exp(i*pi/2) * I, so 0 gates and +pi/2 phase!
    assert_eq!(circuit.num_operations(), 0);
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.windows_fused, 1);
    assert_eq!(stats.operations_removed, 3);
    assert_eq!(stats.operations_added, 0);
    assert_eq!(stats.global_phase_adjustments, 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 2. Rotation Sequences & Fixed-Point Idempotence
// ============================================================================

#[test]
fn test_fusion_rx_rx_merges() {
    let mut builder = CircuitBuilder::new("rx_rx");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], 0.35).unwrap();
    builder.rx(q[0], 0.45).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_fusion_canonical_zyz_idempotent() {
    // Construct a circuit that is already a minimal canonical Euler Z-Y-Z sequence
    let mut builder = CircuitBuilder::new("canonical_zyz");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rz(q[0], 0.3).unwrap();
    builder.ry(q[0], 0.4).unwrap();
    builder.rz(q[0], 0.5).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    // Already in canonical Euler form -> should be skipped as no-op
    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.windows_examined, 1);
    assert_eq!(stats.windows_fused, 0);
    assert_eq!(stats.windows_rejected, 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_fusion_mixed_rotations_and_discrete_gates() {
    let mut builder = CircuitBuilder::new("mixed");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], 0.5).unwrap();
    builder.h(q[0]).unwrap();
    builder.rz(q[0], 0.8).unwrap();
    builder.s(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 4);

    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert!(circuit.num_operations() <= 3);
    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 3. Matrix Multiplication Order Sensitivity
// ============================================================================

#[test]
fn test_fusion_matrix_multiplication_order_sensitivity() {
    // X followed by H: operator is H * X
    let mut b1 = CircuitBuilder::new("x_then_h");
    let q1 = b1.add_quantum_register("q", 1).unwrap();
    b1.x(q1[0]).unwrap();
    b1.h(q1[0]).unwrap();
    let mut c_xh = b1.finish().unwrap();

    // H followed by X: operator is X * H != H * X
    let mut b2 = CircuitBuilder::new("h_then_x");
    let q2 = b2.add_quantum_register("q", 1).unwrap();
    b2.h(q2[0]).unwrap();
    b2.x(q2[0]).unwrap();
    let c_hx = b2.finish().unwrap();

    // Verify original circuits are NOT equivalent
    let v_orig = check_equivalence(&c_xh, &c_hx).unwrap();
    assert!(!v_orig.is_equivalent, "X*H must not equal H*X");

    // Fuse c_xh
    let orig_xh = c_xh.clone();
    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut c_xh, &mut ctx).unwrap();
    assert!(res.modified);

    // Fused c_xh must be equivalent to original X->H
    let v_fused = check_equivalence(&orig_xh, &c_xh).unwrap();
    assert!(v_fused.is_equivalent, "Fused X->H must equal original X->H");

    // Fused c_xh must NOT be equivalent to H->X
    let v_cross = check_equivalence(&c_xh, &c_hx).unwrap();
    assert!(
        !v_cross.is_equivalent,
        "Fused X->H must not equal original H->X"
    );
}

// ============================================================================
// 4. Global Phase Accumulation (Single & Multi-Wire)
// ============================================================================

#[test]
fn test_fusion_global_phase_accumulation_single_wire() {
    let mut builder = CircuitBuilder::new("phase_accum_single");
    let q = builder.add_quantum_register("q", 2).unwrap();
    // Window 1 on q0: H X H = Z (+pi/2 phase)
    builder.h(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    // Separator CX
    builder.cx(q[0], q[1]).unwrap();
    // Window 2 on q0: S S = Z (+pi/2 phase)
    builder.s(q[0]).unwrap();
    builder.s(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.windows_fused, 2);
    // Both windows contribute global phase (+pi/2 + pi/2 = pi)
    assert!(stats.global_phase_adjustments >= 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(
        verdict.is_equivalent,
        "Single-wire phase accumulation must pass equivalence"
    );
}

#[test]
fn test_fusion_global_phase_accumulation_multi_wire() {
    let mut builder = CircuitBuilder::new("phase_accum_multi");
    let q = builder.add_quantum_register("q", 2).unwrap();
    // Wire 0: H X H = Z (+pi/2 phase)
    builder.h(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    // Wire 1: S S = Z (+pi/2 phase)
    builder.s(q[1]).unwrap();
    builder.s(q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.windows_fused, 2);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(
        verdict.is_equivalent,
        "Multi-wire phase accumulation must pass equivalence"
    );
}

// ============================================================================
// 5. Boundary Conditions (Non-Crossing)
// ============================================================================

#[test]
fn test_fusion_does_not_cross_barrier() {
    let mut builder = CircuitBuilder::new("barrier_sep");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.barrier(&[q[0]]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 3);

    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    // Barrier prevents H and H from being in the same window
    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.windows_fused, 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_fusion_does_not_cross_two_qubit_cx() {
    let mut builder = CircuitBuilder::new("cx_sep");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 3);

    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    // CX on q0 prevents H and H from fusing
    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_fusion_does_not_cross_measure() {
    let mut builder = CircuitBuilder::new("measure_sep");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 3);

    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    // Measure terminates single-qubit windows
    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_fusion_does_not_cross_reset() {
    let mut builder = CircuitBuilder::new("reset_sep");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.reset(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 3);

    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    // Reset terminates single-qubit windows
    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_fusion_does_not_cross_unresolved_symbolic_rotation() {
    let mut builder = CircuitBuilder::new("symbolic_sep");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    // Unbound symbolic parameter "theta"
    builder
        .custom_gate("rx", &[q[0]], vec![ParameterExpr::symbol("theta")])
        .unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 3);

    let mut pass = OneQubitFusionPass::new(); // empty symbol env
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    // Unresolved symbol terminates the window
    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_fusion_evaluates_bound_symbolic_rotation() {
    let mut builder = CircuitBuilder::new("symbolic_bound");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder
        .custom_gate("rx", &[q[0]], vec![ParameterExpr::symbol("theta")])
        .unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut env = HashMap::new();
    env.insert("theta".to_string(), 0.0); // rx(0) = I -> H * I * H = H^2 = I

    let mut pass = OneQubitFusionPass::new().with_symbol_env(env.clone());
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    // When theta = 0, H * I * H = I -> 0 operations!
    assert_eq!(circuit.num_operations(), 0);

    let checker = qcc::verification::EquivalenceChecker::new().with_environment(env);
    let verdict = checker.check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 6. Disjoint Wire Independent Fusion
// ============================================================================

#[test]
fn test_fusion_disjoint_wires_independent() {
    let mut builder = CircuitBuilder::new("disjoint");
    let q = builder.add_quantum_register("q", 3).unwrap();
    // Interleaved operations on different wires
    builder.h(q[0]).unwrap();
    builder.x(q[1]).unwrap();
    builder.s(q[2]).unwrap();
    builder.h(q[0]).unwrap(); // H*H on q0 -> I
    builder.x(q[1]).unwrap(); // X*X on q1 -> I
    builder.sdg(q[2]).unwrap(); // S*Sdg on q2 -> I
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 6);

    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.windows_fused, 3);
    assert_eq!(stats.operations_removed, 6);
    assert_eq!(stats.operations_added, 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 7. Provenance & Configuration Constraints
// ============================================================================

#[test]
fn test_fusion_provenance_tracking() {
    let mut builder = CircuitBuilder::new("provenance");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], 0.3).unwrap();
    builder.ry(q[0], 0.4).unwrap();
    let mut circuit = builder.finish().unwrap();

    let op_ids: Vec<_> = circuit
        .graph
        .ordered_nodes()
        .unwrap()
        .iter()
        .map(|n| n.id)
        .collect();

    let mut pass = OneQubitFusionPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    for node in circuit.graph.ordered_nodes().unwrap() {
        assert_eq!(node.provenance, op_ids);
    }
}

#[test]
fn test_fusion_min_window_size_config() {
    let mut builder = CircuitBuilder::new("min_window");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    // With min_window_size = 3, a 2-gate sequence is not fused
    let mut pass = OneQubitFusionPass::new().with_min_window_size(3);
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 2);
}

#[test]
fn test_fusion_max_window_size_config() {
    let mut builder = CircuitBuilder::new("max_window");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 4);

    // Limit window size to 2
    let mut pass = OneQubitFusionPass::new().with_max_window_size(Some(2));
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.windows_fused, 2); // fused as two windows of 2
    assert_eq!(stats.operations_removed, 4);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_fusion_allow_gate_count_increase_false() {
    let mut builder = CircuitBuilder::new("disallow_increase");
    let q = builder.add_quantum_register("q", 1).unwrap();
    // X then H decomposes to 3 Euler rotations
    builder.x(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = OneQubitFusionPass::new().with_allow_gate_count_increase(false);
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    // Should be rejected because 3 > 2
    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 2);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.windows_rejected, 1);
}

// ============================================================================
// 8. Randomized Fused Blocks (EXIT GATE PROOF)
// ============================================================================

#[test]
fn test_random_one_qubit_blocks_exit_gate() {
    let mut rng = SplitMix64::new(42);

    for run in 0..50 {
        let block_len = 2 + (rng.next_u64() % 7) as usize; // 2 to 8 gates
        let mut builder = CircuitBuilder::new(format!("rand_block_{run}"));
        let q = builder.add_quantum_register("q", 2).unwrap();

        let target_wire = (rng.next_u64() % 2) as usize;

        for _ in 0..block_len {
            let gate_type = rng.next_u64() % 10;
            match gate_type {
                0 => {
                    builder.h(q[target_wire]).unwrap();
                }
                1 => {
                    builder.x(q[target_wire]).unwrap();
                }
                2 => {
                    builder.y(q[target_wire]).unwrap();
                }
                3 => {
                    builder.z(q[target_wire]).unwrap();
                }
                4 => {
                    builder.s(q[target_wire]).unwrap();
                }
                5 => {
                    builder.sdg(q[target_wire]).unwrap();
                }
                6 => {
                    builder.t(q[target_wire]).unwrap();
                }
                7 => {
                    let angle = (rng.next_f64() * 4.0 - 2.0) * PI;
                    builder.rx(q[target_wire], angle).unwrap();
                }
                8 => {
                    let angle = (rng.next_f64() * 4.0 - 2.0) * PI;
                    builder.ry(q[target_wire], angle).unwrap();
                }
                9 => {
                    let angle = (rng.next_f64() * 4.0 - 2.0) * PI;
                    builder.rz(q[target_wire], angle).unwrap();
                }
                _ => unreachable!(),
            }
        }

        let mut circuit = builder.finish().unwrap();
        let orig = circuit.clone();

        let mut pass = OneQubitFusionPass::new();
        let mut ctx = PassContext::default();
        let _res = pass.run(&mut circuit, &mut ctx).unwrap();

        assert!(circuit.validate().is_ok());

        let verdict = check_equivalence(&orig, &circuit).unwrap_or_else(|err| {
            panic!("Equivalence check failed on random run {run}: {err}");
        });

        assert!(
            verdict.is_equivalent,
            "Random block {run} (length {block_len}) failed equivalence: max_diff = {:.2e}",
            verdict.max_amplitude_difference
        );
    }
}
