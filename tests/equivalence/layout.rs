//! Integration tests for Phase 45: Layout-Aware Equivalence.
//!
//! Validates:
//! 1. Semantic equivalence between unrouted logical circuits and routed physical circuits.
//! 2. Accurate tracking of initial layout ($L_{\text{initial}}$) and final layout ($L_{\text{final}}$).
//! 3. ADR-0001 strict ancilla zero-state policy: Every physical qubit not occupied
//!    by a logical qubit starts strictly in $|0\rangle$ and must return to $|0\rangle$.
//! 4. Little-endian indexing convention across all permutations.
//! 5. Common global phase estimation.

use std::f64::consts::PI;

use num_complex::Complex64;
use qcc::builder::CircuitBuilder;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::QubitId;
use qcc::routing::{CircuitLayout, Layout};
use qcc::target::operation::PhysicalQubitId;
use qcc::verification::{check_layout_equivalence, embed_logical_state, LayoutEquivalenceChecker};

#[test]
fn test_layout_equivalence_identity_mapping() {
    // Logical 3-qubit circuit
    let mut b_log = CircuitBuilder::new("log_bell");
    let q = b_log.add_quantum_register("q", 3).unwrap();
    b_log.h(q[0]).unwrap();
    b_log.cx(q[0], q[1]).unwrap();
    b_log.rz(q[2], ParameterExpr::constant(PI / 4.0)).unwrap();
    b_log.x(q[1]).unwrap();
    let circ_log = b_log.finish().unwrap();

    // Routed circuit with identical gates on 3 physical qubits
    let mut b_phys = CircuitBuilder::new("phys_bell");
    let p = b_phys.add_quantum_register("p", 3).unwrap();
    b_phys.h(p[0]).unwrap();
    b_phys.cx(p[0], p[1]).unwrap();
    b_phys.rz(p[2], ParameterExpr::constant(PI / 4.0)).unwrap();
    b_phys.x(p[1]).unwrap();
    let circ_phys = b_phys.finish().unwrap();

    let cl = CircuitLayout::identity(3, 3).unwrap();
    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();

    assert!(verdict.is_equivalent);
    assert_eq!(verdict.qubits, 3);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_layout_equivalence_inserted_swap() {
    // Logical 2-qubit circuit:
    // q[0]: --H--*--X--
    //            |
    // q[1]: -----X-----
    let mut b_log = CircuitBuilder::new("log_swap");
    let q = b_log.add_quantum_register("q", 2).unwrap();
    b_log.h(q[0]).unwrap();
    b_log.cx(q[0], q[1]).unwrap();
    b_log.x(q[0]).unwrap();
    let circ_log = b_log.finish().unwrap();

    // Physical circuit on 2 physical qubits with an inserted SWAP:
    // Initial layout: q[0] -> p[0], q[1] -> p[1]
    // 1. H on p[0] (logical q[0])
    // 2. SWAP(p[0], p[1]) => logical q[0] moves to p[1], logical q[1] moves to p[0]
    // 3. CX(p[1], p[0]) => ctrl is logical q[0] (at p[1]), targ is logical q[1] (at p[0])
    // 4. X on p[1] (logical q[0])
    // Final layout: q[0] -> p[1], q[1] -> p[0]
    let mut b_phys = CircuitBuilder::new("phys_swap");
    let p = b_phys.add_quantum_register("p", 2).unwrap();
    b_phys.h(p[0]).unwrap();
    b_phys.swap(p[0], p[1]).unwrap();
    b_phys.cx(p[1], p[0]).unwrap();
    b_phys.x(p[1]).unwrap();
    let circ_phys = b_phys.finish().unwrap();

    let mut cl = CircuitLayout::identity(2, 2).unwrap();
    cl.apply_physical_swap(PhysicalQubitId(0), PhysicalQubitId(1))
        .unwrap();

    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_layout_equivalence_changed_final_permutation_fails_when_unaccounted() {
    // Same logical and physical circuits as test_layout_equivalence_inserted_swap
    let mut b_log = CircuitBuilder::new("log_unaccounted");
    let q = b_log.add_quantum_register("q", 2).unwrap();
    b_log.h(q[0]).unwrap();
    b_log.cx(q[0], q[1]).unwrap();
    b_log.x(q[0]).unwrap();
    let circ_log = b_log.finish().unwrap();

    let mut b_phys = CircuitBuilder::new("phys_unaccounted");
    let p = b_phys.add_quantum_register("p", 2).unwrap();
    b_phys.h(p[0]).unwrap();
    b_phys.swap(p[0], p[1]).unwrap();
    b_phys.cx(p[1], p[0]).unwrap();
    b_phys.x(p[1]).unwrap();
    let circ_phys = b_phys.finish().unwrap();

    // INCORRECT layout: claiming final layout is identity, ignoring the SWAP!
    let wrong_cl = CircuitLayout::identity(2, 2).unwrap();

    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &wrong_cl).unwrap();
    assert!(
        !verdict.is_equivalent,
        "circuit with unaccounted SWAP permutation must fail equivalence"
    );
    assert!(verdict.max_amplitude_difference > 0.1);
}

#[test]
fn test_layout_equivalence_random_valid_layouts_with_idle_ancillae() {
    // Logical 3-qubit circuit
    let mut b_log = CircuitBuilder::new("log_random_layout");
    let q = b_log.add_quantum_register("q", 3).unwrap();
    b_log.h(q[0]).unwrap();
    b_log.cx(q[0], q[1]).unwrap();
    b_log.cx(q[1], q[2]).unwrap();
    b_log.t(q[2]).unwrap();
    let circ_log = b_log.finish().unwrap();

    // Target has 5 physical qubits (2 idle ancillae)
    // Initial layout: q[0] -> p[3], q[1] -> p[1], q[2] -> p[4]
    // Idle ancillae: p[0], p[2]
    let initial = Layout::from_mappings(
        3,
        5,
        [
            (QubitId(0), PhysicalQubitId(3)),
            (QubitId(1), PhysicalQubitId(1)),
            (QubitId(2), PhysicalQubitId(4)),
        ],
    )
    .unwrap();
    let mut cl = CircuitLayout::new(initial);

    // Build physical circuit:
    // 1. H on p[3] (logical q[0])
    // 2. CX on p[3], p[1] (q[0] -> q[1])
    // 3. Router inserts SWAP(p[1], p[0]) => q[1] migrates to idle ancilla p[0], freeing p[1]!
    // 4. CX on p[0], p[4] (q[1] -> q[2])
    // 5. T on p[4] (q[2])
    let mut b_phys = CircuitBuilder::new("phys_random_layout");
    let p = b_phys.add_quantum_register("p", 5).unwrap();
    b_phys.h(p[3]).unwrap();
    b_phys.cx(p[3], p[1]).unwrap();
    b_phys.swap(p[1], p[0]).unwrap();
    b_phys.cx(p[0], p[4]).unwrap();
    b_phys.t(p[4]).unwrap();
    let circ_phys = b_phys.finish().unwrap();

    // Update layout tracker with the routing SWAP
    cl.apply_physical_swap(PhysicalQubitId(1), PhysicalQubitId(0))
        .unwrap();

    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();
    assert!(verdict.is_equivalent);
    assert_eq!(verdict.qubits, 5); // Compared across full 5 physical qubits!
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_deliberately_incorrect_final_layout_rejected() {
    let mut b_log = CircuitBuilder::new("log_tamper");
    let q = b_log.add_quantum_register("q", 2).unwrap();
    b_log.h(q[0]).unwrap();
    b_log.x(q[1]).unwrap();
    let circ_log = b_log.finish().unwrap();

    let mut b_phys = CircuitBuilder::new("phys_tamper");
    let p = b_phys.add_quantum_register("p", 3).unwrap();
    // mapped to p[0], p[1]
    b_phys.h(p[0]).unwrap();
    b_phys.x(p[1]).unwrap();
    let circ_phys = b_phys.finish().unwrap();

    // Correct initial: q[0] -> p[0], q[1] -> p[1]
    let initial = Layout::from_mappings(
        2,
        3,
        [
            (QubitId(0), PhysicalQubitId(0)),
            (QubitId(1), PhysicalQubitId(1)),
        ],
    )
    .unwrap();

    // Tampered final layout: swapped assignments q[0]->p[1], q[1]->p[0]
    let tampered_final = Layout::from_mappings(
        2,
        3,
        [
            (QubitId(0), PhysicalQubitId(1)),
            (QubitId(1), PhysicalQubitId(0)),
        ],
    )
    .unwrap();

    let cl = CircuitLayout::with_layouts(initial.clone(), initial, tampered_final).unwrap();
    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();
    assert!(!verdict.is_equivalent);
}

#[test]
fn test_unused_physical_qubit_clean_uncomputation_passes_adr0001() {
    // 2 logical qubits on a 4-qubit device (ancillae: p[2], p[3])
    let mut b_log = CircuitBuilder::new("log_ancilla");
    let q = b_log.add_quantum_register("q", 2).unwrap();
    b_log.h(q[0]).unwrap();
    b_log.cx(q[0], q[1]).unwrap();
    let circ_log = b_log.finish().unwrap();

    // Physical circuit borrows ancilla p[2] as scratchpad, then cleanly uncomputes it!
    let mut b_phys = CircuitBuilder::new("phys_ancilla_clean");
    let p = b_phys.add_quantum_register("p", 4).unwrap();
    b_phys.h(p[0]).unwrap();
    b_phys.cx(p[0], p[2]).unwrap(); // borrow ancilla p[2]
    b_phys.cx(p[2], p[1]).unwrap();
    b_phys.cx(p[2], p[1]).unwrap(); // uncompute
    b_phys.cx(p[0], p[2]).unwrap(); // uncompute p[2] back to |0>!
    b_phys.cx(p[0], p[1]).unwrap();
    let circ_phys = b_phys.finish().unwrap();

    let cl = CircuitLayout::identity(2, 4).unwrap();
    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();

    assert!(
        verdict.is_equivalent,
        "cleanly uncomputed ancilla must pass ADR-0001"
    );
    assert_eq!(verdict.qubits, 4);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_leakage_into_unused_physical_qubit_fails_adr0001() {
    // 2 logical qubits on a 4-qubit device (ancillae: p[2], p[3])
    let mut b_log = CircuitBuilder::new("log_leakage");
    let q = b_log.add_quantum_register("q", 2).unwrap();
    b_log.h(q[0]).unwrap();
    b_log.cx(q[0], q[1]).unwrap();
    let circ_log = b_log.finish().unwrap();

    // Physical circuit fails to uncompute ancilla p[2], leaving dirty state!
    let mut b_phys = CircuitBuilder::new("phys_leakage");
    let p = b_phys.add_quantum_register("p", 4).unwrap();
    b_phys.h(p[0]).unwrap();
    b_phys.cx(p[0], p[1]).unwrap();
    b_phys.x(p[2]).unwrap(); // DIRTY ANCILLA: leaves p[2] in |1>!
    let circ_phys = b_phys.finish().unwrap();

    let cl = CircuitLayout::identity(2, 4).unwrap();
    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();

    assert!(
        !verdict.is_equivalent,
        "dirty ancilla violating ADR-0001 must fail equivalence"
    );
    // Confirm diagnostic captures the ancilla zero-state violation
    let mismatch = verdict
        .worst_mismatch
        .expect("must report mismatch diagnostic");
    assert!(
        mismatch
            .exceeded_tolerance
            .contains("ADR-0001 ancilla zero-state violation"),
        "diagnostic must identify ADR-0001 violation: {}",
        mismatch.exceeded_tolerance
    );
}

#[test]
fn test_entanglement_with_unused_physical_qubit_fails_adr0001() {
    let mut b_log = CircuitBuilder::new("log_entangle");
    let q = b_log.add_quantum_register("q", 2).unwrap();
    b_log.h(q[0]).unwrap();
    let circ_log = b_log.finish().unwrap();

    // Physical circuit leaves ancilla p[2] entangled with logical q[0]!
    let mut b_phys = CircuitBuilder::new("phys_entangle");
    let p = b_phys.add_quantum_register("p", 3).unwrap();
    b_phys.h(p[0]).unwrap();
    b_phys.cx(p[0], p[2]).unwrap(); // leaves p[2] entangled with p[0]!
    let circ_phys = b_phys.finish().unwrap();

    let cl = CircuitLayout::identity(2, 3).unwrap();
    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();

    assert!(
        !verdict.is_equivalent,
        "entangled ancilla violating ADR-0001 must fail equivalence"
    );
}

#[test]
fn test_global_phase_preservation_under_layout() {
    // Logical circuit: S gate on q[0]
    let mut b_log = CircuitBuilder::new("log_phase");
    let q = b_log.add_quantum_register("q", 1).unwrap();
    b_log.s(q[0]).unwrap();
    let circ_log = b_log.finish().unwrap();

    // Routed circuit: RZ(p[1], pi/2) on a 3-qubit device
    // Note: S = exp(i*pi/4) * RZ(pi/2)
    let mut b_phys = CircuitBuilder::new("phys_phase");
    let p = b_phys.add_quantum_register("p", 3).unwrap();
    b_phys.rz(p[1], ParameterExpr::constant(PI / 2.0)).unwrap();
    let circ_phys = b_phys.finish().unwrap();

    // Layout: q[0] -> p[1], idle: p[0], p[2]
    let layout = Layout::from_mappings(1, 3, [(QubitId(0), PhysicalQubitId(1))]).unwrap();
    let cl = CircuitLayout::new(layout);

    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();

    // Must be equivalent up to global phase exp(-i*pi/4)
    assert!(verdict.is_equivalent);
    let est_phase = verdict.estimated_phase.expect("must estimate global phase");
    let expected_diff = (est_phase - (-PI / 4.0)).abs();
    assert!(
        expected_diff < 1e-10 || (expected_diff - 2.0 * PI).abs() < 1e-10,
        "estimated phase {est_phase} should equal -pi/4"
    );
}

#[test]
fn test_nontrivial_little_endian_permutation() {
    // Logical circuit with 3 distinct non-commuting/asymmetric operations:
    // q[0] (LSB): X
    // q[1]:       Y
    // q[2] (MSB): H
    let mut b_log = CircuitBuilder::new("log_endian");
    let q = b_log.add_quantum_register("q", 3).unwrap();
    b_log.x(q[0]).unwrap();
    b_log.y(q[1]).unwrap();
    b_log.h(q[2]).unwrap();
    let circ_log = b_log.finish().unwrap();

    // Physical circuit on 3 physical qubits with reversed permutation:
    // q[0] -> p[2] (logical LSB -> physical MSB)
    // q[1] -> p[1]
    // q[2] -> p[0] (logical MSB -> physical LSB)
    let mut b_phys = CircuitBuilder::new("phys_endian");
    let p = b_phys.add_quantum_register("p", 3).unwrap();
    b_phys.x(p[2]).unwrap();
    b_phys.y(p[1]).unwrap();
    b_phys.h(p[0]).unwrap();
    let circ_phys = b_phys.finish().unwrap();

    let layout = Layout::from_mappings(
        3,
        3,
        [
            (QubitId(0), PhysicalQubitId(2)),
            (QubitId(1), PhysicalQubitId(1)),
            (QubitId(2), PhysicalQubitId(0)),
        ],
    )
    .unwrap();
    let cl = CircuitLayout::new(layout);

    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();
    assert!(
        verdict.is_equivalent,
        "reversed little-endian layout must preserve equivalence"
    );
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_embed_logical_state_ancilla_zero_state() {
    use qcc::simulator::statevector::StateVector;

    // Logical state: equal superposition (|00> + |01> + |10> + |11>) / 2
    let amplitudes = vec![Complex64::new(0.5, 0.0); 4];
    let log_state = StateVector::from_amplitudes(2, amplitudes).unwrap();

    // Map to 4-qubit physical device: q[0] -> p[1], q[1] -> p[3]
    // Ancillae: p[0], p[2]
    let layout = Layout::from_mappings(
        2,
        4,
        [
            (QubitId(0), PhysicalQubitId(1)),
            (QubitId(1), PhysicalQubitId(3)),
        ],
    )
    .unwrap();

    let phys_state = embed_logical_state(&log_state, &layout).unwrap();
    assert_eq!(phys_state.num_qubits(), 4);

    // Physical basis indices must only have non-zero amplitudes when p[0] == 0 and p[2] == 0
    for (k, &amp) in phys_state.amplitudes().iter().enumerate() {
        let p0 = k & 1;
        let p2 = (k >> 2) & 1;
        if p0 != 0 || p2 != 0 {
            assert_eq!(
                amp,
                Complex64::new(0.0, 0.0),
                "ancillae p0 and p2 must have 0 amplitude"
            );
        } else {
            assert_eq!(
                amp,
                Complex64::new(0.5, 0.0),
                "embedded logical state amplitudes must be 0.5"
            );
        }
    }
}

#[test]
fn test_layout_equivalence_checker_randomized_states_mode() {
    let mut b_log = CircuitBuilder::new("log_rand_mode");
    let q = b_log.add_quantum_register("q", 2).unwrap();
    b_log.h(q[0]).unwrap();
    b_log.cx(q[0], q[1]).unwrap();
    let circ_log = b_log.finish().unwrap();

    let mut b_phys = CircuitBuilder::new("phys_rand_mode");
    let p = b_phys.add_quantum_register("p", 3).unwrap();
    b_phys.h(p[0]).unwrap();
    b_phys.cx(p[0], p[1]).unwrap();
    let circ_phys = b_phys.finish().unwrap();

    let cl = CircuitLayout::identity(2, 3).unwrap();

    // Force randomized states mode by setting max_matrix_qubits to 1 (< 2)
    let checker = LayoutEquivalenceChecker::new()
        .with_max_matrix_qubits(1)
        .with_random_states_count(10);

    let verdict = checker
        .check_layout_equivalence(&circ_log, &circ_phys, &cl)
        .unwrap();
    assert!(verdict.is_equivalent);
    match verdict.method {
        qcc::verification::VerificationMethod::RandomizedStates { num_states } => {
            assert_eq!(num_states, 10);
        }
        other => panic!("expected RandomizedStates, got {other:?}"),
    }
}

#[test]
fn test_rejection_of_state_dependent_phases_requiring_single_common_global_phase() {
    // Reference: Identity circuit on 2 qubits
    let mut b_log = CircuitBuilder::new("log_id2");
    b_log.add_quantum_register("q", 2).unwrap();
    let circ_log = b_log.finish().unwrap();

    // Candidate: Z gate on p[0]
    // Individual basis state outputs:
    // |00> -> +|00> (phase 0)
    // |01> -> -|01> (phase pi)
    // |10> -> +|10> (phase 0)
    // |11> -> -|11> (phase pi)
    // If evaluated per-basis-state with independent phases, each output state
    // would trivially match the reference! But under ONE SINGLE COMMON global phase,
    // the circuits are NOT equivalent and MUST be rejected!
    let mut b_phys = CircuitBuilder::new("phys_z0");
    let p = b_phys.add_quantum_register("p", 2).unwrap();
    b_phys.z(p[0]).unwrap();
    let circ_phys = b_phys.finish().unwrap();

    let cl = CircuitLayout::identity(2, 2).unwrap();
    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();

    assert!(
        !verdict.is_equivalent,
        "circuits with state-dependent phases must fail equivalence"
    );
    assert!(
        verdict.max_amplitude_difference > 0.5,
        "pointwise difference against common global phase must be significant"
    );
}

#[test]
fn test_rejection_of_state_dependent_phases_under_nontrivial_layout() {
    // Reference: Identity circuit on 2 qubits
    let mut b_log = CircuitBuilder::new("log_id_sparse");
    b_log.add_quantum_register("q", 2).unwrap();
    let circ_log = b_log.finish().unwrap();

    // Candidate: S gate on physical site p[2] on a 3-qubit device (ancilla p[1] idle)
    // Layout: q[0] -> p[2], q[1] -> p[0], idle: p[1]
    // |0> on q[0] (at p[2]) has phase 0, |1> on q[0] (at p[2]) has phase pi/2
    let mut b_phys = CircuitBuilder::new("phys_s2");
    let p = b_phys.add_quantum_register("p", 3).unwrap();
    b_phys.s(p[2]).unwrap();
    let circ_phys = b_phys.finish().unwrap();

    let layout = Layout::from_mappings(
        2,
        3,
        [
            (QubitId(0), PhysicalQubitId(2)),
            (QubitId(1), PhysicalQubitId(0)),
        ],
    )
    .unwrap();
    let cl = CircuitLayout::new(layout);

    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();
    assert!(
        !verdict.is_equivalent,
        "S gate vs Identity has state-dependent relative phase and must fail equivalence"
    );
}

#[test]
fn test_rejection_of_state_dependent_phases_in_randomized_mode() {
    let mut b_log = CircuitBuilder::new("log_rand_fail");
    b_log.add_quantum_register("q", 2).unwrap();
    let circ_log = b_log.finish().unwrap();

    let mut b_phys = CircuitBuilder::new("phys_rand_fail");
    let p = b_phys.add_quantum_register("p", 2).unwrap();
    b_phys.z(p[0]).unwrap();
    let circ_phys = b_phys.finish().unwrap();

    let cl = CircuitLayout::identity(2, 2).unwrap();
    let checker = LayoutEquivalenceChecker::new()
        .with_max_matrix_qubits(1)
        .with_random_states_count(15);

    let verdict = checker
        .check_layout_equivalence(&circ_log, &circ_phys, &cl)
        .unwrap();
    assert!(
        !verdict.is_equivalent,
        "state-dependent phases must fail equivalence in randomized mode as well"
    );
}

#[test]
fn test_nontrivial_global_phase_pi_over_3_multiqubit_layout() {
    // Reference logical circuit on 2 qubits: H(q[0]), CX(q[0], q[1]), RZ(q[1], pi/4)
    // 4 computational basis probe states (|00>, |01>, |10>, |11>)
    let mut b_log = CircuitBuilder::new("log_pi3");
    let q = b_log.add_quantum_register("q", 2).unwrap();
    b_log.h(q[0]).unwrap();
    b_log.cx(q[0], q[1]).unwrap();
    b_log.rz(q[1], ParameterExpr::constant(PI / 4.0)).unwrap();
    let circ_log = b_log.finish().unwrap();

    // Candidate physical circuit on 4 physical qubits:
    // Layout: q[0] -> p[2], q[1] -> p[0], idle ancillae: p[1], p[3]
    // Routed gates: H(p[2]), CX(p[2], p[0]), RZ(p[0], pi/4)
    // Plus explicit circuit global phase phi = pi/3 (~1.047197551)
    let phi = PI / 3.0;
    let mut b_phys = CircuitBuilder::new("phys_pi3");
    let p = b_phys.add_quantum_register("p", 4).unwrap();
    b_phys.h(p[2]).unwrap();
    b_phys.cx(p[2], p[0]).unwrap();
    b_phys.rz(p[0], ParameterExpr::constant(PI / 4.0)).unwrap();
    b_phys.set_global_phase(ParameterExpr::constant(phi));
    let circ_phys = b_phys.finish().unwrap();

    let layout = Layout::from_mappings(
        2,
        4,
        [
            (QubitId(0), PhysicalQubitId(2)),
            (QubitId(1), PhysicalQubitId(0)),
        ],
    )
    .unwrap();
    let cl = CircuitLayout::new(layout);

    let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();

    assert!(
        verdict.is_equivalent,
        "circuit with uniform global phase pi/3 across all basis probes must be accepted"
    );
    let est_phase = verdict.estimated_phase.expect("must estimate global phase");
    let phase_diff = (est_phase - phi).abs();
    assert!(
        phase_diff < 1e-10 || (phase_diff - 2.0 * PI).abs() < 1e-10,
        "estimated phase {est_phase} should equal pi/3 ({phi})"
    );
    assert!(
        verdict.max_amplitude_difference < 1e-10,
        "max amplitude difference should be within tolerance"
    );
}
