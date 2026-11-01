//! Integration, reference comparison, and property tests for two-qubit statevector simulation kernels.
//!
//! Validates:
//! - CX truth table in both control-target orientations: distinguishing CX(q0, q1) from CX(q1, q0).
//! - CZ phase action on computational basis states and superposition.
//! - SWAP bit exchange on basis states and registers with additional untouched qubits.
//! - Bell state preparation ($|\Phi^+\rangle = (|00\rangle + |11\rangle)/\sqrt{2}$).
//! - Non-adjacent qubit operands across multi-qubit registers.
//! - Property tests for norm preservation across random normalized complex states.
//! - Comparison against an independent dense full-system matrix reference in tiny systems ($n \in [2, 4]$).
//! - Error handling for identical operands and out-of-bounds indices.

use std::f64::consts::FRAC_1_SQRT_2;

use num_complex::Complex64;
use proptest::prelude::*;

use qcc::quantum::gate_matrix::{matrix_cx, matrix_cz, matrix_swap, Matrix4x4};
use qcc::quantum::Gate;
use qcc::simulator::kernels::two_qubit::quartet_indices;
use qcc::simulator::{SimulatorError, StateVector};

const TOLERANCE: f64 = 1e-10;

// ============================================================================
// 1. Quartet Indexing Structural Tests
// ============================================================================

#[test]
fn test_quartet_partition_exactness_exhaustive() {
    // For n = 2, 3, 4, 5: test all possible distinct qubit pairs (qA, qB)
    for n in 2..=5 {
        let dim = 1usize << n;
        let expected_quartets = 1usize << (n - 2);

        for qa in 0..n {
            for qb in 0..n {
                if qa == qb {
                    continue;
                }

                let iter = quartet_indices(n, qa, qb).unwrap();
                assert_eq!(iter.len(), expected_quartets);

                let mut visited = vec![false; dim];
                let mut count = 0;

                for q in iter {
                    count += 1;
                    // Check four indices in each quartet are distinct
                    let indices = [q.i00, q.i10, q.i01, q.i11];
                    for &idx in &indices {
                        assert!(idx < dim, "Index {} out of bounds for dim {}", idx, dim);
                        assert!(!visited[idx], "Index {} visited multiple times", idx);
                        visited[idx] = true;
                    }

                    // Check bit weights:
                    // i00: qa=0, qb=0
                    // i10: qa=1, qb=0
                    // i01: qa=0, qb=1
                    // i11: qa=1, qb=1
                    assert_eq!((q.i00 >> qa) & 1, 0);
                    assert_eq!((q.i00 >> qb) & 1, 0);

                    assert_eq!((q.i10 >> qa) & 1, 1);
                    assert_eq!((q.i10 >> qb) & 1, 0);

                    assert_eq!((q.i01 >> qa) & 1, 0);
                    assert_eq!((q.i01 >> qb) & 1, 1);

                    assert_eq!((q.i11 >> qa) & 1, 1);
                    assert_eq!((q.i11 >> qb) & 1, 1);
                }

                assert_eq!(count, expected_quartets);
                assert!(
                    visited.iter().all(|&v| v),
                    "Not all basis states were covered"
                );
            }
        }
    }
}

// ============================================================================
// 2. CX Truth Table & Control/Target Discrimination
// ============================================================================

#[test]
fn test_cx_truth_table_c0_t1() {
    // 2-qubit system: q0 is LSB, q1 is MSB
    // Basis index: q0 + 2*q1
    // Basis labels: |q1 q0>
    // CX(q0, q1): q0 is control, q1 is target
    // |00> (index 0) -> |00> (index 0)
    // |01> (index 1: q0=1, q1=0) -> |11> (index 3: q0=1, q1=1)
    // |10> (index 2: q0=0, q1=1) -> |10> (index 2: q0=0, q1=1)
    // |11> (index 3: q0=1, q1=1) -> |01> (index 1: q0=1, q1=0)

    let expected_mappings = [(0, 0), (1, 3), (2, 2), (3, 1)];

    for (in_idx, out_idx) in expected_mappings {
        let mut sv = StateVector::from_basis_state(2, in_idx).unwrap();
        sv.apply_cx(0, 1).unwrap();
        assert_eq!(
            sv.get(out_idx).unwrap(),
            Complex64::new(1.0, 0.0),
            "CX(0, 1) mapping failed for input {}",
            in_idx
        );
        assert_eq!(sv.probability(out_idx).unwrap(), 1.0);
    }
}

#[test]
fn test_cx_truth_table_c1_t0() {
    // 2-qubit system: q1 is control, q0 is target
    // |00> (index 0: q1=0, q0=0) -> |00> (index 0)
    // |01> (index 1: q1=0, q0=1) -> |01> (index 1) (control is 0!)
    // |10> (index 2: q1=1, q0=0) -> |11> (index 3) (control is 1!)
    // |11> (index 3: q1=1, q0=1) -> |10> (index 2) (control is 1!)

    let expected_mappings = [(0, 0), (1, 1), (2, 3), (3, 2)];

    for (in_idx, out_idx) in expected_mappings {
        let mut sv = StateVector::from_basis_state(2, in_idx).unwrap();
        sv.apply_cx(1, 0).unwrap();
        assert_eq!(
            sv.get(out_idx).unwrap(),
            Complex64::new(1.0, 0.0),
            "CX(1, 0) mapping failed for input {}",
            in_idx
        );
        assert_eq!(sv.probability(out_idx).unwrap(), 1.0);
    }
}

#[test]
fn test_cx_orientation_discrimination_on_same_state() {
    // Start with state |01> (index 1: q0=1, q1=0)
    let mut sv_01 = StateVector::from_basis_state(2, 1).unwrap();
    sv_01.apply_cx(0, 1).unwrap(); // q0 is ctrl -> target q1 flips -> |11> (index 3)
    assert_eq!(sv_01.get(3).unwrap(), Complex64::new(1.0, 0.0));

    let mut sv_10 = StateVector::from_basis_state(2, 1).unwrap();
    sv_10.apply_cx(1, 0).unwrap(); // q1 is ctrl -> q1 is 0, so nothing flips -> |01> (index 1)
    assert_eq!(sv_10.get(1).unwrap(), Complex64::new(1.0, 0.0));

    assert!(!sv_01.approx_eq(&sv_10, TOLERANCE));
}

// ============================================================================
// 3. CZ Tests
// ============================================================================

#[test]
fn test_cz_truth_table_and_symmetry() {
    // CZ negates only |11> (index 3)
    for in_idx in 0..4 {
        let mut sv = StateVector::from_basis_state(2, in_idx).unwrap();
        sv.apply_cz(0, 1).unwrap();

        let expected_sign = if in_idx == 3 { -1.0 } else { 1.0 };
        assert_eq!(
            sv.get(in_idx).unwrap(),
            Complex64::new(expected_sign, 0.0),
            "CZ failed for basis state {}",
            in_idx
        );

        // Test symmetry: CZ(0, 1) == CZ(1, 0)
        let mut sv_rev = StateVector::from_basis_state(2, in_idx).unwrap();
        sv_rev.apply_cz(1, 0).unwrap();
        assert!(sv.approx_eq(&sv_rev, TOLERANCE));
    }
}

#[test]
fn test_cz_superposition() {
    // Uniform superposition of 2 qubits: (|00> + |01> + |10> + |11>) / 2
    let mut sv = StateVector::new(2).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_h(1).unwrap();

    sv.apply_cz(0, 1).unwrap();

    assert!((sv.get(0).unwrap() - Complex64::new(0.5, 0.0)).norm() < TOLERANCE);
    assert!((sv.get(1).unwrap() - Complex64::new(0.5, 0.0)).norm() < TOLERANCE);
    assert!((sv.get(2).unwrap() - Complex64::new(0.5, 0.0)).norm() < TOLERANCE);
    assert!((sv.get(3).unwrap() - Complex64::new(-0.5, 0.0)).norm() < TOLERANCE);
    assert!(sv.is_normalized(TOLERANCE));
}

// ============================================================================
// 4. SWAP Tests
// ============================================================================

#[test]
fn test_swap_basis_states() {
    // SWAP: |00> -> |00>, |01> -> |10>, |10> -> |01>, |11> -> |11>
    let mappings = [(0, 0), (1, 2), (2, 1), (3, 3)];
    for (in_idx, out_idx) in mappings {
        let mut sv = StateVector::from_basis_state(2, in_idx).unwrap();
        sv.apply_swap(0, 1).unwrap();
        assert_eq!(sv.get(out_idx).unwrap(), Complex64::new(1.0, 0.0));
    }
}

#[test]
fn test_swap_non_adjacent_in_multi_qubit_register() {
    // 4-qubit register. Swap q1 and q3.
    // Untouched: q0, q2.
    // State: q3=0, q2=1, q1=1, q0=0 => binary 0110 = index 6.
    // Swapping q1 and q3:
    // q3 becomes 1, q1 becomes 0 => binary 1100 = index 12.
    let mut sv = StateVector::from_basis_state(4, 6).unwrap();
    sv.apply_swap(1, 3).unwrap();
    assert_eq!(sv.get(12).unwrap(), Complex64::new(1.0, 0.0));
    assert_eq!(sv.get(6).unwrap(), Complex64::new(0.0, 0.0));

    // Swapping back restores original index 6
    sv.apply_swap(3, 1).unwrap();
    assert_eq!(sv.get(6).unwrap(), Complex64::new(1.0, 0.0));
}

// ============================================================================
// 5. Bell State Synthesis
// ============================================================================

#[test]
fn test_bell_state_phi_plus() {
    // |Phi+> = (|00> + |11>) / sqrt(2)
    let mut sv = StateVector::new(2).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_cx(0, 1).unwrap();

    let expected = Complex64::new(FRAC_1_SQRT_2, 0.0);
    assert!((sv.get(0).unwrap() - expected).norm() < TOLERANCE);
    assert!(sv.get(1).unwrap().norm() < TOLERANCE);
    assert!(sv.get(2).unwrap().norm() < TOLERANCE);
    assert!((sv.get(3).unwrap() - expected).norm() < TOLERANCE);

    assert!((sv.probability(0).unwrap() - 0.5).abs() < TOLERANCE);
    assert!((sv.probability(3).unwrap() - 0.5).abs() < TOLERANCE);
    assert_eq!(sv.probability(1).unwrap(), 0.0);
    assert_eq!(sv.probability(2).unwrap(), 0.0);
    assert!(sv.is_normalized(TOLERANCE));
}

#[test]
fn test_bell_state_in_3_qubit_register_non_adjacent() {
    // Create Bell state between non-adjacent qubits q0 and q2 in a 3-qubit register
    // q1 remains in |0>
    // State: (|000> + |101>) / sqrt(2) -> index 0 and index 5 (1 + 4 = 5)
    let mut sv = StateVector::new(3).unwrap();
    sv.apply_h(0).unwrap();
    sv.apply_cx(0, 2).unwrap();

    let expected = Complex64::new(FRAC_1_SQRT_2, 0.0);
    assert!((sv.get(0).unwrap() - expected).norm() < TOLERANCE);
    assert!((sv.get(5).unwrap() - expected).norm() < TOLERANCE);
    for i in 1..8 {
        if i != 5 {
            assert!(
                sv.get(i).unwrap().norm() < TOLERANCE,
                "Nonzero amplitude at index {}",
                i
            );
        }
    }
    assert!(sv.is_normalized(TOLERANCE));
}

// ============================================================================
// 6. Error Handling Tests
// ============================================================================

#[test]
fn test_identical_operands_error() {
    let mut sv = StateVector::new(3).unwrap();
    assert_eq!(
        sv.apply_cx(1, 1).unwrap_err(),
        SimulatorError::IdenticalQubitOperands { qubit: 1 }
    );
    assert_eq!(
        sv.apply_cz(2, 2).unwrap_err(),
        SimulatorError::IdenticalQubitOperands { qubit: 2 }
    );
    assert_eq!(
        sv.apply_swap(0, 0).unwrap_err(),
        SimulatorError::IdenticalQubitOperands { qubit: 0 }
    );
}

#[test]
fn test_target_out_of_bounds_errors() {
    let mut sv = StateVector::new(2).unwrap();
    assert_eq!(
        sv.apply_cx(0, 2).unwrap_err(),
        SimulatorError::TargetOutOfBounds {
            target: 2,
            num_qubits: 2,
        }
    );
    assert_eq!(
        sv.apply_cx(3, 1).unwrap_err(),
        SimulatorError::TargetOutOfBounds {
            target: 3,
            num_qubits: 2,
        }
    );
}

#[test]
fn test_apply_single_qubit_gate_with_2q_kernel() {
    let mut sv = StateVector::new(2).unwrap();
    let err = sv.apply_gate_2q(&Gate::H, 0, 1).unwrap_err();
    assert!(matches!(err, SimulatorError::UnsupportedGate { .. }));
}

// ============================================================================
// 7. Dense Full-System Matrix Reference (Test-Only Methodology)
// ============================================================================

/// Independently constructs a full-system $2^n \times 2^n$ matrix representation
/// for a 2-qubit gate acting on operands $(q_A, q_B)$ in an $n$-qubit system.
///
/// Under Little-Endian indexing:
/// - $q_A$ has local weight 1 (bit 0 of local basis)
/// - $q_B$ has local weight 2 (bit 1 of local basis)
///
/// NOTE: This function exists solely within tests to establish mathematical ground truth
/// for tiny systems ($n \le 4$), strictly adhering to the architectural requirement that
/// production simulator kernels never allocate dense full-system matrices.
#[allow(clippy::needless_range_loop)]
fn build_dense_full_system_operator(
    num_qubits: usize,
    qa: usize,
    qb: usize,
    matrix_local: &Matrix4x4,
) -> Vec<Vec<Complex64>> {
    let dim = 1usize << num_qubits;
    let mut dense = vec![vec![Complex64::new(0.0, 0.0); dim]; dim];

    for j in 0..dim {
        // Extract bits at qa and qb for column state j
        let bit_a = (j >> qa) & 1;
        let bit_b = (j >> qb) & 1;
        let c_local = bit_a | (bit_b << 1);

        for r_local in 0..4 {
            let out_bit_a = r_local & 1;
            let out_bit_b = (r_local >> 1) & 1;

            // Target row state i: identical to j except at qa and qb
            let i = (j & !(1 << qa) & !(1 << qb)) | (out_bit_a << qa) | (out_bit_b << qb);
            let elem = matrix_local.0[r_local][c_local];
            dense[i][j] += elem;
        }
    }

    dense
}

/// Multiplies a dense $2^n \times 2^n$ matrix with a statevector.
#[allow(clippy::needless_range_loop)]
fn dense_matrix_vector_mult(matrix: &[Vec<Complex64>], state: &[Complex64]) -> Vec<Complex64> {
    let dim = state.len();
    let mut out = vec![Complex64::new(0.0, 0.0); dim];
    for r in 0..dim {
        for c in 0..dim {
            out[r] += matrix[r][c] * state[c];
        }
    }
    out
}

#[test]
fn test_dense_reference_comparison_cx_cz_swap() {
    // Exhaustive comparison on n = 2, 3, 4
    for n in 2..=4 {
        for qa in 0..n {
            for qb in 0..n {
                if qa == qb {
                    continue;
                }

                // Test with 3 test gates: CX, CZ, SWAP
                let test_gates = [
                    (matrix_cx(), Gate::CX),
                    (matrix_cz(), Gate::CZ),
                    (matrix_swap(), Gate::SWAP),
                ];

                for (mat, gate) in test_gates {
                    let dense_op = build_dense_full_system_operator(n, qa, qb, &mat);

                    // Test with a non-trivial superposition state: H on all qubits
                    let mut sv = StateVector::new(n).unwrap();
                    for q in 0..n {
                        sv.apply_h(q).unwrap();
                    }
                    let input_amps = sv.amplitudes().to_vec();

                    // Apply in-place kernel
                    sv.apply_matrix4x4(qa, qb, &mat).unwrap();

                    // Apply dense reference multiplication
                    let expected_amps = dense_matrix_vector_mult(&dense_op, &input_amps);

                    for (idx, &expected) in expected_amps.iter().enumerate() {
                        let diff = (sv.get(idx).unwrap() - expected).norm();
                        assert!(
                            diff < TOLERANCE,
                            "Mismatch for gate {:?} on operands ({}, {}) at index {}: diff = {}",
                            gate,
                            qa,
                            qb,
                            idx,
                            diff
                        );
                    }
                }
            }
        }
    }
}

// ============================================================================
// 8. Property Tests (Proptest)
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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_norm_preservation_cx_random_states(
        sv in arb_normalized_state(3),
        qa in 0usize..3usize,
        qb in 0usize..3usize,
    ) {
        if qa != qb {
            let mut state = sv;
            state.apply_cx(qa, qb).unwrap();
            let norm = state.norm();
            prop_assert!(
                (norm - 1.0).abs() < 1e-6,
                "CX({}, {}) violated norm preservation: got {}",
                qa,
                qb,
                norm
            );
        }
    }

    #[test]
    fn prop_norm_preservation_cz_random_states(
        sv in arb_normalized_state(3),
        qa in 0usize..3usize,
        qb in 0usize..3usize,
    ) {
        if qa != qb {
            let mut state = sv;
            state.apply_cz(qa, qb).unwrap();
            let norm = state.norm();
            prop_assert!(
                (norm - 1.0).abs() < 1e-6,
                "CZ({}, {}) violated norm preservation: got {}",
                qa,
                qb,
                norm
            );
        }
    }

    #[test]
    fn prop_norm_preservation_swap_random_states(
        sv in arb_normalized_state(3),
        qa in 0usize..3usize,
        qb in 0usize..3usize,
    ) {
        if qa != qb {
            let mut state = sv;
            state.apply_swap(qa, qb).unwrap();
            let norm = state.norm();
            prop_assert!(
                (norm - 1.0).abs() < 1e-6,
                "SWAP({}, {}) violated norm preservation: got {}",
                qa,
                qb,
                norm
            );
        }
    }

    #[test]
    fn prop_dense_reference_equivalence_on_random_states(
        sv in arb_normalized_state(3),
        qa in 0usize..3usize,
        qb in 0usize..3usize,
    ) {
        if qa != qb {
            let dense_op = build_dense_full_system_operator(3, qa, qb, &matrix_cx());
            let expected_amps = dense_matrix_vector_mult(&dense_op, sv.amplitudes());

            let mut state = sv;
            state.apply_cx(qa, qb).unwrap();

            for (idx, &expected) in expected_amps.iter().enumerate() {
                let diff = (state.get(idx).unwrap() - expected).norm();
                prop_assert!(
                    diff < 1e-7,
                    "Dense reference discrepancy for CX({}, {}) at {}: diff={}",
                    qa,
                    qb,
                    idx,
                    diff
                );
            }
        }
    }
}
