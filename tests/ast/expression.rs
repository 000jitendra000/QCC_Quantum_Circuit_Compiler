//! Tests for parameter expressions, arithmetic operator precedence, and AST-level evaluation.

use std::collections::HashMap;

use qcc::diagnostics::SourceSpan;
use qcc::frontend::ast::{AstExprKind, AstExpression};

#[test]
fn test_constant_expression() {
    let span = SourceSpan::single_line(0, 3, 1, 1, 4).unwrap();
    let expr = AstExpression::constant(1.25, span);

    assert!(expr.is_constant());
    assert!(!expr.is_identifier());
    assert_eq!(expr.as_constant(), Some(1.25));
    assert_eq!(expr.as_identifier(), None);
    assert_eq!(expr.span(), span);
    assert_eq!(format!("{expr}"), "1.25");

    let val = expr.eval(&HashMap::new()).unwrap();
    assert_eq!(val, 1.25);
}

#[test]
fn test_identifier_expression_and_pi_mapping() {
    let span = SourceSpan::single_line(0, 2, 1, 1, 3).unwrap();
    let pi_expr = AstExpression::identifier("pi", span);
    assert!(pi_expr.is_identifier());
    assert_eq!(pi_expr.as_identifier(), Some("pi"));
    assert_eq!(format!("{pi_expr}"), "pi");

    let val_pi = pi_expr.eval(&HashMap::new()).unwrap();
    assert!((val_pi - std::f64::consts::PI).abs() < 1e-12);

    let theta_expr = AstExpression::identifier("theta", span);
    assert_eq!(theta_expr.as_identifier(), Some("theta"));
    let mut env = HashMap::new();
    env.insert("theta".to_string(), 0.5);
    let val_theta = theta_expr.eval(&env).unwrap();
    assert_eq!(val_theta, 0.5);

    // Unbound symbol returns structured error
    assert!(theta_expr.eval(&HashMap::new()).is_err());
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

    let env = HashMap::new();
    let val = div.eval(&env).unwrap();
    assert!((val - std::f64::consts::FRAC_PI_2).abs() < 1e-12);

    let add = AstExpression::add(pi.clone(), two.clone(), s_all);
    assert_eq!(format!("{add}"), "(pi + 2)");
    let val_add = add.eval(&env).unwrap();
    assert!((val_add - (std::f64::consts::PI + 2.0)).abs() < 1e-12);

    let sub = AstExpression::sub(pi.clone(), two.clone(), s_all);
    assert_eq!(format!("{sub}"), "(pi - 2)");
    let val_sub = sub.eval(&env).unwrap();
    assert!((val_sub - (std::f64::consts::PI - 2.0)).abs() < 1e-12);

    let mul = AstExpression::mul(pi, two, s_all);
    assert_eq!(format!("{mul}"), "(pi * 2)");
    let val_mul = mul.eval(&env).unwrap();
    assert!((val_mul - (std::f64::consts::PI * 2.0)).abs() < 1e-12);
}

#[test]
fn test_unary_negation() {
    let s1 = SourceSpan::single_line(1, 3, 1, 2, 4).unwrap();
    let s_all = SourceSpan::single_line(0, 3, 1, 1, 4).unwrap();

    let theta = AstExpression::identifier("theta", s1);
    let neg = AstExpression::neg(theta, s_all);
    assert_eq!(format!("{neg}"), "-theta");

    let mut env = HashMap::new();
    env.insert("theta".to_string(), 0.75);
    assert_eq!(neg.eval(&env).unwrap(), -0.75);
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

    let mut env = HashMap::new();
    env.insert("x".to_string(), 3.0);
    assert_eq!(pow_expr.eval(&env).unwrap(), 9.0);

    let call_span = SourceSpan::single_line(0, 6, 1, 1, 7).unwrap();
    let call_sin = AstExpression::call("sin", vec![x], call_span);
    assert_eq!(format!("{call_sin}"), "sin(x)");
    assert!(matches!(call_sin.kind, AstExprKind::Call(_, _)));

    let mut env_pi = HashMap::new();
    env_pi.insert("x".to_string(), std::f64::consts::FRAC_PI_2);
    assert!((call_sin.eval(&env_pi).unwrap() - 1.0).abs() < 1e-12);

    let four = AstExpression::constant(4.0, s2);
    let call_sqrt = AstExpression::call("sqrt", vec![four], call_span);
    assert_eq!(call_sqrt.eval(&HashMap::new()).unwrap(), 2.0);
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

    // -( (1 + 2) * (10 - 4) ) = -(3 * 6) = -18
    assert_eq!(neg.eval(&env).unwrap(), -18.0);
}
