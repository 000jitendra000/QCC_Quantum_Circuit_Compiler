use std::f64::consts::{FRAC_1_SQRT_2, PI};

use num_complex::Complex64;
use proptest::prelude::*;
use qcc::quantum::*;

const TOLERANCE: f64 = 1e-12;

// ============================================================================
// 1. Exact Matrix Definitions
// ============================================================================

#[test]
fn test_exact_matrix_x() {
    let m = matrix_x();
    assert_eq!(m.get(0, 0), Complex64::new(0.0, 0.0));
    assert_eq!(m.get(0, 1), Complex64::new(1.0, 0.0));
    assert_eq!(m.get(1, 0), Complex64::new(1.0, 0.0));
    assert_eq!(m.get(1, 1), Complex64::new(0.0, 0.0));
}

#[test]
fn test_exact_matrix_y() {
    let m = matrix_y();
    assert_eq!(m.get(0, 0), Complex64::new(0.0, 0.0));
    assert_eq!(m.get(0, 1), Complex64::new(0.0, -1.0));
    assert_eq!(m.get(1, 0), Complex64::new(0.0, 1.0));
    assert_eq!(m.get(1, 1), Complex64::new(0.0, 0.0));
}

#[test]
fn test_exact_matrix_z() {
    let m = matrix_z();
    assert_eq!(m.get(0, 0), Complex64::new(1.0, 0.0));
    assert_eq!(m.get(0, 1), Complex64::new(0.0, 0.0));
    assert_eq!(m.get(1, 0), Complex64::new(0.0, 0.0));
    assert_eq!(m.get(1, 1), Complex64::new(-1.0, 0.0));
}

#[test]
fn test_exact_matrix_h() {
    let m = matrix_h();
    assert_eq!(m.get(0, 0), Complex64::new(FRAC_1_SQRT_2, 0.0));
    assert_eq!(m.get(0, 1), Complex64::new(FRAC_1_SQRT_2, 0.0));
    assert_eq!(m.get(1, 0), Complex64::new(FRAC_1_SQRT_2, 0.0));
    assert_eq!(m.get(1, 1), Complex64::new(-FRAC_1_SQRT_2, 0.0));
}

#[test]
fn test_exact_matrix_s_and_sdg() {
    let s = matrix_s();
    assert_eq!(s.get(0, 0), Complex64::new(1.0, 0.0));
    assert_eq!(s.get(0, 1), Complex64::new(0.0, 0.0));
    assert_eq!(s.get(1, 0), Complex64::new(0.0, 0.0));
    assert_eq!(s.get(1, 1), Complex64::new(0.0, 1.0));

    let sdg = matrix_sdg();
    assert_eq!(sdg.get(1, 1), Complex64::new(0.0, -1.0));
    assert_eq!(s.adjoint(), sdg);
}

#[test]
fn test_exact_matrix_t_and_tdg() {
    let t = matrix_t();
    let tdg = matrix_tdg();
    assert_eq!(t.get(0, 0), Complex64::new(1.0, 0.0));
    assert_eq!(t.get(1, 1), Complex64::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2));
    assert_eq!(tdg.get(1, 1), Complex64::new(FRAC_1_SQRT_2, -FRAC_1_SQRT_2));
    assert_eq!(t.adjoint(), tdg);
}

#[test]
fn test_exact_matrix_cx() {
    let cx = matrix_cx_c0_t1();
    // Basis: |00>, |01>, |10>, |11> (q0 is LSB)
    // Row 0 (|00>): |00> -> |00>
    assert_eq!(cx.get(0, 0), Complex64::new(1.0, 0.0));
    // Row 1 (|01>): |11> -> |01> (ctrl=q0=1 flips targ=q1 from 1 to 0)
    assert_eq!(cx.get(1, 3), Complex64::new(1.0, 0.0));
    // Row 2 (|10>): |10> -> |10> (ctrl=q0=0 leaves targ=q1=1 unchanged)
    assert_eq!(cx.get(2, 2), Complex64::new(1.0, 0.0));
    // Row 3 (|11>): |01> -> |11> (ctrl=q0=1 flips targ=q1 from 0 to 1)
    assert_eq!(cx.get(3, 1), Complex64::new(1.0, 0.0));
}

#[test]
fn test_exact_matrix_cz() {
    let cz = matrix_cz();
    assert_eq!(cz.get(0, 0), Complex64::new(1.0, 0.0));
    assert_eq!(cz.get(1, 1), Complex64::new(1.0, 0.0));
    assert_eq!(cz.get(2, 2), Complex64::new(1.0, 0.0));
    assert_eq!(cz.get(3, 3), Complex64::new(-1.0, 0.0));
}

#[test]
fn test_exact_matrix_swap() {
    let swap = matrix_swap();
    assert_eq!(swap.get(0, 0), Complex64::new(1.0, 0.0));
    assert_eq!(swap.get(1, 2), Complex64::new(1.0, 0.0)); // |10> -> |01>
    assert_eq!(swap.get(2, 1), Complex64::new(1.0, 0.0)); // |01> -> |10>
    assert_eq!(swap.get(3, 3), Complex64::new(1.0, 0.0));
}

// ============================================================================
// 2. Unitarity & Inverses for Fixed Gates
// ============================================================================

#[test]
fn test_fixed_gates_unitarity_and_inverses() {
    let fixed_gates = [
        Gate::X,
        Gate::Y,
        Gate::Z,
        Gate::H,
        Gate::S,
        Gate::Sdg,
        Gate::T,
        Gate::Tdg,
    ];

    for g in fixed_gates {
        let m = g.matrix2x2().unwrap();
        assert!(m.is_unitary(TOLERANCE), "Gate {} is not unitary", g.name());

        let inv_gate = g.inverse();
        let inv_m = inv_gate.matrix2x2().unwrap();
        assert!(
            inv_m.is_unitary(TOLERANCE),
            "Gate {} inverse is not unitary",
            g.name()
        );

        // U * U† = I and U† * U = I
        let prod1 = m.matmul(&inv_m);
        let prod2 = inv_m.matmul(&m);
        assert!(
            prod1.approx_eq(&Matrix2x2::identity(), TOLERANCE),
            "Gate {} * inv != I",
            g.name()
        );
        assert!(
            prod2.approx_eq(&Matrix2x2::identity(), TOLERANCE),
            "Gate {} inv * U != I",
            g.name()
        );
    }
}

#[test]
fn test_two_qubit_gates_unitarity_and_inverses() {
    let two_qubit_gates = [Gate::CX, Gate::CZ, Gate::SWAP];

    for g in two_qubit_gates {
        let m = g.matrix4x4().unwrap();
        assert!(m.is_unitary(TOLERANCE), "Gate {} is not unitary", g.name());

        let inv_gate = g.inverse();
        let inv_m = inv_gate.matrix4x4().unwrap();
        assert_eq!(
            inv_gate,
            g,
            "Two-qubit gate {} must be self-inverse",
            g.name()
        );

        let prod = m.matmul(&inv_m);
        assert!(
            prod.approx_eq(&Matrix4x4::identity(), TOLERANCE),
            "Gate {} * inv != I",
            g.name()
        );
    }
}

// ============================================================================
// 3. Rotation Identities
// ============================================================================

#[test]
fn test_rotation_zero_is_identity() {
    let id = Matrix2x2::identity();
    assert!(matrix_rx(Angle::zero()).approx_eq(&id, TOLERANCE));
    assert!(matrix_ry(Angle::zero()).approx_eq(&id, TOLERANCE));
    assert!(matrix_rz(Angle::zero()).approx_eq(&id, TOLERANCE));
}

#[test]
fn test_rotation_2pi_is_minus_identity() {
    // R(2π) = -I
    let two_pi = Angle::from_radians(2.0 * PI);
    let neg_id = Matrix2x2::from_rows([
        [Complex64::new(-1.0, 0.0), Complex64::new(0.0, 0.0)],
        [Complex64::new(0.0, 0.0), Complex64::new(-1.0, 0.0)],
    ]);

    assert!(matrix_rx(two_pi).approx_eq(&neg_id, TOLERANCE));
    assert!(matrix_ry(two_pi).approx_eq(&neg_id, TOLERANCE));
    assert!(matrix_rz(two_pi).approx_eq(&neg_id, TOLERANCE));
}

#[test]
fn test_rotation_pi_pauli_relation() {
    // R_k(π) = -i \sigma_k
    let pi_ang = Angle::pi();
    let minus_i = Complex64::new(0.0, -1.0);

    let rx_pi = matrix_rx(pi_ang);
    let expected_rx = Matrix2x2::from_rows([
        [Complex64::new(0.0, 0.0), minus_i],
        [minus_i, Complex64::new(0.0, 0.0)],
    ]);
    assert!(rx_pi.approx_eq(&expected_rx, TOLERANCE));

    let rz_pi = matrix_rz(pi_ang);
    let expected_rz = Matrix2x2::from_rows([
        [minus_i, Complex64::new(0.0, 0.0)],
        [Complex64::new(0.0, 0.0), Complex64::new(0.0, 1.0)], // e^{i\pi/2} = i, -i * (-1) = i
    ]);
    assert!(rz_pi.approx_eq(&expected_rz, TOLERANCE));
}

// ============================================================================
// 4. Endianness & Dirac Basis Notation Tests
// ============================================================================

#[test]
fn test_little_endian_basis_display() {
    // Rightmost displayed character is always q[0]
    assert_eq!(format_basis_state(0, 1), "|0>");
    assert_eq!(format_basis_state(1, 1), "|1>");

    // 2-qubit: index = q1*2 + q0*1
    assert_eq!(format_basis_state(0, 2), "|00>"); // q1=0, q0=0
    assert_eq!(format_basis_state(1, 2), "|01>"); // q1=0, q0=1
    assert_eq!(format_basis_state(2, 2), "|10>"); // q1=1, q0=0
    assert_eq!(format_basis_state(3, 2), "|11>"); // q1=1, q0=1

    // 3-qubit
    assert_eq!(format_basis_state(5, 3), "|101>"); // q2=1, q1=0, q0=1
}

#[test]
fn test_classical_bitstring_conventions() {
    let bits = vec![1, 0, 1]; // c[0]=1, c[1]=0, c[2]=1
                              // MsbFirst: c[2]c[1]c[0] => "101"
    assert_eq!(
        format_measurement_bits(&bits, ClassicalBitOrder::MsbFirst),
        "101"
    );
    // LsbFirst: c[0]c[1]c[2] => "101"
    assert_eq!(
        format_measurement_bits(&bits, ClassicalBitOrder::LsbFirst),
        "101"
    );

    let bits2 = vec![1, 0]; // c[0]=1, c[1]=0
    assert_eq!(
        format_measurement_bits(&bits2, ClassicalBitOrder::MsbFirst),
        "01"
    );
    assert_eq!(
        format_measurement_bits(&bits2, ClassicalBitOrder::LsbFirst),
        "10"
    );
}

// ============================================================================
// 5. Operand Order Discrimination
// ============================================================================

#[test]
fn test_cx_control_target_discrimination() {
    let cx_c0_t1 = matrix_cx_c0_t1();
    let cx_c1_t0 = matrix_cx_c1_t0();

    // The two matrix representations must NOT be equal
    assert!(!cx_c0_t1.approx_eq(&cx_c1_t0, TOLERANCE));

    // Under CX(q0=ctrl, q1=targ), |01> (q1=0, q0=1) maps to |11> (index 1 -> index 3)
    assert_eq!(cx_c0_t1.get(3, 1), Complex64::new(1.0, 0.0));

    // Under CX(q1=ctrl, q0=targ), |01> (q1=0, q0=1) maps to |01> (index 1 -> index 1)
    assert_eq!(cx_c1_t0.get(1, 1), Complex64::new(1.0, 0.0));
}

#[test]
fn test_cz_and_swap_symmetry() {
    let cz = matrix_cz();
    let swap = matrix_swap();

    assert!(cz.is_unitary(TOLERANCE));
    assert!(swap.is_unitary(TOLERANCE));

    // SWAP conjugated CX reverses its control and target
    // SWAP * CX(q0, q1) * SWAP = CX(q1, q0)
    let cx01 = matrix_cx_c0_t1();
    let cx10 = matrix_cx_c1_t0();
    let conj = swap.matmul(&cx01).matmul(&swap);
    assert!(conj.approx_eq(&cx10, TOLERANCE));
}

// ============================================================================
// 6. Global Phase Tracking
// ============================================================================

#[test]
fn test_global_phase_algebra() {
    let p_zero = GlobalPhase::zero();
    assert_eq!(p_zero.factor(), Complex64::new(1.0, 0.0));

    let p_pi = GlobalPhase::from_radians(PI);
    assert!((p_pi.factor() - Complex64::new(-1.0, 0.0)).norm() < TOLERANCE);

    let p_pi_2 = GlobalPhase::from_radians(PI / 2.0);
    assert!((p_pi_2.factor() - Complex64::new(0.0, 1.0)).norm() < TOLERANCE);

    // Phase addition: exp(i * pi/2) * exp(i * pi/2) = exp(i * pi)
    let p_pi_sum = p_pi_2 + p_pi_2;
    assert!(p_pi_sum.approx_eq(&p_pi, TOLERANCE));

    // Inverse
    assert!((p_pi_2 + p_pi_2.inverse()).approx_eq(&p_zero, TOLERANCE));
}

// ============================================================================
// 7. Property-Based Testing (Proptest)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn prop_rotation_unitarity(angle_rad in -50.0f64..50.0f64) {
        let ang = Angle::from_radians(angle_rad);
        prop_assert!(matrix_rx(ang).is_unitary(1e-10));
        prop_assert!(matrix_ry(ang).is_unitary(1e-10));
        prop_assert!(matrix_rz(ang).is_unitary(1e-10));
    }

    #[test]
    fn prop_rotation_inverse_composition(angle_rad in -50.0f64..50.0f64) {
        let ang = Angle::from_radians(angle_rad);
        let id = Matrix2x2::identity();

        // Rx(theta) * Rx(-theta) = I
        let rx = matrix_rx(ang);
        let rx_inv = matrix_rx(-ang);
        prop_assert!(rx.matmul(&rx_inv).approx_eq(&id, 1e-10));

        // Ry(theta) * Ry(-theta) = I
        let ry = matrix_ry(ang);
        let ry_inv = matrix_ry(-ang);
        prop_assert!(ry.matmul(&ry_inv).approx_eq(&id, 1e-10));

        // Rz(theta) * Rz(-theta) = I
        let rz = matrix_rz(ang);
        let rz_inv = matrix_rz(-ang);
        prop_assert!(rz.matmul(&rz_inv).approx_eq(&id, 1e-10));
    }

    #[test]
    fn prop_rotation_angle_addition(a in -25.0f64..25.0f64, b in -25.0f64..25.0f64) {
        let ang_a = Angle::from_radians(a);
        let ang_b = Angle::from_radians(b);
        let ang_sum = Angle::from_radians(a + b);

        // Rx(a) * Rx(b) = Rx(a + b)
        let rx_ab = matrix_rx(ang_a).matmul(&matrix_rx(ang_b));
        prop_assert!(rx_ab.approx_eq(&matrix_rx(ang_sum), 1e-10));

        // Ry(a) * Ry(b) = Ry(a + b)
        let ry_ab = matrix_ry(ang_a).matmul(&matrix_ry(ang_b));
        prop_assert!(ry_ab.approx_eq(&matrix_ry(ang_sum), 1e-10));

        // Rz(a) * Rz(b) = Rz(a + b)
        let rz_ab = matrix_rz(ang_a).matmul(&matrix_rz(ang_b));
        prop_assert!(rz_ab.approx_eq(&matrix_rz(ang_sum), 1e-10));
    }

    #[test]
    fn prop_basis_index_bits_round_trip(index in 0usize..1024usize) {
        let num_qubits = 10;
        let bits = basis_index_to_bits(index, num_qubits);
        let recovered = bits_to_basis_index(&bits);
        prop_assert_eq!(index, recovered);

        let label = format_basis_state(index, num_qubits);
        let (parsed_idx, parsed_qubits) = parse_basis_state(&label).unwrap();
        prop_assert_eq!(index, parsed_idx);
        prop_assert_eq!(num_qubits, parsed_qubits);
    }
}
