//! Integration tests for OpenQASM 3 parameter expression parsing and evaluation (Phase 21).
//!
//! Validates precedence, associativity, unary signs, parentheses, literal & binding division by zero,
//! symbolic preservation, symbol environment evaluation, diagnostics, and gate integration.

use std::collections::HashMap;

use qcc::frontend::ast::{AstExprKind, AstExpression, AstStatement};
use qcc::frontend::parser::expressions::{
    evaluate_expression, is_literal_zero, ExpressionEvaluationError, ExpressionParser,
};
use qcc::frontend::source::SourceBuffer;
use qcc::frontend::token::TokenKind;
use qcc::frontend::{Frontend, OpenQasmFrontend};

/// Helper to parse a standalone expression from source text.
fn parse_expr_helper(src: &str) -> Result<AstExpression, Box<qcc::diagnostics::Diagnostic>> {
    let buffer = SourceBuffer::new("test_expr.qasm", src);
    let (tokens, diags) = qcc::frontend::Lexer::tokenize(&buffer);
    assert!(diags.is_empty(), "Lexing failed: {:?}", diags);
    let mut parser = ExpressionParser::new(&buffer, &tokens);
    let expr = parser.parse_expression(0)?;
    assert!(
        parser.cursor() == tokens.len()
            || parser.peek().kind == TokenKind::Eof
            || parser.peek().kind == TokenKind::Semicolon,
        "Unconsumed tokens remaining"
    );
    Ok(expr)
}

// ============================================================================
// 1. Numeric Constants: Integer and Floating-Point
// ============================================================================

#[test]
fn test_numeric_constant_parsing() {
    let int_expr = parse_expr_helper("42").unwrap();
    assert_eq!(int_expr.as_constant(), Some(42.0));
    assert!(int_expr.is_constant());

    let float_expr = parse_expr_helper("1.25").unwrap();
    assert_eq!(float_expr.as_constant(), Some(1.25));
    assert!(float_expr.is_constant());

    let zero_expr = parse_expr_helper("0").unwrap();
    assert_eq!(zero_expr.as_constant(), Some(0.0));
    assert!(is_literal_zero(&zero_expr));
}

// ============================================================================
// 2. Pi and Symbolic Identifiers
// ============================================================================

#[test]
fn test_pi_and_symbolic_identifiers() {
    let empty_env = HashMap::new();

    // 1. ASCII "pi" constant
    let pi_expr = parse_expr_helper("pi").unwrap();
    // AST representation is preserved symbolically as an identifier (not eagerly converted)
    assert_eq!(pi_expr.as_identifier(), Some("pi"));
    // Evaluates to mathematical pi even with empty environment via both evaluate_expression and eval
    let pi_val1 = evaluate_expression(&pi_expr, &empty_env).unwrap();
    assert!((pi_val1 - std::f64::consts::PI).abs() < 1e-15);
    let pi_val2 = pi_expr.eval(&empty_env).unwrap();
    assert!((pi_val2 - std::f64::consts::PI).abs() < 1e-15);

    // 2. Unicode "π" constant
    let pi_unicode_expr = parse_expr_helper("π").unwrap();
    assert_eq!(pi_unicode_expr.as_identifier(), Some("π"));
    let pi_u_val1 = evaluate_expression(&pi_unicode_expr, &empty_env).unwrap();
    assert!((pi_u_val1 - std::f64::consts::PI).abs() < 1e-15);
    let pi_u_val2 = pi_unicode_expr.eval(&empty_env).unwrap();
    assert!((pi_u_val2 - std::f64::consts::PI).abs() < 1e-15);

    // 3. User parameter "theta" without binding: produces UnresolvedSymbol (never defaults to zero)
    let theta_expr = parse_expr_helper("theta").unwrap();
    assert_eq!(theta_expr.as_identifier(), Some("theta"));
    let theta_res = evaluate_expression(&theta_expr, &empty_env);
    match &theta_res {
        Err(ExpressionEvaluationError::UnresolvedSymbol { name, .. }) => {
            assert_eq!(name, "theta");
        }
        other => panic!("Expected UnresolvedSymbol(theta), got {:?}", other),
    }
    assert!(theta_expr
        .eval(&empty_env)
        .unwrap_err()
        .contains("unbound parameter symbol: 'theta'"));

    // 4. User parameter "theta" with binding: evaluates using the bound value
    let mut bound_env = HashMap::new();
    bound_env.insert("theta".to_string(), 1.234567);
    let theta_bound_val = evaluate_expression(&theta_expr, &bound_env).unwrap();
    assert!((theta_bound_val - 1.234567).abs() < 1e-15);
    let theta_bound_eval = theta_expr.eval(&bound_env).unwrap();
    assert!((theta_bound_eval - 1.234567).abs() < 1e-15);

    // 5. Additional symbols
    let alpha_expr = parse_expr_helper("alpha_1").unwrap();
    assert_eq!(alpha_expr.as_identifier(), Some("alpha_1"));
}

#[test]
fn test_mathematical_constants_and_symbolic_resolution() {
    let empty_env = HashMap::new();

    // 1. Division with ASCII pi: pi / 2
    let half_pi = parse_expr_helper("pi / 2").unwrap();
    match &half_pi.kind {
        AstExprKind::Div(lhs, rhs) => {
            assert_eq!(lhs.as_identifier(), Some("pi"));
            assert_eq!(rhs.as_constant(), Some(2.0));
        }
        other => panic!("Expected Div, got {:?}", other),
    }
    let half_pi_val = evaluate_expression(&half_pi, &empty_env).unwrap();
    assert!((half_pi_val - std::f64::consts::PI / 2.0).abs() < 1e-15);

    // 2. Division with Unicode π: π / 2
    let half_pi_u = parse_expr_helper("π / 2").unwrap();
    match &half_pi_u.kind {
        AstExprKind::Div(lhs, rhs) => {
            assert_eq!(lhs.as_identifier(), Some("π"));
            assert_eq!(rhs.as_constant(), Some(2.0));
        }
        other => panic!("Expected Div, got {:?}", other),
    }
    let half_pi_u_val = evaluate_expression(&half_pi_u, &empty_env).unwrap();
    assert!((half_pi_u_val - std::f64::consts::PI / 2.0).abs() < 1e-15);

    // 3. Composite expression with Unicode π and parameter: (theta + π) / 2
    let comp_expr = parse_expr_helper("(theta + π) / 2").unwrap();
    // Unbound fails on theta
    let unbound_res = evaluate_expression(&comp_expr, &empty_env);
    assert!(matches!(
        unbound_res,
        Err(ExpressionEvaluationError::UnresolvedSymbol { ref name, .. }) if name == "theta"
    ));
    // Bound evaluates correctly
    let mut env = HashMap::new();
    env.insert("theta".to_string(), 1.0);
    let bound_comp_val = evaluate_expression(&comp_expr, &env).unwrap();
    assert!((bound_comp_val - (1.0 + std::f64::consts::PI) / 2.0).abs() < 1e-15);

    // 4. Gate call parsing preserves Unicode π in parameter expression
    let src = "OPENQASM 3.0;\nqubit[1] q;\nrx(π / 4) q[0];\n";
    let frontend = OpenQasmFrontend::new();
    let res = frontend.parse(&SourceBuffer::new("gate_pi_unicode.qasm", src));
    assert!(res.is_ok(), "Parse failed: {:?}", res.diagnostics);
    let program = res.into_result().unwrap();
    match &program.statements[1] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.name, "rx");
            assert_eq!(call.parameters.len(), 1);
            match &call.parameters[0].kind {
                AstExprKind::Div(lhs, rhs) => {
                    assert_eq!(lhs.as_identifier(), Some("π"));
                    assert_eq!(rhs.as_constant(), Some(4.0));
                }
                other => panic!("Expected Div(π, 4), got {:?}", other),
            }
        }
        other => panic!("Expected GateCall, got {:?}", other),
    }

    // 5. Post-binding division by zero involving pi
    let div_zero_pi = parse_expr_helper("(pi + 1.0) / (theta - pi)").unwrap();
    let mut env_zero = HashMap::new();
    env_zero.insert("theta".to_string(), std::f64::consts::PI);
    let div_zero_res = evaluate_expression(&div_zero_pi, &env_zero);
    assert_eq!(
        div_zero_res,
        Err(ExpressionEvaluationError::DivisionByZero {
            span: div_zero_pi.span
        })
    );
    assert!(div_zero_pi
        .eval(&env_zero)
        .unwrap_err()
        .contains("division by zero"));

    // 6. Post-binding division by zero involving Unicode π
    let div_zero_pi_u = parse_expr_helper("(2.0 * π) / (theta - 3.0)").unwrap();
    let mut env_zero_3 = HashMap::new();
    env_zero_3.insert("theta".to_string(), 3.0);
    let div_zero_u_res = evaluate_expression(&div_zero_pi_u, &env_zero_3);
    assert_eq!(
        div_zero_u_res,
        Err(ExpressionEvaluationError::DivisionByZero {
            span: div_zero_pi_u.span
        })
    );
    assert!(div_zero_pi_u
        .eval(&env_zero_3)
        .unwrap_err()
        .contains("division by zero"));
}

// ============================================================================
// 3. Unary Positive and Negative Signs
// ============================================================================

#[test]
fn test_unary_signs() {
    // Unary negative: -theta
    let neg_theta = parse_expr_helper("-theta").unwrap();
    match &neg_theta.kind {
        AstExprKind::Neg(inner) => assert_eq!(inner.as_identifier(), Some("theta")),
        other => panic!("Expected Neg, got {:?}", other),
    }

    // Unary positive: +theta (unwrapped with span preserved)
    let pos_theta = parse_expr_helper("+theta").unwrap();
    assert_eq!(pos_theta.as_identifier(), Some("theta"));

    // Double unary negative: - -theta
    let double_neg = parse_expr_helper("- -theta").unwrap();
    match &double_neg.kind {
        AstExprKind::Neg(inner1) => match &inner1.kind {
            AstExprKind::Neg(inner2) => assert_eq!(inner2.as_identifier(), Some("theta")),
            other => panic!("Expected inner Neg, got {:?}", other),
        },
        other => panic!("Expected outer Neg, got {:?}", other),
    }

    // Unary negative on parenthesized: -(theta + pi)
    let neg_paren = parse_expr_helper("-(theta + pi)").unwrap();
    match &neg_paren.kind {
        AstExprKind::Neg(inner) => match &inner.kind {
            AstExprKind::Add(lhs, rhs) => {
                assert_eq!(lhs.as_identifier(), Some("theta"));
                assert_eq!(rhs.as_identifier(), Some("pi"));
            }
            other => panic!("Expected Add, got {:?}", other),
        },
        other => panic!("Expected Neg, got {:?}", other),
    }
}

// ============================================================================
// 4. Parenthesized Expressions
// ============================================================================

#[test]
fn test_parenthesized_expressions() {
    let expr = parse_expr_helper("((theta))").unwrap();
    assert_eq!(expr.as_identifier(), Some("theta"));

    let add_paren = parse_expr_helper("(theta + pi) * 2").unwrap();
    match &add_paren.kind {
        AstExprKind::Mul(lhs, rhs) => {
            match &lhs.kind {
                AstExprKind::Add(l, r) => {
                    assert_eq!(l.as_identifier(), Some("theta"));
                    assert_eq!(r.as_identifier(), Some("pi"));
                }
                other => panic!("Expected Add on lhs, got {:?}", other),
            }
            assert_eq!(rhs.as_constant(), Some(2.0));
        }
        other => panic!("Expected Mul, got {:?}", other),
    }
}

// ============================================================================
// 5. Addition and Subtraction
// ============================================================================

#[test]
fn test_addition_and_subtraction() {
    let add_expr = parse_expr_helper("theta + pi").unwrap();
    match &add_expr.kind {
        AstExprKind::Add(lhs, rhs) => {
            assert_eq!(lhs.as_identifier(), Some("theta"));
            assert_eq!(rhs.as_identifier(), Some("pi"));
        }
        other => panic!("Expected Add, got {:?}", other),
    }

    let sub_expr = parse_expr_helper("theta - 1.5").unwrap();
    match &sub_expr.kind {
        AstExprKind::Sub(lhs, rhs) => {
            assert_eq!(lhs.as_identifier(), Some("theta"));
            assert_eq!(rhs.as_constant(), Some(1.5));
        }
        other => panic!("Expected Sub, got {:?}", other),
    }
}

// ============================================================================
// 6. Multiplication and Division
// ============================================================================

#[test]
fn test_multiplication_and_division() {
    let mul_expr = parse_expr_helper("theta * 2").unwrap();
    match &mul_expr.kind {
        AstExprKind::Mul(lhs, rhs) => {
            assert_eq!(lhs.as_identifier(), Some("theta"));
            assert_eq!(rhs.as_constant(), Some(2.0));
        }
        other => panic!("Expected Mul, got {:?}", other),
    }

    let div_expr = parse_expr_helper("pi / 4").unwrap();
    match &div_expr.kind {
        AstExprKind::Div(lhs, rhs) => {
            assert_eq!(lhs.as_identifier(), Some("pi"));
            assert_eq!(rhs.as_constant(), Some(4.0));
        }
        other => panic!("Expected Div, got {:?}", other),
    }
}

// ============================================================================
// 7. Operator Precedence
// ============================================================================

#[test]
fn test_operator_precedence() {
    // theta * 2 + pi / 4  =>  (theta * 2) + (pi / 4)
    let expr1 = parse_expr_helper("theta * 2 + pi / 4").unwrap();
    match &expr1.kind {
        AstExprKind::Add(lhs, rhs) => {
            match &lhs.kind {
                AstExprKind::Mul(l, r) => {
                    assert_eq!(l.as_identifier(), Some("theta"));
                    assert_eq!(r.as_constant(), Some(2.0));
                }
                other => panic!("Expected Mul on lhs, got {:?}", other),
            }
            match &rhs.kind {
                AstExprKind::Div(l, r) => {
                    assert_eq!(l.as_identifier(), Some("pi"));
                    assert_eq!(r.as_constant(), Some(4.0));
                }
                other => panic!("Expected Div on rhs, got {:?}", other),
            }
        }
        other => panic!("Expected Add, got {:?}", other),
    }

    // theta + pi * 2  =>  theta + (pi * 2)
    let expr2 = parse_expr_helper("theta + pi * 2").unwrap();
    match &expr2.kind {
        AstExprKind::Add(lhs, rhs) => {
            assert_eq!(lhs.as_identifier(), Some("theta"));
            match &rhs.kind {
                AstExprKind::Mul(l, r) => {
                    assert_eq!(l.as_identifier(), Some("pi"));
                    assert_eq!(r.as_constant(), Some(2.0));
                }
                other => panic!("Expected Mul on rhs, got {:?}", other),
            }
        }
        other => panic!("Expected Add, got {:?}", other),
    }

    // -theta * 2  =>  (-theta) * 2
    let expr3 = parse_expr_helper("-theta * 2").unwrap();
    match &expr3.kind {
        AstExprKind::Mul(lhs, rhs) => {
            match &lhs.kind {
                AstExprKind::Neg(inner) => assert_eq!(inner.as_identifier(), Some("theta")),
                other => panic!("Expected Neg on lhs, got {:?}", other),
            }
            assert_eq!(rhs.as_constant(), Some(2.0));
        }
        other => panic!("Expected Mul, got {:?}", other),
    }
}

// ============================================================================
// 8. Left Associativity
// ============================================================================

#[test]
fn test_left_associativity() {
    // a - b - c  =>  (a - b) - c
    let sub3 = parse_expr_helper("a - b - c").unwrap();
    match &sub3.kind {
        AstExprKind::Sub(lhs, rhs) => {
            match &lhs.kind {
                AstExprKind::Sub(l, r) => {
                    assert_eq!(l.as_identifier(), Some("a"));
                    assert_eq!(r.as_identifier(), Some("b"));
                }
                other => panic!("Expected Sub on lhs, got {:?}", other),
            }
            assert_eq!(rhs.as_identifier(), Some("c"));
        }
        other => panic!("Expected Sub, got {:?}", other),
    }

    // a / b / c  =>  (a / b) / c
    let div3 = parse_expr_helper("a / b / c").unwrap();
    match &div3.kind {
        AstExprKind::Div(lhs, rhs) => {
            match &lhs.kind {
                AstExprKind::Div(l, r) => {
                    assert_eq!(l.as_identifier(), Some("a"));
                    assert_eq!(r.as_identifier(), Some("b"));
                }
                other => panic!("Expected Div on lhs, got {:?}", other),
            }
            assert_eq!(rhs.as_identifier(), Some("c"));
        }
        other => panic!("Expected Div, got {:?}", other),
    }

    // a * b * c  =>  (a * b) * c
    let mul3 = parse_expr_helper("a * b * c").unwrap();
    match &mul3.kind {
        AstExprKind::Mul(lhs, rhs) => {
            match &lhs.kind {
                AstExprKind::Mul(l, r) => {
                    assert_eq!(l.as_identifier(), Some("a"));
                    assert_eq!(r.as_identifier(), Some("b"));
                }
                other => panic!("Expected Mul on lhs, got {:?}", other),
            }
            assert_eq!(rhs.as_identifier(), Some("c"));
        }
        other => panic!("Expected Mul, got {:?}", other),
    }
}

// ============================================================================
// 9. Nested Expressions
// ============================================================================

#[test]
fn test_nested_expressions() {
    // (theta + pi) / 2
    let expr = parse_expr_helper("(theta + pi) / 2").unwrap();
    match &expr.kind {
        AstExprKind::Div(lhs, rhs) => {
            match &lhs.kind {
                AstExprKind::Add(l, r) => {
                    assert_eq!(l.as_identifier(), Some("theta"));
                    assert_eq!(r.as_identifier(), Some("pi"));
                }
                other => panic!("Expected Add, got {:?}", other),
            }
            assert_eq!(rhs.as_constant(), Some(2.0));
        }
        other => panic!("Expected Div, got {:?}", other),
    }

    // (a + b) * (c - d)
    let expr2 = parse_expr_helper("(a + b) * (c - d)").unwrap();
    match &expr2.kind {
        AstExprKind::Mul(lhs, rhs) => {
            assert!(matches!(&lhs.kind, AstExprKind::Add(..)));
            assert!(matches!(&rhs.kind, AstExprKind::Sub(..)));
        }
        other => panic!("Expected Mul, got {:?}", other),
    }
}

// ============================================================================
// 10. Symbolic Preservation (No Lossy Eager Evaluation)
// ============================================================================

#[test]
fn test_symbolic_preservation_when_unresolved() {
    let src = "rx(theta * 2 + pi / 4) q[0];";
    let buffer = SourceBuffer::new(
        "symbolic.qasm",
        format!("OPENQASM 3.0;\nqubit[1] q;\n{src}\n"),
    );
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(result.is_ok(), "Failed: {:?}", result.diagnostics);
    let program = result.into_result().unwrap();

    match &program.statements[1] {
        AstStatement::GateCall(call) => {
            assert_eq!(call.name, "rx");
            assert_eq!(call.parameters.len(), 1);
            let param = &call.parameters[0];

            // Verify AST tree structure is fully preserved
            match &param.kind {
                AstExprKind::Add(lhs, rhs) => {
                    match &lhs.kind {
                        AstExprKind::Mul(l, r) => {
                            assert_eq!(l.as_identifier(), Some("theta"));
                            assert_eq!(r.as_constant(), Some(2.0));
                        }
                        other => panic!("Expected Mul, got {:?}", other),
                    }
                    match &rhs.kind {
                        AstExprKind::Div(l, r) => {
                            assert_eq!(l.as_identifier(), Some("pi"));
                            assert_eq!(r.as_constant(), Some(4.0));
                        }
                        other => panic!("Expected Div, got {:?}", other),
                    }
                }
                other => panic!("Expected Add, got {:?}", other),
            }
        }
        other => panic!("Unexpected: {:?}", other),
    }
}

// ============================================================================
// 11. Explicit Evaluation with Symbol Environment
// ============================================================================

#[test]
fn test_explicit_evaluation_with_environment() {
    let expr = parse_expr_helper("(theta + pi) / 2").unwrap();

    let mut env = HashMap::new();
    env.insert("theta".to_string(), 1.0);

    let val = evaluate_expression(&expr, &env).unwrap();
    let expected = (1.0 + std::f64::consts::PI) / 2.0;
    assert!((val - expected).abs() < 1e-12);

    let expr2 = parse_expr_helper("theta * 2 + pi / 4").unwrap();
    let val2 = evaluate_expression(&expr2, &env).unwrap();
    let expected2 = 1.0 * 2.0 + std::f64::consts::PI / 4.0;
    assert!((val2 - expected2).abs() < 1e-12);
}

// ============================================================================
// 12. Literal Division by Zero Detection
// ============================================================================

#[test]
fn test_literal_division_by_zero_rejected() {
    // 1. Literal integer division by zero in parse_expression
    let res1 = parse_expr_helper("pi / 0");
    assert!(res1.is_err());
    let err1 = res1.unwrap_err();
    assert_eq!(err1.code.as_deref(), Some("E0030"));
    assert!(err1
        .message
        .contains("division by zero in parameter expression"));

    // 2. Literal float division by zero in parse_expression
    let res2 = parse_expr_helper("theta / 0.0");
    assert!(res2.is_err());
    let err2 = res2.unwrap_err();
    assert_eq!(err2.code.as_deref(), Some("E0030"));

    // 3. Literal zero in gate statement: rx(1.0 / 0) q[0];
    let src = "OPENQASM 3.0;\nqubit[1] q;\nrx(1.0 / 0) q[0];\n";
    let res3 = OpenQasmFrontend::new().parse(&SourceBuffer::new("div0_gate.qasm", src));
    assert!(!res3.is_ok());
    assert!(res3.errors().any(|e| e.code.as_deref() == Some("E0030")));
}

// ============================================================================
// 13. Division by Zero After Symbol Binding
// ============================================================================

#[test]
fn test_division_by_zero_after_symbol_binding() {
    let expr = parse_expr_helper("theta / (alpha - 2.0)").unwrap();

    let mut env = HashMap::new();
    env.insert("theta".to_string(), 1.0);
    env.insert("alpha".to_string(), 2.0); // alpha - 2.0 == 0.0!

    let eval_res = evaluate_expression(&expr, &env);
    assert_eq!(
        eval_res,
        Err(ExpressionEvaluationError::DivisionByZero { span: expr.span })
    );

    // Also verify AstExpression::eval string error
    let string_res = expr.eval(&env);
    assert!(string_res.is_err());
    assert!(string_res.unwrap_err().contains("division by zero"));
}

// ============================================================================
// 14. Unresolved Identifiers at Binding/Evaluation Time
// ============================================================================

#[test]
fn test_unresolved_identifiers_at_evaluation_time() {
    let expr = parse_expr_helper("(theta + pi) / 2").unwrap();

    let empty_env = HashMap::new();
    let eval_res = evaluate_expression(&expr, &empty_env);

    match &eval_res {
        Err(ExpressionEvaluationError::UnresolvedSymbol { name, .. }) => {
            assert_eq!(name, "theta");
        }
        other => panic!("Expected UnresolvedSymbol(theta), got {:?}", other),
    }

    // Verify error converts to structured diagnostic E0031
    let diag = eval_res.unwrap_err().to_diagnostic(Some("test.qasm"));
    assert_eq!(diag.code.as_deref(), Some("E0031"));
    assert!(diag.message.contains("unbound parameter symbol: 'theta'"));

    // Verify AstExpression::eval string error does NOT silently default to zero
    let string_err = expr.eval(&empty_env).unwrap_err();
    assert!(string_err.contains("unbound parameter symbol: 'theta'"));
}

// ============================================================================
// 15. Malformed Expressions, Missing Parentheses, and Missing Operands
// ============================================================================

#[test]
fn test_malformed_expressions() {
    // Missing operand after operator: theta +
    let res1 = parse_expr_helper("theta +");
    assert!(res1.is_err());
    assert_eq!(res1.unwrap_err().code.as_deref(), Some("E0001"));

    // Missing closing parenthesis: (theta + pi
    let res2 = parse_expr_helper("(theta + pi");
    assert!(res2.is_err());
    assert_eq!(res2.unwrap_err().code.as_deref(), Some("E0001"));

    // Unexpected leading binary operator: * 2
    let res3 = parse_expr_helper("* 2");
    assert!(res3.is_err());
    assert_eq!(res3.unwrap_err().code.as_deref(), Some("E0001"));

    // Empty expression
    let res4 = parse_expr_helper("");
    assert!(res4.is_err());
    assert_eq!(res4.unwrap_err().code.as_deref(), Some("E0001"));
}

// ============================================================================
// 16. Source-Span Correctness for Nested Nodes
// ============================================================================

#[test]
fn test_nested_expression_source_spans() {
    let expr = parse_expr_helper("theta + pi / 4").unwrap();
    // "theta + pi / 4" starts at 0, ends at 14
    assert_eq!(expr.span.start(), 0);
    assert_eq!(expr.span.end(), 14);

    match &expr.kind {
        AstExprKind::Add(lhs, rhs) => {
            // "theta" [0..5]
            assert_eq!(lhs.span.start(), 0);
            assert_eq!(lhs.span.end(), 5);

            // "pi / 4" [8..14]
            assert_eq!(rhs.span.start(), 8);
            assert_eq!(rhs.span.end(), 14);

            match &rhs.kind {
                AstExprKind::Div(num, den) => {
                    // "pi" [8..10]
                    assert_eq!(num.span.start(), 8);
                    assert_eq!(num.span.end(), 10);
                    // "4" [13..14]
                    assert_eq!(den.span.start(), 13);
                    assert_eq!(den.span.end(), 14);
                }
                other => panic!("Expected Div, got {:?}", other),
            }
        }
        other => panic!("Expected Add, got {:?}", other),
    }
}

// ============================================================================
// 17. Integration with rx, ry, and rz Gate Statements
// ============================================================================

#[test]
fn test_gate_statement_expression_integration() {
    let src = r#"OPENQASM 3.0;
qubit[3] q;

rx(1.2) q[0];
rx(pi) q[0];
ry(theta) q[1];
rz(pi / 4) q[2];
rx(-theta) q[0];
ry((theta + pi) / 2) q[1];
rz(theta * 2 + pi / 4) q[2];
"#;

    let buffer = SourceBuffer::new("gate_exprs.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let result = frontend.parse(&buffer);

    assert!(result.is_ok(), "Parse failed: {:?}", result.diagnostics);
    let program = result.into_result().unwrap();

    // 1 register declaration + 7 gate statements = 8 statements
    assert_eq!(program.statements.len(), 8);

    let gates = [
        ("rx", "1.2"),
        ("rx", "pi"),
        ("ry", "theta"),
        ("rz", "(pi / 4)"),
        ("rx", "-theta"),
        ("ry", "((theta + pi) / 2)"),
        ("rz", "((theta * 2) + (pi / 4))"),
    ];

    for (i, &(expected_gate, expected_display)) in gates.iter().enumerate() {
        match &program.statements[i + 1] {
            AstStatement::GateCall(call) => {
                assert_eq!(call.name, expected_gate);
                assert_eq!(call.parameters.len(), 1);
                assert_eq!(format!("{}", call.parameters[0]), expected_display);
            }
            other => panic!("Expected GateCall at {}, got {:?}", i + 1, other),
        }
    }
}

// ============================================================================
// 18. Phase 19 & Phase 20 Regression Tests
// ============================================================================

#[test]
fn test_phase19_and_phase20_regression() {
    let src = r#"OPENQASM 3.0;
include "stdgates.inc";

qubit[3] q;
bit[3] c;

x q[0];
h q[1];
rx(pi / 2) q[0];
cx q[0], q[1];
swap q[1], q[2];
"#;

    let buffer = SourceBuffer::new("full_regression.qasm", src);
    let frontend = OpenQasmFrontend::new();
    let (result, table) = frontend.parse_with_registers(&buffer);

    assert!(result.is_ok(), "Parse failed: {:?}", result.diagnostics);
    let program = result.into_result().unwrap();

    // Headers
    assert_eq!(program.version.as_ref().unwrap().specifier, "3.0");
    assert_eq!(program.includes.len(), 1);

    // Register Table
    assert_eq!(table.quantum_registers().len(), 1);
    assert_eq!(table.classical_registers().len(), 1);
    assert_eq!(table.total_qubits(), 3);
    assert_eq!(table.total_bits(), 3);

    // Statements: 2 decls + 5 gates = 7 statements
    assert_eq!(program.statements.len(), 7);

    let gate_names: Vec<_> = program
        .statements
        .iter()
        .filter_map(|s| match s {
            AstStatement::GateCall(g) => Some(g.name.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(gate_names, vec!["x", "h", "rx", "cx", "swap"]);
}
