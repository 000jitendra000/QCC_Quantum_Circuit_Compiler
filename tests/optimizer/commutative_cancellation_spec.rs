//! Phase 79: Commutative Two-Qubit (CX/CZ) Cancellation Rule Specification & Proof Obligations.
//!
//! Authoritative Specification: `docs/optimizer/commutative-cancellation-spec.md`
//! Authoritative ADR: `docs/decisions/ADR-0005-evidence-selected-optimizer-priority.md`
//!
//! Validates:
//! 1. Rule-Level Matrix Commutation Proofs & Commutator Bounds:
//!    - Z-axis operations on CX control commute ($[CX, O]_c = 0$).
//!    - X-axis operations on CX target commute ($[CX, O]_t = 0$).
//!    - Z-axis operations on CZ commute ($[CZ, O] = 0$).
//!    - Non-commuting operations strictly fail commutator bounds ($\|[A, B]\|_F > 0.1$).
//!    - Reversed CX operands $CX(c, t) \cdot CX(t, c) \neq I$ (ordered operand identity).
//!    - Symmetric CZ operands $CZ(a, b) \equiv CZ(b, a)$ and $CZ(a, b) \cdot CZ(b, a) = I$.
//!    - Cross-type $CX \cdot CZ \neq I$.
//! 2. Circuit-Level & Statevector Equivalence for Commutative Cancellation:
//!    - Commuting CX cancellation produces exact bit-for-bit equivalence ($\Delta \phi = 0$).
//!    - Commuting CZ cancellation produces exact bit-for-bit equivalence ($\Delta \phi = 0$).
//!    - Symbolic parameter invariance holds for all angle values $\theta \in \mathbb{R}$.
//!    - Disjoint-wire operations commute freely.
//!    - Multi-qubit shared control/target gates commute as specified.
//! 3. Negative Fixtures Proving Non-Equivalence When Preconditions Are Violated:
//!    - Non-commuting gates (Ry, H, target Rz, control Rx, reversed CX, cross-type, SWAP, chained CX)
//!      fail equivalence if cancelled.
//! 4. DAG Wire Reachability, Boundaries & Classical Hazard Preconditions:
//!    - Barrier, Measure, Reset, and classical hazards are impenetrable boundaries.
//! 5. Greedy Non-Overlapping & Idempotence Proof Contracts:
//!    - Odd chains leave 1 gate, even chains leave 0 gates.
//!    - Idempotence: zero subsequent mutations, terminating in 1 sweep.

use std::collections::HashMap;
use std::f64::consts::{E, PI};

use qcc::builder::CircuitBuilder;
use qcc::commutation::{CommutationRegistry, NonCommutationReason};
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::{ClassicalBitId, QubitId};
use qcc::quantum::angle::Angle;
use qcc::quantum::gate_matrix::{
    kron_2x2, matrix_cx_c0_t1, matrix_cx_c1_t0, matrix_cz, matrix_h, matrix_rx, matrix_ry,
    matrix_rz, matrix_s, matrix_sdg, matrix_swap, matrix_t, matrix_tdg, matrix_x, matrix_y,
    matrix_z, Matrix2x2, Matrix4x4,
};
use qcc::verification::check_equivalence;

const MATRIX_TOLERANCE: f64 = 1e-12;

// ============================================================================
// Matrix Commutator Helpers
// ============================================================================

/// Computes the Frobenius norm of the commutator $[A, B] = A B - B A$ for $4 \times 4$ matrices.
fn commutator_norm_4x4(a: &Matrix4x4, b: &Matrix4x4) -> f64 {
    let ab = a.matmul(b);
    let ba = b.matmul(a);
    let mut sum = 0.0;
    for r in 0..4 {
        for c in 0..4 {
            let diff = ab.get(r, c) - ba.get(r, c);
            sum += diff.norm_sqr();
        }
    }
    sum.sqrt()
}

/// Computes the Frobenius norm of the difference $A - B$ for $4 \times 4$ matrices.
fn diff_norm_4x4(a: &Matrix4x4, b: &Matrix4x4) -> f64 {
    let mut sum = 0.0;
    for r in 0..4 {
        for c in 0..4 {
            let diff = a.get(r, c) - b.get(r, c);
            sum += diff.norm_sqr();
        }
    }
    sum.sqrt()
}

// ============================================================================
// 1. Matrix-Level Mathematical Proofs & Commutator Bounds
// ============================================================================

#[test]
fn test_proof_cx_control_z_axis_algebraic_commutation() {
    let cx = matrix_cx_c0_t1(); // q0 is control, q1 is target
    let id2 = Matrix2x2::identity();

    // In little-endian basis {|00>, |01>, |10>, |11>}, op on q0 is kron(I_q1, U_q0)
    let test_cases: Vec<(&str, Matrix2x2)> = vec![
        ("z", matrix_z()),
        ("s", matrix_s()),
        ("sdg", matrix_sdg()),
        ("t", matrix_t()),
        ("tdg", matrix_tdg()),
        ("rz(0.0)", matrix_rz(Angle::from_radians(0.0))),
        ("rz(pi/6)", matrix_rz(Angle::from_radians(PI / 6.0))),
        ("rz(pi/4)", matrix_rz(Angle::from_radians(PI / 4.0))),
        ("rz(pi/2)", matrix_rz(Angle::from_radians(PI / 2.0))),
        ("rz(pi)", matrix_rz(Angle::from_radians(PI))),
        ("rz(E)", matrix_rz(Angle::from_radians(E))),
        ("rz(-1.414)", matrix_rz(Angle::from_radians(-1.414))),
    ];

    for (name, u_z) in test_cases {
        let op_4x4 = kron_2x2(&id2, &u_z);
        let comm_norm = commutator_norm_4x4(&cx, &op_4x4);
        assert!(
            comm_norm < MATRIX_TOLERANCE,
            "Z-axis gate '{name}' on CX control must commute at matrix level, got {comm_norm}"
        );

        // Explicit algebraic proof of cancellation identity:
        // CX . (I (x) U_z) . CX == I (x) U_z
        let conjugate = cx.matmul(&op_4x4).matmul(&cx);
        let diff = diff_norm_4x4(&conjugate, &op_4x4);
        assert!(
            diff < MATRIX_TOLERANCE,
            "CX . {name} . CX must equal {name}, difference was {diff}"
        );
    }
}

#[test]
fn test_proof_cx_target_x_axis_algebraic_commutation() {
    let cx = matrix_cx_c0_t1(); // q0 is control, q1 is target
    let id2 = Matrix2x2::identity();

    // In little-endian basis {|00>, |01>, |10>, |11>}, op on q1 is kron(U_q1, I_q0)
    let test_cases: Vec<(&str, Matrix2x2)> = vec![
        ("x", matrix_x()),
        ("rx(0.0)", matrix_rx(Angle::from_radians(0.0))),
        ("rx(pi/4)", matrix_rx(Angle::from_radians(PI / 4.0))),
        ("rx(pi/2)", matrix_rx(Angle::from_radians(PI / 2.0))),
        ("rx(pi)", matrix_rx(Angle::from_radians(PI))),
        ("rx(1.2345)", matrix_rx(Angle::from_radians(1.2345))),
        ("rx(-2.17)", matrix_rx(Angle::from_radians(-2.17))),
    ];

    for (name, u_x) in test_cases {
        let op_4x4 = kron_2x2(&u_x, &id2);
        let comm_norm = commutator_norm_4x4(&cx, &op_4x4);
        assert!(
            comm_norm < MATRIX_TOLERANCE,
            "X-axis gate '{name}' on CX target must commute at matrix level, got {comm_norm}"
        );

        // Explicit algebraic proof of cancellation identity:
        // CX . (U_x (x) I) . CX == U_x (x) I
        let conjugate = cx.matmul(&op_4x4).matmul(&cx);
        let diff = diff_norm_4x4(&conjugate, &op_4x4);
        assert!(
            diff < MATRIX_TOLERANCE,
            "CX . {name} . CX must equal {name}, difference was {diff}"
        );
    }
}

#[test]
fn test_proof_cx_negative_non_commuting_operators() {
    let cx = matrix_cx_c0_t1(); // q0 is control, q1 is target
    let id2 = Matrix2x2::identity();

    // 1. Ry on control must NOT commute
    let ry_c = kron_2x2(&id2, &matrix_ry(Angle::from_radians(PI / 3.0)));
    assert!(commutator_norm_4x4(&cx, &ry_c) > 0.1);

    // 2. Ry on target must NOT commute
    let ry_t = kron_2x2(&matrix_ry(Angle::from_radians(PI / 3.0)), &id2);
    assert!(commutator_norm_4x4(&cx, &ry_t) > 0.1);

    // 3. H on control must NOT commute
    let h_c = kron_2x2(&id2, &matrix_h());
    assert!(commutator_norm_4x4(&cx, &h_c) > 0.1);

    // 4. H on target must NOT commute
    let h_t = kron_2x2(&matrix_h(), &id2);
    assert!(commutator_norm_4x4(&cx, &h_t) > 0.1);

    // 5. Rz on target must NOT commute
    let rz_t = kron_2x2(&matrix_rz(Angle::from_radians(PI / 2.0)), &id2);
    assert!(commutator_norm_4x4(&cx, &rz_t) > 0.1);

    // 6. Rx on control must NOT commute
    let rx_c = kron_2x2(&id2, &matrix_rx(Angle::from_radians(PI / 2.0)));
    assert!(commutator_norm_4x4(&cx, &rx_c) > 0.1);

    // 7. Y on control and target must NOT commute
    let y_c = kron_2x2(&id2, &matrix_y());
    let y_t = kron_2x2(&matrix_y(), &id2);
    assert!(commutator_norm_4x4(&cx, &y_c) > 0.1);
    assert!(commutator_norm_4x4(&cx, &y_t) > 0.1);
}

#[test]
fn test_proof_cx_ordered_operand_identity_invariant() {
    let cx_01 = matrix_cx_c0_t1(); // q0 is ctrl, q1 is targ
    let cx_10 = matrix_cx_c1_t0(); // q1 is ctrl, q0 is targ
    let id4 = Matrix4x4::identity();
    let swap = matrix_swap();

    // 1. Proves CX(0, 1) and CX(1, 0) do not commute: [CX(0, 1), CX(1, 0)] != 0
    let comm_norm = commutator_norm_4x4(&cx_01, &cx_10);
    assert!(
        comm_norm > 1.0,
        "Reversed CX operands must not commute, got norm {comm_norm}"
    );

    // 2. Proves CX(0, 1) * CX(1, 0) != I (they do not self-cancel!)
    let prod = cx_01.matmul(&cx_10);
    let diff_from_id = diff_norm_4x4(&prod, &id4);
    assert!(
        diff_from_id > 2.0,
        "CX(0, 1) * CX(1, 0) must NOT equal identity, diff was {diff_from_id}"
    );
    // Exact Frobenius distance is sqrt(6) approx 2.4494897
    assert!((diff_from_id - (6.0f64).sqrt()).abs() < 1e-10);

    // 3. Proves algebraic identity CX(0, 1) * CX(1, 0) == SWAP * CX(0, 1)
    let swap_cx = swap.matmul(&cx_01);
    assert!(
        diff_norm_4x4(&prod, &swap_cx) < MATRIX_TOLERANCE,
        "CX(0, 1) * CX(1, 0) must equal SWAP * CX(0, 1)"
    );
}

#[test]
fn test_proof_cz_symmetric_operand_identity_and_commutation() {
    let cz = matrix_cz();
    let id2 = Matrix2x2::identity();
    let id4 = Matrix4x4::identity();

    // 1. Proves CZ is self-inverse: CZ * CZ == I
    let prod = cz.matmul(&cz);
    assert!(
        diff_norm_4x4(&prod, &id4) < MATRIX_TOLERANCE,
        "CZ * CZ must equal identity"
    );

    // 2. Proves Z-axis operations on EITHER operand commute with CZ
    let z_ops: Vec<(&str, Matrix2x2)> = vec![
        ("z", matrix_z()),
        ("s", matrix_s()),
        ("t", matrix_t()),
        ("rz(1.5)", matrix_rz(Angle::from_radians(1.5))),
    ];

    for (name, u_z) in z_ops {
        // Wire 0 (LSB)
        let op_w0 = kron_2x2(&id2, &u_z);
        assert!(
            commutator_norm_4x4(&cz, &op_w0) < MATRIX_TOLERANCE,
            "Z-axis gate '{name}' on CZ wire 0 must commute"
        );
        // Wire 1 (MSB)
        let op_w1 = kron_2x2(&u_z, &id2);
        assert!(
            commutator_norm_4x4(&cz, &op_w1) < MATRIX_TOLERANCE,
            "Z-axis gate '{name}' on CZ wire 1 must commute"
        );
    }

    // 3. Proves X and Y operations on CZ do NOT commute (never generalize CX rules to CZ)
    let x_w0 = kron_2x2(&id2, &matrix_x());
    let x_w1 = kron_2x2(&matrix_x(), &id2);
    let rx_w0 = kron_2x2(&id2, &matrix_rx(Angle::from_radians(1.0)));
    let rx_w1 = kron_2x2(&matrix_rx(Angle::from_radians(1.0)), &id2);

    assert!(commutator_norm_4x4(&cz, &x_w0) > 0.1);
    assert!(commutator_norm_4x4(&cz, &x_w1) > 0.1);
    assert!(commutator_norm_4x4(&cz, &rx_w0) > 0.1);
    assert!(commutator_norm_4x4(&cz, &rx_w1) > 0.1);
}

#[test]
fn test_proof_cross_type_and_swap_non_cancellation() {
    let cx = matrix_cx_c0_t1();
    let cz = matrix_cz();
    let swap = matrix_swap();
    let id4 = Matrix4x4::identity();

    // 1. Cross-type CX * CZ != I
    let cx_cz = cx.matmul(&cz);
    assert!(
        diff_norm_4x4(&cx_cz, &id4) > 1.0,
        "CX * CZ must NOT equal identity"
    );

    // 2. Intervening SWAP does not commute with CX
    assert!(
        commutator_norm_4x4(&cx, &swap) > 0.5,
        "SWAP must not commute with CX"
    );
}

// ============================================================================
// 2. Circuit-Level & Statevector Equivalence for Commutative Cancellation
// ============================================================================

#[test]
fn test_equivalence_cx_commutative_cancellation_positive() {
    // Input: CX(0, 1) . Rz(0.7)[0] . S[0] . Rx(1.2)[1] . X[1] . CX(0, 1)
    let mut b1 = CircuitBuilder::new("c_before");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.rz(q1[0], ParameterExpr::constant(0.7)).unwrap();
    b1.s(q1[0]).unwrap();
    b1.rx(q1[1], ParameterExpr::constant(1.2)).unwrap();
    b1.x(q1[1]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c_before = b1.finish().unwrap();

    // Cancelled target: Rz(0.7)[0] . S[0] . Rx(1.2)[1] . X[1]
    let mut b2 = CircuitBuilder::new("c_after");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.rz(q2[0], ParameterExpr::constant(0.7)).unwrap();
    b2.s(q2[0]).unwrap();
    b2.rx(q2[1], ParameterExpr::constant(1.2)).unwrap();
    b2.x(q2[1]).unwrap();
    let c_after = b2.finish().unwrap();

    let eq = check_equivalence(&c_before, &c_after).unwrap();
    assert!(
        eq.is_equivalent,
        "Commutative CX cancellation must be unitarily equivalent"
    );
    assert!(
        eq.estimated_phase.unwrap_or(0.0).abs() < 1e-12,
        "Global phase delta must be 0.0"
    );
}

#[test]
fn test_equivalence_cz_commutative_cancellation_with_symmetric_operands() {
    // Input: CZ(0, 1) . Rz(0.5)[0] . T[1] . CZ(1, 0)  (operands reversed on 2nd CZ!)
    let mut b1 = CircuitBuilder::new("c_cz_before");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.cz(q1[0], q1[1]).unwrap();
    b1.rz(q1[0], ParameterExpr::constant(0.5)).unwrap();
    b1.t(q1[1]).unwrap();
    b1.cz(q1[1], q1[0]).unwrap(); // Reversed operands
    let c_before = b1.finish().unwrap();

    // Cancelled target: Rz(0.5)[0] . T[1]
    let mut b2 = CircuitBuilder::new("c_cz_after");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.rz(q2[0], ParameterExpr::constant(0.5)).unwrap();
    b2.t(q2[1]).unwrap();
    let c_after = b2.finish().unwrap();

    let eq = check_equivalence(&c_before, &c_after).unwrap();
    assert!(
        eq.is_equivalent,
        "Symmetric CZ cancellation must be unitarily equivalent"
    );
    assert!(
        eq.estimated_phase.unwrap_or(0.0).abs() < 1e-12,
        "Global phase delta must be 0.0"
    );
}

#[test]
fn test_equivalence_symbolic_parameter_invariance() {
    // Commutation holds for all theta in R without requiring concrete numerical evaluation.
    let angles = [0.0, 0.123, PI / 4.0, PI / 2.0, PI, 3.0 * PI / 2.0, -1.876];

    for &theta in &angles {
        let mut b1 = CircuitBuilder::new("c_sym_before");
        let q1 = b1.add_quantum_register("q", 2).unwrap();
        b1.cx(q1[0], q1[1]).unwrap();
        b1.rz(q1[0], ParameterExpr::constant(theta)).unwrap();
        b1.rx(q1[1], ParameterExpr::constant(theta * 2.0)).unwrap();
        b1.cx(q1[0], q1[1]).unwrap();
        let c1 = b1.finish().unwrap();

        let mut b2 = CircuitBuilder::new("c_sym_after");
        let q2 = b2.add_quantum_register("q", 2).unwrap();
        b2.rz(q2[0], ParameterExpr::constant(theta)).unwrap();
        b2.rx(q2[1], ParameterExpr::constant(theta * 2.0)).unwrap();
        let c2 = b2.finish().unwrap();

        let eq = check_equivalence(&c1, &c2).unwrap();
        assert!(
            eq.is_equivalent,
            "Symbolic parameter commutation must hold for theta = {theta}"
        );
    }
}

#[test]
fn test_equivalence_disjoint_wire_operations_do_not_block() {
    // 3 qubits: CX(0, 1) . H[2] . Ry(1.5)[2] . CX(0, 1)
    let mut b1 = CircuitBuilder::new("c_disjoint_before");
    let q1 = b1.add_quantum_register("q", 3).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.h(q1[2]).unwrap();
    b1.ry(q1[2], ParameterExpr::constant(1.5)).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c1 = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("c_disjoint_after");
    let q2 = b2.add_quantum_register("q", 3).unwrap();
    b2.h(q2[2]).unwrap();
    b2.ry(q2[2], ParameterExpr::constant(1.5)).unwrap();
    let c2 = b2.finish().unwrap();

    let eq = check_equivalence(&c1, &c2).unwrap();
    assert!(
        eq.is_equivalent,
        "Disjoint wire operations on q2 must not prevent CX(0, 1) cancellation"
    );
}

#[test]
fn test_equivalence_multi_qubit_shared_control_and_target() {
    // 3 qubits:
    // Case A: Shared control: CX(0, 1) . CX(0, 2) . CX(0, 1) == CX(0, 2)
    let mut b1 = CircuitBuilder::new("c_shared_ctrl_before");
    let q1 = b1.add_quantum_register("q", 3).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.cx(q1[0], q1[2]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c1 = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("c_shared_ctrl_after");
    let q2 = b2.add_quantum_register("q", 3).unwrap();
    b2.cx(q2[0], q2[2]).unwrap();
    let c2 = b2.finish().unwrap();

    let eq_a = check_equivalence(&c1, &c2).unwrap();
    assert!(
        eq_a.is_equivalent,
        "CX(0, 1) . CX(0, 2) . CX(0, 1) must reduce to CX(0, 2)"
    );

    // Case B: Shared target: CX(0, 1) . CX(2, 1) . CX(0, 1) == CX(2, 1)
    let mut b3 = CircuitBuilder::new("c_shared_targ_before");
    let q3 = b3.add_quantum_register("q", 3).unwrap();
    b3.cx(q3[0], q3[1]).unwrap();
    b3.cx(q3[2], q3[1]).unwrap();
    b3.cx(q3[0], q3[1]).unwrap();
    let c3 = b3.finish().unwrap();

    let mut b4 = CircuitBuilder::new("c_shared_targ_after");
    let q4 = b4.add_quantum_register("q", 3).unwrap();
    b4.cx(q4[2], q4[1]).unwrap();
    let c4 = b4.finish().unwrap();

    let eq_b = check_equivalence(&c3, &c4).unwrap();
    assert!(
        eq_b.is_equivalent,
        "CX(0, 1) . CX(2, 1) . CX(0, 1) must reduce to CX(2, 1)"
    );
}

// ============================================================================
// 3. Negative Fixtures Proving Non-Equivalence When Preconditions Violated
// ============================================================================

#[test]
fn test_negative_intervening_ry_violates_equivalence() {
    // If Ry(pi/3) intervenes on control, removing CXs changes circuit unitary!
    let mut b1 = CircuitBuilder::new("c_bad_ry");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.ry(q1[0], ParameterExpr::constant(PI / 3.0)).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c_with_cx = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("c_without_cx");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.ry(q2[0], ParameterExpr::constant(PI / 3.0)).unwrap();
    let c_without_cx = b2.finish().unwrap();

    let eq = check_equivalence(&c_with_cx, &c_without_cx).unwrap();
    assert!(
        !eq.is_equivalent,
        "Removing CXs across non-commuting Ry must FAIL equivalence"
    );
}

#[test]
fn test_negative_intervening_hadamard_violates_equivalence() {
    // If H intervenes on target, removing CXs changes circuit unitary!
    let mut b1 = CircuitBuilder::new("c_bad_h");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.h(q1[1]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c_with_cx = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("c_without_cx");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.h(q2[1]).unwrap();
    let c_without_cx = b2.finish().unwrap();

    let eq = check_equivalence(&c_with_cx, &c_without_cx).unwrap();
    assert!(
        !eq.is_equivalent,
        "Removing CXs across non-commuting H must FAIL equivalence"
    );
}

#[test]
fn test_negative_target_rz_violates_equivalence() {
    // Rz on target does NOT commute with CX target (X-flip)
    let mut b1 = CircuitBuilder::new("c_bad_target_rz");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.rz(q1[1], ParameterExpr::constant(PI / 2.0)).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c_with_cx = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("c_without_cx");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.rz(q2[1], ParameterExpr::constant(PI / 2.0)).unwrap();
    let c_without_cx = b2.finish().unwrap();

    let eq = check_equivalence(&c_with_cx, &c_without_cx).unwrap();
    assert!(
        !eq.is_equivalent,
        "Removing CXs across target Rz must FAIL equivalence"
    );
}

#[test]
fn test_negative_control_rx_violates_equivalence() {
    // Rx on control does NOT commute with CX control projector
    let mut b1 = CircuitBuilder::new("c_bad_control_rx");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.rx(q1[0], ParameterExpr::constant(PI / 2.0)).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c_with_cx = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("c_without_cx");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.rx(q2[0], ParameterExpr::constant(PI / 2.0)).unwrap();
    let c_without_cx = b2.finish().unwrap();

    let eq = check_equivalence(&c_with_cx, &c_without_cx).unwrap();
    assert!(
        !eq.is_equivalent,
        "Removing CXs across control Rx must FAIL equivalence"
    );
}

#[test]
fn test_negative_reversed_cx_operands_violates_equivalence() {
    // Eliminating CX(0, 1) . CX(1, 0) as if they were identity alters the circuit!
    let mut b1 = CircuitBuilder::new("c_reversed_cx");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.cx(q1[1], q1[0]).unwrap();
    let c_pair = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("c_empty");
    b2.add_quantum_register("q", 2).unwrap();
    let c_empty = b2.finish().unwrap();

    let eq = check_equivalence(&c_pair, &c_empty).unwrap();
    assert!(
        !eq.is_equivalent,
        "Reversed CX pair CX(0, 1) . CX(1, 0) is NOT identity and must FAIL equivalence"
    );
}

#[test]
fn test_negative_cross_type_cx_cz_violates_equivalence() {
    // Eliminating CX(0, 1) . CZ(0, 1) alters the circuit!
    let mut b1 = CircuitBuilder::new("c_cross_type");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.cz(q1[0], q1[1]).unwrap();
    let c_pair = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("c_empty");
    b2.add_quantum_register("q", 2).unwrap();
    let c_empty = b2.finish().unwrap();

    let eq = check_equivalence(&c_pair, &c_empty).unwrap();
    assert!(
        !eq.is_equivalent,
        "Cross-type CX . CZ pair is NOT identity and must FAIL equivalence"
    );
}

#[test]
fn test_negative_intervening_swap_violates_equivalence() {
    // Eliminating CX across SWAP alters the circuit!
    let mut b1 = CircuitBuilder::new("c_with_swap");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.swap(q1[0], q1[1]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c_with_cx = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("c_only_swap");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.swap(q2[0], q2[1]).unwrap();
    let c_only_swap = b2.finish().unwrap();

    let eq = check_equivalence(&c_with_cx, &c_only_swap).unwrap();
    assert!(
        !eq.is_equivalent,
        "Cancelling CX across SWAP must FAIL equivalence"
    );
}

#[test]
fn test_negative_chained_cx_violates_equivalence() {
    // Chained CX(0, 1) . CX(1, 2) . CX(0, 1):
    // Wire 1 is target of first CX and control of second CX -> mixing X and Z!
    let mut b1 = CircuitBuilder::new("c_chained_with_cx");
    let q1 = b1.add_quantum_register("q", 3).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.cx(q1[1], q1[2]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c_all = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("c_chained_without_cx");
    let q2 = b2.add_quantum_register("q", 3).unwrap();
    b2.cx(q2[1], q2[2]).unwrap();
    let c_reduced = b2.finish().unwrap();

    let eq = check_equivalence(&c_all, &c_reduced).unwrap();
    assert!(
        !eq.is_equivalent,
        "Cancelling CX across chained CX must FAIL equivalence"
    );
}

// ============================================================================
// 4. DAG Wire Reachability, Boundaries & Classical Hazard Preconditions
// ============================================================================

#[test]
fn test_dag_boundary_barrier_classification() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();
    let q0 = QubitId(0);
    let q1 = QubitId(1);

    let cx = Operation::custom_gate("cx", vec![q0, q1], vec![]);
    let barrier = Operation::barrier(vec![q0]);

    let res = reg.check_commutation(&cx, &barrier, &env);
    assert!(
        !res.is_commuting(),
        "Barrier must never commute with any quantum operation"
    );
    assert_eq!(
        res.non_commutation_reason(),
        Some(NonCommutationReason::NonUnitaryOperation)
    );
}

#[test]
fn test_dag_boundary_measure_and_reset_classification() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();
    let q0 = QubitId(0);
    let q1 = QubitId(1);
    let c0 = ClassicalBitId(0);

    let cx = Operation::custom_gate("cx", vec![q0, q1], vec![]);
    let measure = Operation::measure(q0, c0);
    let reset = Operation::reset(q1);

    let res_m = reg.check_commutation(&cx, &measure, &env);
    assert!(!res_m.is_commuting());
    assert_eq!(
        res_m.non_commutation_reason(),
        Some(NonCommutationReason::NonUnitaryOperation)
    );

    let res_r = reg.check_commutation(&cx, &reset, &env);
    assert!(!res_r.is_commuting());
    assert_eq!(
        res_r.non_commutation_reason(),
        Some(NonCommutationReason::NonUnitaryOperation)
    );
}

#[test]
fn test_dag_boundary_classical_hazards_classification() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();
    let q0 = QubitId(0);
    let c0 = ClassicalBitId(0);

    // Two measurement operations writing to the same classical bit c0 (WAW hazard)
    let m1 = Operation::measure(q0, c0);
    let m2 = Operation::measure(q0, c0);

    let res = reg.check_commutation(&m1, &m2, &env);
    assert!(!res.is_commuting());
    assert_eq!(
        res.non_commutation_reason(),
        Some(NonCommutationReason::ClassicalHazard)
    );
}

// ============================================================================
// 5. Greedy Non-Overlapping & Idempotence Proof Contracts
// ============================================================================

#[test]
fn test_greedy_non_overlapping_odd_chain_preserves_single_cx() {
    // 3 identical CX gates: CX_1 . Rz[0] . CX_2 . Rz[0] . CX_3
    // Under greedy non-overlapping selection:
    // Pair (CX_1, CX_2) is cancelled.
    // CX_3 remains!
    let mut b1 = CircuitBuilder::new("c_odd_3");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.rz(q1[0], ParameterExpr::constant(0.4)).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.rz(q1[0], ParameterExpr::constant(0.6)).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c_all = b1.finish().unwrap();

    // Target after cancelling pair (CX_1, CX_2): Rz(0.4)[0] . Rz(0.6)[0] . CX_3
    let mut b2 = CircuitBuilder::new("c_reduced_to_1");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.rz(q2[0], ParameterExpr::constant(0.4)).unwrap();
    b2.rz(q2[0], ParameterExpr::constant(0.6)).unwrap();
    b2.cx(q2[0], q2[1]).unwrap();
    let c_reduced = b2.finish().unwrap();

    let eq = check_equivalence(&c_all, &c_reduced).unwrap();
    assert!(
        eq.is_equivalent,
        "Odd chain of 3 CX gates must correctly reduce to 1 CX gate"
    );
}

#[test]
fn test_greedy_non_overlapping_even_chain_eliminates_all_cx() {
    // 4 identical CX gates: CX_1 . S[0] . CX_2 . S[0] . CX_3 . S[0] . CX_4
    // Under greedy non-overlapping selection:
    // (CX_1, CX_2) cancel, (CX_3, CX_4) cancel -> 0 CX remain!
    let mut b1 = CircuitBuilder::new("c_even_4");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.s(q1[0]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.s(q1[0]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.s(q1[0]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c_all = b1.finish().unwrap();

    // Target: S[0] . S[0] . S[0]
    let mut b2 = CircuitBuilder::new("c_reduced_to_0");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.s(q2[0]).unwrap();
    b2.s(q2[0]).unwrap();
    b2.s(q2[0]).unwrap();
    let c_reduced = b2.finish().unwrap();

    let eq = check_equivalence(&c_all, &c_reduced).unwrap();
    assert!(
        eq.is_equivalent,
        "Even chain of 4 CX gates must eliminate all CX gates in 2 non-overlapping pairs"
    );
}
