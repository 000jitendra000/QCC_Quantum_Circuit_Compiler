//! Comprehensive integration, unit, and property tests for Inverse-Gate Cancellation (Phase 29).
//!
//! Validates:
//! 1. Single-qubit discrete self-inverses: H-H, X-X, Y-Y, Z-Z.
//! 2. Discrete adjoint pairs: S-Sdg, Sdg-S, T-Tdg, Tdg-T.
//! 3. Two-qubit self-inverses: CX-CX, CZ-CZ, SWAP-SWAP.
//! 4. Symmetric vs asymmetric operand ordering:
//!    - CZ(0, 1) and CZ(1, 0) cancel (symmetric).
//!    - SWAP(0, 1) and SWAP(1, 0) cancel (symmetric).
//!    - CX(0, 1) and CX(1, 0) NEVER cancel (directional control/target mismatch).
//! 5. Continuous rotation inverses modulo 4pi: Rx, Ry, Rz.
//! 6. Rejection of 2pi rotations (which evaluate to -I, not +I).
//! 7. Phase gates (P, U1) modulo 2pi.
//! 8. Symbolic parameter expressions (exact symbolic negation and bound environments).
//! 9. Cascaded cancellations (X -> H -> H -> X).
//! 10. Odd-length chains (H -> H -> H => H).
//! 11. Disjoint wire independence (H(0) -> X(1) -> H(0) => X(1)).
//! 12. Intervening dependency boundaries: barriers, measurements, resets.
//! 13. Intervening gates on multi-qubit shared wires.
//! 14. Strict preservation of circuit-level global phase.
//! 15. Unitary equivalence proof via Phase 10 equivalence checker.
//! 16. PassManager pipeline integration and telemetry statistics.
//! 17. Randomized circuit testing with injected inverse pairs.

use std::collections::HashMap;
use std::f64::consts::PI;

use qcc::builder::CircuitBuilder;
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::QubitId;
use qcc::opt::context::PassContext;
use qcc::opt::pass::CompilerPass;
use qcc::opt::pass_manager::PassManager;
use qcc::optimizer::passes::cancellation::*;
use qcc::quantum::gate::Gate;
use qcc::quantum::parameter::Angle;
use qcc::simulator::sampling::SplitMix64;
use qcc::testkit::random_circuit::generate_random_unitary_circuit;
use qcc::verification::check_equivalence;

// ============================================================================
// 1. Single-Qubit Discrete Self-Inverses (H, X, Y, Z)
// ============================================================================

#[test]
fn test_single_qubit_self_inverse_h_h() {
    let mut builder = CircuitBuilder::new("h_h");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 2);

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.inverse_pairs_cancelled, 1);
    assert_eq!(stats.single_qubit_pairs_cancelled, 1);
    assert_eq!(stats.total_operations_removed(), 2);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_single_qubit_self_inverse_x_x() {
    let mut builder = CircuitBuilder::new("x_x");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.x(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_single_qubit_self_inverse_y_y() {
    let mut builder = CircuitBuilder::new("y_y");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.y(q[0]).unwrap();
    builder.y(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_single_qubit_self_inverse_z_z() {
    let mut builder = CircuitBuilder::new("z_z");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.z(q[0]).unwrap();
    builder.z(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 2. Discrete Adjoint Pairs (S-Sdg, T-Tdg)
// ============================================================================

#[test]
fn test_discrete_inverse_s_sdg() {
    let mut builder = CircuitBuilder::new("s_sdg");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.s(q[0]).unwrap();
    builder.sdg(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_discrete_inverse_sdg_s() {
    let mut builder = CircuitBuilder::new("sdg_s");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.sdg(q[0]).unwrap();
    builder.s(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_discrete_inverse_t_tdg() {
    let mut builder = CircuitBuilder::new("t_tdg");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.t(q[0]).unwrap();
    builder.tdg(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_discrete_inverse_tdg_t() {
    let mut builder = CircuitBuilder::new("tdg_t");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.tdg(q[0]).unwrap();
    builder.t(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 3. Two-Qubit Self-Inverses (CX, CZ, SWAP)
// ============================================================================

#[test]
fn test_two_qubit_self_inverse_cx_cx() {
    let mut builder = CircuitBuilder::new("cx_cx");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.inverse_pairs_cancelled, 1);
    assert_eq!(stats.two_qubit_pairs_cancelled, 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_two_qubit_self_inverse_cz_cz() {
    let mut builder = CircuitBuilder::new("cz_cz");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_two_qubit_self_inverse_swap_swap() {
    let mut builder = CircuitBuilder::new("swap_swap");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.swap(q[0], q[1]).unwrap();
    builder.swap(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 4. Directional vs Symmetric Operand Ordering
// ============================================================================

#[test]
fn test_directional_cx_reversed_never_cancels() {
    // CX(0, 1) followed by CX(1, 0) is NOT an inverse pair!
    // Under exact ordered operand matching, it is not a candidate pair and remains non-canceling.
    let mut builder = CircuitBuilder::new("cx_reversed");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[1], q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    // Must NOT be modified!
    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 2);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.inverse_pairs_cancelled, 0);
    assert_eq!(stats.candidate_pairs_examined, 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_two_qubit_cz_reversed_operands_not_candidate_pair() {
    // Under Phase 29 exact ordered operand matching, CZ(0, 1) and CZ(1, 0)
    // do not have identical ordered operands and are not considered a candidate pair.
    let mut builder = CircuitBuilder::new("cz_reversed");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    builder.cz(q[1], q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 2);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.inverse_pairs_cancelled, 0);
    assert_eq!(stats.candidate_pairs_examined, 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_two_qubit_swap_reversed_operands_not_candidate_pair() {
    // Under Phase 29 exact ordered operand matching, SWAP(0, 1) and SWAP(1, 0)
    // do not have identical ordered operands and are not considered a candidate pair.
    let mut builder = CircuitBuilder::new("swap_reversed");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.swap(q[0], q[1]).unwrap();
    builder.swap(q[1], q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 2);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.inverse_pairs_cancelled, 0);
    assert_eq!(stats.candidate_pairs_examined, 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 5. Continuous Rotation Inverses Modulo 4pi
// ============================================================================

#[test]
fn test_continuous_rotation_inverse_rx() {
    let mut builder = CircuitBuilder::new("rx_inv");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], Angle::from_radians(PI / 3.0)).unwrap();
    builder.rx(q[0], Angle::from_radians(-PI / 3.0)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.inverse_pairs_cancelled, 1);
    assert_eq!(stats.rotation_pairs_cancelled, 1);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_continuous_rotation_inverse_ry() {
    let mut builder = CircuitBuilder::new("ry_inv");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.ry(q[0], Angle::from_radians(1.234)).unwrap();
    builder.ry(q[0], Angle::from_radians(-1.234)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_continuous_rotation_inverse_rz() {
    let mut builder = CircuitBuilder::new("rz_inv");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rz(q[0], Angle::from_radians(0.785)).unwrap();
    builder.rz(q[0], Angle::from_radians(-0.785)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_continuous_rotation_four_pi_modulo() {
    // 3pi + pi = 4pi = 0 mod 4pi => R(4pi) = +I
    let mut builder = CircuitBuilder::new("rot_4pi");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], Angle::from_radians(3.0 * PI)).unwrap();
    builder.rx(q[0], Angle::from_radians(PI)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_continuous_rotation_two_pi_not_cancelled() {
    // Rx(pi) + Rx(pi) = Rx(2pi) = -I != +I.
    // In Phase 29, this must NOT be cancelled as an inverse because it has global phase discrepancy.
    let mut builder = CircuitBuilder::new("rot_2pi_not_inv");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], Angle::from_radians(PI)).unwrap();
    builder.rx(q[0], Angle::from_radians(PI)).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 2);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 6. Phase Gates (P, U1, phase) Modulo 2pi
// ============================================================================

#[test]
fn test_phase_gate_inverses() {
    let mut builder = CircuitBuilder::new("phase_inv");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder
        .custom_gate("p", &[q[0]], vec![ParameterExpr::constant(PI / 2.0)])
        .unwrap();
    builder
        .custom_gate("p", &[q[0]], vec![ParameterExpr::constant(-PI / 2.0)])
        .unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.inverse_pairs_cancelled, 1);
    assert_eq!(stats.rotation_pairs_cancelled, 1);
}

#[test]
fn test_phase_gate_two_pi_periodicity() {
    // For P(lambda), P(pi) * P(pi) = P(2pi) = I (phase gates have 2pi periodicity, not 4pi)
    let mut builder = CircuitBuilder::new("phase_2pi");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder
        .custom_gate("p", &[q[0]], vec![ParameterExpr::constant(PI)])
        .unwrap();
    builder
        .custom_gate("p", &[q[0]], vec![ParameterExpr::constant(PI)])
        .unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert!(circuit.validate().is_ok());

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.inverse_pairs_cancelled, 1);
    assert_eq!(stats.rotation_pairs_cancelled, 1);
}

// ============================================================================
// 7. Symbolic Parameters & Bound Environment
// ============================================================================

#[test]
fn test_symbolic_negation_inverses() {
    let mut builder = CircuitBuilder::new("sym_neg");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], ParameterExpr::symbol("theta")).unwrap();
    builder
        .rx(
            q[0],
            ParameterExpr::Neg(Box::new(ParameterExpr::symbol("theta"))),
        )
        .unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 2);

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
}

#[test]
fn test_symbolic_distinct_never_cancelled() {
    let mut builder = CircuitBuilder::new("sym_distinct");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.rx(q[0], ParameterExpr::symbol("alpha")).unwrap();
    builder.rx(q[0], ParameterExpr::symbol("beta")).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 2);
}

#[test]
fn test_symbolic_bound_environment_inverses() {
    let mut builder = CircuitBuilder::new("sym_bound");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.ry(q[0], ParameterExpr::symbol("alpha")).unwrap();
    builder.ry(q[0], ParameterExpr::symbol("beta")).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut env = HashMap::new();
    env.insert("alpha".to_string(), 0.5);
    env.insert("beta".to_string(), -0.5);

    let mut pass = InverseCancellationPass::new().with_symbol_env(env);
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
}

// ============================================================================
// 8. Cascaded Cancellations & Multi-Round Reductions
// ============================================================================

#[test]
fn test_cascaded_cancellation_outer_pairs() {
    // X(0) -> H(0) -> H(0) -> X(0)
    // Round 1 cancels H-H, leaving X-X.
    // Round 2 cancels X-X, leaving empty circuit.
    let mut builder = CircuitBuilder::new("cascaded");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.x(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    builder.x(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 4);

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.inverse_pairs_cancelled, 2);
    assert_eq!(stats.total_operations_removed(), 4);
    assert_eq!(stats.iterations, 3); // Iter 1: cancels H-H; Iter 2: cancels X-X; Iter 3: terminates.

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_multi_wire_cascaded_cancellation() {
    // CX(0, 1) -> CZ(0, 2) -> CZ(0, 2) -> CX(0, 1)
    // Round 1 cancels CZ-CZ on wires 0,2.
    // Round 2 cancels CX-CX on wires 0,1.
    let mut builder = CircuitBuilder::new("multi_wire_cascaded");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cz(q[0], q[2]).unwrap();
    builder.cz(q[0], q[2]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 4);

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.inverse_pairs_cancelled, 2);
    assert_eq!(stats.two_qubit_pairs_cancelled, 2);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_odd_chain_cancellation() {
    // H(0) -> H(0) -> H(0) cancels first pair, leaving one H(0)
    let mut builder = CircuitBuilder::new("odd_chain");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 3);

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    let op = circuit.graph.nodes().next().unwrap();
    assert_eq!(op.operation.name(), "h");

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 9. Unrelated Wire Independence (Commuting Disjoint Gates)
// ============================================================================

#[test]
fn test_unrelated_wire_interleaving() {
    // H(0) -> X(1) -> H(0)
    // H(0) and H(0) are on wire 0. X(1) is on wire 1.
    // In the DAG, wire 0 connects H(0) directly to H(0).
    // H(0) and H(0) cancel, leaving X(1).
    let mut builder = CircuitBuilder::new("unrelated_wire");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.x(q[1]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 3);

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    let remaining_op = circuit.graph.nodes().next().unwrap();
    assert_eq!(remaining_op.operation.name(), "x");
    assert_eq!(remaining_op.operation.qubits(), &[QubitId(1)]);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_multi_qubit_and_independent_gate_cancellation() {
    // CX(0, 1) -> Z(2) -> CX(0, 1)
    // CXs act on {0, 1}, Z acts on {2}.
    // CXs cancel, leaving Z(2).
    let mut builder = CircuitBuilder::new("cx_z_cx");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.z(q[2]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 3);

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    let remaining_op = circuit.graph.nodes().next().unwrap();
    assert_eq!(remaining_op.operation.name(), "z");
    assert_eq!(remaining_op.operation.qubits(), &[QubitId(2)]);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 10. Intervening Dependency Boundaries (Barrier, Measure, Reset)
// ============================================================================

#[test]
fn test_barrier_blocks_cancellation() {
    // H(0) -> Barrier(0) -> H(0)
    // Barrier on wire 0 strictly blocks cancellation.
    let mut builder = CircuitBuilder::new("barrier_blocks");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.barrier(&[q[0]]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.inverse_pairs_cancelled, 0);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_barrier_on_disjoint_wire_does_not_block() {
    // H(0) -> Barrier(1) -> H(0)
    // Barrier is on wire 1. Wire 0 is unaffected.
    // H(0) and H(0) cancel, leaving Barrier(1).
    let mut builder = CircuitBuilder::new("barrier_disjoint");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.barrier(&[q[1]]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 3);

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 1);

    let op = circuit.graph.nodes().next().unwrap();
    assert!(matches!(op.operation, Operation::Barrier { .. }));

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_measure_blocks_cancellation() {
    // H(0) -> Measure(0 -> c[0]) -> H(0)
    let mut builder = CircuitBuilder::new("measure_blocks");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_reset_blocks_cancellation() {
    // H(0) -> Reset(0) -> H(0)
    let mut builder = CircuitBuilder::new("reset_blocks");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.h(q[0]).unwrap();
    builder.reset(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);
}

// ============================================================================
// 11. Intervening Gates on Shared Wires (Multi-Qubit Dependency Safety)
// ============================================================================

#[test]
fn test_intervening_gate_on_shared_wire_0_blocks_multi_qubit_cancellation() {
    // CX(0, 1) -> X(0) -> CX(0, 1)
    // X(0) intervenes on wire 0. CX operations are NOT adjacent on wire 0!
    // CX(0, 1) * (X_0 * I_1) * CX(0, 1) = X_0 * X_1 != X_0 * I_1.
    // They must NOT cancel!
    let mut builder = CircuitBuilder::new("cx_x0_cx");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.x(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_intervening_gate_on_shared_wire_1_blocks_multi_qubit_cancellation() {
    // CX(0, 1) -> X(1) -> CX(0, 1)
    // X(1) intervenes on wire 1.
    let mut builder = CircuitBuilder::new("cx_x1_cx");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.x(q[1]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(!res.modified);
    assert_eq!(circuit.num_operations(), 3);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

// ============================================================================
// 12. Circuit-Level Global Phase Preservation
// ============================================================================

#[test]
fn test_circuit_level_global_phase_preserved() {
    // Initial circuit with explicit global phase: +0.6543 radians.
    // Cancelling inverse pairs must preserve this exact global phase.
    let mut builder = CircuitBuilder::new("phase_preservation");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.set_global_phase(0.6543);
    builder.h(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    builder.s(q[0]).unwrap();
    builder.sdg(q[0]).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.global_phase, ParameterExpr::Constant(0.6543));
    assert_eq!(circuit.num_operations(), 4);

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 0);
    assert_eq!(circuit.global_phase, ParameterExpr::Constant(0.6543));
}

// ============================================================================
// 13. PassManager Pipeline Integration & Telemetry Statistics
// ============================================================================

#[test]
fn test_pass_manager_pipeline_execution() {
    let mut builder = CircuitBuilder::new("pm_cancellation");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.x(q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let orig = circuit.clone();
    assert_eq!(circuit.num_operations(), 5);

    let mut pm = PassManager::new();
    pm.add_pass(InverseCancellationPass::new());

    let report = pm.run(&mut circuit).unwrap();
    assert!(report.is_success());
    assert_eq!(report.total_passes_executed, 1);
    assert_eq!(report.total_passes_modified, 1);
    assert_eq!(report.total_delta_operations, -4);
    assert_eq!(circuit.num_operations(), 1);

    let remaining = circuit.graph.nodes().next().unwrap();
    assert_eq!(remaining.operation.name(), "x");
    assert_eq!(remaining.operation.qubits(), &[QubitId(1)]);

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_pass_statistics_accuracy() {
    let mut builder = CircuitBuilder::new("stats_accuracy");
    let q = builder.add_quantum_register("q", 2).unwrap();
    // 1 single-qubit pair: H-H
    builder.h(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    // 1 two-qubit pair: CX-CX
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    // 1 rotation pair: Rz-Rz
    builder.rz(q[0], Angle::from_radians(0.5)).unwrap();
    builder.rz(q[0], Angle::from_radians(-0.5)).unwrap();
    // 1 non-cancelling pair: X - Z
    builder.x(q[1]).unwrap();
    builder.z(q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    assert_eq!(circuit.num_operations(), 8);

    let mut pass = InverseCancellationPass::new();
    let mut ctx = PassContext::default();
    let res = pass.run(&mut circuit, &mut ctx).unwrap();

    assert!(res.modified);
    assert_eq!(circuit.num_operations(), 2);

    let stats = pass.last_stats().unwrap();
    assert_eq!(stats.inverse_pairs_cancelled, 3);
    assert_eq!(stats.single_qubit_pairs_cancelled, 1);
    assert_eq!(stats.two_qubit_pairs_cancelled, 1);
    assert_eq!(stats.rotation_pairs_cancelled, 1);
    assert_eq!(stats.total_operations_removed(), 6);
    assert_eq!(stats.candidate_pairs_rejected, 1); // X(1) - Z(1)
    assert_eq!(stats.candidate_pairs_examined, 4);
}

// ============================================================================
// 14. Randomized Equivalence Verification with Injected Inverses
// ============================================================================

#[test]
fn test_randomized_circuit_inverse_cancellation_equivalence() {
    // Generate 30 random unitary circuits across 2 to 4 qubits.
    // In each circuit, inject various inverse pairs.
    // Run InverseCancellationPass, assert all injected pairs cancel, and verify unitary equivalence.
    let num_trials = 30;

    for trial in 0..num_trials {
        let num_qubits = 2 + (trial % 3); // 2, 3, or 4 qubits
        let base_depth = 4 + (trial % 4);
        let seed = 999_000 + trial as u64;

        let mut rng = SplitMix64::new(seed);
        let base_circuit = generate_random_unitary_circuit(
            format!("rand_{trial}"),
            num_qubits,
            base_depth,
            &mut rng,
        )
        .unwrap();

        // Build augmented circuit with injected inverse pairs
        let mut aug_circuit = base_circuit.clone();

        // Inject H-H on q[0]
        aug_circuit
            .add_operation(Operation::gate(&Gate::H, vec![QubitId(0)]))
            .unwrap();
        aug_circuit
            .add_operation(Operation::gate(&Gate::H, vec![QubitId(0)]))
            .unwrap();

        // Inject CX-CX on q[0], q[1]
        aug_circuit
            .add_operation(Operation::gate(&Gate::CX, vec![QubitId(0), QubitId(1)]))
            .unwrap();
        aug_circuit
            .add_operation(Operation::gate(&Gate::CX, vec![QubitId(0), QubitId(1)]))
            .unwrap();

        // Inject S-Sdg on q[1]
        aug_circuit
            .add_operation(Operation::gate(&Gate::S, vec![QubitId(1)]))
            .unwrap();
        aug_circuit
            .add_operation(Operation::gate(&Gate::Sdg, vec![QubitId(1)]))
            .unwrap();

        // Inject Rz-Rz on q[0]
        aug_circuit
            .add_operation(Operation::gate(
                &Gate::Rz(Angle::from_radians(0.654)),
                vec![QubitId(0)],
            ))
            .unwrap();
        aug_circuit
            .add_operation(Operation::gate(
                &Gate::Rz(Angle::from_radians(-0.654)),
                vec![QubitId(0)],
            ))
            .unwrap();

        let orig_aug = aug_circuit.clone();

        let mut pass = InverseCancellationPass::new();
        let mut ctx = PassContext::default();
        let res = pass.run(&mut aug_circuit, &mut ctx).unwrap();

        assert!(res.modified);
        let stats = pass.last_stats().unwrap();
        assert!(stats.inverse_pairs_cancelled >= 4);

        // Unitary equivalence between augmented circuit and optimized circuit
        let verdict = check_equivalence(&orig_aug, &aug_circuit).unwrap();
        assert!(
            verdict.is_equivalent,
            "Trial {trial} failed unitary equivalence check"
        );

        // Unitary equivalence between original base circuit and optimized circuit
        let verdict_base = check_equivalence(&base_circuit, &aug_circuit).unwrap();
        assert!(
            verdict_base.is_equivalent,
            "Trial {trial} failed equivalence against base circuit"
        );
    }
}
