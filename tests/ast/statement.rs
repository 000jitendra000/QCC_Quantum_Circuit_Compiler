//! Tests for gate calls, measurements, resets, barriers, and global phase statements.

use qcc::diagnostics::SourceSpan;
use qcc::frontend::ast::{
    AstBarrier, AstBitRef, AstExpression, AstGateCall, AstGlobalPhase, AstMeasure, AstQubitRef,
    AstRegisterDecl, AstReset, AstStatement, AstUnsupported,
};

#[test]
fn test_gate_call_construction() {
    let q0_span = SourceSpan::single_line(3, 7, 1, 4, 8).unwrap();
    let q1_span = SourceSpan::single_line(9, 13, 1, 10, 14).unwrap();
    let q0 = AstQubitRef::indexed("q", 0, q0_span);
    let q1 = AstQubitRef::indexed("q", 1, q1_span);

    let gate_span = SourceSpan::single_line(0, 14, 1, 1, 15).unwrap();
    let cx = AstGateCall::new("cx", Vec::new(), vec![q0, q1], gate_span);

    assert_eq!(cx.name, "cx");
    assert_eq!(cx.parameters.len(), 0);
    assert_eq!(cx.qubits.len(), 2);
    assert_eq!(cx.qubits[0].name, "q");
    assert_eq!(cx.qubits[0].index, Some(0));
    assert_eq!(cx.qubits[1].name, "q");
    assert_eq!(cx.qubits[1].index, Some(1));
    assert_eq!(cx.span(), gate_span);

    let stmt = AstStatement::from(cx);
    assert!(stmt.is_gate_call());
    assert!(!stmt.is_measure());
    assert_eq!(stmt.span(), gate_span);
}

#[test]
fn test_parameterized_gate_call() {
    let param_span = SourceSpan::single_line(3, 5, 1, 4, 6).unwrap();
    let param = AstExpression::identifier("pi", param_span);

    let q0_span = SourceSpan::single_line(7, 11, 1, 8, 12).unwrap();
    let q0 = AstQubitRef::indexed("q", 0, q0_span);

    let gate_span = SourceSpan::single_line(0, 12, 1, 1, 13).unwrap();
    let rz = AstGateCall::new("rz", vec![param], vec![q0], gate_span);

    assert_eq!(rz.name, "rz");
    assert_eq!(rz.parameters.len(), 1);
    assert_eq!(rz.qubits.len(), 1);
}

#[test]
fn test_measurement_statement() {
    let q_span = SourceSpan::single_line(8, 12, 1, 9, 13).unwrap();
    let c_span = SourceSpan::single_line(16, 20, 1, 17, 21).unwrap();
    let qubit = AstQubitRef::indexed("q", 0, q_span);
    let bit = AstBitRef::indexed("c", 0, c_span);

    let meas_span = SourceSpan::single_line(0, 21, 1, 1, 22).unwrap();
    let meas = AstMeasure::new(qubit, bit, meas_span);

    assert_eq!(meas.qubit.name, "q");
    assert_eq!(meas.qubit.index, Some(0));
    assert_eq!(meas.bit.name, "c");
    assert_eq!(meas.bit.index, Some(0));
    assert_eq!(meas.span(), meas_span);

    let stmt = AstStatement::from(meas);
    assert!(stmt.is_measure());
    assert_eq!(stmt.span(), meas_span);
}

#[test]
fn test_reset_statement() {
    let q_span = SourceSpan::single_line(6, 10, 1, 7, 11).unwrap();
    let qubit = AstQubitRef::indexed("q", 1, q_span);

    let reset_span = SourceSpan::single_line(0, 11, 1, 1, 12).unwrap();
    let reset = AstReset::new(qubit, reset_span);

    assert_eq!(reset.qubit.name, "q");
    assert_eq!(reset.qubit.index, Some(1));
    assert_eq!(reset.span(), reset_span);

    let stmt = AstStatement::from(reset);
    assert!(stmt.is_reset());
    assert_eq!(stmt.span(), reset_span);
}

#[test]
fn test_barrier_statements() {
    // Selective barrier
    let q0_span = SourceSpan::single_line(8, 12, 1, 9, 13).unwrap();
    let q1_span = SourceSpan::single_line(14, 18, 1, 15, 19).unwrap();
    let q0 = AstQubitRef::indexed("q", 0, q0_span);
    let q1 = AstQubitRef::indexed("q", 1, q1_span);

    let b_span = SourceSpan::single_line(0, 19, 1, 1, 20).unwrap();
    let selective = AstBarrier::new(vec![q0, q1], b_span);
    assert!(!selective.is_global());
    assert_eq!(selective.qubits.len(), 2);
    assert_eq!(selective.span(), b_span);

    let stmt1 = AstStatement::from(selective);
    assert!(stmt1.is_barrier());

    // Global barrier
    let glob_span = SourceSpan::single_line(0, 8, 2, 1, 9).unwrap();
    let global = AstBarrier::new(Vec::new(), glob_span);
    assert!(global.is_global());
    assert_eq!(global.qubits.len(), 0);
    assert_eq!(global.span(), glob_span);

    let stmt2 = AstStatement::from(global);
    assert!(stmt2.is_barrier());
}

#[test]
fn test_global_phase_statement() {
    let angle_span = SourceSpan::single_line(7, 13, 1, 8, 14).unwrap();
    let angle = AstExpression::constant(std::f64::consts::FRAC_PI_2, angle_span);

    let gp_span = SourceSpan::single_line(0, 15, 1, 1, 16).unwrap();
    let gp = AstGlobalPhase::new(angle, gp_span);

    assert_eq!(gp.span(), gp_span);
    assert!(gp.angle.is_constant());

    let stmt = AstStatement::from(gp);
    assert!(stmt.is_global_phase());
    assert_eq!(stmt.span(), gp_span);
}

#[test]
fn test_unsupported_statement() {
    let span = SourceSpan::single_line(0, 20, 1, 1, 21).unwrap();
    let unsupp = AstUnsupported::new("while", "while (c == 0) { ... }", span);

    assert_eq!(unsupp.kind, "while");
    assert_eq!(unsupp.raw, "while (c == 0) { ... }");
    assert_eq!(unsupp.span(), span);

    let stmt = AstStatement::from(unsupp);
    assert!(stmt.is_unsupported());
    assert_eq!(stmt.span(), span);
}

#[test]
fn test_statement_predicates_exhaustive() {
    let dummy_span = SourceSpan::single_line(0, 1, 1, 1, 2).unwrap();
    let reg_stmt = AstStatement::from(AstRegisterDecl::qubit("q", None, dummy_span));
    assert!(reg_stmt.is_register_decl());
    assert!(!reg_stmt.is_gate_call());
    assert!(!reg_stmt.is_measure());
    assert!(!reg_stmt.is_reset());
    assert!(!reg_stmt.is_barrier());
    assert!(!reg_stmt.is_global_phase());
    assert!(!reg_stmt.is_unsupported());
}
