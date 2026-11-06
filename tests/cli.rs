//! Integration tests for QCC CLI (`qcc compile` and `qcc inspect`).
//!
//! Validates:
//! - Stable exit codes:
//!   - `0`: Success
//!   - `1`: Operational error (missing file, bad flags, I/O)
//!   - `2`: Compilation/syntax/semantic error
//!   - `3`: Unsupported OpenQASM construct
//! - `qcc compile` output emission, options (format, global phase, measure syntax, bindings), and file writing.
//! - `qcc inspect` view modes (summary, AST, IR, registers).
//! - Source-located diagnostics with exact line and column rendering.

use std::fs;
use std::path::Path;
use std::process::Command;

const QCC_BIN: &str = env!("CARGO_BIN_EXE_qcc");

// ============================================================================
// 1. Compile Subcommand Tests
// ============================================================================

#[test]
fn test_cli_compile_bell_stdout() {
    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("tests/fixtures/qasm3/bell.qasm")
        .output()
        .expect("Execute qcc compile");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("OPENQASM 3.0;"));
    assert!(stdout.contains("qubit[2] q;"));
    assert!(stdout.contains("bit[2] c;"));
    assert!(stdout.contains("h q[0];"));
    assert!(stdout.contains("cx q[0], q[1];"));
    assert!(stdout.contains("c[0] = measure q[0];"));
}

#[test]
fn test_cli_compile_to_file() {
    let temp_out = "target/test_compile_out.qasm";
    let _ = fs::remove_file(temp_out);

    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("tests/fixtures/qasm3/bell.qasm")
        .arg("-o")
        .arg(temp_out)
        .output()
        .expect("Execute qcc compile -o");

    assert_eq!(output.status.code(), Some(0));
    assert!(Path::new(temp_out).exists());
    let content = fs::read_to_string(temp_out).expect("Read output file");
    assert!(content.contains("OPENQASM 3.0;"));
    assert!(content.contains("cx q[0], q[1];"));
    let _ = fs::remove_file(temp_out);
}

#[test]
fn test_cli_compile_qccir_format() {
    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("--format")
        .arg("qccir")
        .arg("tests/fixtures/qasm3/bell.qasm")
        .output()
        .expect("Execute qcc compile --format qccir");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Circuit"));
    assert!(stdout.contains("quantum_registers"));
    assert!(stdout.contains("graph"));
}

#[test]
fn test_cli_compile_arrow_measurement_syntax() {
    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("--measure-syntax")
        .arg("arrow")
        .arg("tests/fixtures/qasm3/bell.qasm")
        .output()
        .expect("Execute qcc compile --measure-syntax arrow");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("measure q[0] -> c[0];"));
    assert!(stdout.contains("measure q[1] -> c[1];"));
}

#[test]
fn test_cli_compile_with_parameter_binding() {
    // Parameterized rotation fixture with symbolic bindings
    let src = "OPENQASM 3.0;\nqubit[1] q;\nrx(theta) q[0];\n";
    let temp_in = "target/test_bind.qasm";
    fs::write(temp_in, src).expect("Write temp_in");

    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("-b")
        .arg("theta=0.7853981633974483")
        .arg(temp_in)
        .output()
        .expect("Execute qcc compile with binding");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rx(0.7853981633974483) q[0];"));

    let _ = fs::remove_file(temp_in);
}

#[test]
fn test_cli_compile_stdin() {
    let mut child = Command::new(QCC_BIN)
        .arg("compile")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Spawn qcc compile -");

    let input = b"OPENQASM 3.0;\nqubit[1] q;\nx q[0];\n";
    use std::io::Write;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input).unwrap();
        // stdin is dropped here, closing the pipe so child sees EOF
    }
    let output = child.wait_with_output().unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout: {stdout}, stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("x q[0];"));
}

#[test]
fn test_cli_compile_missing_file_fails_with_code_1() {
    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("nonexistent_path/nonexistent.qasm")
        .output()
        .expect("Execute qcc compile nonexistent");

    assert_eq!(output.status.code(), Some(1)); // CliError = 1
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to read input file"));
}

#[test]
fn test_cli_compile_syntax_error_fails_with_code_2() {
    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("tests/qasm3/conformance/fixtures/invalid/invalid_division_by_zero.qasm")
        .output()
        .expect("Execute qcc compile invalid");

    assert_eq!(output.status.code(), Some(2)); // CompileError = 2
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E0030]"));
    assert!(stderr.contains("division by zero"));
}

#[test]
fn test_cli_compile_unsupported_fails_with_code_3() {
    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("tests/qasm3/conformance/fixtures/unsupported/unsupported_if_else.qasm")
        .output()
        .expect("Execute qcc compile unsupported");

    assert_eq!(output.status.code(), Some(3)); // UnsupportedError = 3
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E0005]"));
}

// ============================================================================
// 2. Inspect Subcommand Tests
// ============================================================================

#[test]
fn test_cli_inspect_summary_default() {
    let output = Command::new(QCC_BIN)
        .arg("inspect")
        .arg("tests/fixtures/qasm3/bell.qasm")
        .output()
        .expect("Execute qcc inspect");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("QCC Circuit Inspection:"));
    assert!(stdout.contains("Total Qubits:         2"));
    assert!(stdout.contains("Total Classical Bits: 2"));
    assert!(stdout.contains("Declared Quantum Registers:"));
    assert!(stdout.contains("Declared Classical Registers:"));
    assert!(stdout.contains("Topological Operation Sequence:"));
    assert!(stdout.contains("h q[0]"));
    assert!(stdout.contains("cx q[0] q[1]"));
}

#[test]
fn test_cli_inspect_ast() {
    let output = Command::new(QCC_BIN)
        .arg("inspect")
        .arg("--ast")
        .arg("tests/fixtures/qasm3/bell.qasm")
        .output()
        .expect("Execute qcc inspect --ast");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("AstProgram"));
    assert!(stdout.contains("GateCall"));
}

#[test]
fn test_cli_inspect_ir() {
    let output = Command::new(QCC_BIN)
        .arg("inspect")
        .arg("--ir")
        .arg("tests/fixtures/qasm3/bell.qasm")
        .output()
        .expect("Execute qcc inspect --ir");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Circuit"));
    assert!(stdout.contains("graph"));
}

#[test]
fn test_cli_inspect_registers() {
    let output = Command::new(QCC_BIN)
        .arg("inspect")
        .arg("--registers")
        .arg("tests/fixtures/qasm3/multi_register.qasm")
        .output()
        .expect("Execute qcc inspect --registers");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Declared Registers"));
    assert!(stdout.contains("Quantum Registers:"));
    assert!(stdout.contains("q (size: 2, array, base qubit: 0)"));
    assert!(stdout.contains("r (size: 3, array, base qubit: 2)"));
    assert!(stdout.contains("Classical Registers:"));
    assert!(stdout.contains("c (size: 1, array, base bit: 0)"));
    assert!(stdout.contains("d (size: 2, array, base bit: 1)"));
}

#[test]
fn test_cli_inspect_invalid_file_fails_with_code_2() {
    let output = Command::new(QCC_BIN)
        .arg("inspect")
        .arg("tests/qasm3/conformance/fixtures/invalid/invalid_undeclared_reg.qasm")
        .output()
        .expect("Execute qcc inspect invalid");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E0012]"));
}

#[test]
fn test_cli_inspect_unsupported_fails_with_code_3() {
    let output = Command::new(QCC_BIN)
        .arg("inspect")
        .arg("tests/qasm3/conformance/fixtures/unsupported/unsupported_three_qubit_ccx.qasm")
        .output()
        .expect("Execute qcc inspect unsupported");

    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E0003]"));
}

// ============================================================================
// 3. Diagnostic Line and Column Rendering Tests
// ============================================================================

#[test]
fn test_cli_rendered_line_column_diagnostics() {
    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("tests/qasm3/conformance/fixtures/invalid/invalid_out_of_bounds.qasm")
        .output()
        .expect("Execute qcc compile out of bounds");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Formatted line:col span must be rendered
    assert!(
        stderr.contains("tests/qasm3/conformance/fixtures/invalid/invalid_out_of_bounds.qasm:3:"),
        "Diagnostic must contain filename and line number: {stderr}"
    );
    assert!(stderr.contains("error[E0014]"));
    assert!(stderr.contains("index 5 out of bounds"));
}

#[test]
fn test_cli_subcommand_help_flags() {
    let compile_help = Command::new(QCC_BIN)
        .arg("compile")
        .arg("--help")
        .output()
        .expect("Execute compile --help");
    assert_eq!(compile_help.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&compile_help.stdout);
    assert!(stdout.contains("Usage: qcc compile"));

    let inspect_help = Command::new(QCC_BIN)
        .arg("inspect")
        .arg("--help")
        .output()
        .expect("Execute inspect --help");
    assert_eq!(inspect_help.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&inspect_help.stdout);
    assert!(stdout.contains("Usage: qcc inspect"));
}

#[test]
fn test_cli_compile_invalid_register_codes_fail_with_code_2() {
    let cases = [
        (
            "tests/qasm3/conformance/fixtures/invalid/invalid_duplicate_reg.qasm",
            "E0010",
        ),
        (
            "tests/qasm3/conformance/fixtures/invalid/invalid_zero_sized_reg.qasm",
            "E0011",
        ),
        (
            "tests/qasm3/conformance/fixtures/invalid/invalid_undeclared_reg.qasm",
            "E0012",
        ),
        (
            "tests/qasm3/conformance/fixtures/invalid/invalid_type_mismatch.qasm",
            "E0013",
        ),
        (
            "tests/qasm3/conformance/fixtures/invalid/invalid_out_of_bounds.qasm",
            "E0014",
        ),
    ];

    for (fixture, code) in cases {
        let output = Command::new(QCC_BIN)
            .arg("compile")
            .arg(fixture)
            .output()
            .unwrap_or_else(|_| panic!("Execute qcc compile {fixture}"));

        assert_eq!(
            output.status.code(),
            Some(2),
            "Expected exit code 2 (CompileError) for {fixture}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(code),
            "Expected stderr to contain {code} for {fixture}, got:\n{stderr}"
        );
    }
}

#[test]
fn test_cli_compile_invalid_operation_codes_fail_with_code_2() {
    let cases = [
        (
            "tests/qasm3/conformance/fixtures/invalid/invalid_unknown_gate.qasm",
            "E0020",
        ),
        (
            "tests/qasm3/conformance/fixtures/invalid/invalid_param_arity.qasm",
            "E0021",
        ),
        (
            "tests/qasm3/conformance/fixtures/invalid/invalid_qubit_arity.qasm",
            "E0022",
        ),
        (
            "tests/qasm3/conformance/fixtures/invalid/invalid_duplicate_operands.qasm",
            "E0023",
        ),
        (
            "tests/qasm3/conformance/fixtures/invalid/invalid_measurement_size.qasm",
            "E0025",
        ),
    ];

    for (fixture, code) in cases {
        let output = Command::new(QCC_BIN)
            .arg("compile")
            .arg(fixture)
            .output()
            .unwrap_or_else(|_| panic!("Execute qcc compile {fixture}"));

        assert_eq!(
            output.status.code(),
            Some(2),
            "Expected exit code 2 (CompileError) for {fixture}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(code),
            "Expected stderr to contain {code} for {fixture}, got:\n{stderr}"
        );
    }
}

#[test]
fn test_cli_compile_unsupported_gate_p_fails_with_code_3() {
    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("tests/qasm3/conformance/fixtures/unsupported/unsupported_standard_p.qasm")
        .output()
        .expect("Execute qcc compile unsupported_standard_p");

    assert_eq!(
        output.status.code(),
        Some(3),
        "Expected exit code 3 (UnsupportedError) for unsupported gate 'p' (E0004)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E0004"));
    assert!(stderr.contains("without decomposition"));
}

#[test]
fn test_cli_compile_with_target_and_stats() {
    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("--target")
        .arg("linear")
        .arg("--stats")
        .arg("examples/routing.qasm")
        .output()
        .expect("Execute qcc compile --target linear --stats");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Stderr must contain the markdown statistics table
    assert!(stderr.contains("| | Before | After |"));
    assert!(stderr.contains("| Gates | 6 | 9 |"));
    assert!(stderr.contains("| Depth | 6 | 9 |"));
    assert!(stderr.contains("| Router SWAPs inserted | 0 | 1 |"));

    // Stdout must contain the routed OpenQASM 3 output in hardware-native basis
    assert!(stdout.contains("OPENQASM 3.0;"));
    assert!(stdout.contains("cx"));
    // Redundant standalone 'x' gates must have been eliminated by the optimizer
    assert!(!stdout.lines().any(|l| l.trim().starts_with("x ")));
}
