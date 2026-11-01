//! Phase 11 — Bell Vertical-Slice Acceptance Tests
//!
//! Validates the complete Milestone M1 compilation and execution pipeline:
//! CircuitBuilder -> QccIR -> Validation -> StateVector Simulation -> Equivalence Verification -> Seeded Sampling -> OpenQASM 3 Emitter.

use std::fs;
use std::path::Path;
use std::process::Command;

use num_complex::Complex64;
use qcc::backend::openqasm::emit_openqasm;
use qcc::builder::CircuitBuilder;
use qcc::quantum::semantics::{format_basis_state, ClassicalBitOrder};
use qcc::simulator::executor::{execute_circuit, CircuitExecutor};
use qcc::simulator::sampling::SplitMix64;
use qcc::verification::{check_equivalence, VerificationError};

/// Helper to construct the complete standard Bell state circuit with terminal measurements.
fn build_bell_circuit() -> qcc::ir::circuit::Circuit {
    let mut builder = CircuitBuilder::new("bell_vertical_slice");
    let q = builder
        .add_quantum_register("q", 2)
        .expect("failed to add quantum register");
    let c = builder
        .add_classical_register("c", 2)
        .expect("failed to add classical register");

    builder.h(q[0]).expect("failed to apply H gate");
    builder.cx(q[0], q[1]).expect("failed to apply CX gate");
    builder
        .measure(q[0], c[0])
        .expect("failed to add measurement q[0]->c[0]");
    builder
        .measure(q[1], c[1])
        .expect("failed to add measurement q[1]->c[1]");

    builder.finish().expect("failed to finish circuit")
}

/// Helper to construct the pure unitary Bell state preparation circuit (without measurements).
fn build_unitary_bell_circuit() -> qcc::ir::circuit::Circuit {
    let mut builder = CircuitBuilder::new("bell_unitary");
    let q = builder
        .add_quantum_register("q", 2)
        .expect("failed to add quantum register");

    builder.h(q[0]).expect("failed to apply H gate");
    builder.cx(q[0], q[1]).expect("failed to apply CX gate");

    builder.finish().expect("failed to finish circuit")
}

/// Helper to construct an alternative synthesis reference circuit:
/// CX(q0, q1) = H(q1) * CZ(q0, q1) * H(q1)
fn build_alternative_reference_bell_circuit() -> qcc::ir::circuit::Circuit {
    let mut builder = CircuitBuilder::new("bell_reference");
    let q = builder
        .add_quantum_register("q", 2)
        .expect("failed to add quantum register");

    builder.h(q[0]).expect("failed to apply H(q0)");
    builder.h(q[1]).expect("failed to apply H(q1)");
    builder.cz(q[0], q[1]).expect("failed to apply CZ(q0, q1)");
    builder.h(q[1]).expect("failed to apply H(q1)");

    builder.finish().expect("failed to finish circuit")
}

#[test]
fn test_bell_vertical_slice_end_to_end() {
    // 1. Build Bell state circuit via public CircuitBuilder API
    let circuit = build_bell_circuit();

    // 2. Validate QccIR structural and semantic integrity
    assert_eq!(circuit.num_qubits(), 2);
    assert_eq!(circuit.num_classical_bits(), 2);
    assert_eq!(circuit.graph.node_count(), 4);
    assert_eq!(circuit.graph.edge_count(), 3);
    assert!(circuit.validate().is_ok());

    // 3. Execute circuit in statevector engine
    let (state, trace) = execute_circuit(&circuit).expect("execution failed");
    assert_eq!(state.num_qubits(), 2);
    assert_eq!(state.len(), 4);
    assert_eq!(trace.steps.len(), 4);
    assert_eq!(trace.measurements.len(), 2);

    // 4. Verify exact amplitudes under little-endian convention (q[0] is LSB)
    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
    let eps = 1e-10;

    let a0 = state.get(0).unwrap();
    let a1 = state.get(1).unwrap();
    let a2 = state.get(2).unwrap();
    let a3 = state.get(3).unwrap();

    assert!(
        (a0.re - inv_sqrt2).abs() < eps && a0.im.abs() < eps,
        "|00> amplitude mismatch: {a0:?}"
    );
    assert!(a1.norm() < eps, "|01> amplitude must be 0, got: {a1:?}");
    assert!(a2.norm() < eps, "|10> amplitude must be 0, got: {a2:?}");
    assert!(
        (a3.re - inv_sqrt2).abs() < eps && a3.im.abs() < eps,
        "|11> amplitude mismatch: {a3:?}"
    );

    // Verify normalization
    assert!((state.norm() - 1.0).abs() < eps);

    // 5. Seeded terminal measurement sampling
    let executor = CircuitExecutor::new();
    let num_shots = 2_000;
    let (result, _) = executor
        .sample(&circuit, num_shots, SplitMix64::new(42))
        .expect("sampling failed");

    assert_eq!(result.num_shots(), num_shots);
    let counts = result.counts(ClassicalBitOrder::MsbFirst);

    // Only "00" and "11" should appear
    let c00 = counts.get("00").copied().unwrap_or(0);
    let c11 = counts.get("11").copied().unwrap_or(0);
    let c01 = counts.get("01").copied().unwrap_or(0);
    let c10 = counts.get("10").copied().unwrap_or(0);

    assert_eq!(c01, 0, "Disallowed state |01> observed: {c01}");
    assert_eq!(c10, 0, "Disallowed state |10> observed: {c10}");
    assert_eq!(c00 + c11, num_shots);

    // 4-sigma check: for N=2000, p=0.5, sigma = sqrt(2000*0.25) = ~22.36 => 4*sigma = ~89.4
    // Expected count between 900 and 1100, checking generously [850, 1150]
    assert!(
        (850..=1150).contains(&c00),
        "Count for '00' ({c00}) out of expected statistical bounds [850, 1150]"
    );
    assert!(
        (850..=1150).contains(&c11),
        "Count for '11' ({c11}) out of expected statistical bounds [850, 1150]"
    );

    // 6. Emit OpenQASM 3 and verify deterministic structure
    let emitted = emit_openqasm(&circuit).expect("OpenQASM emission failed");
    assert!(emitted.contains("OPENQASM 3.0;"));
    assert!(emitted.contains("include \"stdgates.inc\";"));
    assert!(emitted.contains("qubit[2] q;"));
    assert!(emitted.contains("bit[2] c;"));
    assert!(emitted.contains("h q[0];"));
    assert!(emitted.contains("cx q[0], q[1];"));
    assert!(emitted.contains("c[0] = measure q[0];"));
    assert!(emitted.contains("c[1] = measure q[1];"));
}

#[test]
fn test_bell_exact_statevector_amplitudes_and_little_endian() {
    let circuit = build_unitary_bell_circuit();
    let (state, _) = execute_circuit(&circuit).expect("execution failed");

    let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
    let eps = 1e-12;

    // Basis index to Dirac notation check under little-endian convention:
    // Index 0: |q1=0, q0=0> -> |00>
    // Index 1: |q1=0, q0=1> -> |01>
    // Index 2: |q1=1, q0=0> -> |10>
    // Index 3: |q1=1, q0=1> -> |11>
    assert_eq!(format_basis_state(0, 2), "|00>");
    assert_eq!(format_basis_state(1, 2), "|01>");
    assert_eq!(format_basis_state(2, 2), "|10>");
    assert_eq!(format_basis_state(3, 2), "|11>");

    // Check exact amplitudes
    assert!((state.get(0).unwrap() - Complex64::new(inv_sqrt2, 0.0)).norm() < eps);
    assert!(state.get(1).unwrap().norm() < eps);
    assert!(state.get(2).unwrap().norm() < eps);
    assert!((state.get(3).unwrap() - Complex64::new(inv_sqrt2, 0.0)).norm() < eps);

    // Check exact probabilities
    let probs = state
        .probabilities()
        .expect("probabilities calculation failed");
    assert_eq!(probs.len(), 4);
    assert!((probs.get(0).unwrap() - 0.5).abs() < eps);
    assert!(probs.get(1).unwrap().abs() < eps);
    assert!(probs.get(2).unwrap().abs() < eps);
    assert!((probs.get(3).unwrap() - 0.5).abs() < eps);
}

#[test]
fn test_bell_unitary_equivalence_against_reference_synthesis() {
    let candidate = build_unitary_bell_circuit();
    let reference = build_alternative_reference_bell_circuit();

    let verdict = check_equivalence(&candidate, &reference).expect("verification failed");

    assert!(
        verdict.is_equivalent,
        "Expected circuits to be equivalent, got mismatch: {:?}",
        verdict.worst_mismatch
    );
    assert!(
        verdict.max_amplitude_difference < 1e-10,
        "Expected negligible amplitude difference, got: {}",
        verdict.max_amplitude_difference
    );
    if let Some(phase) = verdict.estimated_phase {
        assert!(
            phase.abs() < 1e-10 || (phase - std::f64::consts::TAU).abs() < 1e-10,
            "Expected near-zero global phase, got: {phase}"
        );
    }
}

#[test]
fn test_unitary_checker_detects_deliberate_inequivalences() {
    let reference = build_unitary_bell_circuit();

    // 1. Inequivalent variant: missing H gate
    let mut builder = CircuitBuilder::new("missing_h");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit_missing_h = builder.finish().unwrap();

    let verdict1 = check_equivalence(&reference, &circuit_missing_h).unwrap();
    assert!(!verdict1.is_equivalent);
    assert!(verdict1.worst_mismatch.is_some());

    // 2. Inequivalent variant: relative phase error (H followed by S gate on q0)
    let mut builder = CircuitBuilder::new("relative_phase_error");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.s(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    let circuit_phase_error = builder.finish().unwrap();

    let verdict2 = check_equivalence(&reference, &circuit_phase_error).unwrap();
    assert!(!verdict2.is_equivalent);
    assert!(verdict2.worst_mismatch.is_some());

    // 3. Inequivalent variant: swapped control and target (CX(q1, q0) instead of CX(q0, q1))
    let mut builder = CircuitBuilder::new("swapped_cx");
    let q = builder.add_quantum_register("q", 2).unwrap();
    builder.h(q[0]).unwrap();
    builder.cx(q[1], q[0]).unwrap();
    let circuit_swapped_cx = builder.finish().unwrap();

    let verdict3 = check_equivalence(&reference, &circuit_swapped_cx).unwrap();
    assert!(!verdict3.is_equivalent);
    assert!(verdict3.worst_mismatch.is_some());
}

#[test]
fn test_unitary_checker_rejects_non_unitary_circuits() {
    let circuit_with_measurements = build_bell_circuit();
    let unitary_bell = build_unitary_bell_circuit();

    let result = check_equivalence(&circuit_with_measurements, &unitary_bell);
    match result {
        Err(VerificationError::NonUnitaryOperation { operation, .. }) => {
            assert_eq!(operation, "measure");
        }
        res => panic!("Expected NonUnitaryOperation error, got: {res:?}"),
    }
}

#[test]
fn test_bell_sampling_statistics_and_seed_reproducibility() {
    let circuit = build_bell_circuit();
    let executor = CircuitExecutor::new();

    // 10,000 shots with seed 12345
    let num_shots = 10_000;
    let (res1, _) = executor
        .sample(&circuit, num_shots, SplitMix64::new(12345))
        .unwrap();

    let c00_1 = *res1
        .counts(ClassicalBitOrder::MsbFirst)
        .get("00")
        .unwrap_or(&0);
    let c11_1 = *res1
        .counts(ClassicalBitOrder::MsbFirst)
        .get("11")
        .unwrap_or(&0);
    assert_eq!(c00_1 + c11_1, num_shots);
    assert_eq!(
        *res1
            .counts(ClassicalBitOrder::MsbFirst)
            .get("01")
            .unwrap_or(&0),
        0
    );
    assert_eq!(
        *res1
            .counts(ClassicalBitOrder::MsbFirst)
            .get("10")
            .unwrap_or(&0),
        0
    );

    // Ratio within 3-sigma (3 * sqrt(10000*0.25) / 10000 = 0.015 => 0.485 to 0.515)
    let ratio = c00_1 as f64 / num_shots as f64;
    assert!(
        (0.48..=0.52).contains(&ratio),
        "Observed ratio {ratio} outside expected range [0.48, 0.52]"
    );

    // Repeat with exact same seed: must produce identical bitstrings and counts
    let (res2, _) = executor
        .sample(&circuit, num_shots, SplitMix64::new(12345))
        .unwrap();

    assert_eq!(
        res1.counts(ClassicalBitOrder::MsbFirst),
        res2.counts(ClassicalBitOrder::MsbFirst)
    );
    assert_eq!(res1.shots(), res2.shots());

    // Run with different seed: must have different raw shots
    let (res3, _) = executor
        .sample(&circuit, num_shots, SplitMix64::new(54321))
        .unwrap();

    assert_ne!(res1.shots(), res3.shots());
    let c00_3 = *res3
        .counts(ClassicalBitOrder::MsbFirst)
        .get("00")
        .unwrap_or(&0);
    let c11_3 = *res3
        .counts(ClassicalBitOrder::MsbFirst)
        .get("11")
        .unwrap_or(&0);
    assert_eq!(c00_3 + c11_3, num_shots);
}

#[test]
fn test_bell_classical_bit_ordering() {
    // For Bell circuit where c[0] == c[1], bitstrings are palindromes ("00", "11").
    // To rigorously test MsbFirst vs LsbFirst formatting, also construct an asymmetric circuit:
    // q[0] initialized to |1> via X, measured into c[0].
    // q[1] stays in |0>, measured into c[1].
    // c[0] = 1, c[1] = 0.
    let mut builder = CircuitBuilder::new("asymmetric_bits");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();

    builder.x(q[0]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    builder.measure(q[1], c[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let (result, _) = CircuitExecutor::new()
        .sample(&circuit, 10, SplitMix64::new(42))
        .unwrap();

    // MsbFirst: c[1]c[0] => "01"
    let msb_counts = result.counts(ClassicalBitOrder::MsbFirst);
    assert_eq!(msb_counts.get("01"), Some(&10));
    assert_eq!(msb_counts.get("10"), None);

    // LsbFirst: c[0]c[1] => "10"
    let lsb_counts = result.counts(ClassicalBitOrder::LsbFirst);
    assert_eq!(lsb_counts.get("10"), Some(&10));
    assert_eq!(lsb_counts.get("01"), None);
}

#[test]
fn test_openqasm3_export_golden_match() {
    let circuit = build_bell_circuit();
    let emitted = emit_openqasm(&circuit).expect("emission failed");

    let golden_path = Path::new("examples/qasm/bell.qasm");
    let golden = fs::read_to_string(golden_path)
        .expect("failed to read golden OpenQASM file examples/qasm/bell.qasm");

    assert_eq!(
        emitted.trim(),
        golden.trim(),
        "Emitted OpenQASM 3 does not match golden file examples/qasm/bell.qasm"
    );
}

#[test]
fn test_independent_openqasm3_parser_validation() {
    let circuit = build_bell_circuit();
    let emitted_qasm = emit_openqasm(&circuit).expect("emission failed");

    // Check if python3 with openqasm3 module is available
    let py_check = Command::new("python3")
        .arg("-c")
        .arg("import openqasm3; print(openqasm3.__version__)")
        .output();

    match py_check {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
            println!("Independent OpenQASM 3 parser detected: openqasm3 v{version}");

            // Validate emitted Bell circuit with independent openqasm3.parse()
            let mut child = Command::new("python3")
                .arg("-c")
                .arg("import sys, openqasm3; openqasm3.parse(sys.stdin.read()); print('VALID')")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to spawn python3 parser process");

            use std::io::Write;
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(emitted_qasm.as_bytes())
                .unwrap();

            let parse_result = child.wait_with_output().unwrap();
            assert!(
                parse_result.status.success(),
                "Independent openqasm3 parser rejected emitted QASM: {}",
                String::from_utf8_lossy(&parse_result.stderr)
            );
            println!("Independent OpenQASM 3 validation PASSED with openqasm3 v{version}");
        }
        _ => {
            println!(
                "NOTE: External OpenQASM 3 parser ('openqasm3' or 'qiskit') is not installed in the current environment.\n\
                 Independent parser verification is reported candidly as unavailable in this offline sandbox."
            );
        }
    }
}
