//! Tests verifying non-omission of unsupported constructs and parser error recovery.

use qcc::frontend::{AstStatement, Frontend, OpenQasmFrontend, SemanticAnalyzer, SourceBuffer};

#[test]
fn test_mixed_program_statement_order_and_non_omission() {
    let src = r#"OPENQASM 3.0;
qubit[3] q;
bit[3] c;
x q[0];
while (c[0] == 0) {
    y q[1];
}
h q[2];
ccx q[0], q[1], q[2];
cz q[0], q[1];
"#;
    let buffer = SourceBuffer::new("mixed_program.qasm", src);
    let frontend = OpenQasmFrontend::new();

    // 1. Standard parse must fail closed
    let parse_res = frontend.parse(&buffer);
    assert!(!parse_res.is_ok());
    assert!(parse_res.output.is_none());

    // 2. Must contain E0005 (while) and E0003 (ccx)
    let error_codes: Vec<Option<&str>> = parse_res.errors().map(|d| d.code.as_deref()).collect();
    assert!(
        error_codes.contains(&Some("E0005")),
        "Expected E0005 for while loop"
    );
    assert!(
        error_codes.contains(&Some("E0003")),
        "Expected E0003 for ccx"
    );

    // 3. Recovering parse must retain ALL statements in exact source order
    let (program, diags) = frontend.parse_recovering(&buffer);
    assert_eq!(diags.len(), 2);

    let stmts = program.statements();
    assert_eq!(stmts.len(), 7, "All 7 statements must be preserved in AST");

    // [0]: qubit[3] q;
    assert!(stmts[0].is_register_decl());
    // [1]: bit[3] c;
    assert!(stmts[1].is_register_decl());
    // [2]: x q[0];
    assert!(stmts[2].is_gate_call());
    // [3]: while (...) { ... }
    match &stmts[3] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "loop");
            assert!(unsupp.raw.contains("while"));
        }
        other => panic!("expected Unsupported at [3], got {other:?}"),
    }
    // [4]: h q[2]; (proves while loop recovery didn't omit subsequent statements)
    match &stmts[4] {
        AstStatement::GateCall(call) => assert_eq!(call.name, "h"),
        other => panic!("expected GateCall(h) at [4], got {other:?}"),
    }
    // [5]: ccx q[0], q[1], q[2];
    match &stmts[5] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "three_qubit_gate");
            assert!(unsupp.raw.contains("ccx"));
        }
        other => panic!("expected Unsupported at [5], got {other:?}"),
    }
    // [6]: cz q[0], q[1]; (proves ccx rejection didn't omit subsequent statements)
    match &stmts[6] {
        AstStatement::GateCall(call) => assert_eq!(call.name, "cz"),
        other => panic!("expected GateCall(cz) at [6], got {other:?}"),
    }
}

#[test]
fn test_semantic_analyzer_fails_closed_on_unsupported_ast_nodes() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
if (q[0] == 1) {
    x q[1];
}
"#;
    let buffer = SourceBuffer::new("semantic_fail_closed.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let (program, _) = frontend.parse_recovering(&buffer);
    let analyzer = SemanticAnalyzer::new(&buffer);

    let result = analyzer.analyze(&program);
    assert!(
        result.is_err(),
        "SemanticAnalyzer must reject AST containing Unsupported nodes"
    );

    let errs = result.unwrap_err();
    assert!(
        errs.iter().any(|d| d.code.as_deref() == Some("E0005")),
        "SemanticAnalyzer must report E0005 for Unsupported conditional"
    );
}

#[test]
fn test_nested_braces_in_unsupported_block() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
while (c[0] == 0) {
    if (c[1] == 1) {
        y q[0];
    }
}
x q[1];
"#;
    let buffer = SourceBuffer::new("nested_braces.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let parse_res = frontend.parse(&buffer);
    assert!(!parse_res.is_ok());
    assert!(parse_res.output.is_none());

    let (program, diags) = frontend.parse_recovering(&buffer);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_deref(), Some("E0005"));

    let stmts = program.statements();
    assert_eq!(
        stmts.len(),
        3,
        "qubit decl, while block, and subsequent x gate must be preserved"
    );

    match &stmts[1] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "loop");
            assert!(unsupp.raw.contains("while"));
            assert!(unsupp.raw.contains("y q[0];"));
        }
        other => panic!("expected Unsupported at [1], got {other:?}"),
    }

    match &stmts[2] {
        AstStatement::GateCall(call) => assert_eq!(call.name, "x"),
        other => panic!("expected GateCall(x) at [2], got {other:?}"),
    }
}

#[test]
fn test_unclosed_block_at_eof() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
while (c[0] == 0) {
    x q[0];
"#;
    let buffer = SourceBuffer::new("unclosed_eof.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let parse_res = frontend.parse(&buffer);
    assert!(!parse_res.is_ok());
    assert!(parse_res.output.is_none());

    let diags: Vec<_> = parse_res.errors().collect();
    // Must contain E0005 for while loop and E0001 for unclosed '{'
    assert!(diags.iter().any(|d| d.code.as_deref() == Some("E0005")));
    assert!(diags.iter().any(|d| d.code.as_deref() == Some("E0001")));
}

#[test]
fn test_multiple_consecutive_unsupported_constructs() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
int[32] count = 0;
defcal rx(pi) q[0] { shift_phase(1.0); }
gate custom_g a { x a; }
for i in [0:3] { h q[i]; }
x q[0];
"#;
    let buffer = SourceBuffer::new("multiple_unsupported.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let parse_res = frontend.parse(&buffer);
    assert!(!parse_res.is_ok());
    assert!(parse_res.output.is_none());

    let (program, diags) = frontend.parse_recovering(&buffer);
    // 4 unsupported constructs must produce 4 diagnostics
    assert_eq!(diags.len(), 4);
    for d in &diags {
        assert_eq!(d.code.as_deref(), Some("E0005"));
    }

    let stmts = program.statements();
    // 1 register decl + 4 unsupported + 1 gate call = 6 total statements
    assert_eq!(stmts.len(), 6);
    assert!(stmts[0].is_register_decl());
    assert!(matches!(&stmts[1], AstStatement::Unsupported(u) if u.kind == "classical_type"));
    assert!(matches!(&stmts[2], AstStatement::Unsupported(u) if u.kind == "calibration"));
    assert!(matches!(&stmts[3], AstStatement::Unsupported(u) if u.kind == "gate_definition"));
    assert!(matches!(&stmts[4], AstStatement::Unsupported(u) if u.kind == "loop"));
    match &stmts[5] {
        AstStatement::GateCall(call) => assert_eq!(call.name, "x"),
        other => panic!("expected GateCall(x) at [5], got {other:?}"),
    }
}

#[test]
fn test_malformed_unsupported_statement_semicolon_termination() {
    let src = r#"OPENQASM 3.0;
qubit[2] q;
while (c == 0);
if (c == 1);
x q[0];
"#;
    let buffer = SourceBuffer::new("semicolon_termination.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let parse_res = frontend.parse(&buffer);
    assert!(!parse_res.is_ok());

    let (program, diags) = frontend.parse_recovering(&buffer);
    assert_eq!(diags.len(), 2);

    let stmts = program.statements();
    assert_eq!(stmts.len(), 4); // qubit, while;, if;, x
    match &stmts[3] {
        AstStatement::GateCall(call) => assert_eq!(call.name, "x"),
        other => panic!("expected GateCall(x) at [3], got {other:?}"),
    }
}
