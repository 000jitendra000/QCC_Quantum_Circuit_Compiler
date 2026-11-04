//! Integration tests for CX <-> CZ fixed decomposition templates.

use num_complex::Complex64;
use qcc::builder::CircuitBuilder;
use qcc::ir::wire::QubitId;
use qcc::quantum::Gate;
use qcc::simulator::StateVector;
use qcc::synthesis::templates::cx_cz::{CxToCzTemplate, CzToCxTemplate};
use qcc::synthesis::templates::{DecompositionTemplate, TemplateError};
use qcc::verification::check_equivalence;

#[test]
fn test_cx_to_cz_matrix_verification() {
    let template = CxToCzTemplate::new();
    assert!(template.verify_matrix().is_ok());
}

#[test]
fn test_cz_to_cx_matrix_verification() {
    let template = CzToCxTemplate::new();
    assert!(template.verify_matrix().is_ok());
}

#[test]
fn test_cx_to_cz_basis_states() {
    // CX(0, 1) = H(1) ; CZ(0, 1) ; H(1)
    for basis in 0..4 {
        let mut sv_cx = StateVector::from_basis_state(2, basis).unwrap();
        sv_cx.apply_cx(0, 1).unwrap();

        let mut sv_decomp = StateVector::from_basis_state(2, basis).unwrap();
        sv_decomp.apply_gate(&Gate::H, 1).unwrap();
        sv_decomp.apply_cz(0, 1).unwrap();
        sv_decomp.apply_gate(&Gate::H, 1).unwrap();

        assert!(
            sv_cx.approx_eq(&sv_decomp, 1e-14),
            "Basis state |{:02b}> failed CX -> CZ equivalence",
            basis
        );
    }
}

#[test]
fn test_cz_to_cx_basis_states() {
    // CZ(0, 1) = H(1) ; CX(0, 1) ; H(1)
    for basis in 0..4 {
        let mut sv_cz = StateVector::from_basis_state(2, basis).unwrap();
        sv_cz.apply_cz(0, 1).unwrap();

        let mut sv_decomp = StateVector::from_basis_state(2, basis).unwrap();
        sv_decomp.apply_gate(&Gate::H, 1).unwrap();
        sv_decomp.apply_cx(0, 1).unwrap();
        sv_decomp.apply_gate(&Gate::H, 1).unwrap();

        assert!(
            sv_cz.approx_eq(&sv_decomp, 1e-14),
            "Basis state |{:02b}> failed CZ -> CX equivalence",
            basis
        );
    }
}

#[test]
fn test_cx_to_cz_reversed_orientation() {
    // When control=1, target=0:
    // CX(1, 0) = H(0) ; CZ(1, 0) ; H(0)
    for basis in 0..4 {
        let mut sv_cx = StateVector::from_basis_state(2, basis).unwrap();
        sv_cx.apply_cx(1, 0).unwrap();

        let mut sv_decomp = StateVector::from_basis_state(2, basis).unwrap();
        sv_decomp.apply_gate(&Gate::H, 0).unwrap();
        sv_decomp.apply_cz(1, 0).unwrap();
        sv_decomp.apply_gate(&Gate::H, 0).unwrap();

        assert!(
            sv_cx.approx_eq(&sv_decomp, 1e-14),
            "Basis state |{:02b}> failed reversed CX(1, 0) -> CZ equivalence",
            basis
        );
    }
}

#[test]
fn test_cx_to_cz_superposition_state() {
    let amps = [
        Complex64::new(0.3, 0.4),
        Complex64::new(-0.1, 0.2),
        Complex64::new(0.5, -0.2),
        Complex64::new(0.4, 0.3),
    ];
    let norm_sq: f64 = amps.iter().map(|a| a.norm_sqr()).sum();
    let norm = norm_sq.sqrt();
    let normalized_amps: Vec<Complex64> = amps.iter().map(|a| a / norm).collect();

    let mut sv_cx = StateVector::from_amplitudes(2, normalized_amps).unwrap();
    let mut sv_decomp = sv_cx.clone();

    sv_cx.apply_cx(0, 1).unwrap();

    sv_decomp.apply_gate(&Gate::H, 1).unwrap();
    sv_decomp.apply_cz(0, 1).unwrap();
    sv_decomp.apply_gate(&Gate::H, 1).unwrap();

    assert!(sv_cx.approx_eq(&sv_decomp, 1e-14));
}

#[test]
fn test_cz_to_cx_superposition_state() {
    let amps = [
        Complex64::new(0.3, 0.4),
        Complex64::new(-0.1, 0.2),
        Complex64::new(0.5, -0.2),
        Complex64::new(0.4, 0.3),
    ];
    let norm_sq: f64 = amps.iter().map(|a| a.norm_sqr()).sum();
    let norm = norm_sq.sqrt();
    let normalized_amps: Vec<Complex64> = amps.iter().map(|a| a / norm).collect();

    let mut sv_cz = StateVector::from_amplitudes(2, normalized_amps).unwrap();
    let mut sv_decomp = sv_cz.clone();

    sv_cz.apply_cz(0, 1).unwrap();

    sv_decomp.apply_gate(&Gate::H, 1).unwrap();
    sv_decomp.apply_cx(0, 1).unwrap();
    sv_decomp.apply_gate(&Gate::H, 1).unwrap();

    assert!(sv_cz.approx_eq(&sv_decomp, 1e-14));
}

#[test]
fn test_cx_to_cz_circuit_equivalence() {
    let mut builder = CircuitBuilder::new("circ_cx_to_cz");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    let cx_op = builder.cx(q[0], q[1]).unwrap();
    builder.x(q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let original = circuit.clone();

    let template = CxToCzTemplate::new();
    let replacements = template.apply(&mut circuit, cx_op).unwrap();

    assert_eq!(replacements.len(), 3);
    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&original, &circuit).unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_cz_to_cx_circuit_equivalence() {
    let mut builder = CircuitBuilder::new("circ_cz_to_cx");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    let cz_op = builder.cz(q[0], q[1]).unwrap();
    builder.z(q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let original = circuit.clone();

    let template = CzToCxTemplate::new();
    let replacements = template.apply(&mut circuit, cz_op).unwrap();

    assert_eq!(replacements.len(), 3);
    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&original, &circuit).unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_cx_cz_roundtrip() {
    // CX -> CZ -> CX round-trip
    let mut builder = CircuitBuilder::new("roundtrip");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    let cx_op = builder.cx(q[0], q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let original = circuit.clone();

    // 1. CX -> CZ
    let cx_to_cz = CxToCzTemplate::new();
    let replacements = cx_to_cz.apply(&mut circuit, cx_op).unwrap();
    assert_eq!(replacements.len(), 3);

    // The middle replacement is the CZ operation
    let cz_op_id = replacements[1];

    // 2. CZ -> CX
    let cz_to_cx = CzToCxTemplate::new();
    cz_to_cx.apply(&mut circuit, cz_op_id).unwrap();

    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&original, &circuit).unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_cx_cz_preconditions() {
    let cx_template = CxToCzTemplate::new();
    let cz_template = CzToCxTemplate::new();
    let q0 = QubitId(0);
    let q1 = QubitId(1);

    // Duplicate qubits
    match cx_template.check_preconditions(&[q0, q0]) {
        Err(TemplateError::DuplicateOperands { template: name, .. }) => {
            assert_eq!(name, "cx_to_cz");
        }
        other => panic!("Expected DuplicateOperands, got {:?}", other),
    }

    match cz_template.check_preconditions(&[q1, q1]) {
        Err(TemplateError::DuplicateOperands { template: name, .. }) => {
            assert_eq!(name, "cz_to_cx");
        }
        other => panic!("Expected DuplicateOperands, got {:?}", other),
    }

    // Arity mismatch
    match cx_template.check_preconditions(&[q0]) {
        Err(TemplateError::ArityMismatch {
            template: name,
            expected,
            actual,
        }) => {
            assert_eq!(name, "cx_to_cz");
            assert_eq!(expected, 2);
            assert_eq!(actual, 1);
        }
        other => panic!("Expected ArityMismatch, got {:?}", other),
    }
}
