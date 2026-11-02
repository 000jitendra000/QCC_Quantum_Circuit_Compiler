//! Integration tests for OpenQASM 3 standard gate statement parsing (Phase 20).
//!
//! Validates gate identity, parameter expression preservation, operand order,
//! case sensitivity, arity checks, and error diagnostics.

use qcc::frontend::ast::{AstExprKind, AstStatement};
use qcc::frontend::source::SourceBuffer;
use qcc::frontend::{Frontend, OpenQasmFrontend};

// ============================================================================
// 1. Parsing Each Supported Gate
// ============================================================================

#[test]
fn test_parse_all_supported_gates() {
    let src = r#"OPENQASM 3.0;
include "stdgates.inc";

qubit[3] q;

// Single-qubit fixed gates
x q[0];
y q[0];
z q[0];
h q[0];
s q[0];
sdg q[0];
t q[0];
tdg q[0];

// Single-qubit parameterized rotations
rx(0.5) q[0];
ry(1.0) q[1];
rz(1.5) q[2];

// Two-qubit entangling gates
cx q[0], q[1];
cz q[0], q[1];
swap q[1], q[2];
"#;

    let buffer = SourceBuffer::new("all_gates.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(
        result.is_ok(),
        "Expected all supported MVP gates to parse without error: {:?}",
        result.diagnostics
    );

    let program = result.into_result().unwrap();
    // 1 register declaration + 14 gate statements = 15 statements
    assert_eq!(program.statements.len(), 15);

    let expected_names = [
        "x", "y", "z", "h", "s", "sdg", "t", "tdg", "rx", "ry", "rz", "cx", "cz", "swap",
    ];

    for (i, &name) in expected_names.iter().enumerate() {
        match &program.statements[i + 1] {
            AstStatement::GateCall(call) => {
                assert_eq!(
                    call.name,
                    name,
                    "Statement {} expected gate {}",
                    i + 1,
                    name
                );
            }
            other => panic!("Expected GateCall for statement {}, got {:?}", i + 1, other),
        }
    }
}

// ============================================================================
// 2. Parameterized Forms and Expression Preservation
// ============================================================================

#[test]
fn test_parameterized_gate_expression_preservation() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;

rx(theta) q[0];
ry(0.5 * pi) q[0];
rz(-pi / 4.0) q[1];
"#;

    let buffer = SourceBuffer::new("params.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(result.is_ok(), "Failed to parse: {:?}", result.diagnostics);
    let program = result.into_result().unwrap();

    // Statement 1: rx(theta) q[0];
    match &program.statements[1] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.name, "rx");
            assert_eq!(call.parameters.len(), 1);
            match &call.parameters[0].kind {
                AstExprKind::Identifier(id) => assert_eq!(id, "theta"),
                other => panic!("Expected Identifier(theta), got {:?}", other),
            }
        }
        other => panic!("Unexpected statement: {:?}", other),
    }

    // Statement 2: ry(0.5 * pi) q[0];
    match &program.statements[2] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.name, "ry");
            assert_eq!(call.parameters.len(), 1);
            match &call.parameters[0].kind {
                AstExprKind::Mul(lhs, rhs) => {
                    assert_eq!(lhs.as_constant(), Some(0.5));
                    assert_eq!(rhs.as_identifier(), Some("pi"));
                }
                other => panic!("Expected Mul, got {:?}", other),
            }
        }
        other => panic!("Unexpected statement: {:?}", other),
    }

    // Statement 3: rz(-pi / 4.0) q[1];
    match &program.statements[3] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.name, "rz");
            assert_eq!(call.parameters.len(), 1);
            match &call.parameters[0].kind {
                AstExprKind::Div(lhs, rhs) => {
                    match &lhs.kind {
                        AstExprKind::Neg(inner) => {
                            assert_eq!(inner.as_identifier(), Some("pi"));
                        }
                        other => panic!("Expected Neg(pi), got {:?}", other),
                    }
                    assert_eq!(rhs.as_constant(), Some(4.0));
                }
                other => panic!("Expected Div, got {:?}", other),
            }
        }
        other => panic!("Unexpected statement: {:?}", other),
    }
}

// ============================================================================
// 3. Single-Qubit and Two-Qubit Operand Handling
// ============================================================================

#[test]
fn test_qubit_operand_handling_scalar_and_indexed() {
    let src = r#"OPENQASM 3.0;
qubit q_scalar;
qubit[3] q_arr;

h q_scalar;
h q_arr[1];
cx q_scalar, q_arr[2];
"#;

    let buffer = SourceBuffer::new("operands.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(result.is_ok(), "Failed to parse: {:?}", result.diagnostics);
    let program = result.into_result().unwrap();

    // h q_scalar;
    match &program.statements[2] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.qubits.len(), 1);
            assert_eq!(call.qubits[0].name, "q_scalar");
            assert_eq!(call.qubits[0].index, None);
        }
        other => panic!("Unexpected: {:?}", other),
    }

    // h q_arr[1];
    match &program.statements[3] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.qubits.len(), 1);
            assert_eq!(call.qubits[0].name, "q_arr");
            assert_eq!(call.qubits[0].index, Some(1));
        }
        other => panic!("Unexpected: {:?}", other),
    }

    // cx q_scalar, q_arr[2];
    match &program.statements[4] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.qubits.len(), 2);
            assert_eq!(call.qubits[0].name, "q_scalar");
            assert_eq!(call.qubits[0].index, None);
            assert_eq!(call.qubits[1].name, "q_arr");
            assert_eq!(call.qubits[1].index, Some(2));
        }
        other => panic!("Unexpected: {:?}", other),
    }
}

// ============================================================================
// 4. CX Control/Target Order Invariant
// ============================================================================

#[test]
fn test_cx_operand_order_preservation() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;

cx q[0], q[1];
cx q[1], q[0];
"#;

    let buffer = SourceBuffer::new("cx_order.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(result.is_ok(), "Failed to parse: {:?}", result.diagnostics);
    let program = result.into_result().unwrap();

    // Forward CX: operand 0 is control (q[0]), operand 1 is target (q[1])
    match &program.statements[1] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.name, "cx");
            assert_eq!(call.qubits[0].to_string(), "q[0]"); // Control
            assert_eq!(call.qubits[1].to_string(), "q[1]"); // Target
        }
        other => panic!("Unexpected statement: {:?}", other),
    }

    // Inverted CX: operand 0 is control (q[1]), operand 1 is target (q[0])
    match &program.statements[2] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.name, "cx");
            assert_eq!(call.qubits[0].to_string(), "q[1]"); // Control
            assert_eq!(call.qubits[1].to_string(), "q[0]"); // Target
        }
        other => panic!("Unexpected statement: {:?}", other),
    }
}

// ============================================================================
// 5. SWAP and CZ Operand Order Preservation
// ============================================================================

#[test]
fn test_swap_and_cz_operand_order_preservation() {
    let src = r#"OPENQASM 3.0;
qubit[3] q;

cz q[0], q[1];
cz q[1], q[0];
swap q[0], q[2];
swap q[2], q[0];
"#;

    let buffer = SourceBuffer::new("swap_cz_order.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(result.is_ok(), "Failed to parse: {:?}", result.diagnostics);
    let program = result.into_result().unwrap();

    match &program.statements[1] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.name, "cz");
            assert_eq!(call.qubits[0].to_string(), "q[0]");
            assert_eq!(call.qubits[1].to_string(), "q[1]");
        }
        other => panic!("Unexpected statement: {:?}", other),
    }

    match &program.statements[2] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.name, "cz");
            assert_eq!(call.qubits[0].to_string(), "q[1]");
            assert_eq!(call.qubits[1].to_string(), "q[0]");
        }
        other => panic!("Unexpected statement: {:?}", other),
    }

    match &program.statements[3] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.name, "swap");
            assert_eq!(call.qubits[0].to_string(), "q[0]");
            assert_eq!(call.qubits[1].to_string(), "q[2]");
        }
        other => panic!("Unexpected statement: {:?}", other),
    }

    match &program.statements[4] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.name, "swap");
            assert_eq!(call.qubits[0].to_string(), "q[2]");
            assert_eq!(call.qubits[1].to_string(), "q[0]");
        }
        other => panic!("Unexpected statement: {:?}", other),
    }
}

// ============================================================================
// 6. Unknown Gate Names
// ============================================================================

#[test]
fn test_unknown_gate_rejected() {
    let src = "OPENQASM 3.0;\nqubit[1] q;\nfoo q[0];\n";
    let buffer = SourceBuffer::new("unknown.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(
        !result.is_ok(),
        "Expected unknown gate 'foo' to be rejected"
    );
    let codes: Vec<Option<&str>> = result.errors().map(|d| d.code.as_deref()).collect();
    assert!(
        codes.contains(&Some("E0020")),
        "Expected E0020 diagnostic code for unknown gate"
    );

    let msg = result.errors().next().unwrap().message.clone();
    assert!(msg.contains("unknown or unsupported gate 'foo'"));
}

// ============================================================================
// 7. Case Sensitivity and Capitalization
// ============================================================================

#[test]
fn test_incorrect_capitalization_rejected_with_suggestion() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;

H q[0];
CX q[0], q[1];
Rx(0.5) q[0];
SWAP q[0], q[1];
"#;

    let buffer = SourceBuffer::new("case.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(!result.is_ok(), "Capitalized gate names must be rejected");
    let errors: Vec<_> = result.errors().collect();
    assert_eq!(errors.len(), 4);

    for err in &errors {
        assert_eq!(err.code.as_deref(), Some("E0020"));
        assert!(
            err.notes.iter().any(|n| n.contains("case-sensitive")),
            "Expected note regarding case sensitivity in: {:?}",
            err.notes
        );
    }

    assert!(errors[0]
        .notes
        .iter()
        .any(|n| n.contains("did you mean 'h'?")));
    assert!(errors[1]
        .notes
        .iter()
        .any(|n| n.contains("did you mean 'cx'?")));
    assert!(errors[2]
        .notes
        .iter()
        .any(|n| n.contains("did you mean 'rx'?")));
    assert!(errors[3]
        .notes
        .iter()
        .any(|n| n.contains("did you mean 'swap'?")));
}

// ============================================================================
// 8. Qubit Operand Arity Violations
// ============================================================================

#[test]
fn test_qubit_operand_arity_too_few_and_too_many() {
    // 1. Single-qubit gate with 0 operands
    let src1 = "OPENQASM 3.0;\nqubit[1] q;\nh;\n";
    let res1 = OpenQasmFrontend::new().parse(&SourceBuffer::new("too_few1.qasm", src1));
    assert!(!res1.is_ok());
    assert!(res1.errors().any(|e| e.code.as_deref() == Some("E0022")));
    assert!(res1.errors().any(|e| e
        .message
        .contains("takes 1 qubit operand(s), but 0 provided")));

    // 2. Single-qubit gate with 2 operands
    let src2 = "OPENQASM 3.0;\nqubit[2] q;\nh q[0], q[1];\n";
    let res2 = OpenQasmFrontend::new().parse(&SourceBuffer::new("too_many1.qasm", src2));
    assert!(!res2.is_ok());
    assert!(res2.errors().any(|e| e.code.as_deref() == Some("E0022")));
    assert!(res2.errors().any(|e| e
        .message
        .contains("takes 1 qubit operand(s), but 2 provided")));

    // 3. Two-qubit gate with 1 operand
    let src3 = "OPENQASM 3.0;\nqubit[2] q;\ncx q[0];\n";
    let res3 = OpenQasmFrontend::new().parse(&SourceBuffer::new("too_few2.qasm", src3));
    assert!(!res3.is_ok());
    assert!(res3.errors().any(|e| e.code.as_deref() == Some("E0022")));
    assert!(res3.errors().any(|e| e
        .message
        .contains("takes 2 qubit operand(s), but 1 provided")));

    // 4. Two-qubit gate with 3 operands
    let src4 = "OPENQASM 3.0;\nqubit[3] q;\nswap q[0], q[1], q[2];\n";
    let res4 = OpenQasmFrontend::new().parse(&SourceBuffer::new("too_many2.qasm", src4));
    assert!(!res4.is_ok());
    assert!(res4.errors().any(|e| e.code.as_deref() == Some("E0022")));
    assert!(res4.errors().any(|e| e
        .message
        .contains("takes 2 qubit operand(s), but 3 provided")));
}

// ============================================================================
// 9. Parameter Arity and Malformed Parameter Lists
// ============================================================================

#[test]
fn test_parameter_arity_and_malformed_lists() {
    // 1. Parameterized gate missing parameter list completely: rx q[0];
    let src1 = "OPENQASM 3.0;\nqubit[1] q;\nrx q[0];\n";
    let res1 = OpenQasmFrontend::new().parse(&SourceBuffer::new("missing_param.qasm", src1));
    assert!(!res1.is_ok());
    assert!(res1.errors().any(|e| e.code.as_deref() == Some("E0021")));
    assert!(res1.errors().any(|e| e
        .message
        .contains("requires 1 parameter(s), but none was provided")));

    // 2. Parameterized gate with empty parameter list: rx() q[0];
    let src2 = "OPENQASM 3.0;\nqubit[1] q;\nrx() q[0];\n";
    let res2 = OpenQasmFrontend::new().parse(&SourceBuffer::new("empty_param.qasm", src2));
    assert!(!res2.is_ok());
    assert!(res2.errors().any(|e| e.code.as_deref() == Some("E0021")));
    assert!(res2
        .errors()
        .any(|e| e.message.contains("takes 1 parameter(s), but 0 provided")));

    // 3. Parameterized gate with too many parameters: rx(1.0, 2.0) q[0];
    let src3 = "OPENQASM 3.0;\nqubit[1] q;\nrx(1.0, 2.0) q[0];\n";
    let res3 = OpenQasmFrontend::new().parse(&SourceBuffer::new("extra_param.qasm", src3));
    assert!(!res3.is_ok());
    assert!(res3.errors().any(|e| e.code.as_deref() == Some("E0021")));
    assert!(res3
        .errors()
        .any(|e| e.message.contains("takes 1 parameter(s), but 2 provided")));

    // 4. Non-parameterized gate given parameters: h(1.0) q[0];
    let src4 = "OPENQASM 3.0;\nqubit[1] q;\nh(1.0) q[0];\n";
    let res4 = OpenQasmFrontend::new().parse(&SourceBuffer::new("unneeded_param.qasm", src4));
    assert!(!res4.is_ok());
    assert!(res4.errors().any(|e| e.code.as_deref() == Some("E0021")));
    assert!(res4.errors().any(|e| e
        .message
        .contains("takes 0 parameters, but 1 parameter(s) provided")));

    // 5. Malformed parameter list missing closing paren: rx(0.5 q[0];
    let src5 = "OPENQASM 3.0;\nqubit[1] q;\nrx(0.5 q[0];\n";
    let res5 = OpenQasmFrontend::new().parse(&SourceBuffer::new("unclosed_param.qasm", src5));
    assert!(!res5.is_ok());
    assert!(res5.errors().any(|e| e.code.as_deref() == Some("E0001")));
    assert!(res5
        .errors()
        .any(|e| e.message.contains("expected ')' after gate parameters")));
}

// ============================================================================
// 10. Register Resolution and Bounds Verification in Gate Statements
// ============================================================================

#[test]
fn test_gate_operand_register_resolution_and_bounds() {
    // 1. Undeclared register
    let src1 = "OPENQASM 3.0;\nqubit[2] q;\nh unknown_reg[0];\n";
    let res1 = OpenQasmFrontend::new().parse(&SourceBuffer::new("undeclared.qasm", src1));
    assert!(!res1.is_ok());
    assert!(res1.errors().any(|e| e.code.as_deref() == Some("E0012")));

    // 2. Index out of bounds (exact size bound check: size is 2, index 2 is invalid)
    let src2 = "OPENQASM 3.0;\nqubit[2] q;\ncx q[0], q[2];\n";
    let res2 = OpenQasmFrontend::new().parse(&SourceBuffer::new("oob_exact.qasm", src2));
    assert!(!res2.is_ok());
    assert!(res2.errors().any(|e| e.code.as_deref() == Some("E0014")));

    // 3. Index far out of bounds
    let src3 = "OPENQASM 3.0;\nqubit[2] q;\nh q[99];\n";
    let res3 = OpenQasmFrontend::new().parse(&SourceBuffer::new("oob_far.qasm", src3));
    assert!(!res3.is_ok());
    assert!(res3.errors().any(|e| e.code.as_deref() == Some("E0014")));

    // 4. Classical register used as qubit operand
    let src4 = "OPENQASM 3.0;\nbit[2] c;\nh c[0];\n";
    let res4 = OpenQasmFrontend::new().parse(&SourceBuffer::new("kind_mismatch.qasm", src4));
    assert!(!res4.is_ok());
    assert!(res4.errors().any(|e| e.code.as_deref() == Some("E0013")));
}

// ============================================================================
// 11. Source Span Accuracy
// ============================================================================

#[test]
fn test_gate_statement_source_spans() {
    let src = "OPENQASM 3.0;\nqubit[2] q;\nh q[0];\ncx q[0], q[1];\n";
    let buffer = SourceBuffer::new("spans.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(result.is_ok());
    let program = result.into_result().unwrap();

    // Statement 1: h q[0]; (Line 3, 1-indexed)
    match &program.statements[1] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.span.start_line(), 3);
            assert_eq!(call.span.start_col(), 1);
            assert_eq!(call.span.end_line(), 3);
            assert_eq!(call.span.end_col(), 8); // "h q[0];" length is 7 -> end_col is 8

            assert_eq!(call.qubits[0].span.start_line(), 3);
            assert_eq!(call.qubits[0].span.start_col(), 3); // "q[0]" starts at col 3
            assert_eq!(call.qubits[0].span.end_col(), 7); // "q[0]" length is 4 -> 3+4=7
        }
        other => panic!("Unexpected: {:?}", other),
    }

    // Statement 2: cx q[0], q[1]; (Line 4)
    match &program.statements[2] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.span.start_line(), 4);
            assert_eq!(call.span.start_col(), 1);
            assert_eq!(call.span.end_line(), 4);
            assert_eq!(call.span.end_col(), 15); // "cx q[0], q[1];" length is 14 -> end_col is 15

            assert_eq!(call.qubits[0].span.start_col(), 4);
            assert_eq!(call.qubits[0].span.end_col(), 8);

            assert_eq!(call.qubits[1].span.start_col(), 10);
            assert_eq!(call.qubits[1].span.end_col(), 14);
        }
        other => panic!("Unexpected: {:?}", other),
    }
}

// ============================================================================
// 12. Multiple Gate Statements and Formatting Robustness
// ============================================================================

#[test]
fn test_multiple_gate_statements_with_whitespace_and_comments() {
    let src = r#"OPENQASM 3.0;
// Prepare Bell state with trailing and inline comments
qubit[2] q;

h    q[0]   ; // Hadamard on qubit 0
cx   q[0],   q[1]; // Entangle qubits 0 and 1
s    q[0];
t    q[1];
"#;

    let buffer = SourceBuffer::new("bell_prog.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(result.is_ok(), "Failed: {:?}", result.diagnostics);
    let program = result.into_result().unwrap();

    assert_eq!(program.statements.len(), 5); // 1 decl + 4 gates
    let names: Vec<_> = program
        .statements
        .iter()
        .filter_map(|s| match s {
            AstStatement::GateCall(g) => Some(g.name.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(names, vec!["h", "cx", "s", "t"]);
}

// ============================================================================
// 13. Semicolon and Comma Syntax Robustness
// ============================================================================

#[test]
fn test_missing_semicolon_and_trailing_comma_rejected() {
    // Missing semicolon after gate
    let src1 = "OPENQASM 3.0;\nqubit[1] q;\nh q[0]\n";
    let res1 = OpenQasmFrontend::new().parse(&SourceBuffer::new("no_semi.qasm", src1));
    assert!(!res1.is_ok());
    assert!(res1.errors().any(|e| e.code.as_deref() == Some("E0001")));
    assert!(res1
        .errors()
        .any(|e| e.message.contains("expected ';' after gate statement")));

    // Trailing comma in operand list: cx q[0], q[1],;
    let src2 = "OPENQASM 3.0;\nqubit[2] q;\ncx q[0], q[1],;\n";
    let res2 = OpenQasmFrontend::new().parse(&SourceBuffer::new("trailing_comma.qasm", src2));
    assert!(!res2.is_ok());
    assert!(res2.errors().any(|e| e.code.as_deref() == Some("E0001")));
    assert!(res2
        .errors()
        .any(|e| e.message.contains("trailing comma in gate operand list")));
}

// ============================================================================
// 14. Phase 19 Regression: Declarations and Symbol Table
// ============================================================================

#[test]
fn test_phase19_declarations_regression() {
    let src = r#"OPENQASM 3.0;
include "stdgates.inc";

qubit[2] q;
bit[2] c;

h q[0];
cx q[0], q[1];
"#;

    let buffer = SourceBuffer::new("regression.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let (result, table) = frontend.parse_with_registers(&buffer);

    assert!(result.is_ok(), "Parse failed: {:?}", result.diagnostics);
    let program = result.into_result().unwrap();

    // Verify program headers
    assert_eq!(program.version.as_ref().unwrap().specifier, "3.0");
    assert_eq!(program.includes.len(), 1);
    assert_eq!(program.includes[0].path, "stdgates.inc");

    // Verify symbol table
    assert_eq!(table.quantum_registers().len(), 1);
    assert_eq!(table.classical_registers().len(), 1);
    assert_eq!(table.total_qubits(), 2);
    assert_eq!(table.total_bits(), 2);

    let q_entry = table.get_quantum("q").expect("Register 'q' must exist");
    assert!(!q_entry.is_scalar);
    assert_eq!(q_entry.size, 2);

    let c_entry = table.get_classical("c").expect("Register 'c' must exist");
    assert!(!c_entry.is_scalar);
    assert_eq!(c_entry.size, 2);
}
