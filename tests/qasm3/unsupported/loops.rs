//! Tests for detection and rejection of classical loops (`while`, `for`, `break`, `continue`).

use qcc::frontend::{AstStatement, Frontend, OpenQasmFrontend, SourceBuffer};

#[test]
fn test_reject_while_loop() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
bit[2] c;
while (c[0] == 0) {
    x q[0];
}
"#;
    let buffer = SourceBuffer::new("while_loop.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());
    assert!(res.output.is_none());

    let diags: Vec<_> = res.errors().collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_deref(), Some("E0005"));
    assert!(diags[0].message.contains("classical control flow loops"));

    let (program, _) = frontend.parse_recovering(&buffer);
    let stmts = program.statements();
    assert_eq!(stmts.len(), 3);
    match &stmts[2] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "loop");
            assert!(unsupp.raw.contains("while"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}

#[test]
fn test_reject_for_loop() {
    let src = r#"OPENQASM 3.0;
qubit[3] q;
for i in [0:2] {
    h q[i];
}
"#;
    let buffer = SourceBuffer::new("for_loop.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());

    let diags: Vec<_> = res.errors().collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_deref(), Some("E0005"));

    let (program, _) = frontend.parse_recovering(&buffer);
    let stmts = program.statements();
    assert_eq!(stmts.len(), 2);
    match &stmts[1] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "loop");
            assert!(unsupp.raw.contains("for i in [0:2]"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}

#[test]
fn test_reject_break_and_continue() {
    let src = r#"OPENQASM 3.0;
break;
continue;
"#;
    let buffer = SourceBuffer::new("break_continue.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());

    let diags: Vec<_> = res.errors().collect();
    assert_eq!(diags.len(), 2);
    assert_eq!(diags[0].code.as_deref(), Some("E0005"));
    assert_eq!(diags[1].code.as_deref(), Some("E0005"));

    let (program, _) = frontend.parse_recovering(&buffer);
    let stmts = program.statements();
    assert_eq!(stmts.len(), 2);
    match &stmts[0] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "loop");
            assert!(unsupp.raw.contains("break;"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
    match &stmts[1] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "loop");
            assert!(unsupp.raw.contains("continue;"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}
