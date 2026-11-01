//! Comprehensive integration, unit, and property tests for Gate Algebra and Identity Registry (Phase 14).
//!
//! Validates:
//! 1. Inverses: self-inverse classification, discrete inverse pairs, continuous rotation inverses,
//!    operand symmetry, non-unitary/uncatalogued rejections.
//! 2. Canonical Identity Registry: every identity verified against independent matrix representations
//!    under strict tolerances ($10^{-12}$).
//! 3. Periodicity and phase distinction: exact $4\pi$ vs phase $\pi$ at $2\pi$.
//! 4. Parameter normalization and expression simplification algebra.
//! 5. Proptest property checks for randomized angles and rotation compositions.
//! 6. Negative injection tests for phase omission, incorrect angles, and invalid operations.

use std::f64::consts::PI;

use num_complex::Complex64;
use proptest::prelude::*;
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::{ClassicalBitId, QubitId};
use qcc::optimizer::algebra::*;
use qcc::quantum::gate::Gate;
use qcc::quantum::gate_matrix::{matrix_rx, matrix_ry, matrix_rz, Matrix2x2};
use qcc::quantum::parameter::Angle;
use qcc::verification::tolerance::VerificationTolerances;

// ============================================================================
// 1. Inverses and Self-Inverse Gate Tests
// ============================================================================

#[test]
fn test_is_self_inverse() {
    // True cases (case-insensitive)
    for name in &[
        "x", "X", "y", "Y", "z", "Z", "h", "H", "cx", "CX", "cz", "CZ", "swap", "SWAP",
    ] {
        assert!(
            is_self_inverse(name),
            "gate '{name}' should be self-inverse"
        );
    }

    // False cases
    for name in &[
        "s", "sdg", "t", "tdg", "rx", "ry", "rz", "u3", "ccx", "unknown",
    ] {
        assert!(
            !is_self_inverse(name),
            "gate '{name}' should NOT be self-inverse"
        );
    }
}

#[test]
fn test_invert_gate_name() {
    assert_eq!(invert_gate_name("x"), Some("x"));
    assert_eq!(invert_gate_name("y"), Some("y"));
    assert_eq!(invert_gate_name("z"), Some("z"));
    assert_eq!(invert_gate_name("h"), Some("h"));
    assert_eq!(invert_gate_name("s"), Some("sdg"));
    assert_eq!(invert_gate_name("sdg"), Some("s"));
    assert_eq!(invert_gate_name("t"), Some("tdg"));
    assert_eq!(invert_gate_name("tdg"), Some("t"));
    assert_eq!(invert_gate_name("rx"), Some("rx"));
    assert_eq!(invert_gate_name("ry"), Some("ry"));
    assert_eq!(invert_gate_name("rz"), Some("rz"));
    assert_eq!(invert_gate_name("cx"), Some("cx"));
    assert_eq!(invert_gate_name("cz"), Some("cz"));
    assert_eq!(invert_gate_name("swap"), Some("swap"));
    assert_eq!(invert_gate_name("custom_gate"), None);
}

#[test]
fn test_invert_typed_gate() {
    assert_eq!(invert_gate(&Gate::X), Gate::X);
    assert_eq!(invert_gate(&Gate::Y), Gate::Y);
    assert_eq!(invert_gate(&Gate::Z), Gate::Z);
    assert_eq!(invert_gate(&Gate::H), Gate::H);
    assert_eq!(invert_gate(&Gate::S), Gate::Sdg);
    assert_eq!(invert_gate(&Gate::Sdg), Gate::S);
    assert_eq!(invert_gate(&Gate::T), Gate::Tdg);
    assert_eq!(invert_gate(&Gate::Tdg), Gate::T);
    assert_eq!(invert_gate(&Gate::CX), Gate::CX);
    assert_eq!(invert_gate(&Gate::CZ), Gate::CZ);
    assert_eq!(invert_gate(&Gate::SWAP), Gate::SWAP);

    let angle = Angle::from_radians(0.75);
    let neg_angle = Angle::from_radians(-0.75);
    assert_eq!(invert_gate(&Gate::Rx(angle)), Gate::Rx(neg_angle));
    assert_eq!(invert_gate(&Gate::Ry(angle)), Gate::Ry(neg_angle));
    assert_eq!(invert_gate(&Gate::Rz(angle)), Gate::Rz(neg_angle));
}

#[test]
fn test_invert_operation_success() {
    let q0 = QubitId(0);
    let q1 = QubitId(1);

    // Self-inverse discrete op
    let op_h = Operation::Gate {
        gate_name: "h".to_string(),
        qubits: vec![q0],
        parameters: vec![],
    };
    let inv_h = invert_operation(&op_h).unwrap();
    assert_eq!(inv_h, op_h);

    // Discrete pair
    let op_s = Operation::Gate {
        gate_name: "s".to_string(),
        qubits: vec![q0],
        parameters: vec![],
    };
    let inv_s = invert_operation(&op_s).unwrap();
    assert_eq!(
        inv_s,
        Operation::Gate {
            gate_name: "sdg".to_string(),
            qubits: vec![q0],
            parameters: vec![],
        }
    );

    // Discrete pair reverse
    let op_tdg = Operation::Gate {
        gate_name: "tdg".to_string(),
        qubits: vec![q0],
        parameters: vec![],
    };
    let inv_tdg = invert_operation(&op_tdg).unwrap();
    assert_eq!(
        inv_tdg,
        Operation::Gate {
            gate_name: "t".to_string(),
            qubits: vec![q0],
            parameters: vec![],
        }
    );

    // Parameterized rotation
    let op_rz = Operation::Gate {
        gate_name: "rz".to_string(),
        qubits: vec![q1],
        parameters: vec![ParameterExpr::Constant(1.23)],
    };
    let inv_rz = invert_operation(&op_rz).unwrap();
    assert_eq!(
        inv_rz,
        Operation::Gate {
            gate_name: "rz".to_string(),
            qubits: vec![q1],
            parameters: vec![ParameterExpr::Constant(-1.23)],
        }
    );

    // Symbolic negation
    let op_rx_sym = Operation::Gate {
        gate_name: "rx".to_string(),
        qubits: vec![q0],
        parameters: vec![ParameterExpr::Symbolic("alpha".to_string())],
    };
    let inv_rx_sym = invert_operation(&op_rx_sym).unwrap();
    assert_eq!(
        inv_rx_sym,
        Operation::Gate {
            gate_name: "rx".to_string(),
            qubits: vec![q0],
            parameters: vec![ParameterExpr::Neg(Box::new(ParameterExpr::Symbolic(
                "alpha".to_string()
            )))],
        }
    );
}

#[test]
fn test_invert_operation_non_unitary_rejections() {
    let q0 = QubitId(0);
    let c0 = ClassicalBitId(0);

    let op_meas = Operation::Measure {
        qubit: q0,
        destination: c0,
    };
    assert_eq!(
        invert_operation(&op_meas),
        Err(AlgebraError::NonUnitaryOperation)
    );

    let op_reset = Operation::Reset { qubit: q0 };
    assert_eq!(
        invert_operation(&op_reset),
        Err(AlgebraError::NonUnitaryOperation)
    );

    let op_barrier = Operation::Barrier { qubits: vec![q0] };
    assert_eq!(
        invert_operation(&op_barrier),
        Err(AlgebraError::NonUnitaryOperation)
    );
}

#[test]
fn test_invert_operation_uncatalogued_rejection() {
    let q0 = QubitId(0);
    let op_custom = Operation::Gate {
        gate_name: "custom_unitary".to_string(),
        qubits: vec![q0],
        parameters: vec![],
    };
    assert_eq!(
        invert_operation(&op_custom),
        Err(AlgebraError::NoInverseRegistered {
            name: "custom_unitary".to_string()
        })
    );
}

#[test]
fn test_are_inverses() {
    let q0 = QubitId(0);
    let q1 = QubitId(1);
    let q2 = QubitId(2);

    let h0 = Operation::Gate {
        gate_name: "h".to_string(),
        qubits: vec![q0],
        parameters: vec![],
    };
    let h1 = Operation::Gate {
        gate_name: "h".to_string(),
        qubits: vec![q1],
        parameters: vec![],
    };
    let x0 = Operation::Gate {
        gate_name: "x".to_string(),
        qubits: vec![q0],
        parameters: vec![],
    };

    // Identical self-inverses on same qubit
    assert!(are_inverses(&h0, &h0));
    // Different qubits
    assert!(!are_inverses(&h0, &h1));
    // Different gates
    assert!(!are_inverses(&h0, &x0));

    // Discrete pairs
    let s0 = Operation::Gate {
        gate_name: "s".to_string(),
        qubits: vec![q0],
        parameters: vec![],
    };
    let sdg0 = Operation::Gate {
        gate_name: "sdg".to_string(),
        qubits: vec![q0],
        parameters: vec![],
    };
    assert!(are_inverses(&s0, &sdg0));
    assert!(are_inverses(&sdg0, &s0));
    assert!(!are_inverses(&s0, &s0));

    // Two-qubit asymmetric: CX
    let cx_01 = Operation::Gate {
        gate_name: "cx".to_string(),
        qubits: vec![q0, q1],
        parameters: vec![],
    };
    let cx_10 = Operation::Gate {
        gate_name: "cx".to_string(),
        qubits: vec![q1, q0],
        parameters: vec![],
    };
    assert!(are_inverses(&cx_01, &cx_01));
    assert!(!are_inverses(&cx_01, &cx_10));

    // Two-qubit symmetric: CZ and SWAP
    let cz_01 = Operation::Gate {
        gate_name: "cz".to_string(),
        qubits: vec![q0, q1],
        parameters: vec![],
    };
    let cz_10 = Operation::Gate {
        gate_name: "cz".to_string(),
        qubits: vec![q1, q0],
        parameters: vec![],
    };
    assert!(are_inverses(&cz_01, &cz_01));
    assert!(are_inverses(&cz_01, &cz_10));

    let swap_01 = Operation::Gate {
        gate_name: "swap".to_string(),
        qubits: vec![q0, q1],
        parameters: vec![],
    };
    let swap_10 = Operation::Gate {
        gate_name: "swap".to_string(),
        qubits: vec![q1, q0],
        parameters: vec![],
    };
    assert!(are_inverses(&swap_01, &swap_01));
    assert!(are_inverses(&swap_01, &swap_10));

    // Rotations: inverse angles mod 4pi
    let rz_pos = Operation::Gate {
        gate_name: "rz".to_string(),
        qubits: vec![q2],
        parameters: vec![ParameterExpr::Constant(0.6)],
    };
    let rz_neg = Operation::Gate {
        gate_name: "rz".to_string(),
        qubits: vec![q2],
        parameters: vec![ParameterExpr::Constant(-0.6)],
    };
    let rz_4pi_minus = Operation::Gate {
        gate_name: "rz".to_string(),
        qubits: vec![q2],
        parameters: vec![ParameterExpr::Constant(4.0 * PI - 0.6)],
    };
    let rz_2pi_minus = Operation::Gate {
        gate_name: "rz".to_string(),
        qubits: vec![q2],
        parameters: vec![ParameterExpr::Constant(2.0 * PI - 0.6)],
    };

    assert!(are_inverses(&rz_pos, &rz_neg));
    assert!(are_inverses(&rz_pos, &rz_4pi_minus));
    // 2pi shift gives -I, NOT I!
    assert!(!are_inverses(&rz_pos, &rz_2pi_minus));
}

// ============================================================================
// 2. Canonical Identity Registry Verification
// ============================================================================

#[test]
fn test_registry_size_and_completeness() {
    let registry = IdentityRegistry::canonical();
    assert!(!registry.is_empty());
    assert!(
        registry.len() >= 30,
        "Expected at least 30 canonical identities, found {}",
        registry.len()
    );

    // Verify categories are populated
    assert!(
        registry
            .by_category(IdentityCategory::SelfInverseCancellation)
            .count()
            > 0
    );
    assert!(
        registry
            .by_category(IdentityCategory::InversePairCancellation)
            .count()
            > 0
    );
    assert!(
        registry
            .by_category(IdentityCategory::PowerEquivalence)
            .count()
            > 0
    );
    assert!(
        registry
            .by_category(IdentityCategory::PauliConjugation)
            .count()
            > 0
    );
    assert!(
        registry
            .by_category(IdentityCategory::RotationComposition)
            .count()
            > 0
    );
    assert!(
        registry
            .by_category(IdentityCategory::RotationPeriodicity)
            .count()
            > 0
    );
    assert!(
        registry
            .by_category(IdentityCategory::TwoQubitDecomposition)
            .count()
            > 0
    );
}

#[test]
fn test_all_canonical_identities_pass_matrix_verification() {
    let registry = IdentityRegistry::canonical();
    let strict_tol = VerificationTolerances::strict().matrix_tolerance;
    assert_eq!(strict_tol, 1.0e-12);

    let test_params = [
        0.0,
        0.25 * PI,
        0.5 * PI,
        0.8123,
        1.0 * PI,
        1.7 * PI,
        2.5 * PI,
        4.0 * PI,
    ];

    for identity in registry.all() {
        for &param in &test_params {
            let res = identity.verify(param, strict_tol);
            assert!(
                res.is_ok(),
                "Identity '{}' ({}) failed verification at param={}: {:?}",
                identity.id,
                identity.name,
                param,
                res.err()
            );
        }
    }
}

#[test]
fn test_discrete_gate_powers() {
    let registry = IdentityRegistry::canonical();
    let strict_tol = VerificationTolerances::strict().matrix_tolerance;

    for id in &["s_squared_is_z", "t_squared_is_s", "t_fourth_is_z"] {
        let identity = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("missing identity '{id}'"));
        assert_eq!(identity.category, IdentityCategory::PowerEquivalence);
        assert_eq!(identity.kind, IdentityKind::Exact);
        assert!(identity.verify(0.0, strict_tol).is_ok());
    }
}

#[test]
fn test_clifford_conjugations() {
    let registry = IdentityRegistry::canonical();
    let strict_tol = VerificationTolerances::strict().matrix_tolerance;

    for id in &["h_x_h_is_z", "h_z_h_is_x", "s_x_sdg_is_y", "s_z_sdg_is_z"] {
        let identity = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("missing identity '{id}'"));
        assert_eq!(identity.category, IdentityCategory::PauliConjugation);
        assert_eq!(identity.kind, IdentityKind::Exact);
        assert!(identity.verify(0.0, strict_tol).is_ok());
    }

    // H · Y · H = -Y = e^{iπ} Y (Global phase factor of π)
    let h_y_h = registry.by_id("h_y_h_is_neg_y").unwrap();
    assert_eq!(h_y_h.category, IdentityCategory::PauliConjugation);
    assert_eq!(
        h_y_h.kind,
        IdentityKind::GlobalPhaseEquivalent { phase: PI }
    );
    assert!(h_y_h.verify(0.0, strict_tol).is_ok());
}

#[test]
fn test_rotation_discrete_conversions() {
    let registry = IdentityRegistry::canonical();
    let strict_tol = VerificationTolerances::strict().matrix_tolerance;

    // Rz(pi) = -i Z = e^{-i pi/2} Z
    let rz_pi = registry.by_id("rz_pi_is_phase_z").unwrap();
    assert_eq!(
        rz_pi.kind,
        IdentityKind::GlobalPhaseEquivalent { phase: -0.5 * PI }
    );
    assert!(rz_pi.verify(0.0, strict_tol).is_ok());

    // Rx(pi) = -i X = e^{-i pi/2} X
    let rx_pi = registry.by_id("rx_pi_is_phase_x").unwrap();
    assert_eq!(
        rx_pi.kind,
        IdentityKind::GlobalPhaseEquivalent { phase: -0.5 * PI }
    );
    assert!(rx_pi.verify(0.0, strict_tol).is_ok());

    // Ry(pi) = -i Y = e^{-i pi/2} Y
    let ry_pi = registry.by_id("ry_pi_is_phase_y").unwrap();
    assert_eq!(
        ry_pi.kind,
        IdentityKind::GlobalPhaseEquivalent { phase: -0.5 * PI }
    );
    assert!(ry_pi.verify(0.0, strict_tol).is_ok());

    // Rz(pi/2) = e^{-i pi/4} S
    let rz_s = registry.by_id("rz_half_pi_is_phase_s").unwrap();
    assert_eq!(
        rz_s.kind,
        IdentityKind::GlobalPhaseEquivalent { phase: -0.25 * PI }
    );
    assert!(rz_s.verify(0.0, strict_tol).is_ok());

    // Rz(pi/4) = e^{-i pi/8} T
    let rz_t = registry.by_id("rz_fourth_pi_is_phase_t").unwrap();
    assert_eq!(
        rz_t.kind,
        IdentityKind::GlobalPhaseEquivalent { phase: -0.125 * PI }
    );
    assert!(rz_t.verify(0.0, strict_tol).is_ok());
}

#[test]
fn test_rotation_periodicity_distinction() {
    let registry = IdentityRegistry::canonical();
    let strict_tol = VerificationTolerances::strict().matrix_tolerance;

    // 4pi exact periodicity
    for axis in &["rz", "rx", "ry"] {
        let id_4pi = format!("{axis}_four_pi_periodicity");
        let ident = registry
            .by_id(&id_4pi)
            .unwrap_or_else(|| panic!("missing {id_4pi}"));
        assert_eq!(ident.kind, IdentityKind::Exact);
        assert!(ident.verify(1.234, strict_tol).is_ok());

        // 2pi carries scalar phase of pi
        let id_2pi = format!("{axis}_two_pi_periodicity");
        let ident_2pi = registry
            .by_id(&id_2pi)
            .unwrap_or_else(|| panic!("missing {id_2pi}"));
        assert_eq!(
            ident_2pi.kind,
            IdentityKind::GlobalPhaseEquivalent { phase: PI }
        );
        assert!(ident_2pi.verify(1.234, strict_tol).is_ok());
    }
}

#[test]
fn test_two_qubit_decompositions() {
    let registry = IdentityRegistry::canonical();
    let strict_tol = VerificationTolerances::strict().matrix_tolerance;

    let swap_3cx = registry.by_id("swap_via_three_cx").unwrap();
    assert_eq!(swap_3cx.kind, IdentityKind::Exact);
    assert_eq!(swap_3cx.arity, 2);
    assert!(swap_3cx.verify(0.0, strict_tol).is_ok());

    let cx_rev = registry.by_id("cx_direction_reversal").unwrap();
    assert_eq!(cx_rev.kind, IdentityKind::Exact);
    assert_eq!(cx_rev.arity, 2);
    assert!(cx_rev.verify(0.0, strict_tol).is_ok());

    let cz_cx = registry.by_id("cz_via_cx_h").unwrap();
    assert_eq!(cz_cx.kind, IdentityKind::Exact);
    assert_eq!(cz_cx.arity, 2);
    assert!(cz_cx.verify(0.0, strict_tol).is_ok());

    let cz_sym = registry.by_id("cz_symmetry").unwrap();
    assert_eq!(cz_sym.kind, IdentityKind::Exact);
    assert_eq!(cz_sym.arity, 2);
    assert!(cz_sym.verify(0.0, strict_tol).is_ok());

    let swap_sym = registry.by_id("swap_symmetry").unwrap();
    assert_eq!(swap_sym.kind, IdentityKind::Exact);
    assert_eq!(swap_sym.arity, 2);
    assert!(swap_sym.verify(0.0, strict_tol).is_ok());
}

// ============================================================================
// 3. Parameter Normalization & Expression Simplification
// ============================================================================

#[test]
fn test_normalize_rotation_angle_exact() {
    let tol = 1e-12;

    // Inside [0, 4pi)
    assert!((normalize_rotation_angle_exact(0.0) - 0.0).abs() < tol);
    assert!((normalize_rotation_angle_exact(PI) - PI).abs() < tol);
    assert!((normalize_rotation_angle_exact(2.0 * PI) - 2.0 * PI).abs() < tol);
    assert!((normalize_rotation_angle_exact(3.5 * PI) - 3.5 * PI).abs() < tol);

    // Shifts by 4pi
    assert!((normalize_rotation_angle_exact(4.0 * PI) - 0.0).abs() < tol);
    assert!((normalize_rotation_angle_exact(5.0 * PI) - PI).abs() < tol);
    assert!((normalize_rotation_angle_exact(8.0 * PI) - 0.0).abs() < tol);

    // Negative shifts
    assert!((normalize_rotation_angle_exact(-PI) - 3.0 * PI).abs() < tol);
    assert!((normalize_rotation_angle_exact(-4.0 * PI) - 0.0).abs() < tol);
    assert!((normalize_rotation_angle_exact(-0.5 * PI) - 3.5 * PI).abs() < tol);
}

#[test]
fn test_normalize_rotation_angle_2pi() {
    let tol = 1e-12;

    // In [0, 2pi) -> phase is 0
    let (angle1, phase1) = normalize_rotation_angle_2pi(0.5 * PI);
    assert!((angle1 - 0.5 * PI).abs() < tol);
    assert!(phase1.abs() < tol);

    // In [2pi, 4pi) -> normalized into [0, 2pi), phase is PI
    let (angle2, phase2) = normalize_rotation_angle_2pi(2.5 * PI);
    assert!((angle2 - 0.5 * PI).abs() < tol);
    assert!((phase2 - PI).abs() < tol);

    // Negative angle: -0.5*PI == 3.5*PI in [0, 4pi), so in [2pi, 4pi) -> normalized to 1.5*PI, phase PI
    let (angle3, phase3) = normalize_rotation_angle_2pi(-0.5 * PI);
    assert!((angle3 - 1.5 * PI).abs() < tol);
    assert!((phase3 - PI).abs() < tol);

    // -2pi == 2pi in [0, 4pi) -> normalized to 0.0, phase PI
    let (angle4, phase4) = normalize_rotation_angle_2pi(-2.0 * PI);
    assert!(angle4.abs() < tol);
    assert!((phase4 - PI).abs() < tol);

    // -4pi == 0.0 in [0, 4pi) -> normalized to 0.0, phase 0
    let (angle5, phase5) = normalize_rotation_angle_2pi(-4.0 * PI);
    assert!(angle5.abs() < tol);
    assert!(phase5.abs() < tol);
}

#[test]
fn test_simplify_parameter_expr() {
    let x = ParameterExpr::Symbolic("x".to_string());
    let zero = ParameterExpr::Constant(0.0);
    let one = ParameterExpr::Constant(1.0);
    let two = ParameterExpr::Constant(2.0);
    let three = ParameterExpr::Constant(3.0);

    // Constant arithmetic folding
    assert_eq!(
        simplify_parameter_expr(&ParameterExpr::Add(
            Box::new(two.clone()),
            Box::new(three.clone())
        )),
        ParameterExpr::Constant(5.0)
    );
    assert_eq!(
        simplify_parameter_expr(&ParameterExpr::Sub(
            Box::new(three.clone()),
            Box::new(two.clone())
        )),
        ParameterExpr::Constant(1.0)
    );
    assert_eq!(
        simplify_parameter_expr(&ParameterExpr::Mul(
            Box::new(two.clone()),
            Box::new(three.clone())
        )),
        ParameterExpr::Constant(6.0)
    );
    assert_eq!(
        simplify_parameter_expr(&ParameterExpr::Div(
            Box::new(ParameterExpr::Constant(6.0)),
            Box::new(two.clone())
        )),
        ParameterExpr::Constant(3.0)
    );

    // Add zero
    assert_eq!(
        simplify_parameter_expr(&ParameterExpr::Add(
            Box::new(x.clone()),
            Box::new(zero.clone())
        )),
        x
    );
    assert_eq!(
        simplify_parameter_expr(&ParameterExpr::Add(
            Box::new(zero.clone()),
            Box::new(x.clone())
        )),
        x
    );

    // Sub zero
    assert_eq!(
        simplify_parameter_expr(&ParameterExpr::Sub(
            Box::new(x.clone()),
            Box::new(zero.clone())
        )),
        x
    );
    // Sub self
    assert_eq!(
        simplify_parameter_expr(&ParameterExpr::Sub(
            Box::new(x.clone()),
            Box::new(x.clone())
        )),
        ParameterExpr::Constant(0.0)
    );

    // Mul one
    assert_eq!(
        simplify_parameter_expr(&ParameterExpr::Mul(
            Box::new(x.clone()),
            Box::new(one.clone())
        )),
        x
    );
    assert_eq!(
        simplify_parameter_expr(&ParameterExpr::Mul(
            Box::new(one.clone()),
            Box::new(x.clone())
        )),
        x
    );

    // Mul zero
    assert_eq!(
        simplify_parameter_expr(&ParameterExpr::Mul(
            Box::new(x.clone()),
            Box::new(zero.clone())
        )),
        ParameterExpr::Constant(0.0)
    );

    // Div one
    assert_eq!(
        simplify_parameter_expr(&ParameterExpr::Div(
            Box::new(x.clone()),
            Box::new(one.clone())
        )),
        x
    );

    // Double negation
    let neg_neg_x = ParameterExpr::Neg(Box::new(ParameterExpr::Neg(Box::new(x.clone()))));
    assert_eq!(simplify_parameter_expr(&neg_neg_x), x);
}

#[test]
fn test_compose_rotation_angles() {
    let c1 = ParameterExpr::Constant(0.4);
    let c2 = ParameterExpr::Constant(0.6);
    let composed = compose_rotation_angles(&c1, &c2);
    assert_eq!(composed, ParameterExpr::Constant(1.0));

    let theta = ParameterExpr::Symbolic("theta".to_string());
    let zero = ParameterExpr::Constant(0.0);
    assert_eq!(compose_rotation_angles(&theta, &zero), theta);
}

// ============================================================================
// 4. Negative and Failure Injection Tests
// ============================================================================

#[test]
fn test_negative_wrong_angle_inverse() {
    let q0 = QubitId(0);
    let op1 = Operation::Gate {
        gate_name: "rx".to_string(),
        qubits: vec![q0],
        parameters: vec![ParameterExpr::Constant(1.0)],
    };
    let op2 = Operation::Gate {
        gate_name: "rx".to_string(),
        qubits: vec![q0],
        parameters: vec![ParameterExpr::Constant(-0.99)],
    };
    assert!(!are_inverses(&op1, &op2));
}

#[test]
fn test_negative_global_phase_omission_fails_exact_check() {
    let registry = IdentityRegistry::canonical();
    let rz_pi = registry.by_id("rz_pi_is_phase_z").unwrap();

    let (lhs, rhs) = rz_pi.evaluate_matrices(0.0);
    // Directly comparing without phase factor must fail!
    let diff = lhs.max_element_difference(&rhs);
    assert!(
        diff > 1.0,
        "LHS and RHS for rz(pi) vs Z should differ without phase correction, got diff = {diff}"
    );

    // And verify that an identity with phase would fail if verified as exact:
    let fake_exact_identity = GateIdentity {
        id: "fake_exact_rz_pi",
        name: "Fake Exact Rz(pi) = Z",
        description: "Intentionally omitting required phase -i",
        category: IdentityCategory::PowerEquivalence,
        kind: IdentityKind::Exact, // Wrong!
        preconditions: &[],
        arity: 1,
        matrix_eval: |_| {
            (
                GateMatrix::Single(matrix_rz(Angle::from_radians(PI))),
                GateMatrix::Single(qcc::quantum::gate_matrix::matrix_z()),
            )
        },
    };
    assert!(fake_exact_identity.verify(0.0, 1e-10).is_err());
}

// ============================================================================
// 5. Property Testing with Proptest
// ============================================================================

proptest! {
    #[test]
    fn prop_normalize_rotation_angle_exact_in_range(angle in -100.0 * PI..100.0 * PI) {
        let norm = normalize_rotation_angle_exact(angle);
        prop_assert!(norm >= 0.0, "norm angle {} must be >= 0", norm);
        prop_assert!(norm < 4.0 * PI, "norm angle {} must be < 4pi", norm);

        // Verify exact SU(2) matrix equivalence: Rz(angle) == Rz(norm)
        let m_orig = matrix_rz(Angle::from_radians(angle));
        let m_norm = matrix_rz(Angle::from_radians(norm));
        prop_assert!(m_orig.approx_eq(&m_norm, 1e-10));

        let mx_orig = matrix_rx(Angle::from_radians(angle));
        let mx_norm = matrix_rx(Angle::from_radians(norm));
        prop_assert!(mx_orig.approx_eq(&mx_norm, 1e-10));

        let my_orig = matrix_ry(Angle::from_radians(angle));
        let my_norm = matrix_ry(Angle::from_radians(norm));
        prop_assert!(my_orig.approx_eq(&my_norm, 1e-10));
    }

    #[test]
    fn prop_normalize_rotation_angle_2pi_phase(angle in -100.0 * PI..100.0 * PI) {
        let (norm, phase) = normalize_rotation_angle_2pi(angle);
        prop_assert!((0.0..2.0 * PI).contains(&norm));
        prop_assert!(phase == 0.0 || (phase - PI).abs() < 1e-12);

        // Rz(angle) == e^{i*phase} * Rz(norm)
        let m_orig = matrix_rz(Angle::from_radians(angle));
        let m_norm = matrix_rz(Angle::from_radians(norm));
        let factor = Complex64::from_polar(1.0, phase);
        let m_scaled = Matrix2x2([
            [m_norm.0[0][0] * factor, m_norm.0[0][1] * factor],
            [m_norm.0[1][0] * factor, m_norm.0[1][1] * factor],
        ]);
        prop_assert!(m_orig.approx_eq(&m_scaled, 1e-10));
    }

    #[test]
    fn prop_rotation_angles_inverse_cancellation(angle in -50.0 * PI..50.0 * PI) {
        let q = QubitId(0);
        let op1 = Operation::Gate {
            gate_name: "rz".to_string(),
            qubits: vec![q],
            parameters: vec![ParameterExpr::Constant(angle)],
        };
        let op2 = Operation::Gate {
            gate_name: "rz".to_string(),
            qubits: vec![q],
            parameters: vec![ParameterExpr::Constant(-angle)],
        };
        prop_assert!(are_inverses(&op1, &op2));
    }
}
