//! Integration tests for Phase 26: QASM Import/Export Round-Trip.
//!
//! Validates that:
//! - Importing supported OpenQASM 3, lowering it to QccIR, and exporting it back
//!   to OpenQASM preserves supported quantum and classical semantics.
//! - Independent OpenQASM 3 parser (`openqasm3[parser]==1.0.1`) validates all emitted text.
//! - Golden fixture round trips preserve structure, parameters, global phase, and wire dependencies.
//! - Double round-trip re-emission is strictly idempotent ($S_1 == S_2$).
//! - Formatting variations (whitespace, comments, multi-line expressions, arrow vs assignment)
//!   are normalized without affecting semantic equivalence.
//! - Unitary equivalence holds for pure unitary circuits and behavioral sampling matches
//!   for non-unitary measurement circuits.

use std::collections::HashMap;
use std::f64::consts::PI;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use qcc::backend::openqasm::{
    emit_openqasm, EmitterOptions, GlobalPhasePolicy, MeasurementSyntax, OpenQasmEmitter,
};
use qcc::frontend::lowering::lower_qasm_str;
use qcc::ir::{Circuit, Operation};
use qcc::quantum::ClassicalBitOrder;
use qcc::simulator::{execute_circuit, sample_circuit, SeededRng, SplitMix64};
use qcc::verification::{check_equivalence, shortest_angle_difference};

const TOLERANCE: f64 = 1e-10;

// ============================================================================
// Helper Functions
// ============================================================================

/// Lowers an OpenQASM 3 source string into a validated [`Circuit`].
fn lower_strict(source: &str, bindings: Option<HashMap<String, f64>>) -> Circuit {
    let res = lower_qasm_str(source, None, bindings);
    assert!(
        res.is_ok(),
        "Lowering failed unexpectedly: {:?}",
        res.diagnostics
    );
    let circuit = res.output.expect("Circuit must be present on success");
    assert!(
        circuit.validate().is_ok(),
        "Lowered circuit must pass validation"
    );
    circuit
}

/// Emits canonical OpenQASM 3 source from a [`Circuit`].
fn emit_strict(circuit: &Circuit) -> String {
    emit_openqasm(circuit).expect("Emitting OpenQASM must succeed")
}

/// Emits OpenQASM 3 source with specific [`EmitterOptions`].
fn emit_with_options_strict(circuit: &Circuit, options: EmitterOptions) -> String {
    OpenQasmEmitter::with_options(options)
        .emit(circuit)
        .expect("Emitting OpenQASM with options must succeed")
}

/// Validates emitted QASM text using the independent Python `openqasm3` reference parser.
///
/// Returns `Some(statement_count)` if Python `openqasm3` is available and succeeds;
/// returns `None` if Python or the parser is not present in the environment.
fn validate_with_independent_parser(qasm: &str) -> Option<usize> {
    let py_check = Command::new("python3")
        .arg("-c")
        .arg("import sys; sys.path.insert(0, '/tmp/py_packages'); import openqasm3; print(openqasm3.__version__)")
        .output();

    match py_check {
        Ok(out) if out.status.success() => {
            let mut child = Command::new("python3")
                .arg("-c")
                .arg(
                    r#"
import sys
sys.path.insert(0, '/tmp/py_packages')
import openqasm3

src = sys.stdin.read()
ast = openqasm3.parse(src)
assert ast.version == '3.0', f'Expected version 3.0, got {ast.version}'
print(len(ast.statements))
"#,
                )
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to spawn python3 parser");

            child
                .stdin
                .as_mut()
                .expect("Failed to open stdin")
                .write_all(qasm.as_bytes())
                .expect("Failed to write to stdin");

            let res = child.wait_with_output().expect("Failed to wait on child");
            assert!(
                res.status.success(),
                "Independent openqasm3 parser rejected emitted QASM:\n---\n{qasm}\n---\nError: {}",
                String::from_utf8_lossy(&res.stderr)
            );
            let count_str = String::from_utf8_lossy(&res.stdout).trim().to_string();
            let count = count_str.parse::<usize>().unwrap_or(0);
            Some(count)
        }
        _ => None,
    }
}

/// Asserts semantic equality between two QccIR circuits.
///
/// Checks:
/// - Qubit and classical bit counts
/// - Quantum and classical register tables (names, sizes, offsets)
/// - Operation sequence in deterministic topological order
/// - Gate types, qubit operands, and parameter values
/// - Measurement, reset, and barrier semantics
/// - Circuit global phase
/// - DAG wire dependency edges
fn assert_circuits_semantically_equal(c1: &Circuit, c2: &Circuit) {
    // 1. System dimensions
    assert_eq!(
        c1.num_qubits(),
        c2.num_qubits(),
        "Qubit count mismatch between circuits"
    );
    assert_eq!(
        c1.num_classical_bits(),
        c2.num_classical_bits(),
        "Classical bit count mismatch between circuits"
    );

    // 2. Registers
    assert_eq!(
        c1.quantum_registers.len(),
        c2.quantum_registers.len(),
        "Quantum register count mismatch"
    );
    for (r1, r2) in c1.quantum_registers.iter().zip(&c2.quantum_registers) {
        assert_eq!(r1.name, r2.name, "Quantum register name mismatch");
        assert_eq!(r1.size, r2.size, "Quantum register size mismatch");
        assert_eq!(
            r1.start_qubit, r2.start_qubit,
            "Quantum register offset mismatch"
        );
    }

    assert_eq!(
        c1.classical_registers.len(),
        c2.classical_registers.len(),
        "Classical register count mismatch"
    );
    for (r1, r2) in c1.classical_registers.iter().zip(&c2.classical_registers) {
        assert_eq!(r1.name, r2.name, "Classical register name mismatch");
        assert_eq!(r1.size, r2.size, "Classical register size mismatch");
        assert_eq!(
            r1.start_bit, r2.start_bit,
            "Classical register offset mismatch"
        );
    }

    // 3. Node count and operations
    let nodes1 = c1
        .graph
        .deterministic_topological_order()
        .expect("Toposort c1");
    let nodes2 = c2
        .graph
        .deterministic_topological_order()
        .expect("Toposort c2");
    assert_eq!(
        nodes1.len(),
        nodes2.len(),
        "Operation count mismatch between circuits"
    );

    for (i, (&n1, &n2)) in nodes1.iter().zip(&nodes2).enumerate() {
        let op1 = c1.graph.get_operation(n1).expect("op1 present");
        let op2 = c2.graph.get_operation(n2).expect("op2 present");

        match (op1, op2) {
            (
                Operation::Gate {
                    gate_name: g1,
                    qubits: q1,
                    parameters: p1,
                },
                Operation::Gate {
                    gate_name: g2,
                    qubits: q2,
                    parameters: p2,
                },
            ) => {
                assert_eq!(
                    g1.to_lowercase(),
                    g2.to_lowercase(),
                    "Gate name mismatch at step {i}"
                );
                assert_eq!(q1, q2, "Gate qubit operand mismatch at step {i}");
                assert_eq!(
                    p1.len(),
                    p2.len(),
                    "Gate parameter count mismatch at step {i}"
                );
                let empty_env = HashMap::new();
                for (param_idx, (p_a, p_b)) in p1.iter().zip(p2).enumerate() {
                    let val_a = p_a.eval(&empty_env).expect("Evaluate param a");
                    let val_b = p_b.eval(&empty_env).expect("Evaluate param b");
                    assert!(
                        (val_a - val_b).abs() < 1e-12,
                        "Parameter {param_idx} mismatch at step {i}: {val_a} vs {val_b}"
                    );
                }
            }
            (
                Operation::Measure {
                    qubit: q1,
                    destination: d1,
                },
                Operation::Measure {
                    qubit: q2,
                    destination: d2,
                },
            ) => {
                assert_eq!(q1, q2, "Measurement source qubit mismatch at step {i}");
                assert_eq!(d1, d2, "Measurement destination bit mismatch at step {i}");
            }
            (Operation::Reset { qubit: q1 }, Operation::Reset { qubit: q2 }) => {
                assert_eq!(q1, q2, "Reset qubit mismatch at step {i}");
            }
            (Operation::Barrier { qubits: q1 }, Operation::Barrier { qubits: q2 }) => {
                assert_eq!(q1, q2, "Barrier qubit list mismatch at step {i}");
            }
            _ => panic!("Operation variant mismatch at step {i}: {op1:?} vs {op2:?}"),
        }
    }

    // 4. Global phase
    let empty_env = HashMap::new();
    let phase1 = c1.global_phase.eval(&empty_env).expect("eval phase 1");
    let phase2 = c2.global_phase.eval(&empty_env).expect("eval phase 2");
    assert!(
        shortest_angle_difference(phase1, phase2).abs() < 1e-12,
        "Global phase mismatch: {phase1} vs {phase2}"
    );

    // 5. Wire dependencies
    let edges1 = c1.graph.all_edges();
    let edges2 = c2.graph.all_edges();
    assert_eq!(
        edges1.len(),
        edges2.len(),
        "Dependency edge count mismatch between circuits"
    );
    for (i, (e1, e2)) in edges1.iter().zip(&edges2).enumerate() {
        assert_eq!(
            e1.2, e2.2,
            "Dependency edge {i} metadata mismatch: {:?} vs {:?}",
            e1.2, e2.2
        );
    }
}

/// Executes a full round-trip test on a given QASM source:
/// $S_0 \xrightarrow{\text{lower}} C_1 \xrightarrow{\text{emit}} S_1 \xrightarrow{\text{validate}} \text{openqasm3} \xrightarrow{\text{lower}} C_2 \xrightarrow{\text{emit}} S_2$.
///
/// Asserts:
/// 1. $C_1$ and $C_2$ are semantically equal.
/// 2. $S_1 == S_2$ (double round-trip re-emission is strictly idempotent).
/// 3. Independent reference parser accepts $S_1$.
fn execute_roundtrip_test(
    source: &str,
    bindings: Option<HashMap<String, f64>>,
) -> (Circuit, Circuit, String) {
    // 1. Lower original QASM into initial circuit C1
    let c1 = lower_strict(source, bindings.clone());

    // 2. Emit QASM S1 from C1
    let s1 = emit_strict(&c1);

    // 3. Independent parser validation of emitted QASM S1
    validate_with_independent_parser(&s1);

    // 4. Re-lower emitted QASM S1 into C2
    let c2 = lower_strict(&s1, bindings);

    // 5. Verify semantic equality between C1 and C2
    assert_circuits_semantically_equal(&c1, &c2);

    // 6. Double round-trip re-emission: emit S2 from C2, assert idempotency
    let s2 = emit_strict(&c2);
    assert_eq!(
        s1, s2,
        "Double round-trip re-emission must be strictly idempotent"
    );

    (c1, c2, s1)
}

// ============================================================================
// 1. Golden Fixture Round Trips
// ============================================================================

#[test]
fn test_golden_fixture_bell_roundtrip() {
    let fixture_path = Path::new("tests/fixtures/qasm3/bell.qasm");
    let source = fs::read_to_string(fixture_path).expect("Read bell.qasm");

    let (c1, c2, emitted) = execute_roundtrip_test(&source, None);

    // Behavioral sampling equivalence on Bell state
    let num_shots = 2000;
    let (res1, _) = sample_circuit(&c1, num_shots, SplitMix64::new(42)).expect("Sample c1");
    let (res2, _) = sample_circuit(&c2, num_shots, SplitMix64::new(42)).expect("Sample c2");
    assert_eq!(
        res1.counts(ClassicalBitOrder::MsbFirst),
        res2.counts(ClassicalBitOrder::MsbFirst)
    );

    let counts = res1.counts(ClassicalBitOrder::MsbFirst);
    assert_eq!(counts.get("01").copied().unwrap_or(0), 0);
    assert_eq!(counts.get("10").copied().unwrap_or(0), 0);
    assert_eq!(
        counts.get("00").copied().unwrap_or(0) + counts.get("11").copied().unwrap_or(0),
        num_shots
    );

    // Emitted source contains standard elements
    assert!(emitted.contains("OPENQASM 3.0;"));
    assert!(emitted.contains("qubit[2] q;"));
    assert!(emitted.contains("bit[2] c;"));
    assert!(emitted.contains("h q[0];"));
    assert!(emitted.contains("cx q[0], q[1];"));
}

#[test]
fn test_golden_fixture_rotations_roundtrip() {
    let fixture_path = Path::new("tests/fixtures/qasm3/rotations.qasm");
    let source = fs::read_to_string(fixture_path).expect("Read rotations.qasm");

    let (c1, c2, _) = execute_roundtrip_test(&source, None);

    // Pure unitary circuit: verify unitary equivalence
    let verdict = check_equivalence(&c1, &c2).expect("Unitary equivalence check");
    assert!(
        verdict.is_equivalent,
        "Rotation circuits must be unitarily equivalent: {:?}",
        verdict
    );
    assert!(verdict.max_amplitude_difference < TOLERANCE);
}

#[test]
fn test_golden_fixture_non_unitary_roundtrip() {
    let fixture_path = Path::new("tests/fixtures/qasm3/non_unitary.qasm");
    let source = fs::read_to_string(fixture_path).expect("Read non_unitary.qasm");

    let (c1, c2, emitted) = execute_roundtrip_test(&source, None);

    // Verify non-unitary elements preserved
    assert!(emitted.contains("reset q[1];"));
    assert!(emitted.contains("barrier q[0], q[1];"));
    assert!(emitted.contains("c[0] = measure q[0];"));
    assert!(emitted.contains("c[1] = measure q[1];"));

    // Both circuits consistently reject terminal-only statevector execution due to active reset
    let err1 = execute_circuit(&c1).unwrap_err();
    let err2 = execute_circuit(&c2).unwrap_err();
    match (err1, err2) {
        (
            qcc::simulator::ExecutionError::UnsupportedOperation { operation: o1, .. },
            qcc::simulator::ExecutionError::UnsupportedOperation { operation: o2, .. },
        ) => {
            assert_eq!(o1, "reset");
            assert_eq!(o2, "reset");
        }
        other => panic!("Expected UnsupportedOperation for reset, got: {other:?}"),
    }
}

#[test]
fn test_golden_fixture_multi_register_roundtrip() {
    let fixture_path = Path::new("tests/fixtures/qasm3/multi_register.qasm");
    let source = fs::read_to_string(fixture_path).expect("Read multi_register.qasm");

    let (c1, c2, emitted) = execute_roundtrip_test(&source, None);

    // Multi-register structure preserved
    assert_eq!(c1.quantum_registers.len(), 2);
    assert_eq!(c2.quantum_registers.len(), 2);
    assert_eq!(c1.classical_registers.len(), 2);
    assert_eq!(c2.classical_registers.len(), 2);

    assert!(emitted.contains("qubit[2] q;"));
    assert!(emitted.contains("qubit[3] r;"));
    assert!(emitted.contains("bit[1] c;"));
    assert!(emitted.contains("bit[2] d;"));
    assert!(emitted.contains("cx q[1], r[2];"));

    // Whole-register barrier q, r expanded to individual qubits in canonical emitted QASM
    assert!(emitted.contains("barrier q[0], q[1], r[0], r[1], r[2];"));
}

#[test]
fn test_golden_fixture_global_phase_roundtrip() {
    let fixture_path = Path::new("tests/fixtures/qasm3/global_phase.qasm");
    let source = fs::read_to_string(fixture_path).expect("Read global_phase.qasm");

    let (c1, c2, emitted) = execute_roundtrip_test(&source, None);

    // Global phase: pi/4 + pi/4 = pi/2
    let empty_env = HashMap::new();
    let p1 = c1.global_phase.eval(&empty_env).unwrap();
    let p2 = c2.global_phase.eval(&empty_env).unwrap();
    assert!((p1 - PI / 2.0).abs() < 1e-12);
    assert!((p2 - PI / 2.0).abs() < 1e-12);

    // Emitted source contains canonical gphase
    assert!(emitted.contains("gphase("));

    // Unitary equivalence check
    let verdict = check_equivalence(&c1, &c2).expect("Unitary equivalence check");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < TOLERANCE);
}

#[test]
fn test_golden_fixture_teleportation_roundtrip() {
    let fixture_path = Path::new("tests/fixtures/qasm3/teleportation.qasm");
    let source = fs::read_to_string(fixture_path).expect("Read teleportation.qasm");

    let (c1, c2, emitted) = execute_roundtrip_test(&source, None);

    assert_eq!(c1.num_qubits(), 3);
    assert_eq!(c1.num_classical_bits(), 2);
    assert!(emitted.contains("barrier q[0], q[1], q[2];"));
    assert!(emitted.contains("c[0] = measure q[0];"));
    assert!(emitted.contains("c[1] = measure q[1];"));

    // Behavioral sampling equivalence
    let (res1, _) = sample_circuit(&c1, 1000, SplitMix64::new(777)).expect("Sample c1");
    let (res2, _) = sample_circuit(&c2, 1000, SplitMix64::new(777)).expect("Sample c2");
    assert_eq!(
        res1.counts(ClassicalBitOrder::MsbFirst),
        res2.counts(ClassicalBitOrder::MsbFirst)
    );
}

// ============================================================================
// 2. Syntax Varieties & Formatting Insensitivity
// ============================================================================

#[test]
fn test_measurement_syntax_variants_roundtrip() {
    // Arrow measurement syntax in source
    let arrow_src = r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;
h q[0];
measure q[0] -> c[0];
measure q[1] -> c[1];
"#;

    let c_arrow = lower_strict(arrow_src, None);

    // Default emission normalizes to assignment syntax
    let emitted_assign = emit_strict(&c_arrow);
    assert!(emitted_assign.contains("c[0] = measure q[0];"));
    assert!(emitted_assign.contains("c[1] = measure q[1];"));

    let c_from_assign = lower_strict(&emitted_assign, None);
    assert_circuits_semantically_equal(&c_arrow, &c_from_assign);

    // Arrow emission preserves arrow syntax when requested
    let arrow_options = EmitterOptions {
        measurement_syntax: MeasurementSyntax::Arrow,
        ..Default::default()
    };
    let emitted_arrow = emit_with_options_strict(&c_arrow, arrow_options);
    assert!(emitted_arrow.contains("measure q[0] -> c[0];"));
    assert!(emitted_arrow.contains("measure q[1] -> c[1];"));

    let c_from_arrow = lower_strict(&emitted_arrow, None);
    assert_circuits_semantically_equal(&c_arrow, &c_from_arrow);
}

#[test]
fn test_whole_register_measurement_roundtrip() {
    let src = r#"OPENQASM 3.0;
qubit[3] q;
bit[3] c;
h q[0];
c = measure q;
"#;

    let c1 = lower_strict(src, None);
    assert_eq!(c1.graph.node_count(), 4); // 1 H + 3 Measures

    let emitted = emit_strict(&c1);
    validate_with_independent_parser(&emitted);

    // Emitted QASM expands to individual scalar measurements
    assert!(emitted.contains("c[0] = measure q[0];"));
    assert!(emitted.contains("c[1] = measure q[1];"));
    assert!(emitted.contains("c[2] = measure q[2];"));

    let c2 = lower_strict(&emitted, None);
    assert_circuits_semantically_equal(&c1, &c2);
}

#[test]
fn test_formatting_insensitivity_roundtrip() {
    // Highly irregular formatting with comments, multi-line expressions, and extra spaces
    let messy_src = r#"
    // Header with comments
    OPENQASM    3.0   ;
    // Include directive
    include "stdgates.inc"  ;

    qubit   [  2  ]    q   ;
    bit [ 2 ]    c ;

    // Multi-line gate with arithmetic
    rx (
        pi  /
        4
    )    q [ 0 ] ;

    // Semicolon spacing and indentation
    h     q[ 1 ]   ;
    cx   q[ 0 ] ,  q[ 1 ] ;

    // Arrow measurement with comments
    measure   q[ 0 ]   ->   c[ 0 ] ; // comment here
    c[ 1 ]   =   measure   q[ 1 ] ;
"#;

    let (c1, c2, clean_qasm) = execute_roundtrip_test(messy_src, None);

    // Emitted QASM is clean and normalized
    assert_eq!(clean_qasm.lines().next().unwrap(), "OPENQASM 3.0;");
    assert!(!clean_qasm.contains("//"));

    // Behavioral sampling matches
    let (res1, _) = sample_circuit(&c1, 1000, SplitMix64::new(999)).expect("Sample c1");
    let (res2, _) = sample_circuit(&c2, 1000, SplitMix64::new(999)).expect("Sample c2");
    assert_eq!(
        res1.counts(ClassicalBitOrder::MsbFirst),
        res2.counts(ClassicalBitOrder::MsbFirst)
    );
}

// ============================================================================
// 3. Parameters, Global Phase, and Constant Folding
// ============================================================================

#[test]
fn test_parameter_expressions_and_constant_folding_roundtrip() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
rx(pi / 4) q[0];
ry(3 * pi / 8) q[1];
rz(-pi / 2 + 0.1) q[0];
rx(2 * pi) q[1];
"#;

    let (c1, c2, emitted) = execute_roundtrip_test(src, None);
    validate_with_independent_parser(&emitted);

    // Unitary equivalence check
    let verdict = check_equivalence(&c1, &c2).expect("Unitary equivalence");
    assert!(verdict.is_equivalent);
    assert!(verdict.max_amplitude_difference < TOLERANCE);
}

#[test]
fn test_global_phase_policies_roundtrip() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
gphase(0.7853981633974483);
h q[0];
"#;

    let c = lower_strict(src, None);

    // 1. EmitGPhase (default)
    let qasm_emit = emit_strict(&c);
    assert!(qasm_emit.contains("gphase(0.7853981633974483);"));
    let c_re = lower_strict(&qasm_emit, None);
    assert_circuits_semantically_equal(&c, &c_re);

    // 2. AlwaysEmit
    let always_opts = EmitterOptions {
        global_phase_policy: GlobalPhasePolicy::AlwaysEmit,
        ..Default::default()
    };
    let qasm_always = emit_with_options_strict(&c, always_opts);
    assert!(qasm_always.contains("gphase("));
    let c_always = lower_strict(&qasm_always, None);
    assert_circuits_semantically_equal(&c, &c_always);

    // 3. Zero phase emission behavior
    let zero_phase_src = "OPENQASM 3.0;\nqubit[1] q;\nh q[0];\n";
    let c_zero = lower_strict(zero_phase_src, None);
    let qasm_zero = emit_strict(&c_zero);
    assert!(!qasm_zero.contains("gphase(")); // Zero phase omitted by default
}

// ============================================================================
// 4. All Canonical Supported Gates Round-Trip
// ============================================================================

#[test]
fn test_all_canonical_gates_roundtrip() {
    let src = r#"OPENQASM 3.0;
qubit[3] q;
x q[0];
y q[1];
z q[2];
h q[0];
s q[1];
sdg q[2];
t q[0];
tdg q[1];
rx(0.5) q[2];
ry(1.0) q[0];
rz(1.5) q[1];
cx q[0], q[1];
cz q[1], q[2];
swap q[0], q[2];
barrier q[0], q[1], q[2];
"#;

    let (c1, c2, emitted) = execute_roundtrip_test(src, None);
    validate_with_independent_parser(&emitted);

    // Unitary equivalence check across all canonical gates
    let verdict = check_equivalence(&c1, &c2).expect("Unitary equivalence");
    assert!(
        verdict.is_equivalent,
        "Circuit containing all canonical gates must be unitarily equivalent: {:?}",
        verdict
    );
    assert!(verdict.max_amplitude_difference < TOLERANCE);

    // Statevector amplitude preservation
    let (s1, _) = execute_circuit(&c1).expect("Execute c1");
    let (s2, _) = execute_circuit(&c2).expect("Execute c2");
    assert_eq!(s1.amplitudes(), s2.amplitudes());
}

// ============================================================================
// 5. Property / Randomized Supported-Circuit Round Trips
// ============================================================================

#[test]
fn test_randomized_circuit_roundtrips() {
    let mut rng = SplitMix64::new(0xDEAD_BEEF_CAFE);

    // Generate 10 distinct pseudo-random circuits
    for seed in 0..10 {
        let num_qubits = (rng.next_u64() % 3 + 2) as usize; // 2 to 4 qubits
        let num_gates = (rng.next_u64() % 10 + 5) as usize; // 5 to 14 gates

        let mut lines = Vec::new();
        lines.push("OPENQASM 3.0;".to_string());
        lines.push("include \"stdgates.inc\";".to_string());
        lines.push(format!("qubit[{num_qubits}] q;"));
        lines.push(format!("bit[{num_qubits}] c;"));

        for _ in 0..num_gates {
            let op_choice = rng.next_u64() % 12;
            let q0 = (rng.next_u64() % num_qubits as u64) as usize;
            let mut q1 = (rng.next_u64() % num_qubits as u64) as usize;
            if q1 == q0 {
                q1 = (q0 + 1) % num_qubits;
            }

            match op_choice {
                0 => lines.push(format!("h q[{q0}];")),
                1 => lines.push(format!("x q[{q0}];")),
                2 => lines.push(format!("y q[{q0}];")),
                3 => lines.push(format!("z q[{q0}];")),
                4 => lines.push(format!("s q[{q0}];")),
                5 => lines.push(format!("t q[{q0}];")),
                6 => lines.push(format!("rx(0.25) q[{q0}];")),
                7 => lines.push(format!("ry(0.5) q[{q0}];")),
                8 => lines.push(format!("rz(0.75) q[{q0}];")),
                9 => lines.push(format!("cx q[{q0}], q[{q1}];")),
                10 => lines.push(format!("cz q[{q0}], q[{q1}];")),
                11 => lines.push(format!("swap q[{q0}], q[{q1}];")),
                _ => unreachable!(),
            }
        }

        // Add barrier and terminal measurements
        lines.push("barrier;".to_string());
        for q in 0..num_qubits {
            lines.push(format!("c[{q}] = measure q[{q}];"));
        }

        let qasm_source = lines.join("\n");
        let (c1, c2, emitted) = execute_roundtrip_test(&qasm_source, None);

        // Verify independent parser on emitted QASM
        validate_with_independent_parser(&emitted);

        // Behavioral sampling equivalence
        let (res1, _) =
            sample_circuit(&c1, 500, SplitMix64::new(seed as u64 + 1)).expect("Sample c1");
        let (res2, _) =
            sample_circuit(&c2, 500, SplitMix64::new(seed as u64 + 1)).expect("Sample c2");
        assert_eq!(
            res1.counts(ClassicalBitOrder::MsbFirst),
            res2.counts(ClassicalBitOrder::MsbFirst),
            "Random circuit {seed} sampling mismatch"
        );
    }
}
