//! Tests for detection and rejection of pulse calibrations (`defcal`, `defcalgrammar`, `cal`).

use qcc::frontend::{AstStatement, Frontend, OpenQasmFrontend, SourceBuffer};

#[test]
fn test_reject_defcal_definition() {
    let src = r#"OPENQASM 3.0;
qubit[1] q;
defcal rx(pi) q[0] {
    shift_phase(1.0);
}
"#;
    let buffer = SourceBuffer::new("defcal.qasm", src);
    let frontend = OpenQasmFrontend::new();

    let res = frontend.parse(&buffer);
    assert!(!res.is_ok());
    assert!(res.output.is_none());

    let diags: Vec<_> = res.errors().collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code.as_deref(), Some("E0005"));
    assert!(diags[0].message.contains("pulse calibration definitions"));

    let (program, _) = frontend.parse_recovering(&buffer);
    let stmts = program.statements();
    assert_eq!(stmts.len(), 2);
    match &stmts[1] {
        AstStatement::Unsupported(unsupp) => {
            assert_eq!(unsupp.kind, "calibration");
            assert!(unsupp.raw.contains("defcal rx(pi)"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}

#[test]
fn test_reject_defcalgrammar() {
    let src = r#"OPENQASM 3.0;
defcalgrammar "openpulse";
"#;
    let buffer = SourceBuffer::new("defcalgrammar.qasm", src);
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
            assert_eq!(unsupp.kind, "calibration");
            assert!(unsupp.raw.contains("defcalgrammar \"openpulse\";"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}

#[test]
fn test_reject_cal_block() {
    let src = r#"OPENQASM 3.0;
cal {
    port d0;
}
"#;
    let buffer = SourceBuffer::new("cal_block.qasm", src);
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
            assert_eq!(unsupp.kind, "calibration");
            assert!(unsupp.raw.contains("cal"));
        }
        other => panic!("expected Unsupported statement, got {other:?}"),
    }
}
