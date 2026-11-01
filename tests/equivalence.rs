//! Comprehensive integration and property tests for unitary equivalence verification.

use std::f64::consts::PI;

use num_complex::Complex64;
use proptest::prelude::*;
use qcc::builder::CircuitBuilder;
use qcc::ir::circuit::Circuit;
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::ClassicalBitId;
use qcc::quantum::parameter::Angle;
use qcc::simulator::sampling::SplitMix64;
use qcc::simulator::statevector::StateVector;
use qcc::testkit::random_circuit::generate_random_unitary_circuit;
use qcc::verification::{check_equivalence, VerificationError, VerificationMethod};

// ===========================================================================
// 1. Known Equivalent Circuits
// ===========================================================================

#[test]
fn test_hadamard_squared_is_identity() {
    // H · H = I
    let mut b_hh = CircuitBuilder::new("hh");
    let q1 = b_hh.add_quantum_register("q", 1).unwrap();
    b_hh.h(q1[0]).unwrap();
    b_hh.h(q1[0]).unwrap();
    let c_hh = b_hh.finish().unwrap();

    let mut b_id = CircuitBuilder::new("id");
    b_id.add_quantum_register("q", 1).unwrap();
    let c_id = b_id.finish().unwrap();

    let verdict = check_equivalence(&c_hh, &c_id).unwrap();
    assert!(verdict.is_equivalent);
    assert_eq!(verdict.qubits, 1);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_swap_squared_is_identity() {
    // SWAP · SWAP = I
    let mut b_ss = CircuitBuilder::new("swap_swap");
    let q1 = b_ss.add_quantum_register("q", 2).unwrap();
    b_ss.swap(q1[0], q1[1]).unwrap();
    b_ss.swap(q1[0], q1[1]).unwrap();
    let c_ss = b_ss.finish().unwrap();

    let mut b_id = CircuitBuilder::new("id2");
    b_id.add_quantum_register("q", 2).unwrap();
    let c_id = b_id.finish().unwrap();

    let verdict = check_equivalence(&c_ss, &c_id).unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_circuits_differing_only_by_global_phase() {
    let mut b1 = CircuitBuilder::new("circ1");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.h(q1[0]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c1 = b1.finish().unwrap();

    let target_phase = PI / 3.0; // 60 degrees
    let mut b2 = CircuitBuilder::new("circ2");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.set_global_phase(Angle::from_radians(target_phase));
    b2.h(q2[0]).unwrap();
    b2.cx(q2[0], q2[1]).unwrap();
    let c2 = b2.finish().unwrap();

    let verdict = check_equivalence(&c1, &c2).unwrap();
    assert!(verdict.is_equivalent);
    let est_phase = verdict.estimated_phase.expect("estimated phase present");
    assert!(
        (est_phase - target_phase).abs() < 1e-9,
        "Expected estimated phase {target_phase}, got {est_phase}"
    );
}

#[test]
fn test_valid_gate_decompositions() {
    // 1. H Z H = X
    let mut b_hzh = CircuitBuilder::new("hzh");
    let q_hzh = b_hzh.add_quantum_register("q", 1).unwrap();
    b_hzh.h(q_hzh[0]).unwrap();
    b_hzh.z(q_hzh[0]).unwrap();
    b_hzh.h(q_hzh[0]).unwrap();
    let c_hzh = b_hzh.finish().unwrap();

    let mut b_x = CircuitBuilder::new("x");
    let q_x = b_x.add_quantum_register("q", 1).unwrap();
    b_x.x(q_x[0]).unwrap();
    let c_x = b_x.finish().unwrap();

    let v_hzh = check_equivalence(&c_hzh, &c_x).unwrap();
    assert!(v_hzh.is_equivalent);

    // 2. CX(0, 1) = H(1) CZ(0, 1) H(1)
    let mut b_cx = CircuitBuilder::new("cx");
    let q_cx = b_cx.add_quantum_register("q", 2).unwrap();
    b_cx.cx(q_cx[0], q_cx[1]).unwrap();
    let c_cx = b_cx.finish().unwrap();

    let mut b_h_cz_h = CircuitBuilder::new("h_cz_h");
    let q_hczh = b_h_cz_h.add_quantum_register("q", 2).unwrap();
    b_h_cz_h.h(q_hczh[1]).unwrap();
    b_h_cz_h.cz(q_hczh[0], q_hczh[1]).unwrap();
    b_h_cz_h.h(q_hczh[1]).unwrap();
    let c_h_cz_h = b_h_cz_h.finish().unwrap();

    let v_cz = check_equivalence(&c_cx, &c_h_cz_h).unwrap();
    assert!(v_cz.is_equivalent);

    // 3. SWAP(0, 1) = CX(0, 1) CX(1, 0) CX(0, 1)
    let mut b_swap = CircuitBuilder::new("swap");
    let q_swap = b_swap.add_quantum_register("q", 2).unwrap();
    b_swap.swap(q_swap[0], q_swap[1]).unwrap();
    let c_swap = b_swap.finish().unwrap();

    let mut b_3cx = CircuitBuilder::new("3cx");
    let q_3cx = b_3cx.add_quantum_register("q", 2).unwrap();
    b_3cx.cx(q_3cx[0], q_3cx[1]).unwrap();
    b_3cx.cx(q_3cx[1], q_3cx[0]).unwrap();
    b_3cx.cx(q_3cx[0], q_3cx[1]).unwrap();
    let c_3cx = b_3cx.finish().unwrap();

    let v_swap = check_equivalence(&c_swap, &c_3cx).unwrap();
    assert!(v_swap.is_equivalent);

    // 4. S · S = Z
    let mut b_ss = CircuitBuilder::new("ss");
    let q_ss = b_ss.add_quantum_register("q", 1).unwrap();
    b_ss.s(q_ss[0]).unwrap();
    b_ss.s(q_ss[0]).unwrap();
    let c_ss = b_ss.finish().unwrap();

    let mut b_z = CircuitBuilder::new("z");
    let q_z = b_z.add_quantum_register("q", 1).unwrap();
    b_z.z(q_z[0]).unwrap();
    let c_z = b_z.finish().unwrap();

    let v_ss = check_equivalence(&c_ss, &c_z).unwrap();
    assert!(v_ss.is_equivalent);

    // 5. T · T = S
    let mut b_tt = CircuitBuilder::new("tt");
    let q_tt = b_tt.add_quantum_register("q", 1).unwrap();
    b_tt.t(q_tt[0]).unwrap();
    b_tt.t(q_tt[0]).unwrap();
    let c_tt = b_tt.finish().unwrap();

    let mut b_s = CircuitBuilder::new("s");
    let q_s = b_s.add_quantum_register("q", 1).unwrap();
    b_s.s(q_s[0]).unwrap();
    let c_s = b_s.finish().unwrap();

    let v_tt = check_equivalence(&c_tt, &c_s).unwrap();
    assert!(v_tt.is_equivalent);

    // 6. Rz(a) Rz(b) = Rz(a + b)
    let a = 0.7;
    let b = 1.1;
    let mut b_rz2 = CircuitBuilder::new("rz2");
    let q_rz2 = b_rz2.add_quantum_register("q", 1).unwrap();
    b_rz2.rz(q_rz2[0], Angle::from_radians(a)).unwrap();
    b_rz2.rz(q_rz2[0], Angle::from_radians(b)).unwrap();
    let c_rz2 = b_rz2.finish().unwrap();

    let mut b_rz_sum = CircuitBuilder::new("rz_sum");
    let q_rz_sum = b_rz_sum.add_quantum_register("q", 1).unwrap();
    b_rz_sum
        .rz(q_rz_sum[0], Angle::from_radians(a + b))
        .unwrap();
    let c_rz_sum = b_rz_sum.finish().unwrap();

    let v_rz = check_equivalence(&c_rz2, &c_rz_sum).unwrap();
    assert!(v_rz.is_equivalent);
}

#[test]
fn test_barrier_is_semantic_noop_in_equivalence() {
    let mut b1 = CircuitBuilder::new("with_barrier");
    let q1 = b1.add_quantum_register("q", 2).unwrap();
    b1.h(q1[0]).unwrap();
    b1.barrier(&[q1[0], q1[1]]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    let c1 = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("no_barrier");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.h(q2[0]).unwrap();
    b2.cx(q2[0], q2[1]).unwrap();
    let c2 = b2.finish().unwrap();

    let verdict = check_equivalence(&c1, &c2).unwrap();
    assert!(verdict.is_equivalent);
}

// ===========================================================================
// 2. Deliberately Inequivalent Circuits
// ===========================================================================

#[test]
fn test_relative_phase_bug_detected() {
    // Relative phase bug: Z vs Identity.
    // Z|0> = |0>, but Z|1> = -|1|.
    // Cannot be reconciled by any single global phase!
    let mut b_z = CircuitBuilder::new("z");
    let q_z = b_z.add_quantum_register("q", 1).unwrap();
    b_z.z(q_z[0]).unwrap();
    let c_z = b_z.finish().unwrap();

    let mut b_id = CircuitBuilder::new("id");
    b_id.add_quantum_register("q", 1).unwrap();
    let c_id = b_id.finish().unwrap();

    let verdict = check_equivalence(&c_z, &c_id).unwrap();
    assert!(!verdict.is_equivalent);
    let mismatch = verdict.worst_mismatch.expect("mismatch recorded");
    assert!(mismatch.amplitude_difference >= 1.0);
}

#[test]
fn test_circuits_agreeing_on_zero_state_but_differing_on_other_basis() {
    // Both circuits map |00> to |00>, but differ on |01>
    let mut b1 = CircuitBuilder::new("circ1");
    b1.add_quantum_register("q", 2).unwrap();
    let c1 = b1.finish().unwrap(); // Identity

    let mut b2 = CircuitBuilder::new("circ2");
    let q2 = b2.add_quantum_register("q", 2).unwrap();
    b2.cz(q2[0], q2[1]).unwrap(); // CZ leaves |00>, |01>, |10> untouched, flips |11>
    let c2 = b2.finish().unwrap();

    let verdict = check_equivalence(&c1, &c2).unwrap();
    assert!(!verdict.is_equivalent);
    let mismatch = verdict.worst_mismatch.expect("mismatch recorded");
    assert_eq!(mismatch.input_index, 3); // basis state |11> = index 3
    assert_eq!(mismatch.output_amplitude_index, 3);
}

#[test]
fn test_identical_measurement_probabilities_differing_relative_phase() {
    // Circuit A: H produces (|0> + |1>)/sqrt(2)
    // Circuit B: H Z produces (|0> - |1>)/sqrt(2)
    // Both produce identical measurement outcome probabilities P(0)=0.5, P(1)=0.5!
    // But they are orthogonal relative phases! The unitary equivalence checker MUST FAIL.
    let mut b_a = CircuitBuilder::new("circ_a");
    let q_a = b_a.add_quantum_register("q", 1).unwrap();
    b_a.h(q_a[0]).unwrap();
    let c_a = b_a.finish().unwrap();

    let mut b_b = CircuitBuilder::new("circ_b");
    let q_b = b_b.add_quantum_register("q", 1).unwrap();
    b_b.h(q_b[0]).unwrap();
    b_b.z(q_b[0]).unwrap();
    let c_b = b_b.finish().unwrap();

    let verdict = check_equivalence(&c_a, &c_b).unwrap();
    assert!(
        !verdict.is_equivalent,
        "Equal measurement distributions must NOT be deemed unitarily equivalent!"
    );
}

#[test]
fn test_column_phase_incoherence_rejected() {
    // Diagonal matrix with diag(1, -1) vs diag(1, 1).
    // Column 0 needs phase 0; Column 1 needs phase pi.
    // A single common global phase cannot satisfy both columns!
    let mut b1 = CircuitBuilder::new("diag_diff");
    let q1 = b1.add_quantum_register("q", 1).unwrap();
    b1.z(q1[0]).unwrap();
    let c1 = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("id");
    b2.add_quantum_register("q", 1).unwrap();
    let c2 = b2.finish().unwrap();

    let verdict = check_equivalence(&c1, &c2).unwrap();
    assert!(!verdict.is_equivalent);
}

#[test]
fn test_localized_amplitude_mismatch_not_hidden_by_aggregate_overlap() {
    // 3-qubit circuit (8 basis states):
    // Circuit A: Identity on 3 qubits.
    // Circuit B: CZ on qubits 0 and 1, leaving qubit 2 alone.
    // 6 out of 8 basis states match identically (aggregate overlap = 6/8 = 0.75).
    // Even if 6/8 match, the component-wise check MUST fail on the other 2 states.
    let mut b_id = CircuitBuilder::new("id3");
    b_id.add_quantum_register("q", 3).unwrap();
    let c_id = b_id.finish().unwrap();

    let mut b_cz = CircuitBuilder::new("cz3");
    let q_cz = b_cz.add_quantum_register("q", 3).unwrap();
    b_cz.cz(q_cz[0], q_cz[1]).unwrap();
    let c_cz = b_cz.finish().unwrap();

    let verdict = check_equivalence(&c_id, &c_cz).unwrap();
    assert!(
        !verdict.is_equivalent,
        "Localized mismatch must not be hidden by high aggregate overlap!"
    );
    let mismatch = verdict.worst_mismatch.expect("worst mismatch recorded");
    assert!(mismatch.amplitude_difference > 1.0);

    // Also test subtle rotation perturbation with 99.999% aggregate overlap:
    let mut b_rot1 = CircuitBuilder::new("rot1");
    let qr1 = b_rot1.add_quantum_register("q", 1).unwrap();
    b_rot1.rz(qr1[0], Angle::from_radians(0.5)).unwrap();
    let c_rot1 = b_rot1.finish().unwrap();

    let mut b_rot2 = CircuitBuilder::new("rot2");
    let qr2 = b_rot2.add_quantum_register("q", 1).unwrap();
    b_rot2
        .rz(qr2[0], Angle::from_radians(0.5 + 1.0e-5))
        .unwrap();
    let c_rot2 = b_rot2.finish().unwrap();

    let verdict_rot = check_equivalence(&c_rot1, &c_rot2).unwrap();
    assert!(
        !verdict_rot.is_equivalent,
        "Perturbation outside tolerance must fail despite near-unity aggregate overlap"
    );
}

// ===========================================================================
// 3. Numerical Edge Cases
// ===========================================================================

#[test]
fn test_zero_and_near_zero_amplitudes() {
    // Circuit creates state with exact 0 amplitudes: e.g. |0> -> X -> |1> (amplitude 0 is 0.0)
    let mut b1 = CircuitBuilder::new("x_gate");
    let q1 = b1.add_quantum_register("q", 1).unwrap();
    b1.x(q1[0]).unwrap();
    let c1 = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("x_with_phase");
    let q2 = b2.add_quantum_register("q", 1).unwrap();
    b2.set_global_phase(Angle::from_radians(PI / 4.0));
    b2.x(q2[0]).unwrap();
    let c2 = b2.finish().unwrap();

    let verdict = check_equivalence(&c1, &c2).unwrap();
    assert!(verdict.is_equivalent);
    assert!(
        (verdict.estimated_phase.unwrap() - PI / 4.0).abs() < 1e-9,
        "Zero amplitude in output must not cause NaN or division by zero"
    );
}

#[test]
fn test_global_phase_near_wraparound_boundary() {
    // Compare global phase +PI - 1e-11 with -PI + 1e-11
    let p1 = PI - 1.0e-11;
    let p2 = -PI + 1.0e-11;

    let mut b1 = CircuitBuilder::new("phase_pos_pi");
    b1.add_quantum_register("q", 1).unwrap();
    b1.set_global_phase(Angle::from_radians(p1));
    let c1 = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("phase_neg_pi");
    b2.add_quantum_register("q", 1).unwrap();
    b2.set_global_phase(Angle::from_radians(p2));
    let c2 = b2.finish().unwrap();

    let verdict = check_equivalence(&c1, &c2).unwrap();
    assert!(
        verdict.is_equivalent,
        "Phase angle wraparound near +/- pi must be recognized as equivalent within tolerance"
    );
}

#[test]
fn test_perturbation_within_tolerance() {
    let theta = 1.2345;
    let perturbation = 1.0e-11; // below default 1e-10

    let mut b1 = CircuitBuilder::new("base");
    let q1 = b1.add_quantum_register("q", 1).unwrap();
    b1.rx(q1[0], Angle::from_radians(theta)).unwrap();
    let c1 = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("perturbed");
    let q2 = b2.add_quantum_register("q", 1).unwrap();
    b2.rx(q2[0], Angle::from_radians(theta + perturbation))
        .unwrap();
    let c2 = b2.finish().unwrap();

    let verdict = check_equivalence(&c1, &c2).unwrap();
    assert!(verdict.is_equivalent);
}

#[test]
fn test_perturbation_outside_tolerance() {
    let theta = 1.2345;
    let perturbation = 1.0e-8; // above default 1e-10

    let mut b1 = CircuitBuilder::new("base");
    let q1 = b1.add_quantum_register("q", 1).unwrap();
    b1.rx(q1[0], Angle::from_radians(theta)).unwrap();
    let c1 = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("perturbed");
    let q2 = b2.add_quantum_register("q", 1).unwrap();
    b2.rx(q2[0], Angle::from_radians(theta + perturbation))
        .unwrap();
    let c2 = b2.finish().unwrap();

    let verdict = check_equivalence(&c1, &c2).unwrap();
    assert!(!verdict.is_equivalent);
}

#[test]
fn test_unnormalized_input_state_rejected() {
    let mut b = CircuitBuilder::new("circ");
    b.add_quantum_register("q", 1).unwrap();
    let _circ = b.finish().unwrap();

    let unnormalized =
        StateVector::from_amplitudes(1, vec![Complex64::new(1.0, 0.0), Complex64::new(1.0, 0.0)]);
    // StateVector constructor already rejects unnormalized states!
    assert!(unnormalized.is_err());
}

#[test]
fn test_dimension_mismatch_rejected() {
    let mut b1 = CircuitBuilder::new("q1");
    b1.add_quantum_register("q", 1).unwrap();
    let c1 = b1.finish().unwrap();

    let mut b2 = CircuitBuilder::new("q2");
    b2.add_quantum_register("q", 2).unwrap();
    let c2 = b2.finish().unwrap();

    let res = check_equivalence(&c1, &c2);
    match res {
        Err(VerificationError::QubitCountMismatch {
            circuit_a,
            circuit_b,
        }) => {
            assert_eq!(circuit_a, 1);
            assert_eq!(circuit_b, 2);
        }
        other => panic!("Expected QubitCountMismatch, got {other:?}"),
    }
}

// ===========================================================================
// 4. Non-Unitary Rejection
// ===========================================================================

#[test]
fn test_rejection_of_measurement() {
    let mut b = CircuitBuilder::new("measure_circ");
    let q = b.add_quantum_register("q", 1).unwrap();
    let c = b.add_classical_register("c", 1).unwrap();
    b.h(q[0]).unwrap();
    b.measure(q[0], c[0]).unwrap();
    let circ_m = b.finish().unwrap();

    let mut b_id = CircuitBuilder::new("id");
    b_id.add_quantum_register("q", 1).unwrap();
    let circ_id = b_id.finish().unwrap();

    match check_equivalence(&circ_m, &circ_id) {
        Err(VerificationError::NonUnitaryOperation { operation, .. }) => {
            assert_eq!(operation, "measure");
        }
        res => panic!("Expected NonUnitaryOperation for measurement, got {res:?}"),
    }
}

#[test]
fn test_rejection_of_reset() {
    let mut b = CircuitBuilder::new("reset_circ");
    let q = b.add_quantum_register("q", 1).unwrap();
    b.reset(q[0]).unwrap();
    let circ_r = b.finish().unwrap();

    let mut b_id = CircuitBuilder::new("id");
    b_id.add_quantum_register("q", 1).unwrap();
    let circ_id = b_id.finish().unwrap();

    match check_equivalence(&circ_r, &circ_id) {
        Err(VerificationError::NonUnitaryOperation { operation, .. }) => {
            assert_eq!(operation, "reset");
        }
        res => panic!("Expected NonUnitaryOperation for reset, got {res:?}"),
    }
}

#[test]
fn test_rejection_of_classical_computation() {
    let mut circ = Circuit::new("classical_circ");
    circ.add_quantum_register("q", 1).unwrap();
    circ.add_classical_register("c", 2).unwrap();
    circ.add_operation(Operation::Classical {
        name: "feedback".to_string(),
        reads: vec![ClassicalBitId(0)],
        writes: vec![ClassicalBitId(1)],
    })
    .unwrap();

    let mut b_id = CircuitBuilder::new("id");
    b_id.add_quantum_register("q", 1).unwrap();
    let circ_id = b_id.finish().unwrap();

    match check_equivalence(&circ, &circ_id) {
        Err(VerificationError::NonUnitaryOperation { operation, .. }) => {
            assert_eq!(operation, "classical");
        }
        res => panic!("Expected NonUnitaryOperation for classical op, got {res:?}"),
    }
}

#[test]
fn test_rejection_of_zero_qubit_circuits() {
    let circ_empty = Circuit::new("empty");
    let mut b = CircuitBuilder::new("id");
    b.add_quantum_register("q", 1).unwrap();
    let circ_id = b.finish().unwrap();

    match check_equivalence(&circ_empty, &circ_id) {
        Err(VerificationError::ZeroQubits) => {}
        res => panic!("Expected ZeroQubits error, got {res:?}"),
    }
}

// ===========================================================================
// 5. Broader Systems: Randomized States Verification
// ===========================================================================

#[test]
fn test_broader_system_randomized_states() {
    // 6-qubit system: exceeds default max_matrix_qubits (5).
    // Verifier automatically switches to randomized state testing.
    let mut b1 = CircuitBuilder::new("circ_6q_1");
    let q1 = b1.add_quantum_register("q", 6).unwrap();
    b1.h(q1[0]).unwrap();
    b1.cx(q1[0], q1[1]).unwrap();
    b1.cx(q1[1], q1[2]).unwrap();
    b1.cx(q1[2], q1[3]).unwrap();
    b1.cx(q1[3], q1[4]).unwrap();
    b1.cx(q1[4], q1[5]).unwrap();
    let c1 = b1.finish().unwrap();

    // Identical circuit with global phase
    let target_phase = 1.456;
    let mut b2 = CircuitBuilder::new("circ_6q_2");
    let q2 = b2.add_quantum_register("q", 6).unwrap();
    b2.set_global_phase(Angle::from_radians(target_phase));
    b2.h(q2[0]).unwrap();
    b2.cx(q2[0], q2[1]).unwrap();
    b2.cx(q2[1], q2[2]).unwrap();
    b2.cx(q2[2], q2[3]).unwrap();
    b2.cx(q2[3], q2[4]).unwrap();
    b2.cx(q2[4], q2[5]).unwrap();
    let c2 = b2.finish().unwrap();

    let verdict = check_equivalence(&c1, &c2).unwrap();
    assert!(verdict.is_equivalent);
    assert_eq!(verdict.qubits, 6);
    assert!(matches!(
        verdict.method,
        VerificationMethod::RandomizedStates { .. }
    ));
    assert!(
        (verdict.estimated_phase.unwrap() - target_phase).abs() < 1e-9,
        "Randomized state testing must extract correct common global phase"
    );
}

// ===========================================================================
// 6. Property-Based Testing
// ===========================================================================

proptest! {
    #[test]
    fn prop_circuit_self_equivalence(
        seed in 1u64..1000u64,
        phase in -PI..PI,
    ) {
        let mut rng = SplitMix64::new(seed);
        let c1 = generate_random_unitary_circuit("prop_circ", 2, 6, &mut rng).unwrap();

        let mut c2 = c1.clone();
        c2.add_global_phase(ParameterExpr::constant(phase));

        let verdict = check_equivalence(&c1, &c2).unwrap();
        prop_assert!(verdict.is_equivalent);
    }

    #[test]
    fn prop_rotation_addition_equivalence(
        theta1 in -PI..PI,
        theta2 in -PI..PI,
    ) {
        let mut b1 = CircuitBuilder::new("c1");
        let q1 = b1.add_quantum_register("q", 1).unwrap();
        b1.rx(q1[0], Angle::from_radians(theta1)).unwrap();
        b1.rx(q1[0], Angle::from_radians(theta2)).unwrap();
        let c1 = b1.finish().unwrap();

        let mut b2 = CircuitBuilder::new("c2");
        let q2 = b2.add_quantum_register("q", 1).unwrap();
        b2.rx(q2[0], Angle::from_radians(theta1 + theta2)).unwrap();
        let c2 = b2.finish().unwrap();

        let verdict = check_equivalence(&c1, &c2).unwrap();
        prop_assert!(verdict.is_equivalent);
    }
}
