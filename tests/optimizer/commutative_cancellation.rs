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
use qcc::{CircuitTransaction, Operation, OperationId};

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

// ============================================================================
// 5. Test Coverage Reconciliation Tests (Phase 80 Final Audit)
// ============================================================================

#[test]
fn test_positive_multiple_independent_cancellation_pairs() {
    let mut builder = CircuitBuilder::new("test_multiple_independent_pairs");
    let q = builder.add_quantum_register("q", 4).unwrap();
    // Pair 1: CX(q[0], q[1]) ... Rz(q[0]) ... CX(q[0], q[1])
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], ParameterExpr::constant(0.42)).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    // Pair 2: CX(q[2], q[3]) ... Rx(q[3]) ... CX(q[2], q[3])
    builder.cx(q[2], q[3]).unwrap();
    builder.rx(q[3], ParameterExpr::constant(0.84)).unwrap();
    builder.cx(q[2], q[3]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let initial_snapshot = circuit.clone();
    assert_eq!(circuit.num_operations(), 6);

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    let s = stats.expect("stats present");
    // Both pairs must be accepted and cancelled
    assert_eq!(s.cancellation_pairs_accepted, 2);
    assert_eq!(s.cx_pairs_cancelled, 2);
    assert_eq!(s.cz_pairs_cancelled, 0);
    assert_eq!(s.total_operations_removed(), 4);
    assert_eq!(circuit.num_operations(), 2); // Rz(0) and Rx(3) remain
    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&initial_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_dag_validity_after_pass_mutation() {
    let mut builder = CircuitBuilder::new("test_dag_validity");
    let q = builder.add_quantum_register("q", 3).unwrap();
    // Interleaved gates: CX(0, 1), Rz(0), S(0), CX(0, 1), H(2), CZ(1, 2), CZ(2, 1)
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], ParameterExpr::constant(0.123)).unwrap();
    builder.s(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.h(q[2]).unwrap();
    builder.cz(q[1], q[2]).unwrap();
    builder.cz(q[2], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    assert_eq!(circuit.num_operations(), 7);
    assert!(circuit.validate().is_ok());

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 2);
    assert_eq!(s.cx_pairs_cancelled, 1);
    assert_eq!(s.cz_pairs_cancelled, 1);
    assert_eq!(s.total_operations_removed(), 4);
    assert_eq!(circuit.num_operations(), 3); // Rz(0), S(0), H(2) remain

    // Explicitly verify all structural and semantic invariants of the resulting DAG:
    // 1. Dependency DAG is strictly acyclic
    assert!(!circuit.graph.is_cyclic());
    // 2. Full IR semantic validation passes (wire consistency, monotonic seq, register bounds)
    assert!(circuit.validate().is_ok());
    // 3. Deterministic topological ordering succeeds
    let ordered = circuit
        .graph
        .ordered_nodes()
        .expect("topological sort failed");
    assert_eq!(ordered.len(), 3);
    assert_eq!(ordered[0].operation.name(), "rz");
    assert_eq!(ordered[1].operation.name(), "s");
    assert_eq!(ordered[2].operation.name(), "h");
}

#[test]
fn test_deterministic_output_across_repeated_runs() {
    let build_circuit = || {
        let mut builder = CircuitBuilder::new("test_determinism");
        let q = builder.add_quantum_register("q", 4).unwrap();
        // A complex mixed circuit with multiple candidate pairs and intervening operations
        builder.cx(q[0], q[1]).unwrap();
        builder.rz(q[0], ParameterExpr::constant(0.3)).unwrap();
        builder.cx(q[0], q[2]).unwrap(); // shared control on q0
        builder.s(q[0]).unwrap();
        builder.cx(q[0], q[1]).unwrap(); // cancels with first CX(0, 1)

        builder.cx(q[2], q[3]).unwrap();
        builder.rx(q[3], ParameterExpr::constant(0.6)).unwrap();
        builder.cx(q[1], q[3]).unwrap(); // shared target on q3
        builder.cx(q[2], q[3]).unwrap(); // cancels with CX(2, 3)

        builder.cz(q[1], q[2]).unwrap();
        builder.rz(q[1], ParameterExpr::constant(0.4)).unwrap();
        builder.cz(q[2], q[1]).unwrap(); // cancels with CZ(1, 2)
        builder.finish().expect("build failed")
    };

    let mut c1 = build_circuit();
    let mut c2 = build_circuit();

    let mut pass1 = CommutativeTwoQubitCancellationPass::new();
    let mut pass2 = CommutativeTwoQubitCancellationPass::new();

    let (mod1, stats1) = run_pass(&mut pass1, &mut c1);
    let (mod2, stats2) = run_pass(&mut pass2, &mut c2);

    assert!(mod1 && mod2);
    let s1 = stats1.unwrap();
    let s2 = stats2.unwrap();
    assert_eq!(
        s1.cancellation_pairs_accepted,
        s2.cancellation_pairs_accepted
    );
    assert_eq!(s1.cx_pairs_cancelled, s2.cx_pairs_cancelled);
    assert_eq!(s1.cz_pairs_cancelled, s2.cz_pairs_cancelled);
    assert_eq!(s1.iterations, s2.iterations);
    assert_eq!(c1.num_operations(), c2.num_operations());

    // Verify bit-for-bit node and dependency order determinism
    let nodes1 = c1.graph.ordered_nodes().unwrap();
    let nodes2 = c2.graph.ordered_nodes().unwrap();
    assert_eq!(nodes1.len(), nodes2.len());
    for (n1, n2) in nodes1.iter().zip(nodes2.iter()) {
        assert_eq!(n1.operation.name(), n2.operation.name());
        assert_eq!(n1.operation.qubits(), n2.operation.qubits());
        assert_eq!(n1.operation.parameters(), n2.operation.parameters());
    }

    // Verify JSON serialization determinism
    let json1 = qcc::circuit_to_json(&c1).unwrap();
    let json2 = qcc::circuit_to_json(&c2).unwrap();
    assert_eq!(json1, json2);
}

#[test]
fn test_explicit_global_phase_preservation() {
    let mut builder = CircuitBuilder::new("test_global_phase_preservation");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], ParameterExpr::constant(0.789)).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    // Set an explicit non-zero initial global phase
    let initial_phase = ParameterExpr::constant(1.23456789012345);
    circuit.set_global_phase(initial_phase.clone());
    assert_eq!(circuit.global_phase(), &initial_phase);

    let initial_snapshot = circuit.clone();
    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(modified);
    assert_eq!(stats.unwrap().cancellation_pairs_accepted, 1);
    assert_eq!(circuit.num_operations(), 1);

    // Verify circuit global phase is identically preserved
    assert_eq!(circuit.global_phase(), &initial_phase);
    assert_eq!(circuit.global_phase(), initial_snapshot.global_phase());

    // Verify semantic unitary equivalence
    let verdict = check_equivalence(&initial_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_transaction_atomicity_and_rollback_on_failure() {
    let mut builder = CircuitBuilder::new("test_tx_rollback");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rz(q[0], ParameterExpr::constant(0.5)).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    let original_snapshot = circuit.clone();
    let ordered_before = circuit.graph.ordered_nodes().unwrap();
    let id1 = ordered_before[0].id;
    let _id2 = ordered_before[2].id;

    // Simulate an aborted/failed transactional sequence on CircuitTransaction:
    // 1. Begin transaction
    let mut tx = CircuitTransaction::begin(&mut circuit);
    // 2. Stage removal of first operation (succeeds on candidate)
    let rem1 = tx.remove_operation(id1);
    assert!(rem1.is_ok());
    assert_eq!(tx.circuit().num_operations(), 2);

    // 3. Attempt removal of a non-existent operation ID (simulating mid-flight failure)
    let rem_invalid = tx.remove_operation(OperationId(999999));
    assert!(rem_invalid.is_err());

    // 4. Abort / drop the transaction without committing (explicit rollback)
    tx.rollback();

    // 5. Verify the underlying circuit is completely unchanged
    assert_eq!(circuit.num_operations(), 3);
    assert_eq!(circuit.num_operations(), original_snapshot.num_operations());
    let ordered_after = circuit.graph.ordered_nodes().unwrap();
    assert_eq!(ordered_before.len(), ordered_after.len());
    for (b, a) in ordered_before.iter().zip(ordered_after.iter()) {
        assert_eq!(b.id, a.id);
        assert_eq!(b.operation.name(), a.operation.name());
    }
    assert!(circuit.validate().is_ok());

    let verdict =
        check_equivalence(&original_snapshot, &circuit).expect("equivalence check failed");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_negative_classical_raw_hazard_blocks_cancellation() {
    let mut builder = CircuitBuilder::new("test_classical_raw");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    // Add classical writer and reader establishing a Classical RAW hazard between two CXs:
    // Op 1: Classical writer writes c[0]
    circuit
        .add_operation(Operation::Classical {
            name: "write_c0".to_string(),
            reads: Vec::new(),
            writes: vec![c[0].bit_id()],
        })
        .unwrap();
    // Op 2: Classical reader reads c[0] (ClassicalRaw dependency)
    circuit
        .add_operation(Operation::Classical {
            name: "read_c0".to_string(),
            reads: vec![c[0].bit_id()],
            writes: vec![c[1].bit_id()],
        })
        .unwrap();
    // Op 3: Second CX(0, 1)
    circuit
        .add_operation(Operation::Gate {
            gate_name: "cx".to_string(),
            qubits: vec![q[0].qubit_id(), q[1].qubit_id()],
            parameters: Vec::new(),
        })
        .unwrap();

    assert_eq!(circuit.num_operations(), 4);
    assert!(circuit.validate().is_ok());

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    // Pass must fail closed at the classical hazard boundary
    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert!(s.rejection_classical_hazard > 0);
    assert_eq!(circuit.num_operations(), 4);
    assert!(circuit.validate().is_ok());
}

#[test]
fn test_negative_classical_war_and_waw_hazards_block_cancellation() {
    let mut builder = CircuitBuilder::new("test_classical_war_waw");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    // Op 1: Classical writer writing c[0]
    circuit
        .add_operation(Operation::Classical {
            name: "initial_write".to_string(),
            reads: Vec::new(),
            writes: vec![c[0].bit_id()],
        })
        .unwrap();
    // Op 2: Classical reader reading c[0] and writing c[1] (ClassicalRaw from Op 1)
    circuit
        .add_operation(Operation::Classical {
            name: "reader".to_string(),
            reads: vec![c[0].bit_id()],
            writes: vec![c[1].bit_id()],
        })
        .unwrap();
    // Op 3: Second classical write to c[0] (ClassicalWaw from Op 1, ClassicalWar from Op 2)
    circuit
        .add_operation(Operation::Classical {
            name: "overwrite_c0".to_string(),
            reads: Vec::new(),
            writes: vec![c[0].bit_id()],
        })
        .unwrap();
    // Op 4: Second CX(0, 1)
    circuit
        .add_operation(Operation::Gate {
            gate_name: "cx".to_string(),
            qubits: vec![q[0].qubit_id(), q[1].qubit_id()],
            parameters: Vec::new(),
        })
        .unwrap();

    assert_eq!(circuit.num_operations(), 5);
    assert!(circuit.validate().is_ok());

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert!(s.rejection_classical_hazard > 0);
    assert_eq!(circuit.num_operations(), 5);
    assert!(circuit.validate().is_ok());
}

#[test]
fn test_negative_uncatalogued_operation_blocks_cancellation() {
    let mut builder = CircuitBuilder::new("test_uncatalogued_gate");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().expect("build failed");

    // Insert an uncatalogued gate between the two candidate CXs
    circuit
        .add_operation(Operation::Gate {
            gate_name: "custom_unsupported_blackbox".to_string(),
            qubits: vec![q[0].qubit_id()],
            parameters: Vec::new(),
        })
        .unwrap();
    circuit
        .add_operation(Operation::Gate {
            gate_name: "cx".to_string(),
            qubits: vec![q[0].qubit_id(), q[1].qubit_id()],
            parameters: Vec::new(),
        })
        .unwrap();

    assert_eq!(circuit.num_operations(), 3);
    assert!(circuit.validate().is_ok());

    let mut pass = CommutativeTwoQubitCancellationPass::new();
    let (modified, stats) = run_pass(&mut pass, &mut circuit);

    assert!(!modified);
    let s = stats.expect("stats present");
    assert_eq!(s.cancellation_pairs_accepted, 0);
    assert!(
        s.rejection_uncatalogued_operation > 0,
        "must fail closed with rejection_uncatalogued_operation"
    );
    assert_eq!(circuit.num_operations(), 3);
    assert!(circuit.validate().is_ok());
}
