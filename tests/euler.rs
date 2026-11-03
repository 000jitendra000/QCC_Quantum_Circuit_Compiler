//! Comprehensive integration test suite for single-qubit Euler decomposition (Phase 31).

use std::f64::consts::PI;

use num_complex::Complex64;
use proptest::prelude::*;
use qcc::builder::CircuitBuilder;
use qcc::decomposition::{
    decompose_euler, decompose_euler_zxz, decompose_euler_zyz, DecompositionError, EulerBasis,
    EulerDecomposition, DEFAULT_EULER_TOLERANCE,
};
use qcc::ir::wire::QubitId;
use qcc::quantum::gate_matrix::{
    matrix_h, matrix_rx, matrix_ry, matrix_rz, matrix_s, matrix_sdg, matrix_t, matrix_tdg,
    matrix_x, matrix_y, matrix_z, Matrix2x2,
};
use qcc::quantum::parameter::Angle;
use qcc::simulator::sampling::{SeededRng, SplitMix64};
use qcc::verification::check_equivalence;

// ============================================================================
// 1. Canonical Single-Qubit Standard Gates
// ============================================================================

#[test]
fn test_identity_matrix_decomposition() {
    let id = Matrix2x2::identity();
    let decomp = decompose_euler_zyz(&id).unwrap();

    assert_eq!(decomp.basis, EulerBasis::Zyz);
    assert!(decomp.theta.radians().abs() < DEFAULT_EULER_TOLERANCE);
    assert!(decomp.phi.radians().abs() < DEFAULT_EULER_TOLERANCE);
    assert!(decomp.lambda.radians().abs() < DEFAULT_EULER_TOLERANCE);
    assert!(decomp.global_phase.angle().radians().abs() < DEFAULT_EULER_TOLERANCE);

    let rec = decomp.reconstruct();
    assert!(
        rec.approx_eq(&id, DEFAULT_EULER_TOLERANCE),
        "Identity reconstruction failed: residual = {:.2e}",
        rec.sub(&id).frobenius_norm()
    );
}

#[test]
fn test_pauli_x_gate_decomposition() {
    let x = matrix_x();
    let decomp = decompose_euler_zyz(&x).unwrap();

    // theta should be pi (rotation by pi around Y/X)
    assert!(
        (decomp.theta.radians() - PI).abs() < DEFAULT_EULER_TOLERANCE,
        "theta was {}",
        decomp.theta.radians()
    );

    let rec = decomp.reconstruct();
    assert!(
        rec.approx_eq(&x, DEFAULT_EULER_TOLERANCE),
        "X reconstruction failed: residual = {:.2e}",
        rec.sub(&x).frobenius_norm()
    );
}

#[test]
fn test_pauli_y_gate_decomposition() {
    let y = matrix_y();
    let decomp = decompose_euler_zyz(&y).unwrap();

    assert!(
        (decomp.theta.radians() - PI).abs() < DEFAULT_EULER_TOLERANCE,
        "theta was {}",
        decomp.theta.radians()
    );

    let rec = decomp.reconstruct();
    assert!(
        rec.approx_eq(&y, DEFAULT_EULER_TOLERANCE),
        "Y reconstruction failed: residual = {:.2e}",
        rec.sub(&y).frobenius_norm()
    );
}

#[test]
fn test_pauli_z_gate_decomposition() {
    let z = matrix_z();
    let decomp = decompose_euler_zyz(&z).unwrap();

    // Pure Z rotation has theta = 0
    assert!(
        decomp.theta.radians().abs() < DEFAULT_EULER_TOLERANCE,
        "theta was {}",
        decomp.theta.radians()
    );

    let rec = decomp.reconstruct();
    assert!(
        rec.approx_eq(&z, DEFAULT_EULER_TOLERANCE),
        "Z reconstruction failed: residual = {:.2e}",
        rec.sub(&z).frobenius_norm()
    );
}

#[test]
fn test_hadamard_gate_decomposition() {
    let h = matrix_h();
    let decomp = decompose_euler_zyz(&h).unwrap();

    // Hadamard in Z-Y-Z: theta = pi/2, phi = 0, lambda = pi
    assert!(
        (decomp.theta.radians() - PI / 2.0).abs() < DEFAULT_EULER_TOLERANCE,
        "theta was {}",
        decomp.theta.radians()
    );

    let rec = decomp.reconstruct();
    assert!(
        rec.approx_eq(&h, DEFAULT_EULER_TOLERANCE),
        "H reconstruction failed: residual = {:.2e}",
        rec.sub(&h).frobenius_norm()
    );
}

#[test]
fn test_phase_gates_s_and_sdg_decomposition() {
    let s = matrix_s();
    let decomp_s = decompose_euler_zyz(&s).unwrap();
    assert!(decomp_s.theta.radians().abs() < DEFAULT_EULER_TOLERANCE);
    let rec_s = decomp_s.reconstruct();
    assert!(rec_s.approx_eq(&s, DEFAULT_EULER_TOLERANCE));

    let sdg = matrix_sdg();
    let decomp_sdg = decompose_euler_zyz(&sdg).unwrap();
    assert!(decomp_sdg.theta.radians().abs() < DEFAULT_EULER_TOLERANCE);
    let rec_sdg = decomp_sdg.reconstruct();
    assert!(rec_sdg.approx_eq(&sdg, DEFAULT_EULER_TOLERANCE));
}

#[test]
fn test_t_and_tdg_gate_decomposition() {
    let t = matrix_t();
    let decomp_t = decompose_euler_zyz(&t).unwrap();
    assert!(decomp_t.theta.radians().abs() < DEFAULT_EULER_TOLERANCE);
    let rec_t = decomp_t.reconstruct();
    assert!(rec_t.approx_eq(&t, DEFAULT_EULER_TOLERANCE));

    let tdg = matrix_tdg();
    let decomp_tdg = decompose_euler_zyz(&tdg).unwrap();
    assert!(decomp_tdg.theta.radians().abs() < DEFAULT_EULER_TOLERANCE);
    let rec_tdg = decomp_tdg.reconstruct();
    assert!(rec_tdg.approx_eq(&tdg, DEFAULT_EULER_TOLERANCE));
}

// ============================================================================
// 2. Continuous Rotations Rx, Ry, Rz
// ============================================================================

#[test]
fn test_continuous_rx_rotations() {
    for &angle_val in &[0.0, 0.1, PI / 4.0, PI / 2.0, PI, 1.5 * PI, -0.7, -PI / 3.0] {
        let angle = Angle::from_radians(angle_val);
        let rx = matrix_rx(angle);

        // ZYZ decomposition
        let decomp_zyz = decompose_euler_zyz(&rx).unwrap();
        let rec_zyz = decomp_zyz.reconstruct();
        assert!(
            rec_zyz.approx_eq(&rx, DEFAULT_EULER_TOLERANCE),
            "Rx({angle_val}) ZYZ failed: residual = {:.2e}",
            rec_zyz.sub(&rx).frobenius_norm()
        );

        // ZXZ decomposition (Rx is native in ZXZ: theta = |angle|, lambda = 0, phi = 0 or sign)
        let decomp_zxz = decompose_euler_zxz(&rx).unwrap();
        let rec_zxz = decomp_zxz.reconstruct();
        assert!(
            rec_zxz.approx_eq(&rx, DEFAULT_EULER_TOLERANCE),
            "Rx({angle_val}) ZXZ failed: residual = {:.2e}",
            rec_zxz.sub(&rx).frobenius_norm()
        );
    }
}

#[test]
fn test_continuous_ry_rotations() {
    for &angle_val in &[0.0, 0.2, PI / 3.0, PI / 2.0, PI, 3.0 * PI / 2.0, -1.2, -PI] {
        let angle = Angle::from_radians(angle_val);
        let ry = matrix_ry(angle);

        let decomp = decompose_euler_zyz(&ry).unwrap();
        let rec = decomp.reconstruct();
        assert!(
            rec.approx_eq(&ry, DEFAULT_EULER_TOLERANCE),
            "Ry({angle_val}) failed: residual = {:.2e}",
            rec.sub(&ry).frobenius_norm()
        );
    }
}

#[test]
fn test_continuous_rz_rotations() {
    for &angle_val in &[0.0, 0.35, PI / 6.0, PI / 2.0, PI, 2.0 * PI - 0.01, -0.45] {
        let angle = Angle::from_radians(angle_val);
        let rz = matrix_rz(angle);

        let decomp = decompose_euler_zyz(&rz).unwrap();
        let rec = decomp.reconstruct();
        assert!(
            rec.approx_eq(&rz, DEFAULT_EULER_TOLERANCE),
            "Rz({angle_val}) failed: residual = {:.2e}",
            rec.sub(&rz).frobenius_norm()
        );
    }
}

// ============================================================================
// 3. Degenerate Boundaries & Gimbal Lock Configurations
// ============================================================================

#[test]
fn test_theta_zero_gimbal_lock() {
    // When theta = 0, Rz(phi) * Ry(0) * Rz(lambda) = Rz(phi + lambda).
    // The decomposition convention sets lambda = 0 and phi = angle.
    let angle = Angle::from_radians(1.23456);
    let rz = matrix_rz(angle);
    let decomp = decompose_euler_zyz(&rz).unwrap();

    assert!(decomp.theta.radians().abs() < 1e-12);
    assert!(decomp.lambda.radians().abs() < 1e-12);

    let rec = decomp.reconstruct();
    assert!(rec.approx_eq(&rz, DEFAULT_EULER_TOLERANCE));
}

#[test]
fn test_theta_pi_gimbal_lock() {
    // When theta = pi, Ry(pi) = [[0, -1], [1, 0]]
    let ry_pi = matrix_ry(Angle::from_radians(PI));
    let decomp = decompose_euler_zyz(&ry_pi).unwrap();

    assert!((decomp.theta.radians() - PI).abs() < 1e-12);
    assert!(decomp.lambda.radians().abs() < 1e-12);

    let rec = decomp.reconstruct();
    assert!(rec.approx_eq(&ry_pi, DEFAULT_EULER_TOLERANCE));
}

#[test]
fn test_near_zero_elements_and_sub_tolerance_perturbations() {
    // Matrix with tiny off-diagonal element (1e-14), well below gimbal tolerance (1e-12)
    let eps = 1e-14;
    let m = Matrix2x2([
        [Complex64::new(1.0, 0.0), Complex64::new(eps, 0.0)],
        [Complex64::new(-eps, 0.0), Complex64::new(1.0, 0.0)],
    ]);
    let decomp = decompose_euler_zyz(&m).unwrap();
    let rec = decomp.reconstruct();
    assert!(rec.approx_eq(&m, DEFAULT_EULER_TOLERANCE));

    // Matrix with tiny diagonal element
    let m_pi = Matrix2x2([
        [Complex64::new(eps, 0.0), Complex64::new(-1.0, 0.0)],
        [Complex64::new(1.0, 0.0), Complex64::new(eps, 0.0)],
    ]);
    let decomp_pi = decompose_euler_zyz(&m_pi).unwrap();
    let rec_pi = decomp_pi.reconstruct();
    assert!(rec_pi.approx_eq(&m_pi, DEFAULT_EULER_TOLERANCE));
}

// ============================================================================
// 4. Global Phase Extraction & Preservation
// ============================================================================

#[test]
fn test_pure_global_phase_extraction() {
    for &phase in &[0.0, 0.25, PI / 4.0, PI / 2.0, PI, -PI / 3.0, 3.0 * PI / 2.0] {
        let phase_factor = Complex64::from_polar(1.0, phase);
        let u = matrix_h().scale(phase_factor);

        let decomp = decompose_euler_zyz(&u).unwrap();
        let rec = decomp.reconstruct();

        // Exact matrix reconstruction must preserve the phase, not merely up to phase
        assert!(
            rec.approx_eq(&u, DEFAULT_EULER_TOLERANCE),
            "Phase {phase} reconstruction failed: residual = {:.2e}",
            rec.sub(&u).frobenius_norm()
        );
    }
}

#[test]
fn test_dropped_phase_fails_reconstruction() {
    // If a buggy implementation dropped global phase, it would reconstruct V in SU(2),
    // which differs from exp(i*phase) * V.
    let phase = PI / 3.0; // 60 degrees
    let phase_factor = Complex64::from_polar(1.0, phase);
    let u = matrix_x().scale(phase_factor);

    let decomp = decompose_euler_zyz(&u).unwrap();
    let v_su2 = decomp.to_su2_matrix();

    // SU(2) matrix V alone does NOT equal U because phase is non-zero
    let diff = v_su2.sub(&u).frobenius_norm();
    assert!(
        diff > 0.5,
        "Dropped phase should cause substantial Frobenius difference, got {diff}"
    );

    // Full reconstruction with global phase matches U
    let rec = decomp.reconstruct();
    assert!(rec.approx_eq(&u, DEFAULT_EULER_TOLERANCE));
}

#[test]
fn test_multiplication_order_sensitivity() {
    // In QCC: V = Rz(phi) * (Ry(theta) * Rz(lambda)).
    // If an implementation mistakenly computed Rz(lambda) * (Ry(theta) * Rz(phi)),
    // it would swap phi and lambda. When phi != lambda, this produces an incorrect matrix!
    let phi = 0.5;
    let theta = 1.0;
    let lambda = 2.0;

    let rz_phi = matrix_rz(Angle::from_radians(phi));
    let ry_theta = matrix_ry(Angle::from_radians(theta));
    let rz_lambda = matrix_rz(Angle::from_radians(lambda));

    // Correct column-vector / left-multiplication order:
    let correct_v = rz_phi.matmul(&ry_theta.matmul(&rz_lambda));

    // Reversed order:
    let reversed_v = rz_lambda.matmul(&ry_theta.matmul(&rz_phi));

    // The two must be distinct
    assert!(
        correct_v.sub(&reversed_v).frobenius_norm() > 0.1,
        "Order swap must produce distinct matrix"
    );

    // Decomposing correct_v must reconstruct correct_v, not reversed_v
    let decomp = decompose_euler_zyz(&correct_v).unwrap();
    let rec = decomp.reconstruct();
    assert!(rec.approx_eq(&correct_v, DEFAULT_EULER_TOLERANCE));
    assert!(!rec.approx_eq(&reversed_v, DEFAULT_EULER_TOLERANCE));
}

// ============================================================================
// 5. Circuit-Level Equivalence Checking via Phase 10 Verifier
// ============================================================================

#[test]
fn test_circuit_equivalence_hadamard() {
    let h = matrix_h();
    let decomp = decompose_euler_zyz(&h).unwrap();

    // Reference circuit: single H gate
    let mut ref_builder = CircuitBuilder::new("ref_h");
    ref_builder.add_quantum_register("q", 1).unwrap();
    let mut ref_circuit = ref_builder.finish().unwrap();
    ref_circuit
        .add_operation(qcc::ir::operation::Operation::custom_gate(
            "h",
            vec![QubitId(0)],
            vec![],
        ))
        .unwrap();

    // Candidate circuit from Euler decomposition
    let cand_circuit = decomp.to_circuit("decomp_h", QubitId(0));

    // Must be unitarily equivalent under a single common global phase!
    let verdict = check_equivalence(&ref_circuit, &cand_circuit).unwrap();
    assert!(
        verdict.is_equivalent,
        "Hadamard decomposition failed Phase 10 equivalence check: {verdict:?}"
    );
}

#[test]
fn test_circuit_equivalence_arbitrary_rotation() {
    let theta = 1.25;
    let rx = matrix_rx(Angle::from_radians(theta));
    let decomp = decompose_euler_zyz(&rx).unwrap();

    let mut ref_builder = CircuitBuilder::new("ref_rx");
    ref_builder.add_quantum_register("q", 1).unwrap();
    let mut ref_circuit = ref_builder.finish().unwrap();
    ref_circuit
        .add_operation(qcc::ir::operation::Operation::custom_gate(
            "rx",
            vec![QubitId(0)],
            vec![qcc::ir::parameter::ParameterExpr::constant(theta)],
        ))
        .unwrap();

    let cand_circuit = decomp.to_circuit("decomp_rx", QubitId(0));
    let verdict = check_equivalence(&ref_circuit, &cand_circuit).unwrap();
    assert!(
        verdict.is_equivalent,
        "Rx decomposition failed Phase 10 equivalence check: {verdict:?}"
    );
}

// ============================================================================
// 6. Error Handling & Validation
// ============================================================================

#[test]
fn test_rejects_non_unitary_matrix() {
    // Non-unitary scaled matrix
    let non_u = Matrix2x2([
        [Complex64::new(2.0, 0.0), Complex64::new(0.0, 0.0)],
        [Complex64::new(0.0, 0.0), Complex64::new(1.0, 0.0)],
    ]);
    let err = decompose_euler_zyz(&non_u).unwrap_err();
    match err {
        DecompositionError::NonUnitaryMatrix { residual, .. } => {
            assert!(residual > 1.0);
        }
        _ => panic!("Expected NonUnitaryMatrix error, got {err:?}"),
    }
}

#[test]
fn test_rejects_zero_matrix() {
    let zero = Matrix2x2([
        [Complex64::new(0.0, 0.0), Complex64::new(0.0, 0.0)],
        [Complex64::new(0.0, 0.0), Complex64::new(0.0, 0.0)],
    ]);
    let err = decompose_euler_zyz(&zero).unwrap_err();
    assert!(matches!(err, DecompositionError::NonUnitaryMatrix { .. }));
}

#[test]
fn test_rejects_nan_and_infinity() {
    let nan_m = Matrix2x2([
        [Complex64::new(f64::NAN, 0.0), Complex64::new(0.0, 0.0)],
        [Complex64::new(0.0, 0.0), Complex64::new(1.0, 0.0)],
    ]);
    assert_eq!(
        decompose_euler_zyz(&nan_m).unwrap_err(),
        DecompositionError::NonFiniteElement
    );

    let inf_m = Matrix2x2([
        [Complex64::new(f64::INFINITY, 0.0), Complex64::new(0.0, 0.0)],
        [Complex64::new(0.0, 0.0), Complex64::new(1.0, 0.0)],
    ]);
    assert_eq!(
        decompose_euler_zyz(&inf_m).unwrap_err(),
        DecompositionError::NonFiniteElement
    );
}

#[test]
fn test_custom_tolerance_decomposition() {
    let h = matrix_h();
    let decomp: EulerDecomposition = decompose_euler(&h, EulerBasis::Zyz, 1e-8).unwrap();
    let rec = decomp.reconstruct();
    assert!(rec.approx_eq(&h, 1e-8));
    assert_eq!(decomp.angles_rad().1, PI / 2.0);
}

// ============================================================================
// 7. Deterministic Randomized Haar-Distributed 2x2 Unitaries
// ============================================================================

/// Generates a Haar-distributed random 2x2 unitary matrix using QR of a complex Gaussian pair.
fn generate_haar_random_u2(rng: &mut SplitMix64) -> Matrix2x2 {
    // Box-Muller transform for Gaussian complex numbers
    let next_gaussian = |rng: &mut SplitMix64| -> Complex64 {
        let u1: f64 = rng.next_f64().clamp(1e-15, 1.0);
        let u2: f64 = rng.next_f64();
        let r: f64 = (-2.0f64 * u1.ln()).sqrt();
        let theta: f64 = 2.0 * PI * u2;
        Complex64::new(r * theta.cos(), r * theta.sin())
    };

    let z00 = next_gaussian(rng);
    let z10 = next_gaussian(rng);
    let z01 = next_gaussian(rng);
    let z11 = next_gaussian(rng);

    // Gram-Schmidt orthogonalization:
    // Column 0:
    let norm0 = (z00.norm_sqr() + z10.norm_sqr()).sqrt();
    let q00 = z00 / norm0;
    let q10 = z10 / norm0;

    // Column 1 projection:
    let proj = q00.conj() * z01 + q10.conj() * z11;
    let p01 = z01 - q00 * proj;
    let p11 = z11 - q10 * proj;
    let norm1 = (p01.norm_sqr() + p11.norm_sqr()).sqrt();
    let q01 = p01 / norm1;
    let q11 = p11 / norm1;

    Matrix2x2([[q00, q01], [q10, q11]])
}

#[test]
fn test_100_random_haar_unitaries_reconstruction_zyz_and_zxz() {
    let mut rng = SplitMix64::new(123456789);

    for trial in 0..100 {
        let u = generate_haar_random_u2(&mut rng);
        assert!(u.is_unitary(1e-12));

        // Test Z-Y-Z
        let decomp_zyz = decompose_euler_zyz(&u).unwrap();
        let rec_zyz = decomp_zyz.reconstruct();
        let residual_zyz = rec_zyz.sub(&u).frobenius_norm();
        assert!(
            residual_zyz <= DEFAULT_EULER_TOLERANCE,
            "Trial {trial} ZYZ reconstruction failed with residual {:.3e}",
            residual_zyz
        );

        // Verify angle ranges
        assert!(
            decomp_zyz.theta.radians() >= 0.0 && decomp_zyz.theta.radians() <= PI + 1e-12,
            "theta out of range: {}",
            decomp_zyz.theta.radians()
        );
        assert!(
            decomp_zyz.phi.radians() >= 0.0 && decomp_zyz.phi.radians() < 2.0 * PI + 1e-12,
            "phi out of range: {}",
            decomp_zyz.phi.radians()
        );
        assert!(
            decomp_zyz.lambda.radians() >= 0.0 && decomp_zyz.lambda.radians() < 2.0 * PI + 1e-12,
            "lambda out of range: {}",
            decomp_zyz.lambda.radians()
        );

        // Test Z-X-Z
        let decomp_zxz = decompose_euler_zxz(&u).unwrap();
        let rec_zxz = decomp_zxz.reconstruct();
        let residual_zxz = rec_zxz.sub(&u).frobenius_norm();
        assert!(
            residual_zxz <= DEFAULT_EULER_TOLERANCE,
            "Trial {trial} ZXZ reconstruction failed with residual {:.3e}",
            residual_zxz
        );
    }
}

// ============================================================================
// 8. Property-Based Testing (proptest)
// ============================================================================

proptest! {
    #[test]
    fn prop_euler_zyz_roundtrip(
        phi in -10.0f64..10.0f64,
        theta in -10.0f64..10.0f64,
        lambda in -10.0f64..10.0f64,
        alpha in -10.0f64..10.0f64,
    ) {
        let rz1 = matrix_rz(Angle::from_radians(phi));
        let ry = matrix_ry(Angle::from_radians(theta));
        let rz2 = matrix_rz(Angle::from_radians(lambda));
        let phase = Complex64::from_polar(1.0, alpha);

        let u = rz1.matmul(&ry.matmul(&rz2)).scale(phase);
        prop_assert!(u.is_unitary(1e-10));

        let decomp = decompose_euler_zyz(&u).unwrap();
        let rec = decomp.reconstruct();

        let residual = rec.sub(&u).frobenius_norm();
        prop_assert!(
            residual <= 1e-10,
            "Residual {:.3e} exceeds tolerance for input angles ({phi}, {theta}, {lambda}, {alpha})",
            residual
        );
    }

    #[test]
    fn prop_euler_zxz_roundtrip(
        phi in -10.0f64..10.0f64,
        theta in -10.0f64..10.0f64,
        lambda in -10.0f64..10.0f64,
        alpha in -10.0f64..10.0f64,
    ) {
        let rz1 = matrix_rz(Angle::from_radians(phi));
        let rx = matrix_rx(Angle::from_radians(theta));
        let rz2 = matrix_rz(Angle::from_radians(lambda));
        let phase = Complex64::from_polar(1.0, alpha);

        let u = rz1.matmul(&rx.matmul(&rz2)).scale(phase);
        prop_assert!(u.is_unitary(1e-10));

        let decomp = decompose_euler_zxz(&u).unwrap();
        let rec = decomp.reconstruct();

        let residual = rec.sub(&u).frobenius_norm();
        prop_assert!(
            residual <= 1e-10,
            "ZXZ Residual {:.3e} exceeds tolerance for input angles ({phi}, {theta}, {lambda}, {alpha})",
            residual
        );
    }
}
