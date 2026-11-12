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

// ============================================================================
// 4. Compilation Manifest Tests (M7 Session 1)
// ============================================================================

#[test]
fn test_cli_compile_help_documents_emit_manifest() {
    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("--help")
        .output()
        .expect("Execute compile --help");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--emit-manifest <FILE>"));
}

#[test]
fn test_cli_compile_emit_manifest_missing_arg_fails_with_code_1() {
    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("examples/routing.qasm")
        .arg("--emit-manifest")
        .output()
        .expect("Execute compile --emit-manifest missing arg");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing argument for '--emit-manifest'"));
}

#[test]
fn test_cli_compile_emit_manifest_showcase_reconciles_emitted_qasm() {
    let qasm_path = "target/test_manifest_showcase.qasm";
    let manifest_path = "target/test_manifest_showcase.json";
    let _ = fs::remove_file(qasm_path);
    let _ = fs::remove_file(manifest_path);

    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("--target")
        .arg("linear")
        .arg("-o")
        .arg(qasm_path)
        .arg("--emit-manifest")
        .arg(manifest_path)
        .arg("examples/showcase.qasm")
        .output()
        .expect("Execute qcc compile on showcase.qasm with --emit-manifest");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(Path::new(qasm_path).exists());
    assert!(Path::new(manifest_path).exists());

    let emitted_qasm = fs::read_to_string(qasm_path).expect("Read emitted QASM");
    let manifest_raw = fs::read_to_string(manifest_path).expect("Read emitted manifest");
    let manifest =
        qcc::CompilationManifest::from_json(&manifest_raw).expect("Parse CompilationManifest JSON");

    assert_eq!(manifest.schema_id, qcc::MANIFEST_SCHEMA_ID);
    assert_eq!(manifest.schema_version, qcc::MANIFEST_SCHEMA_VERSION);
    assert_eq!(manifest.compiler.name, "qcc");
    assert_eq!(manifest.compiler.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(
        manifest.compilation_mode,
        qcc::ManifestCompilationMode::TargetCompiled
    );
    assert_eq!(manifest.logical_qubits, 4);
    assert_eq!(manifest.physical_qubits, Some(4));

    // Layout contract
    let layout = manifest.layout.as_ref().expect("Layout present");
    assert_eq!(layout.mapping_direction, "logical_to_physical");
    assert_eq!(layout.qubit_indexing_convention, "little_endian_q0_lsb");
    assert_eq!(layout.initial_logical_to_physical.len(), 4);
    assert_eq!(layout.final_logical_to_physical.len(), 4);
    assert!(layout.unused_physical_qubits_initial.is_empty());
    assert!(layout.unused_physical_qubits_final.is_empty());
    assert_ne!(
        layout.initial_logical_to_physical,
        layout.final_logical_to_physical
    );

    // Emitted circuit digest and path must match the actual QASM written to disk
    assert_eq!(
        manifest.emitted_circuit.format,
        qcc::ManifestOutputFormat::OpenQasm
    );
    assert_eq!(
        manifest.emitted_circuit.output_destination,
        qcc::ManifestOutputDestination::File
    );
    assert_eq!(manifest.emitted_circuit.path.as_deref(), Some(qasm_path));
    assert_eq!(
        manifest.emitted_circuit.sha256,
        qcc::sha256_hex(emitted_qasm.as_bytes())
    );
    assert_eq!(manifest.emitted_circuit.byte_length, emitted_qasm.len());

    // Reconcile per-gate counts and gphase directly against the emitted OpenQASM lines
    let mut actual_gate_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut found_gphase = false;
    let mut found_phys_reg = false;
    for line in emitted_qasm.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("OPENQASM") || trimmed.starts_with("include") {
            continue;
        }
        if trimmed == "qubit[4] p;" {
            found_phys_reg = true;
            continue;
        }
        if trimmed.starts_with("gphase(") {
            found_gphase = true;
            continue;
        }
        let mnemonic = trimmed
            .split(|c: char| c.is_whitespace() || c == '(')
            .next()
            .unwrap()
            .to_string();
        *actual_gate_counts.entry(mnemonic).or_default() += 1;
    }
    assert!(
        found_phys_reg,
        "Expected physical register declaration qubit[4] p;"
    );
    assert_eq!(manifest.global_phase.emitted_in_openqasm, found_gphase);
    assert_eq!(manifest.metrics.per_gate_counts, actual_gate_counts);
    assert_eq!(
        manifest.metrics.total_gate_count,
        actual_gate_counts.values().sum::<usize>()
    );
    assert_eq!(
        manifest.metrics.cx_count,
        actual_gate_counts.get("cx").copied().unwrap_or(0)
    );
    assert_eq!(manifest.metrics.logical_swaps, 2);
    assert_eq!(manifest.metrics.router_swaps_inserted, 3);
    assert_eq!(manifest.metrics.final_emitted_swaps, 0);
    assert!(manifest.metrics.is_unitary);

    let _ = fs::remove_file(qasm_path);
    let _ = fs::remove_file(manifest_path);
}

#[test]
fn test_cli_compile_emit_manifest_swap_accounting_linear_vs_generic() {
    let manifest_native = "target/test_manifest_routing_native.json";
    let manifest_generic = "target/test_manifest_routing_generic.json";
    let _ = fs::remove_file(manifest_native);
    let _ = fs::remove_file(manifest_generic);

    let out1 = Command::new(QCC_BIN)
        .arg("compile")
        .arg("--target")
        .arg("linear")
        .arg("--emit-manifest")
        .arg(manifest_native)
        .arg("examples/routing.qasm")
        .output()
        .expect("Compile routing.qasm linear");
    assert_eq!(out1.status.code(), Some(0));

    let out2 = Command::new(QCC_BIN)
        .arg("compile")
        .arg("--target")
        .arg("linear-generic")
        .arg("--emit-manifest")
        .arg(manifest_generic)
        .arg("examples/routing.qasm")
        .output()
        .expect("Compile routing.qasm linear-generic");
    assert_eq!(out2.status.code(), Some(0));

    let m_native = qcc::CompilationManifest::from_json(
        &fs::read_to_string(manifest_native).expect("Read native manifest"),
    )
    .unwrap();
    let m_generic = qcc::CompilationManifest::from_json(
        &fs::read_to_string(manifest_generic).expect("Read generic manifest"),
    )
    .unwrap();

    // Native {rz, sx, cx} lowers inserted SWAPs into CX
    assert_eq!(m_native.metrics.logical_swaps, 0);
    assert_eq!(m_native.metrics.router_swaps_inserted, 1);
    assert_eq!(m_native.metrics.final_emitted_swaps, 0);

    // Generic target retains native SWAP in the emitted circuit
    assert_eq!(m_generic.metrics.logical_swaps, 0);
    assert_eq!(m_generic.metrics.router_swaps_inserted, 1);
    assert_eq!(m_generic.metrics.final_emitted_swaps, 1);
    assert_eq!(
        m_generic.metrics.per_gate_counts.get("swap").copied(),
        Some(1)
    );

    let _ = fs::remove_file(manifest_native);
    let _ = fs::remove_file(manifest_generic);
}

#[test]
fn test_cli_compile_emit_manifest_frontend_only_marks_unavailable_null() {
    let manifest_path = "target/test_manifest_frontend_only.json";
    let _ = fs::remove_file(manifest_path);

    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("--emit-manifest")
        .arg(manifest_path)
        .arg("tests/fixtures/qasm3/bell.qasm")
        .output()
        .expect("Compile bell.qasm without --target");

    assert_eq!(output.status.code(), Some(0));
    let stdout_qasm = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    let manifest_raw = fs::read_to_string(manifest_path).expect("Read manifest");
    let json_val: serde_json::Value = serde_json::from_str(&manifest_raw).expect("Valid JSON");

    assert_eq!(json_val["compilation_mode"], "frontend_only");
    assert_eq!(json_val["logical_qubits"], 2);
    assert!(json_val["physical_qubits"].is_null());
    assert!(json_val["target"].is_null());
    assert!(json_val["layout"].is_null());
    assert!(json_val["routing"].is_null());
    assert!(json_val["legality"].is_null());
    assert!(json_val["scheduling"].is_null());
    assert!(json_val["internal_verification"].is_null());
    assert!(json_val["config"]["optimization_level"].is_null());
    assert!(json_val["config"]["router"].is_null());
    assert_eq!(json_val["emitted_circuit"]["output_destination"], "stdout");
    assert!(json_val["emitted_circuit"]["path"].is_null());
    assert_eq!(
        json_val["emitted_circuit"]["sha256"],
        qcc::sha256_hex(stdout_qasm.as_bytes())
    );
    assert_eq!(json_val["metrics"]["is_unitary"], false);

    let _ = fs::remove_file(manifest_path);
}

#[test]
fn test_cli_compile_emit_manifest_deterministic_across_repeated_runs() {
    let qasm1 = "target/test_manifest_det1.qasm";
    let qasm2 = "target/test_manifest_det2.qasm";
    let m1 = "target/test_manifest_det1.json";
    let m2 = "target/test_manifest_det2.json";
    for f in [qasm1, qasm2, m1, m2] {
        let _ = fs::remove_file(f);
    }

    let r1 = Command::new(QCC_BIN)
        .args([
            "compile",
            "--target",
            "linear",
            "-o",
            qasm1,
            "--emit-manifest",
            m1,
            "examples/showcase.qasm",
        ])
        .output()
        .expect("Run 1");
    let r2 = Command::new(QCC_BIN)
        .args([
            "compile",
            "--target",
            "linear",
            "-o",
            qasm1,
            "--emit-manifest",
            m2,
            "examples/showcase.qasm",
        ])
        .output()
        .expect("Run 2");

    assert_eq!(r1.status.code(), Some(0));
    assert_eq!(r2.status.code(), Some(0));
    assert_eq!(
        fs::read_to_string(m1).unwrap(),
        fs::read_to_string(m2).unwrap()
    );

    // Also verify stdout determinism when -o is omitted
    let s1 = "target/test_manifest_det_s1.json";
    let s2 = "target/test_manifest_det_s2.json";
    let r3 = Command::new(QCC_BIN)
        .args([
            "compile",
            "--target",
            "ring",
            "--emit-manifest",
            s1,
            "examples/showcase.qasm",
        ])
        .output()
        .expect("Stdout Run 1");
    let r4 = Command::new(QCC_BIN)
        .args([
            "compile",
            "--target",
            "ring",
            "--emit-manifest",
            s2,
            "examples/showcase.qasm",
        ])
        .output()
        .expect("Stdout Run 2");
    assert_eq!(r3.status.code(), Some(0));
    assert_eq!(r4.status.code(), Some(0));
    assert_eq!(r3.stdout, r4.stdout);
    assert_eq!(
        fs::read_to_string(s1).unwrap(),
        fs::read_to_string(s2).unwrap()
    );

    for f in [qasm1, qasm2, m1, m2, s1, s2] {
        let _ = fs::remove_file(f);
    }
}

#[test]
fn test_cli_compile_emit_manifest_not_written_on_compiler_failure() {
    let manifest_invalid = "target/test_manifest_fail_invalid.json";
    let manifest_unsup = "target/test_manifest_fail_unsup.json";
    let _ = fs::remove_file(manifest_invalid);
    let _ = fs::remove_file(manifest_unsup);

    let res_invalid = Command::new(QCC_BIN)
        .args([
            "compile",
            "--target",
            "linear",
            "--emit-manifest",
            manifest_invalid,
            "tests/qasm3/conformance/fixtures/invalid/invalid_out_of_bounds.qasm",
        ])
        .output()
        .expect("Run compile invalid");
    assert_eq!(res_invalid.status.code(), Some(2));
    assert!(!Path::new(manifest_invalid).exists());

    let res_unsup = Command::new(QCC_BIN)
        .args([
            "compile",
            "--target",
            "linear",
            "--emit-manifest",
            manifest_unsup,
            "tests/qasm3/conformance/fixtures/unsupported/unsupported_standard_p.qasm",
        ])
        .output()
        .expect("Run compile unsupported");
    assert_eq!(res_unsup.status.code(), Some(3));
    assert!(!Path::new(manifest_unsup).exists());
}

#[test]
fn test_cli_compile_emit_manifest_unwritable_path_fails_with_code_1() {
    let unwritable = "target/nonexistent_qcc_manifest_dir/manifest.json";
    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "--target",
            "linear",
            "--emit-manifest",
            unwritable,
            "examples/routing.qasm",
        ])
        .output()
        .expect("Run compile with unwritable manifest path");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "stdout should stay empty when manifest write fails"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to write manifest file"));
}

#[test]
fn test_cli_compile_emit_manifest_with_bindings_and_unbound_symbol_rejection() {
    let in_file = "target/test_manifest_sym_phase.qasm";
    let manifest_unbound = "target/test_manifest_unbound_phase.json";
    let manifest_bound = "target/test_manifest_bound_phase.json";
    let _ = fs::remove_file(manifest_unbound);
    let _ = fs::remove_file(manifest_bound);

    let src = "OPENQASM 3.0;\ninclude \"stdgates.inc\";\nqubit[1] q;\ngphase(theta);\nh q[0];\n";
    fs::write(in_file, src).expect("Write temp QASM");

    // 1. Unbound symbolic parameter is rejected by semantic analyzer (E0031, exit code 2), no manifest written
    let out_unbound = Command::new(QCC_BIN)
        .args(["compile", "--emit-manifest", manifest_unbound, in_file])
        .output()
        .expect("Compile unbound symbolic phase");
    assert_eq!(out_unbound.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out_unbound.stderr).contains("E0031"));
    assert!(!Path::new(manifest_unbound).exists());

    // 2. Bound parameter in target-compiled mode with spare physical qubit (1 logical on 2 physical)
    let out_bound = Command::new(QCC_BIN)
        .args([
            "compile",
            "--target",
            "linear",
            "-b",
            "theta=0.5",
            "--emit-manifest",
            manifest_bound,
            in_file,
        ])
        .output()
        .expect("Compile bound phase with target");
    assert_eq!(
        out_bound.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out_bound.stderr)
    );
    let m_bound =
        qcc::CompilationManifest::from_json(&fs::read_to_string(manifest_bound).unwrap()).unwrap();
    assert_eq!(
        m_bound.config.parameter_bindings.get("theta").copied(),
        Some(0.5)
    );
    assert_eq!(m_bound.input_global_phase.radians, Some(0.5));
    assert!(m_bound.global_phase.is_numeric);
    assert!(m_bound.global_phase.emitted_in_openqasm);
    assert_eq!(m_bound.logical_qubits, 1);
    assert_eq!(m_bound.physical_qubits, Some(2));
    assert_eq!(
        m_bound
            .layout
            .as_ref()
            .unwrap()
            .unused_physical_qubits_final
            .len(),
        1
    );

    for f in [in_file, manifest_unbound, manifest_bound] {
        let _ = fs::remove_file(f);
    }
}
