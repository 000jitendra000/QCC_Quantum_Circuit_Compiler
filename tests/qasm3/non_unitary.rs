//! Integration tests for OpenQASM 3 non-unitary statement parsing (Phase 22).
//!
//! Validates:
//! - Single-bit measurement assignment (`c[i] = measure q[j];` and `measure q[j] -> c[i];`)
//! - Whole-register measurement (`c = measure q;` and `measure q -> c;`)
//! - Correct quantum source and classical destination in AST
//! - Invalid measurement assignments and register-kind mismatches
//! - Reset of single qubits and whole registers (verified as non-unitary)
//! - Global and selective barriers (single qubits, multiple qubits, whole registers)
//! - Barrier operand preservation and rejection of classical bits
//! - Malformed statements, missing operands, and missing semicolons
//! - Source span correctness for non-unitary statements and operands
//! - Preserved source statement ordering across unitary and non-unitary constructs
//! - Regression coverage for Phases 19–21

use qcc::frontend::ast::{AstExprKind, AstStatement};
use qcc::frontend::source::SourceBuffer;
use qcc::frontend::{Frontend, OpenQasmFrontend};

/// Helper to parse source text and ensure success.
fn parse_success(src: &str) -> qcc::frontend::ast::AstProgram {
    let buffer = SourceBuffer::new("test.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);
    assert!(
        result.is_ok(),
        "Expected parse success, got errors: {:?}",
        result.diagnostics
    );
    result.into_result().unwrap()
}

/// Helper to parse source text and return diagnostics.
fn parse_errors(src: &str) -> Vec<qcc::diagnostics::Diagnostic> {
    let buffer = SourceBuffer::new("test.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);
    assert!(!result.is_ok(), "Expected parse failure, but it succeeded");
    result.diagnostics
}

// ============================================================================
// 1. Single-Qubit Measurement Assignment
// ============================================================================

#[test]
fn test_single_qubit_measurement_assignment() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;

c[0] = measure q[0];
c[1] = measure q[1];
"#;

    let program = parse_success(src);
    assert_eq!(program.statements.len(), 4); // 2 decls + 2 measures

    // Inspect first measurement
    match &program.statements[2] {
        AstStatement::Measure(m) => {
            assert_eq!(m.qubit.name, "q");
            assert_eq!(m.qubit.index, Some(0));
            assert_eq!(m.bit.name, "c");
            assert_eq!(m.bit.index, Some(0));
            assert_eq!(m.qubit.to_string(), "q[0]");
            assert_eq!(m.bit.to_string(), "c[0]");
        }
        other => panic!("Expected Measure statement, got {:?}", other),
    }

    // Inspect second measurement
    match &program.statements[3] {
        AstStatement::Measure(m) => {
            assert_eq!(m.qubit.name, "q");
            assert_eq!(m.qubit.index, Some(1));
            assert_eq!(m.bit.name, "c");
            assert_eq!(m.bit.index, Some(1));
        }
        other => panic!("Expected Measure statement, got {:?}", other),
    }

    // Arrow syntax: measure q[0] -> c[0];
    let arrow_src = r#"OPENQASM 3.0;
qubit[1] q;
bit[1] c;

measure q[0] -> c[0];
"#;
    let arrow_program = parse_success(arrow_src);
    match &arrow_program.statements[2] {
        AstStatement::Measure(m) => {
            assert_eq!(m.qubit.name, "q");
            assert_eq!(m.qubit.index, Some(0));
            assert_eq!(m.bit.name, "c");
            assert_eq!(m.bit.index, Some(0));
        }
        other => panic!("Expected Measure statement, got {:?}", other),
    }
}

// ============================================================================
// 2. Whole-Register Measurement
// ============================================================================

#[test]
fn test_whole_register_measurement() {
    // Assignment form: c = measure q;
    let src = r#"OPENQASM 3.0;
qubit[4] q;
bit[4] c;

c = measure q;
"#;
    let program = parse_success(src);
    assert_eq!(program.statements.len(), 3);

    match &program.statements[2] {
        AstStatement::Measure(m) => {
            assert_eq!(m.qubit.name, "q");
            assert_eq!(m.qubit.index, None);
            assert!(m.qubit.is_scalar());
            assert_eq!(m.bit.name, "c");
            assert_eq!(m.bit.index, None);
            assert!(m.bit.is_scalar());
        }
        other => panic!("Expected Measure, got {:?}", other),
    }

    // Arrow form: measure q -> c;
    let arrow_src = r#"OPENQASM 3.0;
qubit[4] q;
bit[4] c;

measure q -> c;
"#;
    let arrow_program = parse_success(arrow_src);
    match &arrow_program.statements[2] {
        AstStatement::Measure(m) => {
            assert_eq!(m.qubit.name, "q");
            assert_eq!(m.qubit.index, None);
            assert_eq!(m.bit.name, "c");
            assert_eq!(m.bit.index, None);
        }
        other => panic!("Expected Measure, got {:?}", other),
    }

    // Scalar declarations: qubit q; bit c; c = measure q;
    let scalar_src = r#"OPENQASM 3.0;
qubit q;
bit c;

c = measure q;
"#;
    let scalar_prog = parse_success(scalar_src);
    match &scalar_prog.statements[2] {
        AstStatement::Measure(m) => {
            assert_eq!(m.qubit.name, "q");
            assert_eq!(m.bit.name, "c");
        }
        other => panic!("Expected Measure, got {:?}", other),
    }
}

// ============================================================================
// 3. Quantum Source and Classical Destination AST Distinction
// ============================================================================

#[test]
fn test_measurement_source_and_destination_preservation() {
    let src = r#"OPENQASM 3.0;
qubit[3] q_reg;
bit[3] c_reg;

c_reg[2] = measure q_reg[1];
"#;
    let program = parse_success(src);
    match &program.statements[2] {
        AstStatement::Measure(m) => {
            // Quantum operand is strictly the source
            assert_eq!(m.qubit.name, "q_reg");
            assert_eq!(m.qubit.index, Some(1));
            // Classical operand is strictly the target destination
            assert_eq!(m.bit.name, "c_reg");
            assert_eq!(m.bit.index, Some(2));
        }
        other => panic!("Expected Measure, got {:?}", other),
    }
}

// ============================================================================
// 4. Invalid Measurement Assignments and Register Mismatches
// ============================================================================

#[test]
fn test_invalid_measurement_and_type_mismatches() {
    // 1. Measuring into a quantum register: q[0] = measure q[1];
    let diags1 = parse_errors(
        r#"OPENQASM 3.0;
qubit[2] q;
q[0] = measure q[1];
"#,
    );
    assert!(diags1.iter().any(|d| d.code.as_deref() == Some("E0013")
        && d.message.contains("was used where a bit was expected")));

    // 2. Measuring from a classical bit: c[0] = measure c[1];
    let diags2 = parse_errors(
        r#"OPENQASM 3.0;
bit[2] c;
c[0] = measure c[1];
"#,
    );
    assert!(diags2.iter().any(|d| d.code.as_deref() == Some("E0013")
        && d.message.contains("was used where a qubit was expected")));

    // 3. Arrow measurement type mismatch: measure c[0] -> q[0];
    let diags3 = parse_errors(
        r#"OPENQASM 3.0;
qubit[1] q;
bit[1] c;
measure c[0] -> q[0];
"#,
    );
    assert!(diags3.iter().any(|d| d.code.as_deref() == Some("E0013")));

    // 4. Undeclared register in measurement
    let diags4 = parse_errors(
        r#"OPENQASM 3.0;
qubit[1] q;
c[0] = measure q[0];
"#,
    );
    assert!(diags4.iter().any(|d| d.code.as_deref() == Some("E0012")
        && d.message.contains("use of undeclared register 'c'")));

    // 5. Out of bounds index in measurement
    let diags5 = parse_errors(
        r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;
c[2] = measure q[0];
"#,
    );
    assert!(diags5
        .iter()
        .any(|d| d.code.as_deref() == Some("E0014") && d.message.contains("out of bounds")));

    // 6. Whole-register measurement size mismatch: qubit[2] vs bit[3]
    let diags6 = parse_errors(
        r#"OPENQASM 3.0;
qubit[2] q;
bit[3] c;
c = measure q;
"#,
    );
    assert!(diags6.iter().any(|d| d.code.as_deref() == Some("E0025")
        && d.message.contains("size mismatch")
        && d.message.contains("has size 2")
        && d.message.contains("has size 3")));

    // 7. Unindexed array measured into single bit
    let diags7 = parse_errors(
        r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;
c[0] = measure q;
"#,
    );
    assert!(diags7
        .iter()
        .any(|d| d.code.as_deref() == Some("E0016") && d.message.contains("must be indexed")));

    // 8. Single qubit measured into unindexed array
    let diags8 = parse_errors(
        r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;
c = measure q[0];
"#,
    );
    assert!(diags8
        .iter()
        .any(|d| d.code.as_deref() == Some("E0016") && d.message.contains("must be indexed")));

    // 9. Assignment without 'measure' keyword: c[0] = q[0];
    let diags9 = parse_errors(
        r#"OPENQASM 3.0;
qubit[1] q;
bit[1] c;
c[0] = q[0];
"#,
    );
    assert!(diags9
        .iter()
        .any(|d| d.code.as_deref() == Some("E0001")
            && d.message.contains("expected 'measure' keyword")));
}

// ============================================================================
// 5. Reset of Valid Qubit Operands
// ============================================================================

#[test]
fn test_reset_statements() {
    // 1. Reset single indexed qubit: reset q[0];
    let src1 = r#"OPENQASM 3.0;
qubit[2] q;
reset q[0];
reset q[1];
"#;
    let program1 = parse_success(src1);
    assert_eq!(program1.statements.len(), 3); // 1 decl + 2 resets

    // Verify parsed as non-unitary AstReset (never a gate call)
    match &program1.statements[1] {
        AstStatement::Reset(r) => {
            assert_eq!(r.qubit.name, "q");
            assert_eq!(r.qubit.index, Some(0));
            assert_eq!(r.qubit.to_string(), "q[0]");
        }
        other => panic!("Expected Reset statement, got {:?}", other),
    }
    assert!(program1.statements[1].is_reset());
    assert!(!program1.statements[1].is_gate_call());

    // 2. Whole-register reset: reset q;
    let src2 = r#"OPENQASM 3.0;
qubit[4] q;
reset q;
"#;
    let program2 = parse_success(src2);
    match &program2.statements[1] {
        AstStatement::Reset(r) => {
            assert_eq!(r.qubit.name, "q");
            assert_eq!(r.qubit.index, None);
            assert!(r.qubit.is_scalar());
        }
        other => panic!("Expected Reset statement, got {:?}", other),
    }

    // 3. Reset of scalar qubit declaration: qubit q; reset q;
    let src3 = r#"OPENQASM 3.0;
qubit q;
reset q;
"#;
    let program3 = parse_success(src3);
    match &program3.statements[1] {
        AstStatement::Reset(r) => {
            assert_eq!(r.qubit.name, "q");
            assert_eq!(r.qubit.index, None);
        }
        other => panic!("Expected Reset statement, got {:?}", other),
    }

    // 4. Rejection of classical bit in reset: reset c[0];
    let diags_classical = parse_errors(
        r#"OPENQASM 3.0;
bit[2] c;
reset c[0];
"#,
    );
    assert!(diags_classical
        .iter()
        .any(|d| d.code.as_deref() == Some("E0013")
            && d.message.contains("was used where a qubit was expected")));

    // 5. Rejection of undeclared register in reset
    let diags_und = parse_errors(
        r#"OPENQASM 3.0;
reset unk[0];
"#,
    );
    assert!(diags_und.iter().any(|d| d.code.as_deref() == Some("E0012")));

    // 6. Rejection of out of bounds index in reset
    let diags_oob = parse_errors(
        r#"OPENQASM 3.0;
qubit[2] q;
reset q[5];
"#,
    );
    assert!(diags_oob.iter().any(|d| d.code.as_deref() == Some("E0014")));
}

// ============================================================================
// 6. Global and Selective Barriers
// ============================================================================

#[test]
fn test_barrier_statements() {
    // 1. Global barrier: barrier;
    let src1 = r#"OPENQASM 3.0;
qubit[2] q;
barrier;
"#;
    let program1 = parse_success(src1);
    match &program1.statements[1] {
        AstStatement::Barrier(b) => {
            assert!(b.is_global());
            assert!(b.qubits.is_empty());
        }
        other => panic!("Expected Barrier, got {:?}", other),
    }

    // 2. Selective barrier over indexed qubits: barrier q[0], q[1];
    let src2 = r#"OPENQASM 3.0;
qubit[3] q;
barrier q[0], q[2];
"#;
    let program2 = parse_success(src2);
    match &program2.statements[1] {
        AstStatement::Barrier(b) => {
            assert!(!b.is_global());
            assert_eq!(b.qubits.len(), 2);
            assert_eq!(b.qubits[0].name, "q");
            assert_eq!(b.qubits[0].index, Some(0));
            assert_eq!(b.qubits[1].name, "q");
            assert_eq!(b.qubits[1].index, Some(2));
        }
        other => panic!("Expected Barrier, got {:?}", other),
    }

    // 3. Selective barrier over register: barrier q;
    let src3 = r#"OPENQASM 3.0;
qubit[4] q;
barrier q;
"#;
    let program3 = parse_success(src3);
    match &program3.statements[1] {
        AstStatement::Barrier(b) => {
            assert!(!b.is_global());
            assert_eq!(b.qubits.len(), 1);
            assert_eq!(b.qubits[0].name, "q");
            assert_eq!(b.qubits[0].index, None);
        }
        other => panic!("Expected Barrier, got {:?}", other),
    }

    // 4. Multi-register and mixed barrier: barrier q, r[0];
    let src4 = r#"OPENQASM 3.0;
qubit[2] q;
qubit[2] r;
barrier q, r[0];
"#;
    let program4 = parse_success(src4);
    match &program4.statements[2] {
        AstStatement::Barrier(b) => {
            assert_eq!(b.qubits.len(), 2);
            assert_eq!(b.qubits[0].name, "q");
            assert_eq!(b.qubits[0].index, None);
            assert_eq!(b.qubits[1].name, "r");
            assert_eq!(b.qubits[1].index, Some(0));
        }
        other => panic!("Expected Barrier, got {:?}", other),
    }
}

// ============================================================================
// 7. Barrier Operand Validation and Rejections
// ============================================================================

#[test]
fn test_barrier_validation_and_rejections() {
    // 1. Classical bit in barrier: barrier c;
    let diags1 = parse_errors(
        r#"OPENQASM 3.0;
bit[2] c;
barrier c;
"#,
    );
    assert!(diags1.iter().any(|d| d.code.as_deref() == Some("E0013")
        && d.message.contains("was used where a qubit was expected")));

    // 2. Mixed quantum and classical: barrier q[0], c[1];
    let diags2 = parse_errors(
        r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;
barrier q[0], c[1];
"#,
    );
    assert!(diags2.iter().any(|d| d.code.as_deref() == Some("E0013")));

    // 3. Trailing comma in barrier: barrier q[0], ;
    let diags3 = parse_errors(
        r#"OPENQASM 3.0;
qubit[2] q;
barrier q[0], ;
"#,
    );
    assert!(diags3
        .iter()
        .any(|d| d.code.as_deref() == Some("E0001") && d.message.contains("trailing comma")));

    // 4. Undeclared register in barrier: barrier foo;
    let diags4 = parse_errors(
        r#"OPENQASM 3.0;
barrier foo;
"#,
    );
    assert!(diags4.iter().any(|d| d.code.as_deref() == Some("E0012")));

    // 5. Index out of bounds in barrier: barrier q[10];
    let diags5 = parse_errors(
        r#"OPENQASM 3.0;
qubit[2] q;
barrier q[10];
"#,
    );
    assert!(diags5.iter().any(|d| d.code.as_deref() == Some("E0014")));
}

// ============================================================================
// 8. Malformed Statements and Syntax Errors
// ============================================================================

#[test]
fn test_malformed_statements() {
    // 1. Reset missing operand: reset;
    let diags1 = parse_errors(
        r#"OPENQASM 3.0;
qubit[1] q;
reset;
"#,
    );
    assert!(diags1.iter().any(|d| d.code.as_deref() == Some("E0001")));

    // 2. Reset missing semicolon: reset q[0]
    let diags2 = parse_errors(
        r#"OPENQASM 3.0;
qubit[1] q;
reset q[0]
"#,
    );
    assert!(diags2
        .iter()
        .any(|d| d.code.as_deref() == Some("E0001") && d.message.contains("expected ';'")));

    // 3. Measurement missing RHS: c[0] = ;
    let diags3 = parse_errors(
        r#"OPENQASM 3.0;
qubit[1] q;
bit[1] c;
c[0] = ;
"#,
    );
    assert!(diags3.iter().any(|d| d.code.as_deref() == Some("E0001")));

    // 4. Measurement missing qubit: c[0] = measure;
    let diags4 = parse_errors(
        r#"OPENQASM 3.0;
qubit[1] q;
bit[1] c;
c[0] = measure;
"#,
    );
    assert!(diags4.iter().any(|d| d.code.as_deref() == Some("E0001")));

    // 5. Measurement missing semicolon: c[0] = measure q[0]
    let diags5 = parse_errors(
        r#"OPENQASM 3.0;
qubit[1] q;
bit[1] c;
c[0] = measure q[0]
"#,
    );
    assert!(diags5
        .iter()
        .any(|d| d.code.as_deref() == Some("E0001") && d.message.contains("expected ';'")));

    // 6. Arrow measurement missing arrow: measure q[0] c[0];
    let diags6 = parse_errors(
        r#"OPENQASM 3.0;
qubit[1] q;
bit[1] c;
measure q[0] c[0];
"#,
    );
    assert!(diags6
        .iter()
        .any(|d| d.code.as_deref() == Some("E0001") && d.message.contains("expected '->'")));

    // 7. Arrow measurement missing destination: measure q[0] -> ;
    let diags7 = parse_errors(
        r#"OPENQASM 3.0;
qubit[1] q;
bit[1] c;
measure q[0] -> ;
"#,
    );
    assert!(diags7.iter().any(|d| d.code.as_deref() == Some("E0001")));

    // 8. Barrier missing semicolon: barrier q[0]
    let diags8 = parse_errors(
        r#"OPENQASM 3.0;
qubit[1] q;
barrier q[0]
"#,
    );
    assert!(diags8
        .iter()
        .any(|d| d.code.as_deref() == Some("E0001") && d.message.contains("expected ';'")));
}

// ============================================================================
// 9. Source Span Correctness
// ============================================================================

#[test]
fn test_source_span_correctness() {
    let src = "OPENQASM 3.0;\nqubit[2] q;\nbit[2] c;\nreset q[0];\nbarrier q[0], q[1];\nc[0] = measure q[0];\n";
    let program = parse_success(src);

    // reset q[0]; on line 4
    let reset_stmt = &program.statements[2];
    assert_eq!(reset_stmt.span().start_line(), 4);
    match reset_stmt {
        AstStatement::Reset(r) => {
            assert_eq!(r.qubit.span.start_line(), 4);
        }
        _ => unreachable!(),
    }

    // barrier q[0], q[1]; on line 5
    let barrier_stmt = &program.statements[3];
    assert_eq!(barrier_stmt.span().start_line(), 5);
    match barrier_stmt {
        AstStatement::Barrier(b) => {
            assert_eq!(b.qubits[0].span.start_line(), 5);
            assert_eq!(b.qubits[1].span.start_line(), 5);
        }
        _ => unreachable!(),
    }

    // c[0] = measure q[0]; on line 6
    let measure_stmt = &program.statements[4];
    assert_eq!(measure_stmt.span().start_line(), 6);
    match measure_stmt {
        AstStatement::Measure(m) => {
            assert_eq!(m.bit.span.start_line(), 6);
            assert_eq!(m.qubit.span.start_line(), 6);
        }
        _ => unreachable!(),
    }
}

// ============================================================================
// 10. Multiple Non-Unitary Statements Preserving Exact Program Order
// ============================================================================

#[test]
fn test_multiple_non_unitary_statements_order_preservation() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;

h q[0];
cx q[0], q[1];
barrier;
c[0] = measure q[0];
reset q[0];
h q[0];
barrier q[0], q[1];
measure q[1] -> c[1];
"#;

    let program = parse_success(src);
    // 2 decls + 8 statements = 10 statements
    assert_eq!(program.statements.len(), 10);

    let types: Vec<&str> = program
        .statements
        .iter()
        .map(|s| match s {
            AstStatement::RegisterDecl(_) => "decl",
            AstStatement::GateCall(g) => g.name.as_str(),
            AstStatement::Barrier(_) => "barrier",
            AstStatement::Measure(_) => "measure",
            AstStatement::Reset(_) => "reset",
            AstStatement::GlobalPhase(_) => "gphase",
            AstStatement::Unsupported(_) => "unsupported",
        })
        .collect();

    assert_eq!(
        types,
        vec!["decl", "decl", "h", "cx", "barrier", "measure", "reset", "h", "barrier", "measure"]
    );
}

// ============================================================================
// 11. Regression: Interoperation with Phases 19, 20, and 21
// ============================================================================

#[test]
fn test_regression_with_phases_19_20_21() {
    let src = r#"OPENQASM 3.0;
include "stdgates.inc";

qubit[3] q;
bit[3] c;

// Parameterized rotation with mathematical constant pi and expressions
rx(pi / 4) q[0];
ry((theta + π) / 2) q[1];
rz(-theta) q[2];

// Unitary gates
cx q[0], q[1];
swap q[1], q[2];

// Non-unitary barrier, measurement, and reset
barrier q;
c[0] = measure q[0];
reset q[0];
c[1] = measure q[1];
measure q[2] -> c[2];
"#;

    let buffer = SourceBuffer::new("regression.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let (res, table) = frontend.parse_with_registers(&buffer);

    assert!(
        res.is_ok(),
        "Regression parse failed: {:?}",
        res.diagnostics
    );
    let program = res.into_result().unwrap();

    // Headers & Registers
    assert_eq!(program.version.as_ref().unwrap().specifier, "3.0");
    assert_eq!(program.includes.len(), 1);
    assert_eq!(table.total_qubits(), 3);
    assert_eq!(table.total_bits(), 3);

    // Statements count: 2 decls + 5 gates + 1 barrier + 4 non-unitary ops = 12 statements
    assert_eq!(program.statements.len(), 12);

    // Verify first parameterized gate preserved expression AST
    match &program.statements[2] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.name, "rx");
            match &call.parameters[0].kind {
                AstExprKind::Div(l, r) => {
                    assert_eq!(l.as_identifier(), Some("pi"));
                    assert_eq!(r.as_constant(), Some(4.0));
                }
                other => panic!("Expected Div(pi, 4), got {:?}", other),
            }
        }
        other => panic!("Expected GateCall, got {:?}", other),
    }

    // Verify barrier over whole register
    match &program.statements[7] {
        AstStatement::Barrier(b) => {
            assert_eq!(b.qubits.len(), 1);
            assert_eq!(b.qubits[0].name, "q");
            assert!(b.qubits[0].is_scalar());
        }
        other => panic!("Expected Barrier, got {:?}", other),
    }

    // Verify reset
    match &program.statements[9] {
        AstStatement::Reset(r) => {
            assert_eq!(r.qubit.name, "q");
            assert_eq!(r.qubit.index, Some(0));
        }
        other => panic!("Expected Reset, got {:?}", other),
    }

    // Verify arrow measure
    match &program.statements[11] {
        AstStatement::Measure(m) => {
            assert_eq!(m.qubit.name, "q");
            assert_eq!(m.qubit.index, Some(2));
            assert_eq!(m.bit.name, "c");
            assert_eq!(m.bit.index, Some(2));
        }
        other => panic!("Expected Measure, got {:?}", other),
    }
}
