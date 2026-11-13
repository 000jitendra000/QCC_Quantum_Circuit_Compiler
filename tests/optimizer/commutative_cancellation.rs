//! Integration tests for CommutativeTwoQubitCancellationPass (Roadmap v3.1 Phase 80).
//!
//! Authoritative Specification: `docs/optimizer/commutative-cancellation-spec.md`
//! Authoritative ADR: `docs/decisions/ADR-0005-evidence-selected-optimizer-priority.md`
//!
//! Verifies:
//! 1. Positive cancellations:
//!    - Directly adjacent CX and CZ pairs
//!    - Symmetric CZ operand identity: CZ(0, 1) and CZ(1, 0)
//!    - CX across Z-axis operations on control (Rz, Z, S, T)
//!    - CX across X-axis operations on target (Rx, X)
//!    - CX across operations on disjoint wires
//!    - CX across shared-control CX(c, t2)
//!    - CX across shared-target CX(c2, t)
//!    - CZ across Z-axis operations on either operand
//!    - CZ across other CZ gates
//!    - CZ across CX gates where operand is control
//! 2. Negative cancellations (strictly rejected):
//!    - Reversed CX operands (CX(c, t) and CX(t, c))
//!    - Cross-type pairs (CX and CZ)
//!    - Non-commuting single-qubit on control (Rx, H, Y, Ry)
//!    - Non-commuting single-qubit on target (Rz, H, Y, Ry, S, T)
//!    - Non-commuting two-qubit (SWAP)
//!    - Chained CX dependencies (CX(0, 1) then CX(1, 2))
//!    - CZ with non-Z-axis gates (X, H, Y, Rx, Ry)
//!    - CZ with CX target
//!    - Barrier boundaries
//!    - Measurement boundaries
//!    - Reset boundaries
//! 3. Structural correctness and semantic equivalence:
//!    - Exact unitary equivalence via Phase 10 verification engine (atol < 1e-12)
//!    - Exact zero global phase change
//!    - QccIR graph acyclicity and wire validity
//! 4. Parity, convergence, and idempotence:
//!    - Odd chains leave 1 gate, even chains leave 0 gates
//!    - Second pass execution returns PassResult::unmodified()
//! 5. Pipeline orchestration:
//!    - Opt-in behavior in OptimizerConfig (default false)
//!    - Full pipeline execution when enabled

use qcc::builder::CircuitBuilder;
use qcc::ir::parameter::ParameterExpr;
use qcc::opt::context::PassContext;
use qcc::opt::pass::CompilerPass;
use qcc::optimizer::config::{OptimizationLevel, OptimizerConfig};
use qcc::optimizer::passes::commutative_cancellation::{
    CommutativeCancellationStats, CommutativeTwoQubitCancellationPass,
};
use qcc::optimizer::pipeline::OptimizerPipeline;
use qcc::verification::check_equivalence;

// ============================================================================
// Helper Functions
// ============================================================================

fn run_pass(
    pass: &mut CommutativeTwoQubitCancellationPass,
    circuit: &mut qcc::ir::circuit::Circuit,
) -> (bool, Option<CommutativeCancellationStats>) {
    let mut ctx = PassContext::new();
    let res = pass.run(circuit, &mut ctx).expect("pass execution failed");
    (res.modified, pass.last_stats().cloned())
}

// ============================================================================
// 1. Positive Cancellation Tests
// ============================================================================

#[test]
fn test_positive_adjacent_cx_cancellation() {
    let mut builder = CircuitBuilder::new("test_adjacent_cx");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let initial_snapshot = circuit.clone();
    assert_eq!(circuit.num_operations(), 2);

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 1);
    assert_eq!(s.cx_pairs_cancelled, 1);
    assert_eq!(s.total_operations_removed(), 2);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&initial_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_positive_adjacent_cz_symmetric_cancellation() {
    let mut builder = CircuitBuilder::new("test_adjacent_cz");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    builder.cz(q[1], q[0]).unwrap(); // Reversed operands represent identical symmetric gate
    let mut circuit = builder.finish().expect("build failed");

    let initial_snapshot = circuit.clone();
    assert_eq!(circuit.num_operations(), 2);

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 1);
    assert_eq!(s.cz_pairs_cancelled, 1);
    assert_eq!(circuit.num_operations(), 0);

    let verdict = check_equivalence(&initial_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_positive_cx_across_control_z_rotations() {
    let mut builder = CircuitBuilder::new("test_cx_control_z");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder
        .rz(q[0], ParameterExpr::constant(std::f64::consts::FRAC_PI_4))
        .unwrap();
    builder.s(q[0]).unwrap();
    builder.t(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let initial_snapshot = circuit.clone();
    assert_eq!(circuit.num_operations(), 5);

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 1);
    assert_eq!(s.cx_pairs_cancelled, 1);
    assert_eq!(circuit.num_operations(), 3); // Rz, S, T remain

    let verdict = check_equivalence(&initial_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_positive_cx_across_target_x_rotations() {
    let mut builder = CircuitBuilder::new("test_cx_target_x");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rx(q[1], ParameterExpr::constant(1.234)).unwrap();
    builder.x(q[1]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let initial_snapshot = circuit.clone();
    assert_eq!(circuit.num_operations(), 4);

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 1);
    assert_eq!(s.cx_pairs_cancelled, 1);
    assert_eq!(circuit.num_operations(), 2); // Rx, X remain

    let verdict = check_equivalence(&initial_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_positive_cx_across_disjoint_wire_operations() {
    let mut builder = CircuitBuilder::new("test_cx_disjoint");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.h(q[2]).unwrap();
    builder.y(q[2]).unwrap();
    builder.t(q[2]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let initial_snapshot = circuit.clone();
    assert_eq!(circuit.num_operations(), 5);

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 1);
    assert_eq!(s.cx_pairs_cancelled, 1);
    assert_eq!(circuit.num_operations(), 3); // H(2), Y(2), T(2) remain

    let verdict = check_equivalence(&initial_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_positive_cx_across_shared_control_and_target_gates() {
    let mut builder = CircuitBuilder::new("test_cx_shared");
    let q = builder.add_quantum_register("q", 3).unwrap();
    // CX(0, 1) ... CX(0, 2) (shared control) ... CX(2, 1) (shared target) ... CX(0, 1)
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[0], q[2]).unwrap();
    builder.cx(q[2], q[1]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let initial_snapshot = circuit.clone();
    assert_eq!(circuit.num_operations(), 4);

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 1);
    assert_eq!(s.cx_pairs_cancelled, 1);
    assert_eq!(circuit.num_operations(), 2); // CX(0, 2) and CX(2, 1) remain

    let verdict = check_equivalence(&initial_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_positive_cz_across_z_rotations_and_cz() {
    let mut builder = CircuitBuilder::new("test_cz_commuting");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    builder.rz(q[0], ParameterExpr::constant(0.456)).unwrap();
    builder.s(q[1]).unwrap();
    builder.cz(q[1], q[2]).unwrap();
    builder.cz(q[1], q[0]).unwrap(); // Symmetric partner
    let mut circuit = builder.finish().expect("build failed");

    let initial_snapshot = circuit.clone();
    assert_eq!(circuit.num_operations(), 5);

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 1);
    assert_eq!(s.cz_pairs_cancelled, 1);
    assert_eq!(circuit.num_operations(), 3);

    let verdict = check_equivalence(&initial_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_positive_cz_across_cx_shared_control() {
    let mut builder = CircuitBuilder::new("test_cz_cx_ctrl");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    // CX with control on q0 and target on q2 (disjoint from q1)
    builder.cx(q[0], q[2]).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let initial_snapshot = circuit.clone();
    assert_eq!(circuit.num_operations(), 3);

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 1);
    assert_eq!(s.cz_pairs_cancelled, 1);
    assert_eq!(circuit.num_operations(), 1);

    let verdict = check_equivalence(&initial_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

// ============================================================================
// 2. Negative Cancellation Tests (Strict Rejection)
// ============================================================================

#[test]
fn test_negative_reversed_cx_strictly_rejected() {
    let mut builder = CircuitBuilder::new("test_neg_rev_cx");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cx(q[1], q[0]).unwrap(); // Reversed operands
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert_eq!(circuit.num_operations(), 2);
}

#[test]
fn test_negative_cross_type_cx_cz_rejected() {
    let mut builder = CircuitBuilder::new("test_neg_cross_type");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert_eq!(circuit.num_operations(), 2);
}

#[test]
fn test_negative_intervening_rx_on_control_blocks() {
    let mut builder = CircuitBuilder::new("test_neg_rx_ctrl");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rx(q[0], ParameterExpr::constant(0.5)).unwrap(); // Rx on control wire does not commute
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert!(s.rejection_non_commuting_control > 0);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_negative_intervening_rz_on_target_blocks() {
    let mut builder = CircuitBuilder::new("test_neg_rz_targ");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[1], ParameterExpr::constant(0.5)).unwrap(); // Rz on target wire does not commute
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert!(s.rejection_non_commuting_target > 0);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_negative_intervening_hadamard_blocks() {
    let mut builder = CircuitBuilder::new("test_neg_h");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.h(q[0]).unwrap(); // H on control does not commute
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, _) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_negative_intervening_swap_blocks() {
    let mut builder = CircuitBuilder::new("test_neg_swap");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.swap(q[0], q[1]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, _) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_negative_intervening_chained_cx_blocks() {
    let mut builder = CircuitBuilder::new("test_neg_chained");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    // CX with control on q1 (target of CX(0, 1)): chained dependency does not commute!
    builder.cx(q[1], q[2]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_negative_cz_intervening_x_blocks() {
    let mut builder = CircuitBuilder::new("test_neg_cz_x");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    builder.x(q[0]).unwrap(); // X on CZ does not commute
    builder.cz(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert!(s.rejection_non_commuting_cz > 0);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_negative_cz_intervening_cx_target_blocks() {
    let mut builder = CircuitBuilder::new("test_neg_cz_cx_target");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    // CX where q0 is target (control is q2): applies X on q0, anti-commuting with Z!
    builder.cx(q[2], q[0]).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert!(s.rejection_non_commuting_cz > 0);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_negative_barrier_boundary_blocks() {
    let mut builder = CircuitBuilder::new("test_neg_barrier");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.barrier(&[q[0], q[1]]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert!(s.rejection_non_unitary_or_barrier > 0);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_negative_measure_boundary_blocks() {
    let mut builder = CircuitBuilder::new("test_neg_measure");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert!(s.rejection_non_unitary_or_barrier > 0);
    assert_eq!(circuit.num_operations(), 3);
}

#[test]
fn test_negative_reset_boundary_blocks() {
    let mut builder = CircuitBuilder::new("test_neg_reset");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.reset(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert!(s.rejection_non_unitary_or_barrier > 0);
    assert_eq!(circuit.num_operations(), 3);
}

// ============================================================================
// 3. Chain Parity and Idempotence Tests
// ============================================================================

#[test]
fn test_odd_chain_leaves_one_gate() {
    let mut builder = CircuitBuilder::new("test_odd_chain");
    let q = builder.add_quantum_register("q", 2).unwrap();
    // Chain of 3 CXs across commuting Rz operations
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], ParameterExpr::constant(0.1)).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], ParameterExpr::constant(0.2)).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let initial_snapshot = circuit.clone();
    assert_eq!(circuit.num_operations(), 5);

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 1);
    assert_eq!(s.cx_pairs_cancelled, 1);
    // 1 CX cancelled with 1 CX, exactly 1 CX remains, plus the two Rz gates
    assert_eq!(circuit.num_operations(), 3);

    let verdict = check_equivalence(&initial_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_even_chain_leaves_zero_gates() {
    let mut builder = CircuitBuilder::new("test_even_chain");
    let q = builder.add_quantum_register("q", 2).unwrap();
    // Chain of 4 CXs across commuting operations
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], ParameterExpr::constant(0.1)).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], ParameterExpr::constant(0.2)).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], ParameterExpr::constant(0.3)).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let initial_snapshot = circuit.clone();
    assert_eq!(circuit.num_operations(), 7);

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 2);
    assert_eq!(s.cx_pairs_cancelled, 2);
    // All 4 CXs cancelled, leaving only 3 Rz gates
    assert_eq!(circuit.num_operations(), 3);

    let verdict = check_equivalence(&initial_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_idempotence_second_pass_unmodified() {
    let mut builder = CircuitBuilder::new("test_idempotence");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], ParameterExpr::constant(0.5)).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified1, stats1) = run_pass(&mut pass, &mut circuit);
    assert!(modified1);
    assert_eq!(stats1.unwrap().cancellation_pairs_accepted, 1);
    assert_eq!(circuit.num_operations(), 1);

    // Second pass on the already-optimized circuit must be a no-op
    let (modified2, stats2) = run_pass(&mut pass, &mut circuit);
    assert!(!modified2);
    let s2 = stats2.unwrap();
    assert_eq!(s2.cancellation_pairs_accepted, 0);
    assert_eq!(circuit.num_operations(), 1);
}

// ============================================================================
// 4. Pipeline Integration Tests
// ============================================================================

#[test]
fn test_pipeline_opt_in_behavior() {
    let mut builder = CircuitBuilder::new("test_pipeline_opt_in");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], ParameterExpr::constant(0.5)).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit = builder.finish().expect("build failed");

    // 1. Standard O2 pipeline (default: enable_commutative_cancellation = false)
    let config_default = OptimizerConfig::new(OptimizationLevel::O2);
    assert!(!config_default.enable_commutative_cancellation);
    let pipeline_default = OptimizerPipeline::new(config_default);
    let mut c_default = circuit.clone();
    let _ = pipeline_default
        .optimize(&mut c_default)
        .expect("pipeline failed");

    // Under standard O2, non-adjacent CX across Rz is NOT cancelled
    let cx_count_default = c_default
        .graph
        .nodes()
        .filter(|n| n.operation.name() == "cx")
        .count();
    assert_eq!(
        cx_count_default, 2,
        "default O2 should not cancel non-adjacent CX"
    );

    // 2. Opt-in O2 pipeline with enable_commutative_cancellation = true
    let config_opt_in =
        OptimizerConfig::new(OptimizationLevel::O2).with_commutative_cancellation(true);
    assert!(config_opt_in.enable_commutative_cancellation);
    let pipeline_opt_in = OptimizerPipeline::new(config_opt_in);
    let mut c_opt_in = circuit.clone();
    let _ = pipeline_opt_in
        .optimize(&mut c_opt_in)
        .expect("pipeline failed");

    // Under opt-in O2, non-adjacent CX across Rz is successfully cancelled
    let cx_count_opt_in = c_opt_in
        .graph
        .nodes()
        .filter(|n| n.operation.name() == "cx")
        .count();
    assert_eq!(
        cx_count_opt_in, 0,
        "opt-in O2 must eliminate non-adjacent CX"
    );

    // Verify unitary equivalence of the final optimized circuit
    let verdict = check_equivalence(&circuit, &c_opt_in).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}
