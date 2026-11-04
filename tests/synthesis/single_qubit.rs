//! Integration tests for native single-qubit unitary synthesis.
//!
//! Validates:
//! 1. Random one-qubit unitary reconstruction across all target bases (Z-Y-Z, Z-X-Z, RZ-SX, U3).
//! 2. Generic Z-Y-Z and Z-X-Z synthesis.
//! 3. Target-specific RZ-SX synthesis when declared by Target.
//! 4. Rejection and structured errors when required target operations are unavailable.
//! 5. Special angles and Euler singularities (gimbal lock at theta = 0, theta = pi, canonical gates).
//! 6. Angle normalization and periodicity with spinor phase compensation.
//! 7. Global-phase-sensitive reconstruction distinguishing exact equality from equality up to phase.
//! 8. Target legality verification of every emitted operation.
//! 9. Target instruction family variation without compiler-core changes.

use std::f64::consts::PI;

use num_complex::Complex64;
use qcc::gate_signature::ParameterDomain;
use qcc::quantum::gate_matrix::{
    matrix_h, matrix_id, matrix_rx, matrix_ry, matrix_rz, matrix_s, matrix_sdg, matrix_sx,
    matrix_t, matrix_tdg, matrix_x, matrix_y, matrix_z, Matrix2x2,
};
use qcc::quantum::parameter::Angle;
use qcc::simulator::sampling::SplitMix64;
use qcc::simulator::SeededRng;
use qcc::synthesis::single_qubit::{
    normalize_angle_with_spinor_shift, SingleQubitSynthesizer, SynthesisError,
};
use qcc::target::{
    OperationConnectivity, PhysicalQubitId, SingleQubitBasis, Target, TargetOperationDescriptor,
};
use qcc::verification::check_equivalence;

// ============================================================================
// Synthetic Target Fixtures
// ============================================================================

/// Creates a target supporting the Rz-Sx microwave/virtual transmon instruction family.
fn create_rz_sx_target(num_qubits: usize) -> Target {
    let mut target = Target::new("rz_sx_target", num_qubits);

    target.add_operation(
        TargetOperationDescriptor::builder("rz", 1)
            .add_parameter_domain(ParameterDomain::Angle4Pi)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .duration(0.0)
            .error_estimate(0.0001)
            .build(),
    );

    target.add_operation(
        TargetOperationDescriptor::builder("sx", 1)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .duration(35.0)
            .error_estimate(0.0005)
            .build(),
    );

    target.add_operation(
        TargetOperationDescriptor::builder("x", 1)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .duration(35.0)
            .error_estimate(0.0005)
            .build(),
    );

    target.validate().expect("valid target");
    target
}

/// Creates a target supporting the Z-X-Z rotation family.
fn create_zxz_target(num_qubits: usize) -> Target {
    let mut target = Target::new("zxz_target", num_qubits);

    target.add_operation(
        TargetOperationDescriptor::builder("rz", 1)
            .add_parameter_domain(ParameterDomain::Angle4Pi)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .build(),
    );

    target.add_operation(
        TargetOperationDescriptor::builder("rx", 1)
            .add_parameter_domain(ParameterDomain::Angle4Pi)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .build(),
    );

    target.validate().expect("valid target");
    target
}

/// Creates a target supporting the generic Z-Y-Z rotation family.
fn create_zyz_target(num_qubits: usize) -> Target {
    let mut target = Target::new("zyz_target", num_qubits);

    target.add_operation(
        TargetOperationDescriptor::builder("rz", 1)
            .add_parameter_domain(ParameterDomain::Angle4Pi)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .build(),
    );

    target.add_operation(
        TargetOperationDescriptor::builder("ry", 1)
            .add_parameter_domain(ParameterDomain::Angle4Pi)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .build(),
    );

    target.validate().expect("valid target");
    target
}

/// Creates a target supporting the OpenQASM 3 U3 single-qubit instruction.
fn create_u3_target(num_qubits: usize) -> Target {
    let mut target = Target::new("u3_target", num_qubits);

    target.add_operation(
        TargetOperationDescriptor::builder("u3", 1)
            .add_parameter_domain(ParameterDomain::Angle4Pi)
            .add_parameter_domain(ParameterDomain::Angle2Pi)
            .add_parameter_domain(ParameterDomain::Angle2Pi)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .build(),
    );

    target.validate().expect("valid target");
    target
}

/// Creates a target that does NOT support arbitrary single-qubit synthesis (discrete H, CX only).
fn create_discrete_target(num_qubits: usize) -> Target {
    let mut target = Target::new("discrete_target", num_qubits);

    target.add_operation(
        TargetOperationDescriptor::builder("h", 1)
            .connectivity(OperationConnectivity::all_single_qubits(num_qubits))
            .build(),
    );

    target.validate().expect("valid target");
    target
}

// ============================================================================
// Haar-Random Unitary Generator
// ============================================================================

fn next_gaussian(rng: &mut SplitMix64) -> f64 {
    let u1 = rng.next_f64().max(1e-15);
    let u2 = rng.next_f64();
    (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
}

/// Generates a pseudo-random $2 \times 2$ unitary matrix with a random global phase.
fn random_unitary_2x2(rng: &mut SplitMix64) -> Matrix2x2 {
    let z0_re = next_gaussian(rng);
    let z0_im = next_gaussian(rng);
    let z1_re = next_gaussian(rng);
    let z1_im = next_gaussian(rng);

    let norm = (z0_re * z0_re + z0_im * z0_im + z1_re * z1_re + z1_im * z1_im).sqrt();
    let a = Complex64::new(z0_re / norm, z0_im / norm);
    let b = Complex64::new(z1_re / norm, z1_im / norm);

    // Special unitary matrix V in SU(2): [[a, b], [-b*, a*]]
    let v = Matrix2x2([[a, b], [-b.conj(), a.conj()]]);

    // Multiply by random global phase in [0, 2pi)
    let phase_angle = rng.next_f64() * 2.0 * PI;
    v.scale(Complex64::from_polar(1.0, phase_angle))
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn test_angle_normalization_spinor_shifts() {
    // 0 rad
    let (n0, k0) = normalize_angle_with_spinor_shift(0.0);
    assert_eq!(n0, 0.0);
    assert_eq!(k0, 0);

    // 2pi rad
    let (n1, k1) = normalize_angle_with_spinor_shift(2.0 * PI);
    assert_eq!(n1, 0.0);
    assert_eq!(k1, 1);

    // 4pi rad
    let (n2, k2) = normalize_angle_with_spinor_shift(4.0 * PI);
    assert_eq!(n2, 0.0);
    assert_eq!(k2, 2);

    // Negative angle: -pi / 2
    let (n_neg, k_neg) = normalize_angle_with_spinor_shift(-PI / 2.0);
    assert!((n_neg - (1.5 * PI)).abs() < 1e-14);
    assert_eq!(k_neg, -1);

    // Large angle: 10.5 * pi
    let (n_lg, k_lg) = normalize_angle_with_spinor_shift(10.5 * PI);
    assert!((n_lg - (0.5 * PI)).abs() < 1e-14);
    assert_eq!(k_lg, 5);
}

#[test]
fn test_random_unitary_reconstruction_rz_sx() {
    let mut rng = SplitMix64::new(12345);
    let target = create_rz_sx_target(4);
    let p0 = PhysicalQubitId::new(0);
    let synthesizer = SingleQubitSynthesizer::new();

    for trial in 0..25 {
        let u = random_unitary_2x2(&mut rng);
        let seq = synthesizer.synthesize(&u, p0, &target).unwrap();

        assert_eq!(seq.basis(), SingleQubitBasis::RzSx);
        assert_eq!(seq.physical_qubit(), p0);
        assert_eq!(seq.len(), 5);

        // Verify target legality of every emitted operation
        assert!(seq.verify_legality(&target).is_ok());

        // Reconstruct matrix and verify equivalence
        let rec = seq.reconstruct_matrix();
        let diff = rec.max_difference(&u);
        assert!(
            diff < 1e-10,
            "Trial {} Rz-Sx diff was {} (exceeds 1e-10)",
            trial,
            diff
        );
    }
}

#[test]
fn test_random_unitary_reconstruction_zxz() {
    let mut rng = SplitMix64::new(67890);
    let target = create_zxz_target(4);
    let p1 = PhysicalQubitId::new(1);
    let synthesizer = SingleQubitSynthesizer::new();

    for trial in 0..25 {
        let u = random_unitary_2x2(&mut rng);
        let seq = synthesizer.synthesize(&u, p1, &target).unwrap();

        assert_eq!(seq.basis(), SingleQubitBasis::Zxz);
        assert_eq!(seq.physical_qubit(), p1);
        assert_eq!(seq.len(), 3);

        assert!(seq.verify_legality(&target).is_ok());

        let rec = seq.reconstruct_matrix();
        let diff = rec.max_difference(&u);
        assert!(
            diff < 1e-10,
            "Trial {} Z-X-Z diff was {} (exceeds 1e-10)",
            trial,
            diff
        );
    }
}

#[test]
fn test_random_unitary_reconstruction_zyz() {
    let mut rng = SplitMix64::new(11223);
    let target = create_zyz_target(4);
    let p2 = PhysicalQubitId::new(2);
    let synthesizer = SingleQubitSynthesizer::new();

    for trial in 0..25 {
        let u = random_unitary_2x2(&mut rng);
        let seq = synthesizer.synthesize(&u, p2, &target).unwrap();

        assert_eq!(seq.basis(), SingleQubitBasis::Zyz);
        assert_eq!(seq.physical_qubit(), p2);
        assert_eq!(seq.len(), 3);

        assert!(seq.verify_legality(&target).is_ok());

        let rec = seq.reconstruct_matrix();
        let diff = rec.max_difference(&u);
        assert!(
            diff < 1e-10,
            "Trial {} Z-Y-Z diff was {} (exceeds 1e-10)",
            trial,
            diff
        );
    }
}

#[test]
fn test_random_unitary_reconstruction_u3() {
    let mut rng = SplitMix64::new(44556);
    let target = create_u3_target(4);
    let p3 = PhysicalQubitId::new(3);
    let synthesizer = SingleQubitSynthesizer::new();

    for trial in 0..25 {
        let u = random_unitary_2x2(&mut rng);
        let seq = synthesizer.synthesize(&u, p3, &target).unwrap();

        assert_eq!(seq.basis(), SingleQubitBasis::U3);
        assert_eq!(seq.physical_qubit(), p3);
        assert_eq!(seq.len(), 1);

        assert!(seq.verify_legality(&target).is_ok());

        let rec = seq.reconstruct_matrix();
        let diff = rec.max_difference(&u);
        assert!(
            diff < 1e-10,
            "Trial {} U3 diff was {} (exceeds 1e-10)",
            trial,
            diff
        );
    }
}

#[test]
fn test_exact_global_phase_reconstruction_sensitivity() {
    // Proves that reconstruct_matrix matches the target unitary EXACTLY,
    // not merely up to an arbitrary global phase.
    let target = create_rz_sx_target(2);
    let p0 = PhysicalQubitId::new(0);
    let synthesizer = SingleQubitSynthesizer::new();

    let theta_phase = 0.75;
    let u_with_phase = matrix_h().scale(Complex64::from_polar(1.0, theta_phase));

    let seq = synthesizer.synthesize(&u_with_phase, p0, &target).unwrap();
    let rec = seq.reconstruct_matrix();

    // Exact equality check
    let diff = rec.max_difference(&u_with_phase);
    assert!(
        diff < 1e-10,
        "Reconstructed matrix must match phase exactly, diff was {}",
        diff
    );

    // If we intentionally modify the phase by pi/3, reconstruction difference must be large!
    let wrong_phase = matrix_h().scale(Complex64::from_polar(1.0, theta_phase + PI / 3.0));
    let diff_wrong = rec.max_difference(&wrong_phase);
    assert!(
        diff_wrong > 0.5,
        "Different global phase must produce significant difference, got {}",
        diff_wrong
    );
}

#[test]
fn test_euler_singularities_and_canonical_gates() {
    let target = create_rz_sx_target(2);
    let p0 = PhysicalQubitId::new(0);
    let synthesizer = SingleQubitSynthesizer::new();

    // Test cases covering singularities:
    // - theta = 0 (Identity, Z, S, Sdg, T, Tdg)
    // - theta = pi (X, Y)
    // - theta = pi / 2 (H, SX)
    let test_gates: Vec<(&str, Matrix2x2)> = vec![
        ("Id", matrix_id()),
        ("X", matrix_x()),
        ("Y", matrix_y()),
        ("Z", matrix_z()),
        ("H", matrix_h()),
        ("S", matrix_s()),
        ("Sdg", matrix_sdg()),
        ("T", matrix_t()),
        ("Tdg", matrix_tdg()),
        ("SX", matrix_sx()),
        ("Rx(0)", matrix_rx(Angle::zero())),
        ("Rx(pi)", matrix_rx(Angle::pi())),
        ("Ry(0)", matrix_ry(Angle::zero())),
        ("Ry(pi)", matrix_ry(Angle::pi())),
        ("Rz(0)", matrix_rz(Angle::zero())),
        ("Rz(pi)", matrix_rz(Angle::pi())),
    ];

    for (name, mat) in test_gates {
        let seq = synthesizer.synthesize(&mat, p0, &target).unwrap();
        assert!(
            seq.verify_legality(&target).is_ok(),
            "Legality check failed for {}",
            name
        );
        let rec = seq.reconstruct_matrix();
        let diff = rec.max_difference(&mat);
        assert!(
            diff < 1e-10,
            "Gate {} reconstruction failed with diff {}",
            name,
            diff
        );
    }
}

#[test]
fn test_large_angle_normalization_and_periodicity() {
    let target = create_zxz_target(2);
    let p0 = PhysicalQubitId::new(0);
    let synthesizer = SingleQubitSynthesizer::new();

    // Large angles with multiple 2pi and 4pi shifts
    let test_angles = [
        0.5 + 2.0 * PI,
        0.5 + 4.0 * PI,
        0.5 + 10.0 * PI,
        -0.5,
        -2.5 * PI,
        -10.0 * PI,
    ];

    for &rad in &test_angles {
        let mat = matrix_rx(Angle::from_radians(rad));
        let seq = synthesizer.synthesize(&mat, p0, &target).unwrap();

        // Check that every emitted angle parameter is in [0, 2pi)
        for op in seq.operations() {
            for param in op.parameters() {
                if let Some(val) = param.as_constant() {
                    assert!(
                        (0.0..2.0 * PI).contains(&val) || (val - 2.0 * PI).abs() < 1e-12,
                        "Angle {} was not in canonical [0, 2pi)",
                        val
                    );
                }
            }
        }

        // Reconstructed matrix must exactly match matrix_rx(rad)
        let rec = seq.reconstruct_matrix();
        let diff = rec.max_difference(&mat);
        assert!(
            diff < 1e-10,
            "Large angle rad={} reconstruction failed with diff {}",
            rad,
            diff
        );
    }
}

#[test]
fn test_unsupported_target_rejection() {
    let discrete_target = create_discrete_target(2);
    let p0 = PhysicalQubitId::new(0);
    let synthesizer = SingleQubitSynthesizer::new();

    let u = matrix_h();
    let res = synthesizer.synthesize(&u, p0, &discrete_target);

    match res {
        Err(SynthesisError::NoSupportedBasis { qubit, available }) => {
            assert_eq!(qubit, p0);
            assert!(available.contains(&"h".to_string()));
        }
        other => panic!("Expected NoSupportedBasis, got {:?}", other),
    }

    // Explicitly requesting Zyz on an Rz-Sx target should fail with UnsupportedBasis
    let rz_sx_target = create_rz_sx_target(2);
    let res_explicit =
        synthesizer.synthesize_with_basis(&u, p0, &rz_sx_target, SingleQubitBasis::Zyz);
    match res_explicit {
        Err(SynthesisError::UnsupportedBasis { basis, qubit }) => {
            assert_eq!(basis, SingleQubitBasis::Zyz);
            assert_eq!(qubit, p0);
        }
        other => panic!("Expected UnsupportedBasis, got {:?}", other),
    }
}

#[test]
fn test_qubit_out_of_bounds_rejection() {
    let target = create_rz_sx_target(2); // capacity = 2 (p0, p1)
    let p_invalid = PhysicalQubitId::new(99);
    let synthesizer = SingleQubitSynthesizer::new();

    let res = synthesizer.synthesize(&matrix_h(), p_invalid, &target);
    match res {
        Err(SynthesisError::QubitOutOfBounds { qubit, capacity }) => {
            assert_eq!(qubit, p_invalid);
            assert_eq!(capacity, 2);
        }
        other => panic!("Expected QubitOutOfBounds, got {:?}", other),
    }
}

#[test]
fn test_non_unitary_matrix_rejection() {
    let target = create_rz_sx_target(2);
    let p0 = PhysicalQubitId::new(0);
    let synthesizer = SingleQubitSynthesizer::new();

    // Non-unitary singular matrix
    let singular = Matrix2x2([
        [Complex64::new(1.0, 0.0), Complex64::new(1.0, 0.0)],
        [Complex64::new(1.0, 0.0), Complex64::new(1.0, 0.0)],
    ]);

    let res = synthesizer.synthesize(&singular, p0, &target);
    match res {
        Err(SynthesisError::Decomposition(..)) => {}
        other => panic!("Expected Decomposition error, got {:?}", other),
    }
}

#[test]
fn test_into_circuit_unitary_equivalence() {
    let target = create_rz_sx_target(2);
    let p0 = PhysicalQubitId::new(0);
    let synthesizer = SingleQubitSynthesizer::new();

    let u = matrix_h();
    let seq = synthesizer.synthesize(&u, p0, &target).unwrap();

    let circuit = seq.into_circuit("h_synthesized");
    assert!(circuit.validate().is_ok());

    // Compare with a direct circuit with H gate
    let mut builder = qcc::builder::CircuitBuilder::new("h_direct");
    let q = builder.add_quantum_register("p", 1).unwrap();
    builder.h(q[0]).unwrap();
    let c_direct = builder.finish().unwrap();

    let verdict = check_equivalence(&circuit, &c_direct).unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-10);
}
