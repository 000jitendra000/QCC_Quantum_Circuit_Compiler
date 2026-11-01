//! Integration and property tests for OpenQASM 3 emitter.

use std::f64::consts::PI;
use std::process::Command;

use proptest::prelude::*;
use qcc::backend::openqasm::{
    emit_openqasm, EmitterOptions, GlobalPhasePolicy, MeasurementSyntax, OpenQasmEmitter,
    OpenQasmError,
};
use qcc::builder::CircuitBuilder;
use qcc::ir::circuit::Circuit;
use qcc::ir::operation::Operation;
use qcc::ir::parameter::ParameterExpr;
use qcc::ir::wire::{ClassicalBitId, QubitId};
use qcc::quantum::parameter::Angle;

#[test]
fn test_bell_circuit_golden_output() {
    let mut builder = CircuitBuilder::new("bell");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();

    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    builder.measure(q[1], c[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let emitted = emit_openqasm(&circuit).unwrap();

    let expected_golden = "\
OPENQASM 3.0;
include \"stdgates.inc\";

qubit[2] q;
bit[2] c;

h q[0];
cx q[0], q[1];
c[0] = measure q[0];
c[1] = measure q[1];
";

    assert_eq!(emitted, expected_golden);
}

#[test]
fn test_version_header_and_stdgates_inclusion() {
    // 1. Circuit with only reset and measurement: no gates used -> stdgates.inc omitted by default
    let mut builder = CircuitBuilder::new("no_gates");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();
    builder.reset(q[0]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    let circuit = builder.finish().unwrap();

    let emitted = emit_openqasm(&circuit).unwrap();
    assert!(emitted.starts_with("OPENQASM 3.0;\n\nqubit[1] q;"));
    assert!(!emitted.contains("include \"stdgates.inc\";"));

    // 2. Explicitly force inclusion of stdgates.inc
    let opts = EmitterOptions {
        include_stdgates: Some(true),
        ..Default::default()
    };
    let emitter = OpenQasmEmitter::with_options(opts);
    let emitted_forced = emitter.emit(&circuit).unwrap();
    assert!(emitted_forced.contains("include \"stdgates.inc\";"));

    // 3. Explicitly force omission of stdgates.inc even when gates exist
    let mut builder_gates = CircuitBuilder::new("with_gates");
    let q2 = builder_gates.add_quantum_register("q", 1).unwrap();
    builder_gates.x(q2[0]).unwrap();
    let circuit_gates = builder_gates.finish().unwrap();

    let opts_suppress = EmitterOptions {
        include_stdgates: Some(false),
        ..Default::default()
    };
    let emitter_suppress = OpenQasmEmitter::with_options(opts_suppress);
    let emitted_suppressed = emitter_suppress.emit(&circuit_gates).unwrap();
    assert!(!emitted_suppressed.contains("include \"stdgates.inc\";"));
}

#[test]
fn test_register_declarations_and_indexing() {
    let mut builder = CircuitBuilder::new("multi_register");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let anc = builder.add_quantum_register("ancilla", 1).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();
    let flag = builder.add_classical_register("flag", 2).unwrap();

    builder.h(q[0]).unwrap();
    builder.cx(q[1], anc[0]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    builder.measure(q[1], flag[0]).unwrap();
    builder.measure(anc[0], flag[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let emitted = emit_openqasm(&circuit).unwrap();

    assert!(emitted.contains("qubit[2] q;\n"));
    assert!(emitted.contains("qubit[1] ancilla;\n"));
    assert!(emitted.contains("bit[1] c;\n"));
    assert!(emitted.contains("bit[2] flag;\n"));

    assert!(emitted.contains("h q[0];"));
    assert!(emitted.contains("cx q[1], ancilla[0];"));
    assert!(emitted.contains("c[0] = measure q[0];"));
    assert!(emitted.contains("flag[0] = measure q[1];"));
    assert!(emitted.contains("flag[1] = measure ancilla[0];"));
}

#[test]
fn test_lowercase_standard_gate_names() {
    let mut builder = CircuitBuilder::new("all_gates");
    let q = builder.add_quantum_register("q", 2).unwrap();

    builder.x(q[0]).unwrap();
    builder.y(q[0]).unwrap();
    builder.z(q[0]).unwrap();
    builder.h(q[0]).unwrap();
    builder.s(q[0]).unwrap();
    builder.sdg(q[0]).unwrap();
    builder.t(q[0]).unwrap();
    builder.tdg(q[0]).unwrap();
    builder.cz(q[0], q[1]).unwrap();
    builder.swap(q[0], q[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let emitted = emit_openqasm(&circuit).unwrap();

    for gate in &[
        "x q[0];",
        "y q[0];",
        "z q[0];",
        "h q[0];",
        "s q[0];",
        "sdg q[0];",
        "t q[0];",
        "tdg q[0];",
        "cz q[0], q[1];",
        "swap q[0], q[1];",
    ] {
        assert!(
            emitted.contains(gate),
            "Expected emitted output to contain '{gate}'"
        );
    }
}

#[test]
fn test_control_target_operand_ordering() {
    let mut builder = CircuitBuilder::new("operand_order");
    let q = builder.add_quantum_register("q", 2).unwrap();

    // Control 0, target 1
    builder.cx(q[0], q[1]).unwrap();
    // Control 1, target 0
    builder.cx(q[1], q[0]).unwrap();

    let circuit = builder.finish().unwrap();
    let emitted = emit_openqasm(&circuit).unwrap();

    assert!(emitted.contains("cx q[0], q[1];\n"));
    assert!(emitted.contains("cx q[1], q[0];\n"));
}

#[test]
fn test_parameterized_rotations_syntax() {
    let mut builder = CircuitBuilder::new("rotations");
    let q = builder.add_quantum_register("q", 2).unwrap();

    builder.rx(q[0], Angle::from_radians(PI / 2.0)).unwrap();
    builder.ry(q[1], Angle::from_radians(PI / 4.0)).unwrap();
    builder.rz(q[0], Angle::from_radians(PI)).unwrap();

    let circuit = builder.finish().unwrap();
    let emitted = emit_openqasm(&circuit).unwrap();

    assert!(emitted.contains(&format!("rx({}) q[0];", PI / 2.0)));
    assert!(emitted.contains(&format!("ry({}) q[1];", PI / 4.0)));
    assert!(emitted.contains(&format!("rz({}) q[0];", PI)));
}

#[test]
fn test_symbolic_and_composite_parameters() {
    let mut circuit = Circuit::new("symbolic_params");
    circuit.add_quantum_register("q", 2).unwrap();

    let alpha = ParameterExpr::symbol("alpha");
    let beta = ParameterExpr::symbol("beta");
    let gamma = ParameterExpr::symbol("gamma");

    // 1. Bare symbol: rx(alpha) q[0]
    circuit
        .add_operation(Operation::custom_gate(
            "rx",
            vec![QubitId(0)],
            vec![alpha.clone()],
        ))
        .unwrap();

    // 2. Sum: ry(alpha + beta) q[1]
    circuit
        .add_operation(Operation::custom_gate(
            "ry",
            vec![QubitId(1)],
            vec![alpha.clone() + beta.clone()],
        ))
        .unwrap();

    // 3. Parenthesized product of sum: rz((alpha + beta) * gamma) q[0]
    circuit
        .add_operation(Operation::custom_gate(
            "rz",
            vec![QubitId(0)],
            vec![(alpha.clone() + beta.clone()) * gamma.clone()],
        ))
        .unwrap();

    // 4. Negated sum: rx(-(alpha + beta)) q[1]
    circuit
        .add_operation(Operation::custom_gate(
            "rx",
            vec![QubitId(1)],
            vec![-(alpha + beta)],
        ))
        .unwrap();

    let emitted = emit_openqasm(&circuit).unwrap();

    assert!(emitted.contains("rx(alpha) q[0];"));
    assert!(emitted.contains("ry(alpha + beta) q[1];"));
    assert!(emitted.contains("rz((alpha + beta) * gamma) q[0];"));
    assert!(emitted.contains("rx(-(alpha + beta)) q[1];"));
}

#[test]
fn test_measurement_syntax_options() {
    let mut builder = CircuitBuilder::new("measure_syntax");
    let q = builder.add_quantum_register("q", 1).unwrap();
    let c = builder.add_classical_register("c", 1).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    let circuit = builder.finish().unwrap();

    // 1. Default: Assignment syntax
    let emitted_assign = emit_openqasm(&circuit).unwrap();
    assert!(emitted_assign.contains("c[0] = measure q[0];"));

    // 2. Arrow syntax
    let opts = EmitterOptions {
        measurement_syntax: MeasurementSyntax::Arrow,
        ..Default::default()
    };
    let emitter_arrow = OpenQasmEmitter::with_options(opts);
    let emitted_arrow = emitter_arrow.emit(&circuit).unwrap();
    assert!(emitted_arrow.contains("measure q[0] -> c[0];"));
}

#[test]
fn test_reset_and_barrier_output() {
    let mut builder = CircuitBuilder::new("reset_barrier");
    let q = builder.add_quantum_register("q", 3).unwrap();

    builder.reset(q[0]).unwrap();
    builder.barrier(&[q[0], q[1]]).unwrap();
    builder.barrier(&[q[0], q[1], q[2]]).unwrap();

    let circuit = builder.finish().unwrap();
    let emitted = emit_openqasm(&circuit).unwrap();

    assert!(emitted.contains("reset q[0];\n"));
    assert!(emitted.contains("barrier q[0], q[1];\n"));
    assert!(emitted.contains("barrier q[0], q[1], q[2];\n"));
}

#[test]
fn test_global_barrier_output() {
    let mut circuit = Circuit::new("global_barrier");
    circuit.add_quantum_register("q", 2).unwrap();
    circuit
        .add_operation(Operation::Barrier { qubits: vec![] })
        .unwrap();

    let emitted = emit_openqasm(&circuit).unwrap();
    assert!(emitted.contains("barrier;\n"));
}

#[test]
fn test_deterministic_output_across_repeated_exports() {
    let mut builder = CircuitBuilder::new("deterministic_test");
    let q = builder.add_quantum_register("q", 3).unwrap();
    let c = builder.add_classical_register("c", 3).unwrap();

    builder.h(q[0]).unwrap();
    builder.h(q[1]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.rx(q[2], Angle::from_radians(1.2345)).unwrap();
    builder.cz(q[1], q[2]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    builder.measure(q[1], c[1]).unwrap();
    builder.measure(q[2], c[2]).unwrap();

    let circuit = builder.finish().unwrap();

    let baseline = emit_openqasm(&circuit).unwrap();
    for _ in 0..50 {
        let run = emit_openqasm(&circuit).unwrap();
        assert_eq!(
            baseline, run,
            "Output must be deterministically identical across repeated runs"
        );
    }
}

#[test]
fn test_global_phase_policies() {
    let mut builder = CircuitBuilder::new("phase_test");
    let q = builder.add_quantum_register("q", 1).unwrap();
    builder.set_global_phase(Angle::from_radians(PI / 2.0));
    builder.x(q[0]).unwrap();
    let circuit = builder.finish().unwrap();

    // 1. Default (EmitGPhase): non-zero phase is emitted as gphase(...)
    let emitted = emit_openqasm(&circuit).unwrap();
    assert!(emitted.contains(&format!("gphase({});\n", PI / 2.0)));

    // 2. RejectNonZero: non-zero phase returns structured error
    let opts_reject = EmitterOptions {
        global_phase_policy: GlobalPhasePolicy::RejectNonZero,
        ..Default::default()
    };
    let emitter_reject = OpenQasmEmitter::with_options(opts_reject);
    match emitter_reject.emit(&circuit) {
        Err(OpenQasmError::NonZeroGlobalPhase { phase }) => {
            assert!(phase.contains(&format!("{}", PI / 2.0)));
        }
        res => panic!("Expected NonZeroGlobalPhase error, got: {res:?}"),
    }

    // 3. AlwaysEmit with zero phase: emits gphase(0)
    let mut builder_zero = CircuitBuilder::new("zero_phase");
    let q0 = builder_zero.add_quantum_register("q", 1).unwrap();
    builder_zero.h(q0[0]).unwrap();
    let circuit_zero = builder_zero.finish().unwrap();

    let opts_always = EmitterOptions {
        global_phase_policy: GlobalPhasePolicy::AlwaysEmit,
        ..Default::default()
    };
    let emitter_always = OpenQasmEmitter::with_options(opts_always);
    let emitted_always = emitter_always.emit(&circuit_zero).unwrap();
    assert!(emitted_always.contains("gphase(0);\n"));

    // 4. Default with zero phase: omits gphase statement
    let emitted_default_zero = emit_openqasm(&circuit_zero).unwrap();
    assert!(!emitted_default_zero.contains("gphase"));
}

#[test]
fn test_unsupported_operation_rejection() {
    let mut circuit = Circuit::new("unsupported_op");
    circuit.add_quantum_register("q", 1).unwrap();
    circuit.add_classical_register("c", 2).unwrap();

    circuit
        .add_operation(Operation::Classical {
            name: "add_bits".to_string(),
            reads: vec![ClassicalBitId(0)],
            writes: vec![ClassicalBitId(1)],
        })
        .unwrap();

    match emit_openqasm(&circuit) {
        Err(OpenQasmError::UnsupportedOperation { operation, .. }) => {
            assert_eq!(operation, "classical");
        }
        res => panic!("Expected UnsupportedOperation, got: {res:?}"),
    }
}

#[test]
fn test_unsupported_gate_rejection() {
    let mut circuit = Circuit::new("unsupported_gate");
    circuit.add_quantum_register("q", 1).unwrap();

    circuit
        .add_operation(Operation::custom_gate(
            "unknown_gate_xyz",
            vec![QubitId(0)],
            vec![],
        ))
        .unwrap();

    // Default: rejected
    match emit_openqasm(&circuit) {
        Err(OpenQasmError::UnsupportedGate { gate, .. }) => {
            assert_eq!(gate, "unknown_gate_xyz");
        }
        res => panic!("Expected UnsupportedGate, got: {res:?}"),
    }

    // With allow_custom_gates: emitted
    let opts = EmitterOptions {
        allow_custom_gates: true,
        ..Default::default()
    };
    let emitter = OpenQasmEmitter::with_options(opts);
    let emitted = emitter.emit(&circuit).unwrap();
    assert!(emitted.contains("unknown_gate_xyz q[0];"));
}

#[test]
fn test_gate_arity_and_parameter_mismatches() {
    let mut circuit = Circuit::new("gate_mismatches");
    circuit.add_quantum_register("q", 3).unwrap();

    // Standard OpenQASM 3 3-qubit gate `ccx` provided only 2 qubits
    circuit
        .add_operation(Operation::custom_gate(
            "ccx",
            vec![QubitId(0), QubitId(1)],
            vec![],
        ))
        .unwrap();

    match emit_openqasm(&circuit) {
        Err(OpenQasmError::GateArityMismatch {
            gate,
            expected,
            found,
        }) => {
            assert_eq!(gate, "ccx");
            assert_eq!(expected, 3);
            assert_eq!(found, 2);
        }
        res => panic!("Expected GateArityMismatch, got: {res:?}"),
    }
}

#[test]
fn test_non_finite_parameter_rejection() {
    let mut circuit = Circuit::new("nan_param");
    circuit.add_quantum_register("q", 1).unwrap();

    circuit
        .add_operation(Operation::custom_gate(
            "rx",
            vec![QubitId(0)],
            vec![ParameterExpr::constant(f64::NAN)],
        ))
        .unwrap();

    match emit_openqasm(&circuit) {
        Err(OpenQasmError::NonFiniteParameter { value }) => {
            assert!(value.is_nan());
        }
        res => panic!("Expected NonFiniteParameter, got: {res:?}"),
    }
}

#[test]
fn test_invalid_identifier_rejection() {
    let mut circuit = Circuit::new("bad_ident");
    // Register name with space
    let _ = circuit.add_quantum_register("my qubit", 1);

    // If a circuit has an invalid identifier in register name
    let mut circuit_raw = Circuit::new("raw_bad");
    circuit_raw.quantum_registers.push(qcc::QuantumRegister {
        name: "0qubit".to_string(), // starts with digit
        size: 1,
        start_qubit: 0,
    });
    circuit_raw
        .add_operation(Operation::gate(&qcc::Gate::X, vec![QubitId(0)]))
        .unwrap();

    match emit_openqasm(&circuit_raw) {
        Err(OpenQasmError::InvalidIdentifier { identifier }) => {
            assert_eq!(identifier, "0qubit");
        }
        res => panic!("Expected InvalidIdentifier, got: {res:?}"),
    }
}

#[test]
fn test_empty_circuit_rejected() {
    let circuit = Circuit::new("empty");
    match emit_openqasm(&circuit) {
        Err(OpenQasmError::EmptyCircuit) => {}
        res => panic!("Expected EmptyCircuit, got: {res:?}"),
    }
}

/// Independent parser validation test.
///
/// If an external OpenQASM 3 parser (such as Python `openqasm3` or `qiskit`) is available
/// in the runtime environment, this test parses the emitted program using that independent parser.
/// If unavailable, it clearly and candidly logs the environment limitation without faking validation.
#[test]
fn test_independent_parser_validation() {
    let mut builder = CircuitBuilder::new("bell");
    let q = builder.add_quantum_register("q", 2).unwrap();
    let c = builder.add_classical_register("c", 2).unwrap();

    builder.h(q[0]).unwrap();
    builder.cx(q[0], q[1]).unwrap();
    builder.measure(q[0], c[0]).unwrap();
    builder.measure(q[1], c[1]).unwrap();

    let circuit = builder.finish().unwrap();
    let emitted_qasm = emit_openqasm(&circuit).unwrap();

    // Check if python3 with openqasm3 package is available
    let py_check = Command::new("python3")
        .arg("-c")
        .arg("import sys; sys.path.insert(0, '/tmp/py_packages'); import openqasm3; print(openqasm3.__version__)")
        .output();

    match py_check {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
            println!("Independent OpenQASM 3 parser detected: openqasm3 v{version}");

            // Validate emitted Bell circuit with independent openqasm3.parse()
            let mut child = Command::new("python3")
                .arg("-c")
                .arg(
                    r#"
import sys
sys.path.insert(0, '/tmp/py_packages')
import openqasm3

ast = openqasm3.parse(sys.stdin.read())
assert ast.version == '3.0'
assert len(ast.statements) == 7
print('VALID')
"#,
                )
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
            panic!(
                "CRITICAL: Independent OpenQASM 3 parser ('openqasm3[parser]==1.0.1') is required for acceptance validation, but was not found.\n\
                 Ensure the parser is installed via: pip install 'openqasm3[parser]==1.0.1'"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Property Tests
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_arbitrary_rotations_emit_valid_qasm(
        theta in -2.0 * PI..2.0 * PI,
        phi in -2.0 * PI..2.0 * PI,
    ) {
        let mut builder = CircuitBuilder::new("prop_rotations");
        let q = builder.add_quantum_register("q", 2).unwrap();

        builder.rx(q[0], Angle::from_radians(theta)).unwrap();
        builder.ry(q[1], Angle::from_radians(phi)).unwrap();
        builder.cx(q[0], q[1]).unwrap();

        let circuit = builder.finish().unwrap();
        let emitted = emit_openqasm(&circuit).unwrap();

        prop_assert!(emitted.starts_with("OPENQASM 3.0;\n"));
        prop_assert!(emitted.contains("include \"stdgates.inc\";\n"));
        prop_assert!(emitted.contains("qubit[2] q;\n"));
        prop_assert!(emitted.contains("cx q[0], q[1];\n"));
        prop_assert!(emitted.ends_with('\n'));
    }

    #[test]
    fn prop_deterministic_emission_on_random_angles(
        theta in -10.0..10.0,
    ) {
        let mut builder = CircuitBuilder::new("prop_determ");
        let q = builder.add_quantum_register("q", 1).unwrap();
        builder.rz(q[0], Angle::from_radians(theta)).unwrap();
        let circuit = builder.finish().unwrap();

        let emit1 = emit_openqasm(&circuit).unwrap();
        let emit2 = emit_openqasm(&circuit).unwrap();
        prop_assert_eq!(emit1, emit2);
    }
}
