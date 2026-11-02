//! Tests for detection and rejection of subroutines and linkage (`def`, `return`, `extern`).

use qcc::frontend::{AstStatement, Frontend, OpenQasmFrontend, SourceBuffer};

#[test]
fn test_reject_def_subroutine() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
def apply_teleport(qubit src) {
    x src;
}
"#;
    let buffer = SourceBuffer::new("def_sub.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());
    assert!(res.output.is_none());

    let diags: Vec<_> = res.errors().collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_deref(), Some("E0005"));
    assert!(diags[0].message.contains("subroutines"));

    let (program, _) = frontend.parse_recovering(&buffer);
    let stmts = program.statements();
    assert_eq!(stmts.len(), 2);
    match &stmts[1] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "subroutine");
            assert!(unsupp.raw.contains("def apply_teleport"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}

#[test]
fn test_reject_return_statement() {
    let src = r#"OPENQASM 3.0;
return;
"#;
    let buffer = SourceBuffer::new("return.qasm", src);
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
            assert_eq!(unsupp.kind, "subroutine");
            assert!(unsupp.raw.contains("return;"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}

#[test]
fn test_reject_extern_declaration() {
    let src = r#"OPENQASM 3.0;
extern my_hw_readout(int[32]) -> bit;
"#;
    let buffer = SourceBuffer::new("extern.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());

    let diags: Vec<_> = res.errors().collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_deref(), Some("E0005"));
    assert!(diags[0].message.contains("extern symbol declarations"));

    let (program, _) = frontend.parse_recovering(&buffer);
    let stmts = program.statements();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "subroutine");
            assert!(unsupp.raw.contains("extern my_hw_readout"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}
