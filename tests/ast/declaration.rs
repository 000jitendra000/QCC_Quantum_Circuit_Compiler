//! Tests for quantum and classical register declarations and operands.

use qcc::diagnostics::SourceSpan;
use qcc::frontend::ast::{AstBitRef, AstQubitRef, AstRegisterDecl, AstRegisterKind};

#[test]
fn test_register_kinds() {
    assert_eq!(AstRegisterKind::Qubit.as_str(), "qubit");
    assert_eq!(AstRegisterKind::Bit.as_str(), "bit");
    assert_eq!(format!("{}", AstRegisterKind::Qubit), "qubit");
    assert_eq!(format!("{}", AstRegisterKind::Bit), "bit");
}

#[test]
fn test_quantum_register_declaration() {
    let span = SourceSpan::single_line(0, 12, 1, 1, 12).unwrap();
    let decl = AstRegisterDecl::qubit("q", Some(5), span);

    assert_eq!(decl.kind, AstRegisterKind::Qubit);
    assert!(decl.is_qubit());
    assert!(!decl.is_bit());
    assert!(!decl.is_scalar());
    assert_eq!(decl.name, "q");
    assert_eq!(decl.size, Some(5));
    assert_eq!(decl.span(), span);
    assert_eq!(format!("{decl}"), "qubit[5] q;");
}

#[test]
fn test_scalar_quantum_register_declaration() {
    let span = SourceSpan::single_line(0, 8, 1, 1, 8).unwrap();
    let decl = AstRegisterDecl::qubit("ancilla", None, span);

    assert!(decl.is_qubit());
    assert!(decl.is_scalar());
    assert_eq!(decl.size, None);
    assert_eq!(format!("{decl}"), "qubit ancilla;");
}

#[test]
fn test_classical_register_declaration() {
    let span = SourceSpan::single_line(0, 10, 1, 1, 10).unwrap();
    let decl = AstRegisterDecl::bit("meas", Some(3), span);

    assert_eq!(decl.kind, AstRegisterKind::Bit);
    assert!(decl.is_bit());
    assert!(!decl.is_qubit());
    assert!(!decl.is_scalar());
    assert_eq!(decl.name, "meas");
    assert_eq!(decl.size, Some(3));
    assert_eq!(decl.span(), span);
    assert_eq!(format!("{decl}"), "bit[3] meas;");
}

#[test]
fn test_scalar_classical_register_declaration() {
    let span = SourceSpan::single_line(0, 6, 1, 1, 6).unwrap();
    let decl = AstRegisterDecl::bit("flag", None, span);

    assert!(decl.is_bit());
    assert!(decl.is_scalar());
    assert_eq!(decl.size, None);
    assert_eq!(format!("{decl}"), "bit flag;");
}

#[test]
fn test_qubit_and_bit_operands() {
    let q_span = SourceSpan::single_line(10, 14, 2, 5, 9).unwrap();
    let q_indexed = AstQubitRef::indexed("q", 2, q_span);
    assert!(q_indexed.is_indexed());
    assert!(!q_indexed.is_scalar());
    assert_eq!(q_indexed.name, "q");
    assert_eq!(q_indexed.index, Some(2));
    assert_eq!(format!("{q_indexed}"), "q[2]");

    let q_scalar = AstQubitRef::scalar("ancilla", q_span);
    assert!(q_scalar.is_scalar());
    assert!(!q_scalar.is_indexed());
    assert_eq!(q_scalar.index, None);
    assert_eq!(format!("{q_scalar}"), "ancilla");

    let c_span = SourceSpan::single_line(20, 24, 2, 15, 19).unwrap();
    let c_indexed = AstBitRef::indexed("c", 0, c_span);
    assert!(c_indexed.is_indexed());
    assert!(!c_indexed.is_scalar());
    assert_eq!(c_indexed.name, "c");
    assert_eq!(c_indexed.index, Some(0));
    assert_eq!(format!("{c_indexed}"), "c[0]");

    let c_scalar = AstBitRef::scalar("flag", c_span);
    assert!(c_scalar.is_scalar());
    assert!(!c_scalar.is_indexed());
    assert_eq!(format!("{c_scalar}"), "flag");
}
