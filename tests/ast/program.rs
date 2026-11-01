//! Tests for OpenQASM program, version, and include AST nodes.

use qcc::diagnostics::SourceSpan;
use qcc::frontend::ast::{AstInclude, AstProgram, AstRegisterDecl, AstStatement, AstVersion};

#[test]
fn test_program_construction_with_headers() {
    let version_span = SourceSpan::single_line(0, 15, 1, 1, 15).unwrap();
    let version = AstVersion::new("3.0", version_span);
    assert_eq!(version.specifier, "3.0");
    assert_eq!(version.span(), version_span);

    let inc_span = SourceSpan::single_line(16, 39, 2, 1, 23).unwrap();
    let include = AstInclude::new("stdgates.inc", inc_span);
    assert_eq!(include.path, "stdgates.inc");
    assert_eq!(include.span(), inc_span);

    let reg_span = SourceSpan::single_line(40, 52, 3, 1, 12).unwrap();
    let reg = AstRegisterDecl::qubit("q", Some(2), reg_span);
    let stmt = AstStatement::from(reg);

    let prog_span = SourceSpan::new(0, 52, 1, 1, 3, 12).unwrap();
    let prog = AstProgram::new(
        Some(version.clone()),
        vec![include.clone()],
        vec![stmt],
        prog_span,
    );

    assert_eq!(prog.span(), prog_span);
    assert_eq!(prog.version(), Some(&version));
    assert_eq!(prog.includes().len(), 1);
    assert_eq!(prog.includes()[0].path, "stdgates.inc");
    assert_eq!(prog.statements().len(), 1);
}

#[test]
fn test_minimal_program_without_version_or_includes() {
    let prog_span = SourceSpan::single_line(0, 10, 1, 1, 10).unwrap();
    let prog = AstProgram::new(None, Vec::new(), Vec::new(), prog_span);

    assert!(prog.version().is_none());
    assert!(prog.includes().is_empty());
    assert!(prog.statements().is_empty());
    assert_eq!(prog.span(), prog_span);
}

#[test]
fn test_multiple_includes() {
    let inc1_span = SourceSpan::single_line(0, 23, 1, 1, 23).unwrap();
    let inc2_span = SourceSpan::single_line(24, 45, 2, 1, 21).unwrap();
    let inc1 = AstInclude::new("stdgates.inc", inc1_span);
    let inc2 = AstInclude::new("custom.inc", inc2_span);

    let prog_span = SourceSpan::new(0, 45, 1, 1, 2, 21).unwrap();
    let prog = AstProgram::new(None, vec![inc1, inc2], Vec::new(), prog_span);

    assert_eq!(prog.includes().len(), 2);
    assert_eq!(prog.includes()[0].path, "stdgates.inc");
    assert_eq!(prog.includes()[1].path, "custom.inc");
}
