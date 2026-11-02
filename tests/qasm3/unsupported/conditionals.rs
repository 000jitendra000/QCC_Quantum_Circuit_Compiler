//! Tests for detection and rejection of classical conditionals (`if`, `else`).

use qcc::frontend::{AstStatement, Frontend, OpenQasmFrontend, SourceBuffer};

#[test]
fn test_reject_if_with_block() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;
if (c[0] == 1) {
    x q[0];
}
"#;
    let buffer = SourceBuffer::new("if_block.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());
    assert!(res.output.is_none());

    let diags: Vec<_> = res.errors().collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_deref(), Some("E0005"));
    assert!(diags[0].message.contains("classical conditional branching"));

    // Verify AST preservation in recovering mode
    let (program, recovering_diags) = frontend.parse_recovering(&buffer);
    assert_eq!(recovering_diags.len(), 1);
    let stmts = program.statements();
    assert_eq!(stmts.len(), 3); // qubit[2] q; bit[2] c; if (...) { ... }
    match &stmts[2] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "conditional");
            assert!(unsupp.raw.contains("if (c[0] == 1)"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}

#[test]
fn test_reject_if_else_with_blocks() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
bit[1] c;
if (c[0] == 1) {
    x q[0];
} else {
    y q[0];
}
"#;
    let buffer = SourceBuffer::new("if_else.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());
    assert!(res.output.is_none());

    let diags: Vec<_> = res.errors().collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_deref(), Some("E0005"));

    let (program, _) = frontend.parse_recovering(&buffer);
    let stmts = program.statements();
    assert_eq!(stmts.len(), 3);
    match &stmts[2] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "conditional");
            assert!(unsupp.raw.contains("else"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}

#[test]
fn test_reject_if_single_statement() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
bit[1] c;
if (c[0] == 1) x q[0];
"#;
    let buffer = SourceBuffer::new("if_single.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());

    let diags: Vec<_> = res.errors().collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_deref(), Some("E0005"));

    let (program, _) = frontend.parse_recovering(&buffer);
    let stmts = program.statements();
    assert_eq!(stmts.len(), 3);
    match &stmts[2] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "conditional");
            assert!(unsupp.raw.contains("if (c[0] == 1) x q[0];"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}

#[test]
fn test_reject_lone_else() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
else {
    x q[0];
}
"#;
    let buffer = SourceBuffer::new("lone_else.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());

    let diags: Vec<_> = res.errors().collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_deref(), Some("E0005"));
}
