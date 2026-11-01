//! Tests for parameter expressions, arithmetic operator precedence, and lowering.

use std::collections::HashMap;

use qcc::diagnostics::SourceSpan;
use qcc::frontend::ast::{AstExprKind, AstExpression};
use qcc::ir::parameter::ParameterExpr;

#[test]
fn test_constant_expression() {
    let span = SourceSpan::single_line(0, 3, 1, 1, 4).unwrap();
    let expr = AstExpression::constant(1.25, span);

    assert!(expr.is_constant());
    assert!(!expr.is_identifier());
    assert_eq!(expr.span(), span);
    assert_eq!(format!("{expr}"), "1.25");

    let p_expr = expr.to_parameter_expr().unwrap();
    assert_eq!(p_expr, ParameterExpr::Constant(1.25));
}

#[test]
fn test_identifier_expression_and_pi_mapping() {
    let span = SourceSpan::single_line(0, 2, 1, 1, 3).unwrap();
    let pi_expr = AstExpression::identifier("pi", span);
    assert!(pi_expr.is_identifier());
    assert_eq!(format!("{pi_expr}"), "pi");

    let p_pi = pi_expr.to_parameter_expr().unwrap();
    assert_eq!(p_pi, ParameterExpr::Constant(std::f64::consts::PI));

    let theta_expr = AstExpression::identifier("theta", span);
    let p_theta = theta_expr.to_parameter_expr().unwrap();
    assert_eq!(p_theta, ParameterExpr::Symbolic("theta".to_string()));
}

#[test]
fn test_binary_arithmetic_expressions() {
    let s1 = SourceSpan::single_line(0, 2, 1, 1, 3).unwrap();
    let s2 = SourceSpan::single_line(5, 6, 1, 6, 7).unwrap();
    let s_all = SourceSpan::single_line(0, 6, 1, 1, 7).unwrap();

    let pi = AstExpression::identifier("pi", s1);
    let two = AstExpression::constant(2.0, s2);

    let div = AstExpression::div(pi.clone(), two.clone(), s_all);
    assert_eq!(format!("{div}"), "(pi / 2)");

    let p_div = div.to_parameter_expr().unwrap();
    let env = HashMap::new();
    let val = p_div.eval(&env).unwrap();
    assert!((val - std::f64::consts::FRAC_PI_2).abs() < 1e-12);

    let add = AstExpression::add(pi.clone(), two.clone(), s_all);
    assert_eq!(format!("{add}"), "(pi + 2)");

    let sub = AstExpression::sub(pi.clone(), two.clone(), s_all);
    assert_eq!(format!("{sub}"), "(pi - 2)");

    let mul = AstExpression::mul(pi, two, s_all);
    assert_eq!(format!("{mul}"), "(pi * 2)");
}

#[test]
fn test_unary_negation() {
    let s1 = SourceSpan::single_line(1, 3, 1, 2, 4).unwrap();
    let s_all = SourceSpan::single_line(0, 3, 1, 1, 4).unwrap();

    let theta = AstExpression::identifier("theta", s1);
    let neg = AstExpression::neg(theta, s_all);
    assert_eq!(format!("{neg}"), "-theta");

    let p_neg = neg.to_parameter_expr().unwrap();
    let mut env = HashMap::new();
    env.insert("theta".to_string(), 0.75);
    assert_eq!(p_neg.eval(&env).unwrap(), -0.75);
}

#[test]
fn test_pow_and_function_call_expressions() {
    let s1 = SourceSpan::single_line(0, 1, 1, 1, 2).unwrap();
    let s2 = SourceSpan::single_line(4, 5, 1, 5, 6).unwrap();
    let s_all = SourceSpan::single_line(0, 5, 1, 1, 6).unwrap();

    let x = AstExpression::identifier("x", s1);
    let two = AstExpression::constant(2.0, s2);
    let pow_expr = AstExpression::pow(x.clone(), two, s_all);

    assert_eq!(format!("{pow_expr}"), "(x ^ 2)");
    assert!(matches!(pow_expr.kind, AstExprKind::Pow(_, _)));
    assert!(pow_expr.to_parameter_expr().is_err());

    let call_span = SourceSpan::single_line(0, 6, 1, 1, 7).unwrap();
    let call_expr = AstExpression::call("sin", vec![x], call_span);
    assert_eq!(format!("{call_expr}"), "sin(x)");
    assert!(matches!(call_expr.kind, AstExprKind::Call(_, _)));
    assert!(call_expr.to_parameter_expr().is_err());
}

#[test]
fn test_deep_expression_tree() {
    let s = SourceSpan::single_line(0, 1, 1, 1, 2).unwrap();
    // -( (a + b) * (c - d) )
    let a = AstExpression::identifier("a", s);
    let b = AstExpression::identifier("b", s);
    let c = AstExpression::identifier("c", s);
    let d = AstExpression::identifier("d", s);

    let add = AstExpression::add(a, b, s);
    let sub = AstExpression::sub(c, d, s);
    let mul = AstExpression::mul(add, sub, s);
    let neg = AstExpression::neg(mul, s);

    assert_eq!(format!("{neg}"), "-((a + b) * (c - d))");

    let mut env = HashMap::new();
    env.insert("a".to_string(), 1.0);
    env.insert("b".to_string(), 2.0);
    env.insert("c".to_string(), 10.0);
    env.insert("d".to_string(), 4.0);

    let p = neg.to_parameter_expr().unwrap();
    // -( (1 + 2) * (10 - 4) ) = -(3 * 6) = -18
    assert_eq!(p.eval(&env).unwrap(), -18.0);
}
