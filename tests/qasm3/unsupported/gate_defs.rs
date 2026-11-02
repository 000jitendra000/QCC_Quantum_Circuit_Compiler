//! Tests for detection and rejection of custom gate definitions (`gate`).

use qcc::frontend::{AstStatement, Frontend, OpenQasmFrontend, SourceBuffer};

#[test]
fn test_reject_custom_gate_definition() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
gate bell a, b {
    h a;
    cx a, b;
}
bell q[0], q[1];
"#;
    let buffer = SourceBuffer::new("gate_def.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());
    assert!(res.output.is_none());

    let diags: Vec<_> = res.errors().collect();
    assert!(diags.iter().any(|d| d.code.as_deref() == Some("E0005")));
    assert!(diags
        .iter()
        .any(|d| d.message.contains("custom gate definitions ('gate')")));

    let (program, _) = frontend.parse_recovering(&buffer);
    let stmts = program.statements();
    assert!(stmts.len() >= 2);
    match &stmts[1] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "gate_definition");
            assert!(unsupp.raw.contains("gate bell"));
            assert!(unsupp.raw.contains("cx a, b;"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}

#[test]
fn test_reject_parameterized_gate_definition() {
    let src = r#"OPENQASM 3.0;
gate custom_rot(theta) q {
    rx(theta) q;
    rz(theta) q;
}
"#;
    let buffer = SourceBuffer::new("param_gate_def.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());

    let diags: Vec<_> = res.errors().collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_deref(), Some("E0005"));

    let (program, _) = frontend.parse_recovering(&buffer);
    let stmts = program.statements();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "gate_definition");
            assert!(unsupp.raw.contains("gate custom_rot"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}
