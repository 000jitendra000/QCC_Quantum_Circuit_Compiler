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

// ============================================================================
// 5. Per-Pass Diagnostic CLI & Inspect/Explain Tests (Milestone M8 Phase 73)
// ============================================================================

#[test]
fn test_cli_compile_pass_report_basic() {
    let report_path = "target/test_p73_basic.json";
    let _ = fs::remove_file(report_path);

    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--pass-report",
            report_path,
        ])
        .output()
        .expect("Execute compile with --pass-report");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(Path::new(report_path).exists());

    let content = fs::read_to_string(report_path).expect("Read report file");
    let reports = qcc::pass_diagnostics_from_json(&content).expect("Parse JSON reports");
    assert!(!reports.is_empty(), "Reports should not be empty");

    for rep in &reports {
        rep.validate_invariants().expect("Invariants valid");
        assert!(rep.measurements.before.is_none());
        assert!(rep.measurements.after.is_none());
        assert_eq!(
            rep.measurements.get_availability("before"),
            qcc::MeasurementAvailability::NotMeasured
        );
        assert_eq!(
            rep.measurements.get_availability("after"),
            qcc::MeasurementAvailability::NotMeasured
        );
        assert!(rep.fingerprints.input_fingerprint.is_none());
        assert!(rep.fingerprints.output_fingerprint.is_none());
        assert!(!rep.fingerprints.semantic_equivalence_guaranteed);
    }

    let _ = fs::remove_file(report_path);
}

#[test]
fn test_cli_compile_pass_report_metrics() {
    let report_path = "target/test_p73_metrics.json";
    let _ = fs::remove_file(report_path);

    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--pass-diagnostics",
            "metrics",
            "--pass-report",
            report_path,
        ])
        .output()
        .expect("Execute compile with --pass-diagnostics metrics");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(report_path).expect("Read report file");
    let reports = qcc::pass_diagnostics_from_json(&content).expect("Parse JSON reports");
    assert!(!reports.is_empty());

    for rep in &reports {
        rep.validate_invariants().expect("Invariants valid");
        if rep.status == qcc::PassStatus::Ran {
            assert!(rep.measurements.before.is_some());
            assert!(rep.measurements.after.is_some());
            assert!(rep
                .measurements
                .before
                .as_ref()
                .unwrap()
                .total_gate_count
                .is_some());
            assert_eq!(
                rep.measurements.get_availability("before"),
                qcc::MeasurementAvailability::Available
            );
            assert_eq!(
                rep.measurements.get_availability("after"),
                qcc::MeasurementAvailability::Available
            );
        }
        // Fingerprints must remain uncomputed in metrics-only mode
        assert!(rep.fingerprints.input_fingerprint.is_none());
        assert!(rep.fingerprints.output_fingerprint.is_none());
    }

    let _ = fs::remove_file(report_path);
}

#[test]
fn test_cli_compile_pass_report_fingerprints() {
    let report_path = "target/test_p73_fingerprints.json";
    let _ = fs::remove_file(report_path);

    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--pass-diagnostics",
            "fingerprints",
            "--pass-report",
            report_path,
        ])
        .output()
        .expect("Execute compile with --pass-diagnostics fingerprints");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(report_path).expect("Read report file");
    let reports = qcc::pass_diagnostics_from_json(&content).expect("Parse JSON reports");
    assert!(!reports.is_empty());

    for rep in &reports {
        rep.validate_invariants().expect("Invariants valid");
        if rep.status == qcc::PassStatus::Ran {
            assert!(rep.fingerprints.input_fingerprint.is_some());
            assert!(rep.fingerprints.output_fingerprint.is_some());
            assert!(!rep.fingerprints.semantic_equivalence_guaranteed);
        }
        // Metrics must remain unmeasured in fingerprints-only mode
        assert!(rep.measurements.before.is_none());
        assert!(rep.measurements.after.is_none());
    }

    let _ = fs::remove_file(report_path);
}

#[test]
fn test_cli_compile_pass_report_verify() {
    let report_path = "target/test_p73_full.json";
    let _ = fs::remove_file(report_path);

    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--pass-diagnostics",
            "full",
            "--pass-report",
            report_path,
        ])
        .output()
        .expect("Execute compile with --pass-diagnostics full");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let content = fs::read_to_string(report_path).expect("Read report file");
    let reports = qcc::pass_diagnostics_from_json(&content).expect("Parse JSON reports");
    assert!(!reports.is_empty());

    let mut any_passed = false;
    for rep in &reports {
        rep.validate_invariants().expect("Invariants valid");
        if rep.verification.status == qcc::PassVerificationStatus::Passed {
            any_passed = true;
            assert_eq!(
                rep.verification.provenance,
                Some(qcc::VerifierProvenance::Internal)
            );
            assert!(rep.verification.verifier_name.is_some());
        }
    }
    assert!(
        any_passed,
        "At least one pass must have passed verification under full mode"
    );

    let _ = fs::remove_file(report_path);
}

#[test]
fn test_cli_compile_pass_report_schema_version() {
    let report_path = "target/test_p73_schema.json";
    let _ = fs::remove_file(report_path);

    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--pass-report",
            report_path,
        ])
        .output()
        .expect("Execute compile for schema version check");

    assert_eq!(output.status.code(), Some(0));
    let content = fs::read_to_string(report_path).expect("Read report file");
    let reports = qcc::pass_diagnostics_from_json(&content).expect("Parse JSON reports");

    for rep in &reports {
        assert_eq!(rep.schema_id, "qcc.pass_diagnostic");
        assert_eq!(rep.schema_version, "1.0.0");
    }

    let _ = fs::remove_file(report_path);
}

#[test]
fn test_cli_compile_pass_report_execution_order() {
    let report_path = "target/test_p73_order.json";
    let _ = fs::remove_file(report_path);

    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--pass-report",
            report_path,
        ])
        .output()
        .expect("Execute compile for execution order check");

    assert_eq!(output.status.code(), Some(0));
    let content = fs::read_to_string(report_path).expect("Read report file");
    let reports = qcc::pass_diagnostics_from_json(&content).expect("Parse JSON reports");

    for (i, rep) in reports.iter().enumerate() {
        assert_eq!(rep.execution_order, i);
    }

    let _ = fs::remove_file(report_path);
}

#[test]
fn test_cli_compile_pass_report_skipped_o0() {
    let report_path = "target/test_p73_skipped_o0.json";
    let _ = fs::remove_file(report_path);

    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "tests/fixtures/qasm3/bell.qasm",
            "-O0",
            "--pass-report",
            report_path,
        ])
        .output()
        .expect("Execute compile with -O0");

    assert_eq!(output.status.code(), Some(0));
    let content = fs::read_to_string(report_path).expect("Read report file");
    let reports = qcc::pass_diagnostics_from_json(&content).expect("Parse JSON reports");
    assert!(!reports.is_empty());

    for rep in &reports {
        rep.validate_invariants().expect("Invariants valid");
        assert_eq!(rep.status, qcc::PassStatus::Skipped);
        assert_eq!(rep.verification.status, qcc::PassVerificationStatus::NotRun);
        assert_eq!(
            rep.measurements.get_availability("after"),
            qcc::MeasurementAvailability::NotApplicable
        );
    }

    let _ = fs::remove_file(report_path);
}

// Low-level programmatic unit test verifying in-memory PassManager and PassContext
// preservation of diagnostic reports upon pass failure.
#[test]
fn test_cli_compile_pass_report_failed_pass_preservation_in_memory() {
    use qcc::builder::CircuitBuilder;
    use qcc::compiler::pass_diagnostic::{PassDiagnosticConfig, PassStatus};
    use qcc::opt::context::PassContext;
    use qcc::opt::errors::PassError;
    use qcc::opt::pass::{CompilerPass, PassId, PassResult, SemanticDomain};
    use qcc::opt::pass_manager::PassManager;

    struct StepPass(usize);
    impl CompilerPass for StepPass {
        fn id(&self) -> PassId {
            match self.0 {
                0 => PassId::new("step_zero"),
                1 => PassId::new("step_one"),
                _ => PassId::new("step_other"),
            }
        }
        fn description(&self) -> &'static str {
            "Step Pass"
        }
        fn semantic_domain(&self) -> SemanticDomain {
            SemanticDomain::GeneralStructural
        }
        fn run(
            &mut self,
            _circuit: &mut qcc::ir::circuit::Circuit,
            _context: &mut PassContext,
        ) -> Result<PassResult, PassError> {
            Ok(PassResult::unmodified())
        }
    }

    struct FailingPass;
    impl CompilerPass for FailingPass {
        fn id(&self) -> PassId {
            PassId::new("deliberate_failure")
        }
        fn description(&self) -> &'static str {
            "Deliberate Failure Pass"
        }
        fn semantic_domain(&self) -> SemanticDomain {
            SemanticDomain::GeneralStructural
        }
        fn run(
            &mut self,
            _circuit: &mut qcc::ir::circuit::Circuit,
            _context: &mut PassContext,
        ) -> Result<PassResult, PassError> {
            Err(PassError::ExecutionFailed {
                pass_id: self.id(),
                reason: "injected failure for diagnostic preservation test".to_string(),
            })
        }
    }

    let mut b = CircuitBuilder::new("fail_circ");
    let q = b.add_quantum_register("q", 2).unwrap();
    b.h(q[0]).unwrap();
    let mut circuit = b.finish().unwrap();

    let mut pm = PassManager::new().with_diagnostic_config(PassDiagnosticConfig::standard());
    pm.add_pass(StepPass(0));
    pm.add_pass(StepPass(1));
    pm.add_pass(FailingPass);
    pm.add_pass(StepPass(2));

    let mut context = PassContext::new();
    let res = pm.run_with_context(&mut circuit, &mut context);
    assert!(res.is_err(), "Expected pass failure");

    let records = context.diagnostic_reports();
    assert_eq!(
        records.len(),
        3,
        "Expected 2 ran passes + 1 failed pass record"
    );
    assert_eq!(records[0].status, PassStatus::Ran);
    assert_eq!(records[1].status, PassStatus::Ran);
    assert_eq!(records[2].status, PassStatus::Failed);
    assert_eq!(records[2].pass_id, "deliberate_failure");
    assert_eq!(
        records[2].measurements.get_availability("after"),
        qcc::MeasurementAvailability::Unavailable
    );

    for r in records {
        r.validate_invariants().expect("Invariants must hold");
    }
    let json = qcc::pass_diagnostics_to_json_pretty(records).expect("Serialize preserved reports");
    let deserialized =
        qcc::pass_diagnostics_from_json(&json).expect("Deserialize preserved reports");
    assert_eq!(deserialized.len(), 3);
}

/// End-to-end CLI integration test: verifies that when compilation using `--target`
/// encounters a pass failure during optimization, the compiled CLI binary exits
/// with code 2, preserves all diagnostic records accumulated prior to and including
/// the failed pass, writes `--pass-report`, and produces valid Phase 71 schema records.
#[test]
fn test_cli_compile_target_pass_report_preservation_on_pass_failure() {
    let report_path = "target/test_cli_target_failed_pass_report.json";
    let _ = fs::remove_file(report_path);

    let output = Command::new(QCC_BIN)
        .env("QCC_TEST_FAIL_PASS", "rotation_merge")
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--pass-report",
            report_path,
        ])
        .output()
        .expect("Execute qcc compile with injected pass failure");

    // 1. Non-zero compilation exit status (exit code 2 for CompileError)
    assert_eq!(
        output.status.code(),
        Some(2),
        "Compilation must fail with exit code 2 (CompileError)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error: compilation pipeline failed"),
        "Stderr must report pipeline failure"
    );
    assert!(
        stderr.contains("rotation_merge"),
        "Stderr must identify the failing pass"
    );

    // 2. Diagnostic report file exists
    assert!(
        Path::new(report_path).exists(),
        "Diagnostic report file must be written even when compilation fails"
    );

    let content = fs::read_to_string(report_path).expect("Read report file");
    let reports = qcc::pass_diagnostics_from_json(&content).expect("Parse JSON reports");

    // 3. Previously successful pass records are preserved + failed pass record
    assert_eq!(
        reports.len(),
        3,
        "Expected 2 successful passes + 1 failed pass record"
    );

    // Pass 0: identity_elimination -> ran
    assert_eq!(reports[0].pass_id, "identity_elimination");
    assert_eq!(reports[0].status, qcc::PassStatus::Ran);

    // Pass 1: inverse_cancellation -> ran
    assert_eq!(reports[1].pass_id, "inverse_cancellation");
    assert_eq!(reports[1].status, qcc::PassStatus::Ran);

    // 4. The failed pass is represented with status "failed"
    assert_eq!(reports[2].pass_id, "rotation_merge");
    assert_eq!(reports[2].status, qcc::PassStatus::Failed);
    assert_eq!(
        reports[2].measurements.get_availability("after"),
        qcc::MeasurementAvailability::Unavailable
    );

    // 5. Each record passes invariant validation
    for rep in &reports {
        rep.validate_invariants().expect("Invariants valid");
    }

    let _ = fs::remove_file(report_path);
}

/// End-to-end CLI integration test: verifies that target-free frontend compilation
/// preserves diagnostics and writes `--pass-report` on pass failure.
#[test]
fn test_cli_compile_target_free_pass_report_preservation_on_pass_failure() {
    let report_path = "target/test_cli_target_free_failed_pass_report.json";
    let _ = fs::remove_file(report_path);

    let output = Command::new(QCC_BIN)
        .env("QCC_TEST_FAIL_PASS", "rotation_merge")
        .args([
            "compile",
            "examples/routing.qasm",
            "--pass-report",
            report_path,
        ])
        .output()
        .expect("Execute qcc compile target-free with injected pass failure");

    assert_eq!(
        output.status.code(),
        Some(2),
        "Compilation must fail with exit code 2 (CompileError)"
    );
    assert!(
        Path::new(report_path).exists(),
        "Diagnostic report file must be written"
    );

    let content = fs::read_to_string(report_path).expect("Read report file");
    let reports = qcc::pass_diagnostics_from_json(&content).expect("Parse JSON reports");
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0].status, qcc::PassStatus::Ran);
    assert_eq!(reports[1].status, qcc::PassStatus::Ran);
    assert_eq!(reports[2].status, qcc::PassStatus::Failed);
    assert_eq!(
        reports[2].measurements.get_availability("after"),
        qcc::MeasurementAvailability::Unavailable
    );

    for rep in &reports {
        rep.validate_invariants().expect("Invariants valid");
    }

    let _ = fs::remove_file(report_path);
}

#[test]
fn test_cli_compile_inspect_pass_existing() {
    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--inspect-pass",
            "identity_elimination",
        ])
        .output()
        .expect("Execute compile with --inspect-pass");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Stdout must contain clean OpenQASM 3 output
    assert!(stdout.contains("OPENQASM 3.0;"));

    // Stderr must contain detailed pass inspection
    assert!(stderr.contains("Pass ID:        identity_elimination"));
    assert!(stderr.contains("Pass Name:"));
    assert!(stderr.contains("Execution Order:"));
    assert!(stderr.contains("Status:         ran"));
    assert!(stderr.contains("Configuration:"));
    assert!(stderr.contains("Measurements:"));
    assert!(stderr.contains("Transformation Accounting:"));
    assert!(stderr.contains("Circuit Fingerprints:"));
    assert!(stderr.contains("Verification:"));
}

#[test]
fn test_cli_compile_inspect_pass_missing() {
    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--inspect-pass",
            "non_existent_pass_xyz",
        ])
        .output()
        .expect("Execute compile with invalid --inspect-pass");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr
        .contains("error: pass ID 'non_existent_pass_xyz' was not found in pass diagnostics."));
    assert!(stderr.contains("Available pass IDs:"));
    assert!(stderr.contains("identity_elimination"));
}

#[test]
fn test_cli_compile_inspect_pass_repeated() {
    // Multi-gate sequence to trigger multiple fixed-point optimization passes
    let src = "OPENQASM 3.0;\nqubit[2] q;\nx q[0];\nx q[0];\nx q[0];\nx q[0];\nh q[0];\nh q[0];\ncx q[0], q[1];\n";
    let in_file = "target/test_p73_repeated.qasm";
    fs::write(in_file, src).expect("Write multi-iteration fixture");

    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            in_file,
            "-O3",
            "--inspect-pass",
            "identity_elimination",
        ])
        .output()
        .expect("Execute compile inspecting repeated pass");

    let _ = fs::remove_file(in_file);

    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Pass ID:        identity_elimination"));
    assert!(stderr.contains("=== Execution 1 of ") || stderr.contains("=== Pass Inspection:"));
}

#[test]
fn test_cli_compile_pass_report_write_failure() {
    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "tests/fixtures/qasm3/bell.qasm",
            "--pass-report",
            "/nonexistent_directory_p73_xyz/report.json",
        ])
        .output()
        .expect("Execute compile with unwritable pass report path");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error: failed to write pass diagnostic report file"));
}

#[test]
fn test_cli_compile_behavior_unchanged_without_diagnostics() {
    let output = Command::new(QCC_BIN)
        .args(["compile", "tests/fixtures/qasm3/bell.qasm"])
        .output()
        .expect("Execute normal compile");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("OPENQASM 3.0;"));
    assert!(stdout.contains("qubit[2] q;"));
    assert!(!stderr.contains("Pass Execution Summary:"));
    assert!(!stderr.contains("Pass ID:"));
}

#[test]
fn test_cli_compile_pass_report_deterministic_output() {
    let file1 = "target/test_p73_det1.json";
    let file2 = "target/test_p73_det2.json";
    let _ = fs::remove_file(file1);
    let _ = fs::remove_file(file2);

    let out1 = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--pass-diagnostics",
            "standard",
            "--pass-report",
            file1,
        ])
        .output()
        .expect("Execute compile 1");
    assert_eq!(out1.status.code(), Some(0));

    let out2 = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--pass-diagnostics",
            "standard",
            "--pass-report",
            file2,
        ])
        .output()
        .expect("Execute compile 2");
    assert_eq!(out2.status.code(), Some(0));

    let content1 = fs::read_to_string(file1).expect("Read file1");
    let content2 = fs::read_to_string(file2).expect("Read file2");

    let mut reps1 = qcc::pass_diagnostics_from_json(&content1).expect("Parse reps1");
    let mut reps2 = qcc::pass_diagnostics_from_json(&content2).expect("Parse reps2");

    assert_eq!(reps1.len(), reps2.len());

    // Duration is non-deterministic wall-clock telemetry; normalize to verify bit-for-bit repeatability of all semantic fields
    for r in &mut reps1 {
        r.measurements.duration_ms = Some(0.0);
    }
    for r in &mut reps2 {
        r.measurements.duration_ms = Some(0.0);
    }

    for (idx, (r1, r2)) in reps1.iter().zip(reps2.iter()).enumerate() {
        if r1 != r2 {
            panic!("Difference at index {idx}:\nr1: {r1:#?}\nr2: {r2:#?}");
        }
    }

    let norm1 = qcc::pass_diagnostics_to_json_pretty(&reps1).unwrap();
    let norm2 = qcc::pass_diagnostics_to_json_pretty(&reps2).unwrap();
    assert_eq!(
        norm1, norm2,
        "Normalized pass reports must be bit-for-bit identical"
    );

    let _ = fs::remove_file(file1);
    let _ = fs::remove_file(file2);
}

#[test]
fn test_cli_compile_pass_summary() {
    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--pass-summary",
        ])
        .output()
        .expect("Execute compile with --pass-summary");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Stdout must contain clean OpenQASM 3
    assert!(stdout.contains("OPENQASM 3.0;"));

    // Stderr must contain formatted summary table
    assert!(stderr.contains("Pass Execution Summary:"));
    assert!(stderr.contains("+------+-----------------------+---------+----------+------------------+----------------+--------------+"));
    assert!(stderr.contains(
        "| Step | Pass ID               | Status  | Time(ms) | Gates (In -> Out)| Delta (+/-/~ ) | Verification |"
    ));
    assert!(stderr.contains("Total passes:"));
}

#[test]
fn test_cli_compile_help_documents_routing_candidates() {
    let output = Command::new(QCC_BIN)
        .arg("compile")
        .arg("--help")
        .output()
        .expect("Execute compile --help");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--routing-candidates <N>"));
    assert!(stdout.contains("--routing-objective <OBJ>"));
}

#[test]
fn test_cli_compile_routing_candidates_without_target_fails() {
    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--routing-candidates",
            "4",
        ])
        .output()
        .expect("Execute compile without target");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("'--routing-candidates' requires a target topology"));
}

#[test]
fn test_cli_compile_routing_candidates_invalid_count_fails() {
    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--routing-candidates",
            "0",
        ])
        .output()
        .expect("Execute compile with 0 candidates");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid candidate count '0'"));
}

#[test]
fn test_cli_compile_multi_candidate_search_emit_manifest() {
    let manifest_path = "target/routing_search_cli_manifest.json";
    let out_qasm = "target/routing_search_cli_routed.qasm";
    let _ = fs::remove_file(manifest_path);
    let _ = fs::remove_file(out_qasm);

    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            "examples/routing.qasm",
            "--target",
            "linear",
            "--routing-candidates",
            "4",
            "-o",
            out_qasm,
            "--emit-manifest",
            manifest_path,
        ])
        .output()
        .expect("Execute compile with multi-candidate search");

    assert_eq!(output.status.code(), Some(0));
    assert!(Path::new(out_qasm).exists());
    assert!(Path::new(manifest_path).exists());

    let manifest_str = fs::read_to_string(manifest_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest_str).unwrap();

    let router_name = manifest["routing"]["router_name"].as_str().unwrap();
    assert!(
        router_name.contains("MultiCandidate Provisional:"),
        "Router name must indicate multi-candidate selection, got: {router_name}"
    );
    assert_eq!(manifest["target"]["name"], "linear_target");
    assert!(manifest["metrics"]["router_swaps_inserted"].is_number());

    let _ = fs::remove_file(manifest_path);
    let _ = fs::remove_file(out_qasm);
}

#[test]
fn test_cli_compile_enable_commutative_cancellation_logical() {
    let input_path = "target/test_comm_cancel_input.qasm";
    let out_control = "target/test_comm_cancel_ctrl.qasm";
    let out_treatment = "target/test_comm_cancel_treat.qasm";
    let report_treatment = "target/test_comm_cancel_treat_report.json";

    let src = "OPENQASM 3.0;\nqubit[2] q;\ncx q[0], q[1];\nrz(0.3) q[0];\ncx q[0], q[1];\n";
    fs::write(input_path, src).expect("write input");

    // Control: without flag (default false)
    let output_ctrl = Command::new(QCC_BIN)
        .args(["compile", input_path, "-o", out_control])
        .output()
        .expect("compile control");
    assert_eq!(output_ctrl.status.code(), Some(0));
    let ctrl_qasm = fs::read_to_string(out_control).unwrap();
    // Default does not run commutative cancellation across intervening Rz
    assert!(ctrl_qasm.contains("cx q[0], q[1];"));

    // Treatment: with flag enabled
    let output_treat = Command::new(QCC_BIN)
        .args([
            "compile",
            input_path,
            "--enable-commutative-cancellation",
            "-o",
            out_treatment,
            "--pass-report",
            report_treatment,
        ])
        .output()
        .expect("compile treatment");
    assert_eq!(output_treat.status.code(), Some(0));
    let treat_qasm = fs::read_to_string(out_treatment).unwrap();
    // With flag enabled, the two CXs commute past Rz(q0) and cancel!
    assert!(!treat_qasm.contains("cx q[0], q[1];"));
    assert!(treat_qasm.contains("rz(0.3) q[0];"));

    // Verify pass report recorded commutative cancellation pass
    let rep_str = fs::read_to_string(report_treatment).unwrap();
    let rep_json: serde_json::Value = serde_json::from_str(&rep_str).unwrap();
    let comm_pass = rep_json
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["pass_id"] == "commutative_cancellation")
        .expect("commutative_cancellation pass should be present in report");
    assert_eq!(comm_pass["status"], "ran");
    assert_eq!(comm_pass["transformation"]["operations_removed"], 2);

    let _ = fs::remove_file(input_path);
    let _ = fs::remove_file(out_control);
    let _ = fs::remove_file(out_treatment);
    let _ = fs::remove_file(report_treatment);
}

#[test]
fn test_cli_compile_enable_commutative_cancellation_target() {
    let input_path = "target/test_comm_cancel_tgt_input.qasm";
    let out_manifest_ctrl = "target/test_comm_cancel_tgt_ctrl_manifest.json";
    let out_manifest_treat = "target/test_comm_cancel_tgt_treat_manifest.json";

    let src = "OPENQASM 3.0;\nqubit[2] q;\ncx q[0], q[1];\nrz(0.3) q[0];\ncx q[0], q[1];\n";
    fs::write(input_path, src).expect("write input");

    // Control: target compilation without flag
    let output_ctrl = Command::new(QCC_BIN)
        .args([
            "compile",
            input_path,
            "--target",
            "linear",
            "--emit-manifest",
            out_manifest_ctrl,
        ])
        .output()
        .expect("compile control target");
    assert_eq!(output_ctrl.status.code(), Some(0));
    let ctrl_man_str = fs::read_to_string(out_manifest_ctrl).unwrap();
    let ctrl_man: serde_json::Value = serde_json::from_str(&ctrl_man_str).unwrap();
    let ctrl_gates = ctrl_man["metrics"]["total_gate_count"].as_u64().unwrap();

    // Treatment: target compilation with flag
    let output_treat = Command::new(QCC_BIN)
        .args([
            "compile",
            input_path,
            "--target",
            "linear",
            "--enable-commutative-cancellation",
            "--emit-manifest",
            out_manifest_treat,
        ])
        .output()
        .expect("compile treatment target");
    assert_eq!(output_treat.status.code(), Some(0));
    let treat_man_str = fs::read_to_string(out_manifest_treat).unwrap();
    let treat_man: serde_json::Value = serde_json::from_str(&treat_man_str).unwrap();
    let treat_gates = treat_man["metrics"]["total_gate_count"].as_u64().unwrap();

    // Treatment should have eliminated the 2 CX gates
    assert!(
        treat_gates < ctrl_gates,
        "Treatment final gate count ({treat_gates}) should be less than control ({ctrl_gates})"
    );

    let _ = fs::remove_file(input_path);
    let _ = fs::remove_file(out_manifest_ctrl);
    let _ = fs::remove_file(out_manifest_treat);
}

#[test]
fn test_cli_compile_elide_swaps_help() {
    let output = Command::new(QCC_BIN)
        .args(["compile", "--help"])
        .output()
        .expect("compile --help");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--elide-swaps"),
        "Help text must document '--elide-swaps'"
    );
}

#[test]
fn test_cli_compile_elide_swaps_reduces_cx_and_updates_manifest() {
    let input_path = "target/test_elide_swaps_cli_input.qasm";
    let manifest_ctrl = "target/test_elide_swaps_cli_ctrl_manifest.json";
    let manifest_treat = "target/test_elide_swaps_cli_treat_manifest.json";

    let src = "OPENQASM 3.0;\ninclude \"stdgates.inc\";\nqubit[2] q;\nh q[0];\nx q[1];\nswap q[0], q[1];\n";
    fs::write(input_path, src).expect("write input");

    // Control: without --elide-swaps (emits 3 CX)
    let output_ctrl = Command::new(QCC_BIN)
        .args([
            "compile",
            input_path,
            "--target",
            "linear",
            "--emit-manifest",
            manifest_ctrl,
        ])
        .output()
        .expect("compile control");
    assert_eq!(output_ctrl.status.code(), Some(0));
    let ctrl_val: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(manifest_ctrl).unwrap()).unwrap();
    let ctrl_cx = ctrl_val["metrics"]["cx_count"].as_u64().unwrap();
    assert_eq!(ctrl_cx, 3);

    // Treatment: with --elide-swaps (emits 0 CX, absorbs permutation into final_layout)
    let output_treat = Command::new(QCC_BIN)
        .args([
            "compile",
            input_path,
            "--target",
            "linear",
            "--elide-swaps",
            "--emit-manifest",
            manifest_treat,
        ])
        .output()
        .expect("compile treatment");
    assert_eq!(output_treat.status.code(), Some(0));
    let treat_val: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(manifest_treat).unwrap()).unwrap();
    let treat_cx = treat_val["metrics"]["cx_count"].as_u64().unwrap();
    assert_eq!(treat_cx, 0, "Logical SWAP elision must eliminate all 3 CX");

    // Verify final layout reflects swap
    let final_layout = &treat_val["layout"]["final_logical_to_physical"];
    assert_eq!(final_layout["0"], 1);
    assert_eq!(final_layout["1"], 0);

    let _ = fs::remove_file(input_path);
    let _ = fs::remove_file(manifest_ctrl);
    let _ = fs::remove_file(manifest_treat);
}

#[test]
fn test_cli_compile_elide_swaps_pass_report() {
    let input_path = "target/test_elide_swaps_report_input.qasm";
    let report_path = "target/test_elide_swaps_cli_report.json";

    let src = "OPENQASM 3.0;\ninclude \"stdgates.inc\";\nqubit[2] q;\nh q[0];\nswap q[0], q[1];\n";
    fs::write(input_path, src).expect("write input");

    let output = Command::new(QCC_BIN)
        .args([
            "compile",
            input_path,
            "--target",
            "linear",
            "--elide-swaps",
            "--pass-report",
            report_path,
        ])
        .output()
        .expect("compile with pass report");
    assert_eq!(output.status.code(), Some(0));

    let report_str = fs::read_to_string(report_path).expect("read pass report");
    let reports: Vec<serde_json::Value> = serde_json::from_str(&report_str).unwrap();

    let elision_rep = reports
        .iter()
        .find(|r| r["pass_id"] == "logical_swap_elision");
    assert!(
        elision_rep.is_some(),
        "Pass report must contain 'logical_swap_elision'"
    );
    let r = elision_rep.unwrap();
    assert_eq!(r["status"], "ran");
    assert_eq!(r["pass_config"]["swaps_elided"], 1);

    let _ = fs::remove_file(input_path);
    let _ = fs::remove_file(report_path);
}
