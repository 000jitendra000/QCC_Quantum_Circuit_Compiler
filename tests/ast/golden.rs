//! Golden structural serialization tests for OpenQASM ASTs.

use qcc::diagnostics::SourceSpan;
use qcc::frontend::ast::{
    AstBarrier, AstBitRef, AstExpression, AstGateCall, AstGlobalPhase, AstInclude, AstMeasure,
    AstProgram, AstQubitRef, AstRegisterDecl, AstReset, AstStatement, AstUnsupported, AstVersion,
};

#[test]
fn test_golden_bell_circuit_ast() {
    let span_prog = SourceSpan::new(0, 115, 1, 1, 8, 20).unwrap();
    let span_v = SourceSpan::single_line(0, 14, 1, 1, 15).unwrap();
    let span_inc = SourceSpan::single_line(15, 38, 2, 1, 24).unwrap();
    let span_qreg = SourceSpan::single_line(39, 50, 3, 1, 12).unwrap();
    let span_creg = SourceSpan::single_line(51, 60, 4, 1, 10).unwrap();
    let span_h = SourceSpan::single_line(61, 68, 5, 1, 8).unwrap();
    let span_cx = SourceSpan::single_line(69, 83, 6, 1, 15).unwrap();
    let span_m0 = SourceSpan::single_line(84, 98, 7, 1, 15).unwrap();
    let span_m1 = SourceSpan::single_line(99, 113, 8, 1, 15).unwrap();

    let version = AstVersion::new("3.0", span_v);
    let include = AstInclude::new("stdgates.inc", span_inc);
    let qreg = AstRegisterDecl::qubit("q", Some(2), span_qreg);
    let creg = AstRegisterDecl::bit("c", Some(2), span_creg);

    let h_q0 = AstQubitRef::indexed("q", 0, SourceSpan::single_line(63, 67, 5, 3, 7).unwrap());
    let h_gate = AstGateCall::new("h", Vec::new(), vec![h_q0], span_h);

    let cx_q0 = AstQubitRef::indexed("q", 0, SourceSpan::single_line(72, 76, 6, 4, 8).unwrap());
    let cx_q1 = AstQubitRef::indexed("q", 1, SourceSpan::single_line(78, 82, 6, 10, 14).unwrap());
    let cx_gate = AstGateCall::new("cx", Vec::new(), vec![cx_q0, cx_q1], span_cx);

    let m0_q = AstQubitRef::indexed("q", 0, SourceSpan::single_line(86, 90, 7, 3, 7).unwrap());
    let m0_c = AstBitRef::indexed("c", 0, SourceSpan::single_line(94, 98, 7, 11, 15).unwrap());
    let m0 = AstMeasure::new(m0_q, m0_c, span_m0);

    let m1_q = AstQubitRef::indexed("q", 1, SourceSpan::single_line(101, 105, 8, 3, 7).unwrap());
    let m1_c = AstBitRef::indexed(
        "c",
        1,
        SourceSpan::single_line(109, 113, 8, 11, 15).unwrap(),
    );
    let m1 = AstMeasure::new(m1_q, m1_c, span_m1);

    let statements = vec![
        AstStatement::from(qreg),
        AstStatement::from(creg),
        AstStatement::from(h_gate),
        AstStatement::from(cx_gate),
        AstStatement::from(m0),
        AstStatement::from(m1),
    ];

    let prog = AstProgram::new(Some(version), vec![include], statements, span_prog);

    let expected = "\
Program [1:1-8:20]
  Version: 3.0 [1:1-15]
  Include: \"stdgates.inc\" [2:1-24]
  RegisterDecl(qubit): q[2] [3:1-12]
  RegisterDecl(bit): c[2] [4:1-10]
  GateCall: h [5:1-8]
    Qubit: q[0] [5:3-7]
  GateCall: cx [6:1-15]
    Qubit: q[0] [6:4-8]
    Qubit: q[1] [6:10-14]
  Measure [7:1-15]
    Qubit: q[0] [7:3-7]
    Bit: c[0] [7:11-15]
  Measure [8:1-15]
    Qubit: q[1] [8:3-7]
    Bit: c[1] [8:11-15]
";

    assert_eq!(prog.to_structural_string(), expected);
}

#[test]
fn test_golden_parameterized_circuit_ast() {
    let span_prog = SourceSpan::new(0, 60, 1, 1, 3, 17).unwrap();
    let span_v = SourceSpan::single_line(0, 14, 1, 1, 15).unwrap();
    let span_rz = SourceSpan::single_line(15, 33, 2, 1, 19).unwrap();
    let span_gp = SourceSpan::single_line(34, 52, 3, 1, 19).unwrap();

    let version = AstVersion::new("3.0", span_v);

    let pi_span = SourceSpan::single_line(18, 20, 2, 4, 6).unwrap();
    let two_span = SourceSpan::single_line(23, 24, 2, 9, 10).unwrap();
    let div_span = SourceSpan::single_line(18, 24, 2, 4, 10).unwrap();
    let angle_rz = AstExpression::div(
        AstExpression::identifier("pi", pi_span),
        AstExpression::constant(2.0, two_span),
        div_span,
    );

    let q0_span = SourceSpan::single_line(26, 30, 2, 12, 16).unwrap();
    let rz_gate = AstGateCall::new(
        "rz",
        vec![angle_rz],
        vec![AstQubitRef::indexed("q", 0, q0_span)],
        span_rz,
    );

    let gp_angle_span = SourceSpan::single_line(41, 49, 3, 8, 16).unwrap();
    let gp_angle = AstExpression::div(
        AstExpression::identifier("pi", SourceSpan::single_line(41, 43, 3, 8, 10).unwrap()),
        AstExpression::constant(4.0, SourceSpan::single_line(46, 47, 3, 13, 14).unwrap()),
        gp_angle_span,
    );
    let gp = AstGlobalPhase::new(gp_angle, span_gp);

    let prog = AstProgram::new(
        Some(version),
        Vec::new(),
        vec![AstStatement::from(rz_gate), AstStatement::from(gp)],
        span_prog,
    );

    let expected = "\
Program [1:1-3:17]
  Version: 3.0 [1:1-15]
  GateCall: rz [2:1-19]
    Param: (pi / 2) [2:4-10]
    Qubit: q[0] [2:12-16]
  GlobalPhase [3:1-19]
    Angle: (pi / 4) [3:8-16]
";

    assert_eq!(prog.to_structural_string(), expected);
}

#[test]
fn test_golden_full_statement_variety_ast() {
    let span_prog = SourceSpan::new(0, 200, 1, 1, 9, 30).unwrap();
    let span_decl1 = SourceSpan::single_line(0, 8, 1, 1, 9).unwrap();
    let span_decl2 = SourceSpan::single_line(9, 15, 2, 1, 7).unwrap();
    let span_reset = SourceSpan::single_line(16, 26, 3, 1, 11).unwrap();
    let span_b_glob = SourceSpan::single_line(27, 35, 4, 1, 9).unwrap();
    let span_b_sel = SourceSpan::single_line(36, 54, 5, 1, 19).unwrap();
    let span_unsupp = SourceSpan::single_line(55, 80, 6, 1, 26).unwrap();

    let q_scalar = AstRegisterDecl::qubit("ancilla", None, span_decl1);
    let c_scalar = AstRegisterDecl::bit("flag", None, span_decl2);

    let reset_q = AstQubitRef::scalar(
        "ancilla",
        SourceSpan::single_line(22, 29, 3, 7, 14).unwrap(),
    );
    let reset = AstReset::new(reset_q, span_reset);

    let b_glob = AstBarrier::new(Vec::new(), span_b_glob);

    let q1 = AstQubitRef::indexed("q", 0, SourceSpan::single_line(44, 48, 5, 9, 13).unwrap());
    let q2 = AstQubitRef::indexed("q", 1, SourceSpan::single_line(50, 54, 5, 15, 19).unwrap());
    let b_sel = AstBarrier::new(vec![q1, q2], span_b_sel);

    let unsupp = AstUnsupported::new("defcal", "defcal rx(pi) q { ... }", span_unsupp);

    let prog = AstProgram::new(
        None,
        Vec::new(),
        vec![
            AstStatement::from(q_scalar),
            AstStatement::from(c_scalar),
            AstStatement::from(reset),
            AstStatement::from(b_glob),
            AstStatement::from(b_sel),
            AstStatement::from(unsupp),
        ],
        span_prog,
    );

    let expected = "\
Program [1:1-9:30]
  RegisterDecl(qubit): ancilla [1:1-9]
  RegisterDecl(bit): flag [2:1-7]
  Reset [3:1-11]
    Qubit: ancilla [3:7-14]
  Barrier (global) [4:1-9]
  Barrier [5:1-19]
    Qubit: q[0] [5:9-13]
    Qubit: q[1] [5:15-19]
  Unsupported(defcal): \"defcal rx(pi) q { ... }\" [6:1-26]
";

    assert_eq!(prog.to_structural_string(), expected);
}
