//! Integration and property tests for single-qubit statevector simulation kernels.
//!
//! Validates:
//! - Basis state initialization, bounds checking, and amplitude validation.
//! - Little-endian qubit indexing ($q_0$ is LSB).
//! - Exact mathematical transformations for $X, Y, Z, H, S, S^\dagger, T, T^\dagger, R_x, R_y, R_z$.
//! - Self-inverses and inverse compositions ($G \cdot G^{-1} |\psi\rangle = |\psi\rangle$).
//! - Arbitrary target qubit indices in multi-qubit registers.
//! - Property tests: norm preservation across random normalized states for all gates.
//! - Error handling for out-of-bounds targets, non-normalized inputs, and unsupported gates.

use std::f64::consts::{FRAC_1_SQRT_2, PI};

use num_complex::Complex64;
use proptest::prelude::*;

use qcc::quantum::parameter::Angle;
use qcc::quantum::Gate;
use qcc::simulator::{SimulatorError, StateVector};

const TOLERANCE: f64 = 1e-10;

// ============================================================================
// 1. Initialization and Validation Tests
// ============================================================================

#[test]
fn test_init_zero_state() {
    for n in 1..=6 {
        let sv = StateVector::new(n).unwrap();
        assert_eq!(sv.num_qubits(), n);
        assert_eq!(sv.len(), 1 << n);
        assert_eq!(sv.get(0).unwrap(), Complex64::new(1.0, 0.0));
        for i in 1..sv.len() {
            assert_eq!(sv.get(i).unwrap(), Complex64::new(0.0, 0.0));
        }
        assert!((sv.norm() - 1.0).abs() < TOLERANCE);
        assert!(sv.is_normalized(TOLERANCE));
    }
}

#[test]
fn test_init_basis_states() {
    let n = 3;
    let dim = 1 << n;
    for k in 0..dim {
        let sv = StateVector::from_basis_state(n, k).unwrap();
        assert_eq!(sv.get(k).unwrap(), Complex64::new(1.0, 0.0));
        for j in 0..dim {
            if j != k {
                assert_eq!(sv.get(j).unwrap(), Complex64::new(0.0, 0.0));
            }
        }
        assert!(sv.is_normalized(TOLERANCE));
        assert_eq!(sv.probability(k).unwrap(), 1.0);
    }
}

#[test]
fn test_init_basis_state_out_of_bounds() {
    let err = StateVector::from_basis_state(2, 4).unwrap_err();
    assert_eq!(
        err,
        SimulatorError::BasisIndexOutOfBounds {
            index: 4,
            num_qubits: 2,
            max_index: 3,
        }
    );
}

#[test]
fn test_init_invalid_qubit_count() {
    assert!(matches!(
        StateVector::new(0),
        Err(SimulatorError::InvalidQubitCount { .. })
    ));
    assert!(matches!(
        StateVector::with_max_qubits(10, 5),
        Err(SimulatorError::InvalidQubitCount { .. })
    ));
}

#[test]
fn test_from_amplitudes_unnormalized_rejection() {
    let amps = vec![
        Complex64::new(0.5, 0.0),
        Complex64::new(0.5, 0.0),
        Complex64::new(0.0, 0.0),
        Complex64::new(0.0, 0.0),
    ];
    let err = StateVector::from_amplitudes(2, amps).unwrap_err();
    assert!(matches!(err, SimulatorError::NonNormalizedState { .. }));
}

#[test]
fn test_from_amplitudes_dimension_mismatch() {
    let amps = vec![Complex64::new(1.0, 0.0); 5];
    let err = StateVector::from_amplitudes(2, amps).unwrap_err();
    assert_eq!(
        err,
        SimulatorError::DimensionMismatch {
            num_qubits: 2,
            expected: 4,
            actual: 5,
        }
    );
}

#[test]
fn test_from_amplitudes_nan_infinity_rejection() {
    let amps_nan = vec![Complex64::new(f64::NAN, 0.0), Complex64::new(1.0, 0.0)];
    assert!(matches!(
        StateVector::from_amplitudes(1, amps_nan),
        Err(SimulatorError::InvalidAmplitude { .. })
    ));

    let amps_inf = vec![Complex64::new(f64::INFINITY, 0.0), Complex64::new(0.0, 0.0)];
    assert!(matches!(
        StateVector::from_amplitudes(1, amps_inf),
        Err(SimulatorError::InvalidAmplitude { .. })
    ));
}

// ============================================================================
// 2. Exact Gate Action on Computational Basis States
// ============================================================================

#[test]
fn test_hadamard_single_qubit() {
    // H|0> = (|0> + |1>) / sqrt(2)
    let mut sv0 = StateVector::new(1).unwrap();
    sv0.apply_h(0).unwrap();
    let expected_plus = Complex64::new(FRAC_1_SQRT_2, 0.0);
    assert!((sv0.get(0).unwrap() - expected_plus).norm() < TOLERANCE);
    assert!((sv0.get(1).unwrap() - expected_plus).norm() < TOLERANCE);

    // H|1> = (|0> - |1>) / sqrt(2)
    let mut sv1 = StateVector::from_basis_state(1, 1).unwrap();
    sv1.apply_h(0).unwrap();
    let expected_minus = Complex64::new(-FRAC_1_SQRT_2, 0.0);
    assert!((sv1.get(0).unwrap() - expected_plus).norm() < TOLERANCE);
    assert!((sv1.get(1).unwrap() - expected_minus).norm() < TOLERANCE);

    // H^2 = I
    sv0.apply_h(0).unwrap();
    assert!((sv0.get(0).unwrap() - Complex64::new(1.0, 0.0)).norm() < TOLERANCE);
    assert!(sv0.get(1).unwrap().norm() < TOLERANCE);
}

#[test]
fn test_pauli_gates() {
    // X|0> = |1>, X|1> = |0>
    let mut sv = StateVector::new(1).unwrap();
    sv.apply_x(0).unwrap();
    assert_eq!(sv.get(1).unwrap(), Complex64::new(1.0, 0.0));
    sv.apply_x(0).unwrap();
    assert_eq!(sv.get(0).unwrap(), Complex64::new(1.0, 0.0));

    // Y|0> = i|1>, Y|1> = -i|0>
    sv.apply_y(0).unwrap();
    assert_eq!(sv.get(1).unwrap(), Complex64::new(0.0, 1.0));
    sv.apply_y(0).unwrap(); // Y(i|1>) = i(-i|0>) = |0>
    assert!((sv.get(0).unwrap() - Complex64::new(1.0, 0.0)).norm() < TOLERANCE);

    // Z|0> = |0>, Z|1> = -|1>
    sv.apply_z(0).unwrap();
    assert_eq!(sv.get(0).unwrap(), Complex64::new(1.0, 0.0));
    sv.apply_x(0).unwrap(); // State is now |1>
    sv.apply_z(0).unwrap();
    assert_eq!(sv.get(1).unwrap(), Complex64::new(-1.0, 0.0));
}

#[test]
fn test_phase_gates_s_and_t() {
    let mut sv = StateVector::from_basis_state(1, 1).unwrap(); // |1>

    // S|1> = i|1>
    sv.apply_s(0).unwrap();
    assert_eq!(sv.get(1).unwrap(), Complex64::new(0.0, 1.0));

    // Sdg(i|1>) = |1>
    sv.apply_sdg(0).unwrap();
    assert!((sv.get(1).unwrap() - Complex64::new(1.0, 0.0)).norm() < TOLERANCE);

    // T|1> = exp(i*pi/4)|1>
    sv.apply_t(0).unwrap();
    let exp_t = Complex64::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2);
    assert!((sv.get(1).unwrap() - exp_t).norm() < TOLERANCE);

    // T^2 = S
    sv.apply_t(0).unwrap();
    assert!((sv.get(1).unwrap() - Complex64::new(0.0, 1.0)).norm() < TOLERANCE);

    // Tdg * S = T
    sv.apply_tdg(0).unwrap();
    assert!((sv.get(1).unwrap() - exp_t).norm() < TOLERANCE);
}

#[test]
fn test_rotation_gates() {
    let mut sv = StateVector::new(1).unwrap(); // |0>

    // Rx(pi)|0> = -i|1>
    sv.apply_rx(0, Angle::from_radians(PI)).unwrap();
    assert!((sv.get(0).unwrap()).norm() < TOLERANCE);
    assert!((sv.get(1).unwrap() - Complex64::new(0.0, -1.0)).norm() < TOLERANCE);

    // Reset to |0>
    let mut sv_y = StateVector::new(1).unwrap();
    // Ry(pi)|0> = |1>
    sv_y.apply_ry(0, Angle::from_radians(PI)).unwrap();
    assert!((sv_y.get(0).unwrap()).norm() < TOLERANCE);
    assert!((sv_y.get(1).unwrap() - Complex64::new(1.0, 0.0)).norm() < TOLERANCE);

    // Rz(pi)|0> = exp(-i*pi/2)|0> = -i|0>
    let mut sv_z = StateVector::new(1).unwrap();
    sv_z.apply_rz(0, Angle::from_radians(PI)).unwrap();
    assert!((sv_z.get(0).unwrap() - Complex64::new(0.0, -1.0)).norm() < TOLERANCE);
    assert!(sv_z.get(1).unwrap().norm() < TOLERANCE);

    // Rz(theta1) + Rz(theta2) = Rz(theta1 + theta2)
    let theta1 = Angle::from_radians(0.7);
    let theta2 = Angle::from_radians(1.1);
    let mut sv_rz1 = StateVector::new(1).unwrap();
    sv_rz1.apply_h(0).unwrap();
    let mut sv_rz2 = sv_rz1.clone();

    sv_rz1.apply_rz(0, theta1).unwrap();
    sv_rz1.apply_rz(0, theta2).unwrap();

    sv_rz2.apply_rz(0, theta1 + theta2).unwrap();
    assert!(sv_rz1.approx_eq(&sv_rz2, TOLERANCE));
}

// ============================================================================
// 3. Multi-Qubit Register and Little-Endian Qubit Indexing Tests
// ============================================================================

#[test]
fn test_little_endian_target_indices() {
    // 3 qubits: q0 is LSB, q2 is MSB
    // |000> initially
    let mut sv = StateVector::new(3).unwrap();

    // Apply X to q0: |001> (index 1)
    sv.apply_x(0).unwrap();
    assert_eq!(sv.get(1).unwrap(), Complex64::new(1.0, 0.0));
    assert_eq!(sv.get(0).unwrap(), Complex64::new(0.0, 0.0));

    // Reset and apply X to q1: |010> (index 2)
    let mut sv1 = StateVector::new(3).unwrap();
    sv1.apply_x(1).unwrap();
    assert_eq!(sv1.get(2).unwrap(), Complex64::new(1.0, 0.0));
    assert_eq!(sv1.get(0).unwrap(), Complex64::new(0.0, 0.0));

    // Reset and apply X to q2: |100> (index 4)
    let mut sv2 = StateVector::new(3).unwrap();
    sv2.apply_x(2).unwrap();
    assert_eq!(sv2.get(4).unwrap(), Complex64::new(1.0, 0.0));
    assert_eq!(sv2.get(0).unwrap(), Complex64::new(0.0, 0.0));

    // Apply X to both q0 and q2: |101> (index 1 + 4 = 5)
    let mut sv_both = StateVector::new(3).unwrap();
    sv_both.apply_x(0).unwrap();
    sv_both.apply_x(2).unwrap();
    assert_eq!(sv_both.get(5).unwrap(), Complex64::new(1.0, 0.0));
}

#[test]
fn test_multi_qubit_hadamard_tensor() {
    // Apply H to all 3 qubits: produces uniform superposition 1/sqrt(8)
    let mut sv = StateVector::new(3).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_h(1).unwrap();
    sv.apply_h(2).unwrap();

    let expected_amp = Complex64::new(1.0 / (8.0_f64).sqrt(), 0.0);
    for i in 0..8 {
        assert!(
            (sv.get(i).unwrap() - expected_amp).norm() < TOLERANCE,
            "Amplitude at {} differed",
            i
        );
        assert!((sv.probability(i).unwrap() - 0.125).abs() < TOLERANCE);
    }
    assert!(sv.is_normalized(TOLERANCE));
}

#[test]
fn test_multi_qubit_target_independence() {
    // In a 4-qubit register, applying gates on disjoint qubits must commute
    let mut sv1 = StateVector::new(4).unwrap();
    sv1.apply_h(1).unwrap();
    sv1.apply_x(3).unwrap();

    let mut sv2 = StateVector::new(4).unwrap();
    sv2.apply_x(3).unwrap();
    sv2.apply_h(1).unwrap();

    assert!(sv1.approx_eq(&sv2, TOLERANCE));
}

// ============================================================================
// 4. Error Handling Tests
// ============================================================================

#[test]
fn test_target_out_of_bounds_errors() {
    let mut sv = StateVector::new(3).unwrap();
    assert_eq!(
        sv.apply_x(3).unwrap_err(),
        SimulatorError::TargetOutOfBounds {
            target: 3,
            num_qubits: 3,
        }
    );
    assert_eq!(
        sv.apply_h(5).unwrap_err(),
        SimulatorError::TargetOutOfBounds {
            target: 5,
            num_qubits: 3,
        }
    );
}

#[test]
fn test_apply_unsupported_two_qubit_gate() {
    let mut sv = StateVector::new(2).unwrap();
    let err = sv.apply_gate(&Gate::CX, 0).unwrap_err();
    assert!(matches!(err, SimulatorError::UnsupportedGate { .. }));

    let err_cz = sv.apply_gate(&Gate::CZ, 1).unwrap_err();
    assert!(matches!(err_cz, SimulatorError::UnsupportedGate { .. }));

    let err_swap = sv.apply_gate(&Gate::SWAP, 0).unwrap_err();
    assert!(matches!(err_swap, SimulatorError::UnsupportedGate { .. }));
}

// ============================================================================
// 5. Property Tests (Proptest)
// ============================================================================

/// Strategy to generate a random normalized statevector for $n$ qubits.
fn arb_normalized_state(num_qubits: usize) -> impl Strategy<Value = StateVector> {
    let dim = 1usize << num_qubits;
    proptest::collection::vec((-1.0f64..=1.0f64, -1.0f64..=1.0f64), dim).prop_map(move |raw| {
        let mut amps: Vec<Complex64> = raw
            .into_iter()
            .map(|(re, im)| Complex64::new(re, im))
            .collect();

        let norm_sq: f64 = amps.iter().map(|c| c.norm_sqr()).sum();
        if norm_sq < 1e-12 {
            amps[0] = Complex64::new(1.0, 0.0);
            for a in amps.iter_mut().skip(1) {
                *a = Complex64::new(0.0, 0.0);
            }
        } else {
            let inv_norm = 1.0 / norm_sq.sqrt();
            for a in amps.iter_mut() {
                *a *= inv_norm;
            }
        }

        StateVector::from_amplitudes(num_qubits, amps).expect("valid normalized state")
    })
}

/// Strategy to generate any single-qubit gate.
fn arb_single_qubit_gate() -> impl Strategy<Value = Gate> {
    prop_oneof![
        Just(Gate::X),
        Just(Gate::Y),
        Just(Gate::Z),
        Just(Gate::H),
        Just(Gate::S),
        Just(Gate::Sdg),
        Just(Gate::T),
        Just(Gate::Tdg),
        (-2.0 * PI..=2.0 * PI).prop_map(|rad| Gate::Rx(Angle::from_radians(rad))),
        (-2.0 * PI..=2.0 * PI).prop_map(|rad| Gate::Ry(Angle::from_radians(rad))),
        (-2.0 * PI..=2.0 * PI).prop_map(|rad| Gate::Rz(Angle::from_radians(rad))),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_norm_preservation_on_random_states(
        sv in arb_normalized_state(3),
        gate in arb_single_qubit_gate(),
        target in 0usize..3usize,
    ) {
        let mut state = sv;
        state.apply_gate(&gate, target).unwrap();
        let norm = state.norm();
        prop_assert!(
            (norm - 1.0).abs() < 1e-6,
            "Gate {:?} on target {} violated norm: got {}",
            gate,
            target,
            norm
        );
    }

    #[test]
    fn prop_norm_preservation_1_to_4_qubits(
        num_qubits in 1usize..=4usize,
        gate in arb_single_qubit_gate(),
    ) {
        for target in 0..num_qubits {
            let mut sv = StateVector::new(num_qubits).unwrap();
            // Apply H to create superposition
            sv.apply_h(target).unwrap();
            sv.apply_gate(&gate, target).unwrap();
            let norm = sv.norm();
            prop_assert!(
                (norm - 1.0).abs() < 1e-7,
                "Gate {:?} on target {} violated norm: got {}",
                gate,
                target,
                norm
            );
        }
    }

    #[test]
    fn prop_gate_inverse_reconstruction(
        num_qubits in 1usize..=3usize,
        gate in arb_single_qubit_gate(),
    ) {
        for target in 0..num_qubits {
            let mut sv = StateVector::new(num_qubits).unwrap();
            // Put in non-trivial state
            for q in 0..num_qubits {
                sv.apply_h(q).unwrap();
            }
            let original = sv.clone();

            // Apply G then G^-1
            sv.apply_gate(&gate, target).unwrap();
            sv.apply_gate(&gate.inverse(), target).unwrap();

            prop_assert!(
                sv.approx_eq(&original, 1e-7),
                "Gate {:?} inverse reconstruction failed on target {}",
                gate,
                target
            );
        }
    }
}
