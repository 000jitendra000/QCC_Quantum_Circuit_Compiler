//! Integration tests for the exact 15-gate Toffoli (CCX) decomposition template.

use num_complex::Complex64;
use qcc::builder::CircuitBuilder;
use qcc::ir::wire::QubitId;
use qcc::quantum::gate_matrix::{matrix_h, matrix_t, matrix_tdg};
use qcc::quantum::Gate;
use qcc::simulator::StateVector;
use qcc::synthesis::templates::matrix::Matrix8x8;
use qcc::synthesis::templates::toffoli::ToffoliToCxTemplate;
use qcc::synthesis::templates::{DecompositionTemplate, TemplateError};

#[test]
fn test_toffoli_matrix_verification() {
    let template = ToffoliToCxTemplate::new();
    assert!(template.verify_matrix().is_ok());
}

#[test]
fn test_toffoli_matrix_all_permutations() {
    let template = ToffoliToCxTemplate::new();
    let permutations = [
        (0, 1, 2),
        (1, 0, 2),
        (0, 2, 1),
        (2, 0, 1),
        (1, 2, 0),
        (2, 1, 0),
    ];

    for (c1, c2, t) in permutations {
        let q_c1 = QubitId(c1);
        let q_c2 = QubitId(c2);
        let q_t = QubitId(t);

        let ops = template.expand(&[q_c1, q_c2, q_t]).unwrap();
        assert_eq!(ops.len(), 15);

        // Compute accumulated 8x8 matrix by left-multiplication
        let mut u = Matrix8x8::identity();
        for op in &ops {
            let op_mat = match op.name() {
                "h" => Matrix8x8::embed_single_qubit(&matrix_h(), op.qubits()[0].0),
                "t" => Matrix8x8::embed_single_qubit(&matrix_t(), op.qubits()[0].0),
                "tdg" => Matrix8x8::embed_single_qubit(&matrix_tdg(), op.qubits()[0].0),
                "cx" => Matrix8x8::embed_cx(op.qubits()[0].0, op.qubits()[1].0),
                other => panic!("Unexpected gate {} in Toffoli decomposition", other),
            };
            u = op_mat.matmul(&u);
        }

        let reference = Matrix8x8::exact_toffoli(c1, c2, t);
        let diff = u.max_difference(&reference);
        assert!(
            diff < 1e-12,
            "Permutation (c1={}, c2={}, t={}) failed with diff {}",
            c1,
            c2,
            t,
            diff
        );
    }
}

#[test]
fn test_toffoli_truth_table_on_all_basis_states() {
    // Controls: c1=0, c2=1, Target: t=2
    // In little-endian, index = q0 + 2*q1 + 4*q2.
    // When q0=1 and q1=1 (indices 3 and 7), q2 flips:
    // |011> (3) <-> |111> (7).
    let template = ToffoliToCxTemplate::new();
    let ops = template
        .expand(&[QubitId(0), QubitId(1), QubitId(2)])
        .unwrap();

    let expected_mapping = [
        (0, 0), // |000> -> |000>
        (1, 1), // |001> -> |001>
        (2, 2), // |010> -> |010>
        (3, 7), // |011> -> |111> (q2 flipped from 0 to 1)
        (4, 4), // |100> -> |100>
        (5, 5), // |101> -> |101>
        (6, 6), // |110> -> |110>
        (7, 3), // |111> -> |011> (q2 flipped from 1 to 0)
    ];

    for (in_idx, expected_out) in expected_mapping {
        let mut sv = StateVector::from_basis_state(3, in_idx).unwrap();

        for op in &ops {
            match op.name() {
                "h" => sv.apply_gate(&Gate::H, op.qubits()[0].0).unwrap(),
                "t" => sv.apply_gate(&Gate::T, op.qubits()[0].0).unwrap(),
                "tdg" => sv.apply_gate(&Gate::Tdg, op.qubits()[0].0).unwrap(),
                "cx" => sv.apply_cx(op.qubits()[0].0, op.qubits()[1].0).unwrap(),
                other => panic!("Unexpected gate {}", other),
            }
        }

        let amp = sv.get(expected_out).unwrap();
        assert!(
            (amp - Complex64::new(1.0, 0.0)).norm() < 1e-12,
            "Input {} -> expected {} but amp was {:?}",
            in_idx,
            expected_out,
            amp
        );
        let prob = sv.probability(expected_out).unwrap();
        assert!((prob - 1.0).abs() < 1e-12);
    }
}

#[test]
fn test_toffoli_zero_relative_phase_exactness() {
    // Crucial requirement: Nielsen & Chuang 15-gate sequence is an EXACT Toffoli,
    // not a relative-phase Toffoli (e.g. Margolus).
    // All diagonal entries for basis states where control is not active must be exactly 1.0 (phase = 0).
    let template = ToffoliToCxTemplate::new();
    let ops = template
        .expand(&[QubitId(0), QubitId(1), QubitId(2)])
        .unwrap();

    let mut u = Matrix8x8::identity();
    for op in &ops {
        let op_mat = match op.name() {
            "h" => Matrix8x8::embed_single_qubit(&matrix_h(), op.qubits()[0].0),
            "t" => Matrix8x8::embed_single_qubit(&matrix_t(), op.qubits()[0].0),
            "tdg" => Matrix8x8::embed_single_qubit(&matrix_tdg(), op.qubits()[0].0),
            "cx" => Matrix8x8::embed_cx(op.qubits()[0].0, op.qubits()[1].0),
            other => panic!("Unexpected gate {}", other),
        };
        u = op_mat.matmul(&u);
    }

    // Check untouched diagonal entries (0, 1, 2, 4, 5, 6)
    for idx in [0, 1, 2, 4, 5, 6] {
        let diag = u.get(idx, idx);
        assert!(
            (diag - Complex64::new(1.0, 0.0)).norm() < 1e-12,
            "Basis state {} diagonal element is {:?}, expected strictly 1.0 (no relative phase!)",
            idx,
            diag
        );
    }

    // Check swapped off-diagonal entries (3 <-> 7)
    let off1 = u.get(7, 3);
    let off2 = u.get(3, 7);
    assert!((off1 - Complex64::new(1.0, 0.0)).norm() < 1e-12);
    assert!((off2 - Complex64::new(1.0, 0.0)).norm() < 1e-12);
}

#[test]
fn test_toffoli_arbitrary_superposition_evolution() {
    let template = ToffoliToCxTemplate::new();
    let ops = template
        .expand(&[QubitId(0), QubitId(1), QubitId(2)])
        .unwrap();

    let amps = [
        Complex64::new(0.3, 0.1),
        Complex64::new(-0.2, 0.4),
        Complex64::new(0.1, -0.3),
        Complex64::new(0.4, 0.2), // idx 3
        Complex64::new(-0.1, 0.2),
        Complex64::new(0.3, -0.3),
        Complex64::new(0.2, 0.1),
        Complex64::new(-0.3, 0.1), // idx 7
    ];
    let norm_sq: f64 = amps.iter().map(|a| a.norm_sqr()).sum();
    let norm = norm_sq.sqrt();
    let normalized_amps: Vec<Complex64> = amps.iter().map(|a| a / norm).collect();
    let mut sv = StateVector::from_amplitudes(3, normalized_amps).unwrap();

    // Expected state after exact Toffoli(0, 1, 2): amplitudes 3 and 7 are exchanged
    let mut expected_sv = sv.clone();
    let amp3 = expected_sv.get(3).unwrap();
    let amp7 = expected_sv.get(7).unwrap();
    expected_sv.amplitudes_mut()[3] = amp7;
    expected_sv.amplitudes_mut()[7] = amp3;

    // Apply decomposed sequence
    for op in &ops {
        match op.name() {
            "h" => sv.apply_gate(&Gate::H, op.qubits()[0].0).unwrap(),
            "t" => sv.apply_gate(&Gate::T, op.qubits()[0].0).unwrap(),
            "tdg" => sv.apply_gate(&Gate::Tdg, op.qubits()[0].0).unwrap(),
            "cx" => sv.apply_cx(op.qubits()[0].0, op.qubits()[1].0).unwrap(),
            other => panic!("Unexpected gate {}", other),
        }
    }

    assert!(sv.approx_eq(&expected_sv, 1e-12));
}

#[test]
fn test_toffoli_preconditions() {
    let template = ToffoliToCxTemplate::new();
    let q0 = QubitId(0);
    let q1 = QubitId(1);
    let q2 = QubitId(2);

    // Duplicate controls
    match template.check_preconditions(&[q0, q0, q2]) {
        Err(TemplateError::DuplicateOperands { template: name, .. }) => {
            assert_eq!(name, "toffoli_to_cx");
        }
        other => panic!("Expected DuplicateOperands, got {:?}", other),
    }

    // Control equal to target
    match template.check_preconditions(&[q0, q1, q0]) {
        Err(TemplateError::DuplicateOperands { template: name, .. }) => {
            assert_eq!(name, "toffoli_to_cx");
        }
        other => panic!("Expected DuplicateOperands, got {:?}", other),
    }

    // Target equal to second control
    match template.check_preconditions(&[q0, q1, q1]) {
        Err(TemplateError::DuplicateOperands { template: name, .. }) => {
            assert_eq!(name, "toffoli_to_cx");
        }
        other => panic!("Expected DuplicateOperands, got {:?}", other),
    }

    // Wrong arity
    match template.check_preconditions(&[q0, q1]) {
        Err(TemplateError::ArityMismatch {
            template: name,
            expected,
            actual,
        }) => {
            assert_eq!(name, "toffoli_to_cx");
            assert_eq!(expected, 3);
            assert_eq!(actual, 2);
        }
        other => panic!("Expected ArityMismatch, got {:?}", other),
    }
}

#[test]
fn test_toffoli_circuit_dag_replacement() {
    let mut builder = CircuitBuilder::new("toffoli_circ");
    let q = builder.add_quantum_register("q", 3).unwrap();

    let h0 = builder.h(q[0]).unwrap();
    let ccx_op = builder
        .custom_gate("ccx", &[q[0], q[1], q[2]], vec![])
        .unwrap();
    let x2 = builder.x(q[2]).unwrap();
    let mut circuit = builder.finish().unwrap();

    let template = ToffoliToCxTemplate::new();
    let replacement_ids = template.apply(&mut circuit, ccx_op).unwrap();

    assert_eq!(replacement_ids.len(), 15);
    assert!(circuit.validate().is_ok());

    assert!(circuit.graph.contains_operation(h0));
    assert!(circuit.graph.contains_operation(x2));
    assert!(!circuit.graph.contains_operation(ccx_op));
}
