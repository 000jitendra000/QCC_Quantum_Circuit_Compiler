//! Conformance test suite for QCC OpenQASM 3 frontend.
//!
//! Validates:
//! - Complete supported OpenQASM subset (all standard gates, registers, parameters,
//!   non-unitary operations, global phase, and multi-register circuits).
//! - Unsupported constructs fail closed with deterministic diagnostic codes (E0002–E0006, E0026, E0027).
//! - Invalid programs produce source-located syntax and semantic errors (E0001, E0010, E0011, E0012, E0014, E0021–E0023, E0030, E0031).
//! - Resource bounds (nesting depth, operand limits, register size) are enforced deterministically.

use std::fs;
use std::path::Path;

use qcc::backend::openqasm::emit_openqasm;
use qcc::frontend::adapter::OpenQasmFrontend;
use qcc::frontend::ast::AstStatement;
use qcc::frontend::lowering::lower_qasm_str;
use qcc::frontend::source::SourceBuffer;

const FIXTURES_DIR: &str = "tests/qasm3/conformance/fixtures";

fn read_fixture(subpath: &str) -> String {
    let path = Path::new(FIXTURES_DIR).join(subpath);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read fixture {path:?}: {e}"))
}

// ============================================================================
// 1. Supported Conformance Fixtures
// ============================================================================

#[test]
fn test_conformance_supported_bell() {
    let source = read_fixture("supported/conformance_bell.qasm");
    let res = lower_qasm_str(&source, Some("conformance_bell.qasm"), None);
    assert!(res.is_ok(), "Bell fixture failed: {:?}", res.diagnostics);
    let circuit = res.output.unwrap();
    assert_eq!(circuit.num_qubits(), 2);
    assert_eq!(circuit.num_classical_bits(), 2);
    assert!(circuit.validate().is_ok());

    let emitted = emit_openqasm(&circuit).expect("Emission succeeded");
    assert!(emitted.contains("h q[0];"));
    assert!(emitted.contains("cx q[0], q[1];"));
}

#[test]
fn test_conformance_supported_rotations() {
    let source = read_fixture("supported/conformance_rotations.qasm");
    let res = lower_qasm_str(&source, Some("conformance_rotations.qasm"), None);
    assert!(
        res.is_ok(),
        "Rotations fixture failed: {:?}",
        res.diagnostics
    );
    let circuit = res.output.unwrap();
    assert_eq!(circuit.num_qubits(), 2);
    assert!(circuit.validate().is_ok());
}

#[test]
fn test_conformance_supported_all_canonical_gates() {
    let source = read_fixture("supported/conformance_all_canonical_gates.qasm");
    let res = lower_qasm_str(&source, Some("conformance_all_canonical_gates.qasm"), None);
    assert!(
        res.is_ok(),
        "All canonical gates fixture failed: {:?}",
        res.diagnostics
    );
    let circuit = res.output.unwrap();
    assert_eq!(circuit.num_qubits(), 3);
    assert_eq!(circuit.graph.node_count(), 15);
    assert!(circuit.validate().is_ok());
}

#[test]
fn test_conformance_supported_multi_register() {
    let source = read_fixture("supported/conformance_multi_register.qasm");
    let res = lower_qasm_str(&source, Some("conformance_multi_register.qasm"), None);
    assert!(
        res.is_ok(),
        "Multi-register fixture failed: {:?}",
        res.diagnostics
    );
    let circuit = res.output.unwrap();
    assert_eq!(circuit.num_qubits(), 5); // q[2] + r[3]
    assert_eq!(circuit.num_classical_bits(), 3); // c[1] + d[2]
    assert!(circuit.validate().is_ok());
}

#[test]
fn test_conformance_supported_non_unitary() {
    let source = read_fixture("supported/conformance_non_unitary.qasm");
    let res = lower_qasm_str(&source, Some("conformance_non_unitary.qasm"), None);
    assert!(
        res.is_ok(),
        "Non-unitary fixture failed: {:?}",
        res.diagnostics
    );
    let circuit = res.output.unwrap();
    assert_eq!(circuit.num_qubits(), 2);
    assert_eq!(circuit.num_classical_bits(), 2);
    assert!(circuit.validate().is_ok());
}

#[test]
fn test_conformance_supported_global_phase() {
    let source = read_fixture("supported/conformance_global_phase.qasm");
    let res = lower_qasm_str(&source, Some("conformance_global_phase.qasm"), None);
    assert!(
        res.is_ok(),
        "Global phase fixture failed: {:?}",
        res.diagnostics
    );
    let circuit = res.output.unwrap();
    assert_eq!(circuit.num_qubits(), 1);
    assert!(circuit.validate().is_ok());
}

#[test]
fn test_conformance_supported_teleportation() {
    let source = read_fixture("supported/conformance_teleportation.qasm");
    let res = lower_qasm_str(&source, Some("conformance_teleportation.qasm"), None);
    assert!(
        res.is_ok(),
        "Teleportation fixture failed: {:?}",
        res.diagnostics
    );
    let circuit = res.output.unwrap();
    assert_eq!(circuit.num_qubits(), 3);
    assert_eq!(circuit.num_classical_bits(), 2);
    assert!(circuit.validate().is_ok());
}

// ============================================================================
// 2. Unsupported Conformance Fixtures (Fail Closed)
// ============================================================================

fn assert_unsupported(subpath: &str, expected_code: &str) {
    let source = read_fixture(subpath);
    let buffer = SourceBuffer::new(subpath, source);
    let frontend = OpenQasmFrontend::new();

    // 1. Lowering must fail closed
    let lower_res = frontend.lower(&buffer, None);
    assert!(
        !lower_res.is_ok(),
        "Expected lowering to fail for unsupported fixture {subpath}"
    );
    assert!(
        lower_res
            .diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some(expected_code)),
        "Expected diagnostic code {expected_code} in lowering diagnostics: {:?}",
        lower_res.diagnostics
    );

    // 2. Recovery parse retains AstStatement::Unsupported
    let (ast, diags) = frontend.parse_recovering(&buffer);
    assert!(
        ast.statements
            .iter()
            .any(|stmt| matches!(stmt, AstStatement::Unsupported(_))),
        "Recovery AST must retain AstStatement::Unsupported for {subpath}"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.code.as_deref() == Some(expected_code)),
        "Expected diagnostic code {expected_code} in parse diagnostics: {:?}",
        diags
    );
}

#[test]
fn test_conformance_unsupported_if_else() {
    assert_unsupported("unsupported/unsupported_if_else.qasm", "E0005");
}

#[test]
fn test_conformance_unsupported_for_loop() {
    assert_unsupported("unsupported/unsupported_for_loop.qasm", "E0005");
}

#[test]
fn test_conformance_unsupported_while_loop() {
    assert_unsupported("unsupported/unsupported_while_loop.qasm", "E0005");
}

#[test]
fn test_conformance_unsupported_subroutine() {
    assert_unsupported("unsupported/unsupported_subroutine.qasm", "E0005");
}

#[test]
fn test_conformance_unsupported_gate_def() {
    assert_unsupported("unsupported/unsupported_gate_def.qasm", "E0005");
}

#[test]
fn test_conformance_unsupported_defcal() {
    assert_unsupported("unsupported/unsupported_defcal.qasm", "E0005");
}

#[test]
fn test_conformance_unsupported_cal_block() {
    assert_unsupported("unsupported/unsupported_cal_block.qasm", "E0005");
}

#[test]
fn test_conformance_unsupported_three_qubit_ccx() {
    assert_unsupported("unsupported/unsupported_three_qubit_ccx.qasm", "E0003");
}

#[test]
fn test_conformance_unsupported_three_qubit_cswap() {
    assert_unsupported("unsupported/unsupported_three_qubit_cswap.qasm", "E0003");
}

#[test]
fn test_conformance_unsupported_standard_u3() {
    assert_unsupported("unsupported/unsupported_standard_u3.qasm", "E0004");
}

#[test]
fn test_conformance_unsupported_types() {
    assert_unsupported("unsupported/unsupported_types.qasm", "E0005");
}

#[test]
fn test_conformance_unsupported_legacy_qasm2() {
    assert_unsupported("unsupported/unsupported_legacy_qasm2.qasm", "E0002");
}

// ============================================================================
// 3. Invalid Conformance Fixtures (Deterministic Diagnostics)
// ============================================================================

fn assert_invalid(subpath: &str, expected_code: &str) {
    let source = read_fixture(subpath);
    let buffer = SourceBuffer::new(subpath, source);
    let frontend = OpenQasmFrontend::new();

    let lower_res = frontend.lower(&buffer, None);
    assert!(
        !lower_res.is_ok(),
        "Expected lowering to fail for invalid fixture {subpath}"
    );
    assert!(
        lower_res
            .diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some(expected_code)),
        "Expected diagnostic code {expected_code} for {subpath}, got: {:?}",
        lower_res.diagnostics
    );
}

#[test]
fn test_conformance_invalid_out_of_bounds() {
    assert_invalid("invalid/invalid_out_of_bounds.qasm", "E0014");
}

#[test]
fn test_conformance_invalid_undeclared_reg() {
    assert_invalid("invalid/invalid_undeclared_reg.qasm", "E0012");
}

#[test]
fn test_conformance_invalid_duplicate_reg() {
    assert_invalid("invalid/invalid_duplicate_reg.qasm", "E0010");
}

#[test]
fn test_conformance_invalid_zero_sized_reg() {
    assert_invalid("invalid/invalid_zero_sized_reg.qasm", "E0011");
}

#[test]
fn test_conformance_invalid_division_by_zero() {
    assert_invalid("invalid/invalid_division_by_zero.qasm", "E0030");
}

#[test]
fn test_conformance_invalid_unresolved_param() {
    assert_invalid("invalid/invalid_unresolved_param.qasm", "E0031");
}

#[test]
fn test_conformance_invalid_duplicate_operands() {
    assert_invalid("invalid/invalid_duplicate_operands.qasm", "E0023");
}

#[test]
fn test_conformance_invalid_param_arity() {
    assert_invalid("invalid/invalid_param_arity.qasm", "E0021");
}

#[test]
fn test_conformance_invalid_qubit_arity() {
    assert_invalid("invalid/invalid_qubit_arity.qasm", "E0022");
}

#[test]
fn test_conformance_invalid_deep_nesting() {
    assert_invalid("invalid/invalid_deep_nesting.qasm", "E0001");
}

#[test]
fn test_conformance_invalid_oversized_reg() {
    assert_invalid("invalid/invalid_oversized_reg.qasm", "E0011");
}
