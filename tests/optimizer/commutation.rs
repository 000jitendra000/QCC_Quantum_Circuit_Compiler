//! Integration tests for Phase 33 commutation analysis rules and registry.
//!
//! Validates:
//! 1. Matrix-level pairwise verification for all enabled rules ($B \cdot A = A \cdot B$).
//! 2. Disjoint-wire commutation.
//! 3. $Z$-axis operations on $CX$ control.
//! 4. $X$-axis operations on $CX$ target.
//! 5. Single-qubit and multi-qubit Pauli commutation and anticommutation.
//! 6. Multi-qubit controlled gate commutation ($CZ$ with $Z$-axis, $CX$ shared controls, $CX$ shared targets).
//! 7. Operand reversal and control/target sensitivity.
//! 8. Deliberately non-commuting operation pairs.
//! 9. Non-unitary exclusions and classical hazard rejection.
//! 10. Phase 10 unitary equivalence validation for commuting and non-commuting circuit pairs.

use std::collections::HashMap;
use std::f64::consts::PI;

use num_complex::Complex64;
use qcc::builder::CircuitBuilder;
use qcc::commutation::{
    operations_commute, CommutationRegistry, CommutationRule, NonCommutationReason,
};
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::{ClassicalBitId, QubitId};
use qcc::quantum::angle::Angle;
use qcc::quantum::gate_matrix::{
    kron_2x2, matrix_cx_c0_t1, matrix_cx_c1_t0, matrix_cz, matrix_h, matrix_rx, matrix_ry,
    matrix_rz, matrix_s, matrix_sdg, matrix_t, matrix_tdg, matrix_x, matrix_y, matrix_z, Matrix2x2,
    Matrix4x4,
};
use qcc::verification::check_equivalence;

const MATRIX_TOLERANCE: f64 = 1e-12;

// ============================================================================
// Matrix Commutator Helper Functions
// ============================================================================

/// Computes the Frobenius norm of the commutator $[A, B] = A B - B A$ for $2 \times 2$ matrices.
fn commutator_norm_2x2(a: &Matrix2x2, b: &Matrix2x2) -> f64 {
    let ab = a.matmul(b);
    let ba = b.matmul(a);
    ab.sub(&ba).frobenius_norm()
}

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

// ============================================================================
// 1. Matrix-Level Pairwise Verification for Z-Axis on CX Control
// ============================================================================

#[test]
fn test_matrix_cx_control_z_axis_commutation() {
    let cx = matrix_cx_c0_t1(); // q0 is control, q1 is target
    let id2 = Matrix2x2::identity();

    // In little-endian, a single-qubit operator U on q0 acts as I(q1) (x) U(q0)
    let z_gates: Vec<(&str, Matrix2x2)> = vec![
        ("z", matrix_z()),
        ("s", matrix_s()),
        ("sdg", matrix_sdg()),
        ("t", matrix_t()),
        ("tdg", matrix_tdg()),
        ("rz(pi/3)", matrix_rz(Angle::from_radians(PI / 3.0))),
        ("rz(2.5)", matrix_rz(Angle::from_radians(2.5))),
        ("rz(-1.1)", matrix_rz(Angle::from_radians(-1.1))),
    ];

    for (name, u_z) in z_gates {
        let op_4x4 = kron_2x2(&id2, &u_z);
        let comm_norm = commutator_norm_4x4(&cx, &op_4x4);
        assert!(
            comm_norm < MATRIX_TOLERANCE,
            "Z-axis gate '{name}' on CX control must commute at matrix level, got commutator norm {comm_norm}"
        );
    }
}

#[test]
fn test_matrix_cx_control_non_z_axis_strictly_fails() {
    let cx = matrix_cx_c0_t1(); // q0 is control
    let id2 = Matrix2x2::identity();

    let non_z_gates: Vec<(&str, Matrix2x2)> = vec![
        ("x", matrix_x()),
        ("y", matrix_y()),
        ("h", matrix_h()),
        ("rx(pi/3)", matrix_rx(Angle::from_radians(PI / 3.0))),
        ("ry(pi/4)", matrix_ry(Angle::from_radians(PI / 4.0))),
    ];

    for (name, u) in non_z_gates {
        let op_4x4 = kron_2x2(&id2, &u);
        let comm_norm = commutator_norm_4x4(&cx, &op_4x4);
        assert!(
            comm_norm > 0.1,
            "Non-Z gate '{name}' on CX control must NOT commute at matrix level, got commutator norm {comm_norm}"
        );
    }
}

// ============================================================================
// 2. Matrix-Level Pairwise Verification for X-Axis on CX Target
// ============================================================================

#[test]
fn test_matrix_cx_target_x_axis_commutation() {
    let cx = matrix_cx_c0_t1(); // q0 is control, q1 is target
    let id2 = Matrix2x2::identity();

    // In little-endian, a single-qubit operator U on q1 acts as U(q1) (x) I(q0)
    let x_gates: Vec<(&str, Matrix2x2)> = vec![
        ("x", matrix_x()),
        ("rx(pi/4)", matrix_rx(Angle::from_radians(PI / 4.0))),
        ("rx(1.234)", matrix_rx(Angle::from_radians(1.234))),
        ("rx(-2.7)", matrix_rx(Angle::from_radians(-2.7))),
    ];

    for (name, u_x) in x_gates {
        let op_4x4 = kron_2x2(&u_x, &id2);
        let comm_norm = commutator_norm_4x4(&cx, &op_4x4);
        assert!(
            comm_norm < MATRIX_TOLERANCE,
            "X-axis gate '{name}' on CX target must commute at matrix level, got commutator norm {comm_norm}"
        );
    }
}

#[test]
fn test_matrix_cx_target_non_x_axis_strictly_fails() {
    let cx = matrix_cx_c0_t1(); // q1 is target
    let id2 = Matrix2x2::identity();

    let non_x_gates: Vec<(&str, Matrix2x2)> = vec![
        ("z", matrix_z()),
        ("y", matrix_y()),
        ("h", matrix_h()),
        ("s", matrix_s()),
        ("t", matrix_t()),
        ("ry(pi/3)", matrix_ry(Angle::from_radians(PI / 3.0))),
        ("rz(pi/4)", matrix_rz(Angle::from_radians(PI / 4.0))),
    ];

    for (name, u) in non_x_gates {
        let op_4x4 = kron_2x2(&u, &id2);
        let comm_norm = commutator_norm_4x4(&cx, &op_4x4);
        assert!(
            comm_norm > 0.1,
            "Non-X gate '{name}' on CX target must NOT commute at matrix level, got commutator norm {comm_norm}"
        );
    }
}

// ============================================================================
// 3. Reversed CX Orientation (q1 is Control, q0 is Target)
// ============================================================================

#[test]
fn test_matrix_reversed_cx_orientation_commutation() {
    let cx_rev = matrix_cx_c1_t0(); // q1 is control, q0 is target
    let id2 = Matrix2x2::identity();

    // Z-axis on q1 (control) -> U_z (x) I
    let z_on_c = kron_2x2(&matrix_z(), &id2);
    assert!(
        commutator_norm_4x4(&cx_rev, &z_on_c) < MATRIX_TOLERANCE,
        "Z on reversed CX control must commute"
    );

    // X-axis on q0 (target) -> I (x) U_x
    let x_on_t = kron_2x2(&id2, &matrix_x());
    assert!(
        commutator_norm_4x4(&cx_rev, &x_on_t) < MATRIX_TOLERANCE,
        "X on reversed CX target must commute"
    );

    // X-axis on q1 (control) -> must NOT commute
    let x_on_c = kron_2x2(&matrix_x(), &id2);
    assert!(
        commutator_norm_4x4(&cx_rev, &x_on_c) > 0.1,
        "X on reversed CX control must not commute"
    );

    // Z-axis on q0 (target) -> must NOT commute
    let z_on_t = kron_2x2(&id2, &matrix_z());
    assert!(
        commutator_norm_4x4(&cx_rev, &z_on_t) > 0.1,
        "Z on reversed CX target must not commute"
    );
}

// ============================================================================
// 4. Matrix-Level Disjoint Wire Commutation
// ============================================================================

#[test]
fn test_matrix_disjoint_wire_commutation() {
    let id2 = Matrix2x2::identity();

    let gates = vec![
        matrix_x(),
        matrix_y(),
        matrix_z(),
        matrix_h(),
        matrix_s(),
        matrix_t(),
        matrix_rx(Angle::from_radians(1.0)),
        matrix_ry(Angle::from_radians(2.0)),
        matrix_rz(Angle::from_radians(3.0)),
    ];

    for (i, a) in gates.iter().enumerate() {
        for (j, b) in gates.iter().enumerate() {
            // A on q0 -> I (x) A
            let op_a = kron_2x2(&id2, a);
            // B on q1 -> B (x) I
            let op_b = kron_2x2(b, &id2);

            let comm_norm = commutator_norm_4x4(&op_a, &op_b);
            assert!(
                comm_norm < MATRIX_TOLERANCE,
                "Disjoint gates ({i}, {j}) must commute at matrix level, got {comm_norm}"
            );
        }
    }
}

// ============================================================================
// 5. Matrix-Level CZ Commutation
// ============================================================================

#[test]
fn test_matrix_cz_commutation_rules() {
    let cz = matrix_cz();
    let id2 = Matrix2x2::identity();

    // Z on q0 -> I (x) Z
    assert!(commutator_norm_4x4(&cz, &kron_2x2(&id2, &matrix_z())) < MATRIX_TOLERANCE);
    // Z on q1 -> Z (x) I
    assert!(commutator_norm_4x4(&cz, &kron_2x2(&matrix_z(), &id2)) < MATRIX_TOLERANCE);
    // S on q0
    assert!(commutator_norm_4x4(&cz, &kron_2x2(&id2, &matrix_s())) < MATRIX_TOLERANCE);
    // T on q1
    assert!(commutator_norm_4x4(&cz, &kron_2x2(&matrix_t(), &id2)) < MATRIX_TOLERANCE);
    // Rz on q0
    assert!(
        commutator_norm_4x4(&cz, &kron_2x2(&id2, &matrix_rz(Angle::from_radians(1.5))))
            < MATRIX_TOLERANCE
    );

    // Negative: X or Y on either operand of CZ does not commute
    assert!(commutator_norm_4x4(&cz, &kron_2x2(&id2, &matrix_x())) > 0.1);
    assert!(commutator_norm_4x4(&cz, &kron_2x2(&matrix_x(), &id2)) > 0.1);
    assert!(commutator_norm_4x4(&cz, &kron_2x2(&id2, &matrix_y())) > 0.1);
    assert!(commutator_norm_4x4(&cz, &kron_2x2(&matrix_y(), &id2)) > 0.1);
}

// ============================================================================
// 6. Matrix-Level CX-CX Commutation and Reversed Operands
// ============================================================================

#[test]
fn test_matrix_cx_cx_reversed_operands_do_not_commute() {
    let cx_01 = matrix_cx_c0_t1();
    let cx_10 = matrix_cx_c1_t0();

    let comm_norm = commutator_norm_4x4(&cx_01, &cx_10);
    assert!(
        comm_norm > 1.0,
        "Reversed CX(0,1) and CX(1,0) must NOT commute, got norm {comm_norm}"
    );
}

// ============================================================================
// 7. Pauli Commutation and Anticommutation Tests
// ============================================================================

#[test]
fn test_pauli_algebra_and_anticommutation() {
    let px = matrix_x();
    let py = matrix_y();
    let pz = matrix_z();

    // Self-commutation [P, P] = 0
    assert!(commutator_norm_2x2(&px, &px) < MATRIX_TOLERANCE);
    assert!(commutator_norm_2x2(&py, &py) < MATRIX_TOLERANCE);
    assert!(commutator_norm_2x2(&pz, &pz) < MATRIX_TOLERANCE);

    // Anticommutation: {P_i, P_j} = 0, but [P_i, P_j] != 0
    assert!(commutator_norm_2x2(&px, &py) > 1.0);
    assert!(commutator_norm_2x2(&px, &pz) > 1.0);
    assert!(commutator_norm_2x2(&py, &pz) > 1.0);

    // Check anti-commutator {A, B} = AB + BA == 0
    let anti_xy = px.matmul(&py).scale(Complex64::new(1.0, 0.0));
    let yx = py.matmul(&px);
    for r in 0..2 {
        for c in 0..2 {
            let sum = anti_xy.get(r, c) + yx.get(r, c);
            assert!(
                sum.norm() < MATRIX_TOLERANCE,
                "Pauli X and Y must anticommute"
            );
        }
    }
}

#[test]
fn test_pauli_string_tensor_commutation_matrix() {
    // X (x) X and Z (x) Z
    let xx = kron_2x2(&matrix_x(), &matrix_x());
    let zz = kron_2x2(&matrix_z(), &matrix_z());
    assert!(
        commutator_norm_4x4(&xx, &zz) < MATRIX_TOLERANCE,
        "X(x)X and Z(x)Z must commute (2 anticommuting factors)"
    );

    // X (x) Z and Z (x) X
    let xz = kron_2x2(&matrix_x(), &matrix_z());
    let zx = kron_2x2(&matrix_z(), &matrix_x());
    assert!(
        commutator_norm_4x4(&xz, &zx) < MATRIX_TOLERANCE,
        "X(x)Z and Z(x)X must commute"
    );

    // X (x) I and Z (x) Z (1 anticommuting factor -> anticommutes, does not commute!)
    let xi = kron_2x2(&matrix_x(), &Matrix2x2::identity());
    assert!(
        commutator_norm_4x4(&xi, &zz) > 1.0,
        "X(x)I and Z(x)Z must NOT commute"
    );
}

// ============================================================================
// 8. CommutationRegistry and operations_commute API Tests
// ============================================================================

#[test]
fn test_registry_disjoint_wires() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();

    let op1 = Operation::gate(&qcc::quantum::gate::Gate::H, vec![QubitId(0)]);
    let op2 = Operation::gate(&qcc::quantum::gate::Gate::X, vec![QubitId(1)]);

    let res = reg.check_commutation(&op1, &op2, &env);
    assert!(res.is_commuting());
    assert_eq!(res.rule(), Some(CommutationRule::DisjointWires));
}

#[test]
fn test_registry_same_axis_rotations() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();
    let q = QubitId(0);

    let z_gates = ["z", "s", "sdg", "t", "tdg", "rz"];
    for &g1 in &z_gates {
        for &g2 in &z_gates {
            let p1 = if g1 == "rz" {
                vec![ParameterExpr::constant(0.5)]
            } else {
                vec![]
            };
            let p2 = if g2 == "rz" {
                vec![ParameterExpr::constant(1.2)]
            } else {
                vec![]
            };
            let op1 = Operation::custom_gate(g1, vec![q], p1);
            let op2 = Operation::custom_gate(g2, vec![q], p2);

            let res = reg.check_commutation(&op1, &op2, &env);
            assert!(
                res.is_commuting(),
                "Z-axis gates '{g1}' and '{g2}' on same qubit must commute"
            );
        }
    }
}

#[test]
fn test_registry_cx_control_and_target_rules() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();
    let c = QubitId(0);
    let t = QubitId(1);

    let cx = Operation::custom_gate("cx", vec![c, t], vec![]);

    // Z on control commutes
    let z_ctrl = Operation::custom_gate("z", vec![c], vec![]);
    let res = reg.check_commutation(&cx, &z_ctrl, &env);
    assert_eq!(res.rule(), Some(CommutationRule::ZAxisOnCxControl));

    // Rz on control commutes
    let rz_ctrl = Operation::custom_gate("rz", vec![c], vec![ParameterExpr::constant(0.7)]);
    let res = reg.check_commutation(&cx, &rz_ctrl, &env);
    assert_eq!(res.rule(), Some(CommutationRule::ZAxisOnCxControl));

    // X on target commutes
    let x_targ = Operation::custom_gate("x", vec![t], vec![]);
    let res = reg.check_commutation(&cx, &x_targ, &env);
    assert_eq!(res.rule(), Some(CommutationRule::XAxisOnCxTarget));

    // Rx on target commutes
    let rx_targ = Operation::custom_gate("rx", vec![t], vec![ParameterExpr::constant(1.4)]);
    let res = reg.check_commutation(&cx, &rx_targ, &env);
    assert_eq!(res.rule(), Some(CommutationRule::XAxisOnCxTarget));

    // Negative: X on control does NOT commute
    let x_ctrl = Operation::custom_gate("x", vec![c], vec![]);
    let res = reg.check_commutation(&cx, &x_ctrl, &env);
    assert!(!res.is_commuting());
    assert_eq!(
        res.non_commutation_reason(),
        Some(NonCommutationReason::CxControlNonZAxis)
    );

    // Negative: Z on target does NOT commute
    let z_targ = Operation::custom_gate("z", vec![t], vec![]);
    let res = reg.check_commutation(&cx, &z_targ, &env);
    assert!(!res.is_commuting());
    assert_eq!(
        res.non_commutation_reason(),
        Some(NonCommutationReason::CxTargetNonXAxis)
    );
}

#[test]
fn test_registry_cx_cx_commutation_cases() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();

    let q0 = QubitId(0);
    let q1 = QubitId(1);
    let q2 = QubitId(2);

    let cx_01 = Operation::custom_gate("cx", vec![q0, q1], vec![]);
    let cx_02 = Operation::custom_gate("cx", vec![q0, q2], vec![]);
    let cx_12 = Operation::custom_gate("cx", vec![q1, q2], vec![]);
    let cx_10 = Operation::custom_gate("cx", vec![q1, q0], vec![]);

    // Shared control: CX(0,1) and CX(0,2) commute
    let res = reg.check_commutation(&cx_01, &cx_02, &env);
    assert_eq!(res.rule(), Some(CommutationRule::CxSharedControl));

    // Shared target: CX(0,2) and CX(1,2) commute
    let res = reg.check_commutation(&cx_02, &cx_12, &env);
    assert_eq!(res.rule(), Some(CommutationRule::CxSharedTarget));

    // Chained: CX(0,1) and CX(1,2) do NOT commute
    let res = reg.check_commutation(&cx_01, &cx_12, &env);
    assert_eq!(
        res.non_commutation_reason(),
        Some(NonCommutationReason::CxChainedDependency)
    );

    // Reversed: CX(0,1) and CX(1,0) do NOT commute
    let res = reg.check_commutation(&cx_01, &cx_10, &env);
    assert_eq!(
        res.non_commutation_reason(),
        Some(NonCommutationReason::CxReversedOperands)
    );
}

#[test]
fn test_registry_identity_operands() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();
    let q = QubitId(0);

    let id_gate = Operation::custom_gate("id", vec![q], vec![]);
    let any_gate = Operation::custom_gate("h", vec![q], vec![]);

    let res = reg.check_commutation(&id_gate, &any_gate, &env);
    assert_eq!(res.rule(), Some(CommutationRule::IdentityOperand));

    // Rotation with 2pi angle evaluates to identity (-I) and commutes
    let rx_2pi = Operation::custom_gate("rx", vec![q], vec![ParameterExpr::constant(2.0 * PI)]);
    let res = reg.check_commutation(&rx_2pi, &any_gate, &env);
    assert_eq!(res.rule(), Some(CommutationRule::IdentityOperand));
}

// ============================================================================
// 9. Non-Unitary Exclusions and Classical Hazards
// ============================================================================

#[test]
fn test_non_unitary_exclusions() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();
    let q = QubitId(0);
    let c = ClassicalBitId(0);

    let h_gate = Operation::custom_gate("h", vec![q], vec![]);
    let measure = Operation::measure(q, c);
    let reset = Operation::reset(q);
    let barrier = Operation::barrier(vec![q]);
    let classical = Operation::classical("add", vec![c], vec![c]);

    for non_unitary in [measure, reset, barrier, classical] {
        let res = reg.check_commutation(&h_gate, &non_unitary, &env);
        assert!(!res.is_commuting());
        assert_eq!(
            res.non_commutation_reason(),
            Some(NonCommutationReason::NonUnitaryOperation)
        );
    }
}

#[test]
fn test_classical_hazard_exclusion() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();
    let c0 = ClassicalBitId(0);

    // op1 writes c0, op2 reads c0 (RAW hazard)
    let op1 = Operation::classical("write_c0", vec![], vec![c0]);
    let op2 = Operation::classical("read_c0", vec![c0], vec![]);

    let res = reg.check_commutation(&op1, &op2, &env);
    assert!(!res.is_commuting());
    // Non-unitary takes precedence or classical hazard
    assert!(matches!(
        res.non_commutation_reason(),
        Some(NonCommutationReason::NonUnitaryOperation | NonCommutationReason::ClassicalHazard)
    ));
}

#[test]
fn test_operations_commute_direct_function() {
    let env = HashMap::new();
    let q0 = QubitId(0);
    let q1 = QubitId(1);

    let op1 = Operation::custom_gate("x", vec![q0], vec![]);
    let op2 = Operation::custom_gate("z", vec![q1], vec![]);

    let res = operations_commute(&op1, &op2, &env);
    assert!(res.is_commuting());
    assert_eq!(res.rule(), Some(CommutationRule::DisjointWires));
}

#[test]
fn test_deliberately_non_commuting_single_qubit_pairs() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();
    let q = QubitId(0);

    let pairs = [
        ("h", "x"),
        ("h", "z"),
        ("s", "x"),
        ("t", "y"),
        ("rx", "rz"),
        ("ry", "rx"),
        ("ry", "rz"),
    ];

    for &(g1, g2) in &pairs {
        let p1 = if g1.starts_with('r') {
            vec![ParameterExpr::constant(PI / 3.0)]
        } else {
            vec![]
        };
        let p2 = if g2.starts_with('r') {
            vec![ParameterExpr::constant(PI / 4.0)]
        } else {
            vec![]
        };
        let op1 = Operation::custom_gate(g1, vec![q], p1);
        let op2 = Operation::custom_gate(g2, vec![q], p2);

        let res = reg.check_commutation(&op1, &op2, &env);
        assert!(
            !res.is_commuting(),
            "Gates '{g1}' and '{g2}' on same qubit must NOT commute"
        );
        assert!(matches!(
            res.non_commutation_reason(),
            Some(
                NonCommutationReason::CrossAxisRotations | NonCommutationReason::PauliAnticommuting
            )
        ));
    }
}

#[test]
fn test_uncatalogued_and_unsupported_gates() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();
    let q = QubitId(0);

    let op1 = Operation::custom_gate("custom_unknown_gate_xyz", vec![q], vec![]);
    let op2 = Operation::custom_gate("x", vec![q], vec![]);

    let res = reg.check_commutation(&op1, &op2, &env);
    assert!(!res.is_commuting());
    assert_eq!(
        res.non_commutation_reason(),
        Some(NonCommutationReason::UnsupportedOperation)
    );
}

#[test]
fn test_registry_preconditions_coverage() {
    let reg = CommutationRegistry::new();
    let pre = reg.preconditions();
    assert!(!pre.is_empty());
    for p in pre {
        assert!(p.is_exact);
        assert!(!p.supported_operations.is_empty());
        assert!(!p.operand_constraint.is_empty());
        assert!(!p.parameter_constraint.is_empty());
        assert!(!p.rule.title().is_empty());
    }
}

#[test]
fn test_parameterized_rotation_sweeps() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();
    let q = QubitId(0);

    let angles = [
        0.0,
        PI / 6.0,
        PI / 4.0,
        PI / 2.0,
        PI,
        1.5 * PI,
        2.0 * PI,
        4.0 * PI,
        -PI / 3.0,
    ];

    for &theta1 in &angles {
        for &theta2 in &angles {
            let rx1 = Operation::custom_gate("rx", vec![q], vec![ParameterExpr::constant(theta1)]);
            let rx2 = Operation::custom_gate("rx", vec![q], vec![ParameterExpr::constant(theta2)]);

            let res = reg.check_commutation(&rx1, &rx2, &env);
            assert!(
                res.is_commuting(),
                "Rx({theta1}) and Rx({theta2}) must commute on the same qubit"
            );
        }
    }
}

// ============================================================================
// 10. Phase 10 Unitary Equivalence Checker Cross-Verification
// ============================================================================

#[test]
fn test_phase10_equivalence_cross_verification_commuting_pairs() {
    // Case 1: Z on CX control
    let mut b1 = CircuitBuilder::new("c1");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.z(q1[0]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c1 = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("c2");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.cx(q2[0], q2[1]).unwrap();
    b2.z(q2[0]).unwrap();
    let c2 = b2.finish().unwrap();

    let eq_result = check_equivalence(&c1, &c2).unwrap();
    assert!(
        eq_result.is_equivalent,
        "Z on CX control must be unitarily equivalent when swapped"
    );

    // Case 2: X on CX target
    let mut b3 = CircuitBuilder::new("c3");
    let q3 = b3.add_quantum_register("q", 2).unwrap();
    b3.x(q3[1]).unwrap();
    b3.cx(q3[0], q3[1]).unwrap();
    let c3 = b3.finish().unwrap();

    let mut b4 = CircuitBuilder::new("c4");
    let q4 = b4.add_quantum_register("q", 2).unwrap();
    b4.cx(q4[0], q4[1]).unwrap();
    b4.x(q4[1]).unwrap();
    let c4 = b4.finish().unwrap();

    let eq_result = check_equivalence(&c3, &c4).unwrap();
    assert!(
        eq_result.is_equivalent,
        "X on CX target must be unitarily equivalent when swapped"
    );

    // Case 3: Shared control CX(0,1) and CX(0,2)
    let mut b5 = CircuitBuilder::new("c5");
    let q5 = b5.add_quantum_register("q", 3).unwrap();
    b5.cx(q5[0], q5[1]).unwrap();
    b5.cx(q5[0], q5[2]).unwrap();
    let c5 = b5.finish().unwrap();

    let mut b6 = CircuitBuilder::new("c6");
    let q6 = b6.add_quantum_register("q", 3).unwrap();
    b6.cx(q6[0], q6[2]).unwrap();
    b6.cx(q6[0], q6[1]).unwrap();
    let c6 = b6.finish().unwrap();

    let eq_result = check_equivalence(&c5, &c6).unwrap();
    assert!(
        eq_result.is_equivalent,
        "CX gates with shared control must be unitarily equivalent when swapped"
    );
}

#[test]
fn test_phase10_equivalence_cross_verification_non_commuting_pairs() {
    // Case 1: X on CX control (does NOT commute)
    let mut b1 = CircuitBuilder::new("c1");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.x(q1[0]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c1 = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("c2");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.cx(q2[0], q2[1]).unwrap();
    b2.x(q2[0]).unwrap();
    let c2 = b2.finish().unwrap();

    let eq_result = check_equivalence(&c1, &c2).unwrap();
    assert!(
        !eq_result.is_equivalent,
        "X on CX control must NOT be unitarily equivalent when swapped"
    );

    // Case 2: Z on CX target (does NOT commute)
    let mut b3 = CircuitBuilder::new("c3");
    let q3 = b3.add_quantum_register("q", 2).unwrap();
    b3.z(q3[1]).unwrap();
    b3.cx(q3[0], q3[1]).unwrap();
    let c3 = b3.finish().unwrap();

    let mut b4 = CircuitBuilder::new("c4");
    let q4 = b4.add_quantum_register("q", 2).unwrap();
    b4.cx(q4[0], q4[1]).unwrap();
    b4.z(q4[1]).unwrap();
    let c4 = b4.finish().unwrap();

    let eq_result = check_equivalence(&c3, &c4).unwrap();
    assert!(
        !eq_result.is_equivalent,
        "Z on CX target must NOT be unitarily equivalent when swapped"
    );

    // Case 3: Reversed CX(0,1) vs CX(1,0)
    let mut b5 = CircuitBuilder::new("c5");
    let q5 = b5.add_quantum_register("q", 2).unwrap();
    b5.cx(q5[0], q5[1]).unwrap();
    b5.cx(q5[1], q5[0]).unwrap();
    let c5 = b5.finish().unwrap();

    let mut b6 = CircuitBuilder::new("c6");
    let q6 = b6.add_quantum_register("q", 2).unwrap();
    b6.cx(q6[1], q6[0]).unwrap();
    b6.cx(q6[0], q6[1]).unwrap();
    let c6 = b6.finish().unwrap();

    let eq_result = check_equivalence(&c5, &c6).unwrap();
    assert!(
        !eq_result.is_equivalent,
        "Reversed CX gates must NOT be unitarily equivalent"
    );
}

// ============================================================================
// 11. Basis-State Action Proof for Kronecker Embeddings (Little-Endian)
// ============================================================================

#[test]
fn test_basis_state_action_proves_kronecker_little_endian() {
    let id2 = Matrix2x2::identity();
    let x = matrix_x();

    // In little-endian |q1 q0>:
    // q0 is LSB (inner factor in standard Kronecker product)
    // q1 is MSB (outer factor in standard Kronecker product)
    //
    // Therefore, X on q0 is kron(I, X)
    // and X on q1 is kron(X, I)
    let x_on_q0 = kron_2x2(&id2, &x);
    let x_on_q1 = kron_2x2(&x, &id2);

    // Basis vectors e_0 = |00>, e_1 = |01>, e_2 = |10>, e_3 = |11>
    let basis_vectors = [
        [
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ],
    ];

    let matmul_vec = |m: &Matrix4x4, v: &[Complex64; 4]| -> [Complex64; 4] {
        let mut out = [Complex64::new(0.0, 0.0); 4];
        for (r, out_elem) in out.iter_mut().enumerate() {
            for (c, &v_elem) in v.iter().enumerate() {
                *out_elem += m.get(r, c) * v_elem;
            }
        }
        out
    };

    // Assert x_on_q0 (kron(I, X)) flips q0:
    // |00> (index 0) -> |01> (index 1)
    // |01> (index 1) -> |00> (index 0)
    // |10> (index 2) -> |11> (index 3)
    // |11> (index 3) -> |10> (index 2)
    assert_eq!(matmul_vec(&x_on_q0, &basis_vectors[0]), basis_vectors[1]);
    assert_eq!(matmul_vec(&x_on_q0, &basis_vectors[1]), basis_vectors[0]);
    assert_eq!(matmul_vec(&x_on_q0, &basis_vectors[2]), basis_vectors[3]);
    assert_eq!(matmul_vec(&x_on_q0, &basis_vectors[3]), basis_vectors[2]);

    // Assert x_on_q1 (kron(X, I)) flips q1:
    // |00> (index 0) -> |10> (index 2)
    // |01> (index 1) -> |11> (index 3)
    // |10> (index 2) -> |00> (index 0)
    // |11> (index 3) -> |01> (index 1)
    assert_eq!(matmul_vec(&x_on_q1, &basis_vectors[0]), basis_vectors[2]);
    assert_eq!(matmul_vec(&x_on_q1, &basis_vectors[1]), basis_vectors[3]);
    assert_eq!(matmul_vec(&x_on_q1, &basis_vectors[2]), basis_vectors[0]);
    assert_eq!(matmul_vec(&x_on_q1, &basis_vectors[3]), basis_vectors[1]);
}

// ============================================================================
// 12. Matrix Verification for Same-Axis Rotations and Scalar Identity
// ============================================================================

#[test]
fn test_matrix_same_axis_rotations_pairwise() {
    // X-axis rotations
    let rx1 = matrix_rx(Angle::from_radians(0.7));
    let rx2 = matrix_rx(Angle::from_radians(2.1));
    assert!(commutator_norm_2x2(&rx1, &rx2) < MATRIX_TOLERANCE);
    assert!(commutator_norm_2x2(&matrix_x(), &rx1) < MATRIX_TOLERANCE);

    // Y-axis rotations
    let ry1 = matrix_ry(Angle::from_radians(1.3));
    let ry2 = matrix_ry(Angle::from_radians(-0.9));
    assert!(commutator_norm_2x2(&ry1, &ry2) < MATRIX_TOLERANCE);
    assert!(commutator_norm_2x2(&matrix_y(), &ry1) < MATRIX_TOLERANCE);

    // Z-axis rotations and discrete gates
    let rz1 = matrix_rz(Angle::from_radians(0.4));
    let rz2 = matrix_rz(Angle::from_radians(1.8));
    assert!(commutator_norm_2x2(&rz1, &rz2) < MATRIX_TOLERANCE);
    assert!(commutator_norm_2x2(&matrix_z(), &rz1) < MATRIX_TOLERANCE);
    assert!(commutator_norm_2x2(&matrix_s(), &matrix_t()) < MATRIX_TOLERANCE);
    assert!(commutator_norm_2x2(&matrix_sdg(), &matrix_tdg()) < MATRIX_TOLERANCE);
    assert!(commutator_norm_2x2(&matrix_s(), &rz2) < MATRIX_TOLERANCE);
}

#[test]
fn test_matrix_identity_operand_pairwise() {
    let id2 = Matrix2x2::identity();
    let neg_id2 = id2.scale(Complex64::new(-1.0, 0.0)); // -I

    for gate in [
        matrix_x(),
        matrix_y(),
        matrix_z(),
        matrix_h(),
        matrix_s(),
        matrix_t(),
        matrix_rx(Angle::from_radians(1.23)),
        matrix_ry(Angle::from_radians(2.34)),
        matrix_rz(Angle::from_radians(3.45)),
    ] {
        assert!(commutator_norm_2x2(&id2, &gate) < MATRIX_TOLERANCE);
        assert!(commutator_norm_2x2(&neg_id2, &gate) < MATRIX_TOLERANCE);
    }
}

// ============================================================================
// 13. Independent 3-Qubit Basis Action Verification for Controlled Gates
// ============================================================================

#[test]
fn test_3qubit_controlled_commutation_pairwise() {
    // Basis state integer index j in 0..8:
    // bit 0 = q0, bit 1 = q1, bit 2 = q2
    // |q2 q1 q0>

    let apply_cx = |j: usize, ctrl: usize, targ: usize| -> usize {
        let c_val = (j >> ctrl) & 1;
        if c_val == 1 {
            j ^ (1 << targ)
        } else {
            j
        }
    };

    let apply_cz_phase = |j: usize, q0: usize, q1: usize| -> f64 {
        let b0 = (j >> q0) & 1;
        let b1 = (j >> q1) & 1;
        if b0 == 1 && b1 == 1 {
            -1.0
        } else {
            1.0
        }
    };

    // 1. Shared control CX(0,1) and CX(0,2):
    for j in 0..8 {
        let order1 = apply_cx(apply_cx(j, 0, 1), 0, 2);
        let order2 = apply_cx(apply_cx(j, 0, 2), 0, 1);
        assert_eq!(
            order1, order2,
            "Shared control CX(0,1) and CX(0,2) must commute on basis state {j}"
        );
    }

    // 2. Shared target CX(0,2) and CX(1,2):
    for j in 0..8 {
        let order1 = apply_cx(apply_cx(j, 0, 2), 1, 2);
        let order2 = apply_cx(apply_cx(j, 1, 2), 0, 2);
        assert_eq!(
            order1, order2,
            "Shared target CX(0,2) and CX(1,2) must commute on basis state {j}"
        );
    }

    // 3. Shared control CX(0,1) and CZ(0,2):
    for j in 0..8 {
        // CX then CZ
        let state_after_cx = apply_cx(j, 0, 1);
        let phase_cz1 = apply_cz_phase(state_after_cx, 0, 2);

        // CZ then CX
        let phase_cz2 = apply_cz_phase(j, 0, 2);
        let state_after_cx2 = apply_cx(j, 0, 1);

        assert_eq!(state_after_cx, state_after_cx2);
        assert_eq!(
            phase_cz1, phase_cz2,
            "CX(0,1) and CZ(0,2) must commute on basis state {j}"
        );
    }

    // 4. Negative: Chained CX(0,1) and CX(1,2) fail on j = 1 (|001>, q0=1):
    let j_test = 1; // q0 = 1, q1 = 0, q2 = 0
    let chained1 = apply_cx(apply_cx(j_test, 0, 1), 1, 2); // CX(0,1) flips q1 to 1; CX(1,2) flips q2 to 1 -> j = 7 (|111>)
    let chained2 = apply_cx(apply_cx(j_test, 1, 2), 0, 1); // CX(1,2) sees q1=0 (noop); CX(0,1) flips q1 to 1 -> j = 3 (|011>)
    assert_ne!(
        chained1, chained2,
        "Chained CX(0,1) and CX(1,2) must NOT commute"
    );
}

#[test]
fn test_registry_cx_cz_shared_control_rule() {
    let reg = CommutationRegistry::new();
    let env = HashMap::new();

    let q0 = QubitId(0);
    let q1 = QubitId(1);
    let q2 = QubitId(2);

    let cx_01 = Operation::custom_gate("cx", vec![q0, q1], vec![]);
    let cz_02 = Operation::custom_gate("cz", vec![q0, q2], vec![]);
    let cz_12 = Operation::custom_gate("cz", vec![q1, q2], vec![]);

    // CX(0,1) and CZ(0,2) share control q0 with disjoint targets -> Commutes under CxCzSharedControl
    let res = reg.check_commutation(&cx_01, &cz_02, &env);
    assert!(res.is_commuting());
    assert_eq!(res.rule(), Some(CommutationRule::CxCzSharedControl));

    // Reverse query order: CZ(0,2) and CX(0,1) -> Commutes under CxCzSharedControl
    let res_rev = reg.check_commutation(&cz_02, &cx_01, &env);
    assert!(res_rev.is_commuting());
    assert_eq!(res_rev.rule(), Some(CommutationRule::CxCzSharedControl));

    // CX(0,1) and CZ(1,2) touch CX target q1 -> Fails with CxTargetNonXAxis
    let res_fail = reg.check_commutation(&cx_01, &cz_12, &env);
    assert!(!res_fail.is_commuting());
    assert_eq!(
        res_fail.non_commutation_reason(),
        Some(NonCommutationReason::CxTargetNonXAxis)
    );
}
