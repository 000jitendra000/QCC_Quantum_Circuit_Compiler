//! Tests verifying source-span validity, containment, and provenance across AST nodes.

use qcc::diagnostics::SourceSpan;
use qcc::frontend::ast::{
    AstBitRef, AstExpression, AstGateCall, AstMeasure, AstProgram, AstQubitRef, AstRegisterDecl,
    AstStatement,
};

fn assert_span_contains(parent: SourceSpan, child: SourceSpan) {
    assert!(
        parent.start() <= child.start(),
        "parent start {} > child start {}",
        parent.start(),
        child.start()
    );
    assert!(
        parent.end() >= child.end(),
        "parent end {} < child end {}",
        parent.end(),
        child.end()
    );
    assert!(
        parent.start_line() <= child.start_line(),
        "parent start_line {} > child start_line {}",
        parent.start_line(),
        child.start_line()
    );
    assert!(
        parent.end_line() >= child.end_line(),
        "parent end_line {} < child end_line {}",
        parent.end_line(),
        child.end_line()
    );
}

#[test]
fn test_program_span_contains_all_statements() {
    let prog_span = SourceSpan::new(0, 100, 1, 1, 5, 20).unwrap();

    let s1_span = SourceSpan::single_line(0, 12, 1, 1, 13).unwrap();
    let s2_span = SourceSpan::single_line(20, 45, 2, 1, 26).unwrap();
    let s3_span = SourceSpan::single_line(50, 95, 4, 1, 46).unwrap();

    let reg = AstRegisterDecl::qubit("q", Some(2), s1_span);
    let q0 = AstQubitRef::indexed("q", 0, SourceSpan::single_line(23, 27, 2, 4, 8).unwrap());
    let gate = AstGateCall::new("h", Vec::new(), vec![q0], s2_span);

    let m_q = AstQubitRef::indexed("q", 0, SourceSpan::single_line(58, 62, 4, 9, 13).unwrap());
    let m_c = AstBitRef::indexed("c", 0, SourceSpan::single_line(66, 70, 4, 17, 21).unwrap());
    let meas = AstMeasure::new(m_q, m_c, s3_span);

    let prog = AstProgram::new(
        None,
        Vec::new(),
        vec![
            AstStatement::from(reg),
            AstStatement::from(gate),
            AstStatement::from(meas),
        ],
        prog_span,
    );

    for stmt in prog.statements() {
        assert_span_contains(prog.span(), stmt.span());
    }
}

#[test]
fn test_gate_call_span_contains_operands_and_params() {
    let gate_span = SourceSpan::single_line(10, 40, 2, 1, 31).unwrap();
    let p_span = SourceSpan::single_line(13, 21, 2, 4, 12).unwrap();
    let q_span = SourceSpan::single_line(23, 27, 2, 14, 18).unwrap();

    let param = AstExpression::identifier("theta", p_span);
    let qubit = AstQubitRef::indexed("q", 0, q_span);

    let gate = AstGateCall::new("rx", vec![param], vec![qubit], gate_span);

    for p in &gate.parameters {
        assert_span_contains(gate.span(), p.span());
    }
    for q in &gate.qubits {
        assert_span_contains(gate.span(), q.span());
    }
}

#[test]
fn test_measure_span_contains_qubit_and_bit() {
    let m_span = SourceSpan::single_line(10, 35, 3, 1, 26).unwrap();
    let q_span = SourceSpan::single_line(18, 22, 3, 9, 13).unwrap();
    let c_span = SourceSpan::single_line(26, 30, 3, 17, 21).unwrap();

    let q = AstQubitRef::indexed("q", 1, q_span);
    let c = AstBitRef::indexed("c", 1, c_span);

    let meas = AstMeasure::new(q, c, m_span);

    assert_span_contains(meas.span(), meas.qubit.span());
    assert_span_contains(meas.span(), meas.bit.span());
}

#[test]
fn test_expression_span_containment() {
    let full_span = SourceSpan::single_line(0, 15, 1, 1, 16).unwrap();
    let l_span = SourceSpan::single_line(1, 6, 1, 2, 7).unwrap();
    let r_span = SourceSpan::single_line(9, 14, 1, 10, 15).unwrap();

    let lhs = AstExpression::identifier("alpha", l_span);
    let rhs = AstExpression::identifier("beta", r_span);
    let binary = AstExpression::add(lhs.clone(), rhs.clone(), full_span);

    assert_span_contains(binary.span(), lhs.span());
    assert_span_contains(binary.span(), rhs.span());

    let neg_span = SourceSpan::single_line(0, 7, 1, 1, 8).unwrap();
    let neg = AstExpression::neg(lhs.clone(), neg_span);
    assert_span_contains(neg.span(), lhs.span());

    let call_span = SourceSpan::single_line(0, 20, 1, 1, 21).unwrap();
    let call = AstExpression::call("f", vec![lhs.clone(), rhs.clone()], call_span);
    for arg in match &call.kind {
        qcc::frontend::ast::AstExprKind::Call(_, args) => args,
        _ => unreachable!(),
    } {
        assert_span_contains(call.span(), arg.span());
    }
}
