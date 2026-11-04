//! Integration tests for the SWAP -> 3 CX fixed decomposition template.

use num_complex::Complex64;
use qcc::builder::CircuitBuilder;
use qcc::ir::wire::QubitId;
use qcc::simulator::StateVector;
use qcc::synthesis::templates::swap::SwapToCxTemplate;
use qcc::synthesis::templates::{DecompositionTemplate, TemplateError};
use qcc::verification::check_equivalence;

#[test]
fn test_swap_matrix_verification() {
    let template = SwapToCxTemplate::new();
    assert!(template.verify_matrix().is_ok());
}

#[test]
fn test_swap_vs_decomposed_cx_basis_states() {
    // Test that SWAP and the 3-CX sequence have identical action on all 4 basis states.
    for basis in 0..4 {
        let mut sv_swap = StateVector::from_basis_state(2, basis).unwrap();
        sv_swap.apply_swap(0, 1).unwrap();

        let mut sv_decomp = StateVector::from_basis_state(2, basis).unwrap();
        // CX(0, 1) ; CX(1, 0) ; CX(0, 1)
        sv_decomp.apply_cx(0, 1).unwrap();
        sv_decomp.apply_cx(1, 0).unwrap();
        sv_decomp.apply_cx(0, 1).unwrap();

        assert!(
            sv_swap.approx_eq(&sv_decomp, 1e-14),
            "Basis state |{:02b}> failed SWAP vs 3-CX equivalence",
            basis
        );
    }
}

#[test]
fn test_swap_decomposed_superposition_state() {
    // Create arbitrary non-trivial superposition state:
    // |psi> = 0.5|00> - 0.5|01> + 0.5i|10> + 0.5|11>
    let amps = vec![
        Complex64::new(0.5, 0.0),
        Complex64::new(-0.5, 0.0),
        Complex64::new(0.0, 0.5),
        Complex64::new(0.5, 0.0),
    ];
    let mut sv_swap = StateVector::from_amplitudes(2, amps).unwrap();
    let mut sv_decomp = sv_swap.clone();

    sv_swap.apply_swap(0, 1).unwrap();

    sv_decomp.apply_cx(0, 1).unwrap();
    sv_decomp.apply_cx(1, 0).unwrap();
    sv_decomp.apply_cx(0, 1).unwrap();

    assert!(sv_swap.approx_eq(&sv_decomp, 1e-14));
}

#[test]
fn test_swap_circuit_equivalence() {
    let mut b_swap = CircuitBuilder::new("circ_swap");
    let q = b_swap.add_quantum_register("q", 2).unwrap();
    b_swap.swap(q[0], q[1]).unwrap();
    let c_swap = b_swap.finish().unwrap();

    let mut b_cx = CircuitBuilder::new("circ_cx");
    let q_cx = b_cx.add_quantum_register("q", 2).unwrap();
    b_cx.cx(q_cx[0], q_cx[1]).unwrap();
    b_cx.cx(q_cx[1], q_cx[0]).unwrap();
    b_cx.cx(q_cx[0], q_cx[1]).unwrap();
    let c_cx = b_cx.finish().unwrap();

    let verdict = check_equivalence(&c_swap, &c_cx).unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_swap_template_apply_on_circuit() {
    let mut builder = CircuitBuilder::new("circ_swap_apply");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    let swap_op = builder.swap(q[0], q[1]).unwrap();
    builder.x(q[1]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let original_circuit = circuit.clone();

    let template = SwapToCxTemplate::new();
    let replacement_ids = template.apply(&mut circuit, swap_op).unwrap();

    assert_eq!(replacement_ids.len(), 3);
    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&original_circuit, &circuit).unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_swap_reversed_operands() {
    let template = SwapToCxTemplate::new();
    let q0 = QubitId(1);
    let q1 = QubitId(0);

    let ops = template.expand(&[q0, q1]).unwrap();
    assert_eq!(ops.len(), 3);
    assert_eq!(ops[0].qubits(), &[q0, q1]);
    assert_eq!(ops[1].qubits(), &[q1, q0]);
    assert_eq!(ops[2].qubits(), &[q0, q1]);

    // Apply in circuit
    let mut b_orig = CircuitBuilder::new("orig_rev");
    let q = b_orig.add_quantum_register("q", 2).unwrap();
    let swap_op = b_orig.swap(q[1], q[0]).unwrap();
    let mut circuit = b_orig.finish().unwrap();
    let orig = circuit.clone();

    template.apply(&mut circuit, swap_op).unwrap();

    let verdict = check_equivalence(&orig, &circuit).unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}

#[test]
fn test_swap_precondition_duplicate_qubits() {
    let template = SwapToCxTemplate::new();
    let q0 = QubitId(0);
    let result = template.check_preconditions(&[q0, q0]);
    match result {
        Err(TemplateError::DuplicateOperands { template: name, .. }) => {
            assert_eq!(name, "swap_to_cx");
        }
        other => panic!("Expected DuplicateOperands, got {:?}", other),
    }
}

#[test]
fn test_swap_precondition_arity_mismatch() {
    let template = SwapToCxTemplate::new();
    let q0 = QubitId(0);
    let q1 = QubitId(1);
    let q2 = QubitId(2);

    match template.check_preconditions(&[q0]) {
        Err(TemplateError::ArityMismatch {
            template: name,
            expected,
            actual,
        }) => {
            assert_eq!(name, "swap_to_cx");
            assert_eq!(expected, 2);
            assert_eq!(actual, 1);
        }
        other => panic!("Expected ArityMismatch, got {:?}", other),
    }

    match template.check_preconditions(&[q0, q1, q2]) {
        Err(TemplateError::ArityMismatch {
            template: name,
            expected,
            actual,
        }) => {
            assert_eq!(name, "swap_to_cx");
            assert_eq!(expected, 2);
            assert_eq!(actual, 3);
        }
        other => panic!("Expected ArityMismatch, got {:?}", other),
    }
}

#[test]
fn test_swap_dag_replacement_in_larger_context() {
    let mut builder = CircuitBuilder::new("context_swap");
    let q = builder.add_quantum_register("q", 3).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let swap_op = builder.swap(q[1], q[2]).unwrap();
    builder.x(q[0]).unwrap();
    builder.cz(q[0], q[2]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let original_circuit = circuit.clone();

    let template = SwapToCxTemplate::new();
    template.apply(&mut circuit, swap_op).unwrap();

    assert!(circuit.validate().is_ok());

    let verdict = check_equivalence(&original_circuit, &circuit).unwrap();
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < 1e-12);
}
