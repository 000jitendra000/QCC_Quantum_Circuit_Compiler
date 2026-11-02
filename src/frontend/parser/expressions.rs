//! Bounded OpenQASM 3 parameter-expression parser.
//!
//! Implements operator precedence parsing for arithmetic parameter expressions,
//! supporting numeric constants, `pi`, symbolic identifiers, unary signs (`+`, `-`),
//! parentheses, and binary arithmetic (`+`, `-`, `*`, `/`) with left associativity.
//! Preserves expressions symbolically without eager lossy floating-point evaluation,
//! and provides explicit division-by-zero and unresolved symbol diagnostics.

use std::collections::HashMap;
use std::fmt;

use crate::diagnostics::{Diagnostic, SourceSpan};
use crate::frontend::ast::{AstExprKind, AstExpression};
use crate::frontend::source::SourceBuffer;
use crate::frontend::token::{Token, TokenKind};

/// Structured error produced during AST expression binding or numerical evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionEvaluationError {
    /// An identifier was encountered that is not `"pi"`/`"π"` and was not provided in the environment.
    UnresolvedSymbol {
        /// Name of the unbound symbol.
        name: String,
        /// Source span of the identifier node.
        span: SourceSpan,
    },
    /// Division by zero was detected during numerical evaluation.
    DivisionByZero {
        /// Source span of the division operator or expression.
        span: SourceSpan,
    },
    /// Numerical or function domain evaluation failure.
    EvaluationFailed {
        /// Human-readable explanation.
        message: String,
        /// Source span where the failure occurred.
        span: SourceSpan,
    },
}

impl ExpressionEvaluationError {
    /// Converts this evaluation error into a compiler [`Diagnostic`].
    pub fn to_diagnostic(&self, file_path: Option<&str>) -> Diagnostic {
        match self {
            Self::UnresolvedSymbol { name, span } => {
                Diagnostic::error(format!("unbound parameter symbol: '{name}'"))
                    .with_code("E0031")
                    .with_span(*span)
                    .with_file_path(file_path.unwrap_or(""))
            }
            Self::DivisionByZero { span } => {
                Diagnostic::error("division by zero in parameter expression")
                    .with_code("E0030")
                    .with_span(*span)
                    .with_file_path(file_path.unwrap_or(""))
            }
            Self::EvaluationFailed { message, span } => Diagnostic::error(message.clone())
                .with_code("E0032")
                .with_span(*span)
                .with_file_path(file_path.unwrap_or("")),
        }
    }
}

impl fmt::Display for ExpressionEvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedSymbol { name, span } => {
                write!(f, "unbound parameter symbol: '{name}' at {span}")
            }
            Self::DivisionByZero { span } => {
                write!(f, "division by zero in parameter expression at {span}")
            }
            Self::EvaluationFailed { message, span } => {
                write!(f, "{message} at {span}")
            }
        }
    }
}

impl std::error::Error for ExpressionEvaluationError {}

/// Checks whether an expression represents a literal zero constant (e.g. `0`, `0.0`, `-0.0`).
pub fn is_literal_zero(expr: &AstExpression) -> bool {
    match &expr.kind {
        AstExprKind::Constant(val) => *val == 0.0,
        AstExprKind::Neg(inner) => is_literal_zero(inner),
        _ => false,
    }
}

/// Evaluates an [`AstExpression`] against a supplied symbol environment, resolving `"pi"`
/// and `"π"` to mathematical $\pi \approx 3.141592653589793$ without requiring bindings in `env`.
///
/// # Errors
/// Returns [`ExpressionEvaluationError::UnresolvedSymbol`] if a symbol is missing from `env`.
/// Returns [`ExpressionEvaluationError::DivisionByZero`] if a denominator evaluates to zero.
pub fn evaluate_expression(
    expr: &AstExpression,
    env: &HashMap<String, f64>,
) -> Result<f64, ExpressionEvaluationError> {
    match &expr.kind {
        AstExprKind::Constant(val) => Ok(*val),
        AstExprKind::Identifier(name) => {
            if name == "pi" || name == "π" {
                Ok(std::f64::consts::PI)
            } else if let Some(&val) = env.get(name) {
                Ok(val)
            } else {
                Err(ExpressionEvaluationError::UnresolvedSymbol {
                    name: name.clone(),
                    span: expr.span,
                })
            }
        }
        AstExprKind::Add(lhs, rhs) => {
            Ok(evaluate_expression(lhs, env)? + evaluate_expression(rhs, env)?)
        }
        AstExprKind::Sub(lhs, rhs) => {
            Ok(evaluate_expression(lhs, env)? - evaluate_expression(rhs, env)?)
        }
        AstExprKind::Mul(lhs, rhs) => {
            Ok(evaluate_expression(lhs, env)? * evaluate_expression(rhs, env)?)
        }
        AstExprKind::Div(lhs, rhs) => {
            let num = evaluate_expression(lhs, env)?;
            let denom = evaluate_expression(rhs, env)?;
            if denom == 0.0 {
                return Err(ExpressionEvaluationError::DivisionByZero { span: expr.span });
            }
            let res = num / denom;
            if !res.is_finite() {
                return Err(ExpressionEvaluationError::EvaluationFailed {
                    message: "division resulted in non-finite value (overflow)".to_string(),
                    span: expr.span,
                });
            }
            Ok(res)
        }
        AstExprKind::Neg(inner) => Ok(-evaluate_expression(inner, env)?),
        AstExprKind::Pow(base, exp) => {
            Ok(evaluate_expression(base, env)?.powf(evaluate_expression(exp, env)?))
        }
        AstExprKind::Call(name, args) => match name.as_str() {
            "sin" => {
                if args.len() != 1 {
                    return Err(ExpressionEvaluationError::EvaluationFailed {
                        message: "sin expects 1 argument".to_string(),
                        span: expr.span,
                    });
                }
                Ok(evaluate_expression(&args[0], env)?.sin())
            }
            "cos" => {
                if args.len() != 1 {
                    return Err(ExpressionEvaluationError::EvaluationFailed {
                        message: "cos expects 1 argument".to_string(),
                        span: expr.span,
                    });
                }
                Ok(evaluate_expression(&args[0], env)?.cos())
            }
            "tan" => {
                if args.len() != 1 {
                    return Err(ExpressionEvaluationError::EvaluationFailed {
                        message: "tan expects 1 argument".to_string(),
                        span: expr.span,
                    });
                }
                Ok(evaluate_expression(&args[0], env)?.tan())
            }
            "sqrt" => {
                if args.len() != 1 {
                    return Err(ExpressionEvaluationError::EvaluationFailed {
                        message: "sqrt expects 1 argument".to_string(),
                        span: expr.span,
                    });
                }
                let val = evaluate_expression(&args[0], env)?;
                if val < 0.0 {
                    return Err(ExpressionEvaluationError::EvaluationFailed {
                        message: "cannot take square root of negative number".to_string(),
                        span: expr.span,
                    });
                }
                Ok(val.sqrt())
            }
            "exp" => {
                if args.len() != 1 {
                    return Err(ExpressionEvaluationError::EvaluationFailed {
                        message: "exp expects 1 argument".to_string(),
                        span: expr.span,
                    });
                }
                Ok(evaluate_expression(&args[0], env)?.exp())
            }
            "ln" => {
                if args.len() != 1 {
                    return Err(ExpressionEvaluationError::EvaluationFailed {
                        message: "ln expects 1 argument".to_string(),
                        span: expr.span,
                    });
                }
                let val = evaluate_expression(&args[0], env)?;
                if val <= 0.0 {
                    return Err(ExpressionEvaluationError::EvaluationFailed {
                        message: "cannot take logarithm of non-positive number".to_string(),
                        span: expr.span,
                    });
                }
                Ok(val.ln())
            }
            _ => Err(ExpressionEvaluationError::EvaluationFailed {
                message: format!("unsupported function in AST evaluation: '{name}'"),
                span: expr.span,
            }),
        },
    }
}

/// Maximum allowed nesting depth for parameter expressions to prevent stack exhaustion on malformed input.
pub const MAX_EXPRESSION_DEPTH: usize = 128;

/// Recursive-descent / Pratt operator precedence parser for parameter expressions.
pub struct ExpressionParser<'a> {
    buffer: &'a SourceBuffer,
    tokens: &'a [Token],
    cursor: usize,
    eof_token: Token,
    depth: usize,
    max_depth: usize,
}

impl<'a> ExpressionParser<'a> {
    /// Constructs a new [`ExpressionParser`] over the given token slice and source buffer.
    pub fn new(buffer: &'a SourceBuffer, tokens: &'a [Token]) -> Self {
        let eof_offset = buffer.len();
        let eof_span = buffer
            .point_span(eof_offset)
            .unwrap_or_else(|_| SourceSpan::point(eof_offset, 1, 1).unwrap());
        let eof_token = Token::eof(eof_offset, eof_span);
        Self {
            buffer,
            tokens,
            cursor: 0,
            eof_token,
            depth: 0,
            max_depth: MAX_EXPRESSION_DEPTH,
        }
    }

    /// Configures a custom maximum expression nesting depth limit.
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Constructs an [`ExpressionParser`] starting at a specific token cursor position.
    pub fn with_cursor(buffer: &'a SourceBuffer, tokens: &'a [Token], cursor: usize) -> Self {
        let mut parser = Self::new(buffer, tokens);
        parser.set_cursor(cursor);
        parser
    }

    /// Returns the current token cursor position.
    #[inline]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Sets the token cursor position.
    #[inline]
    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.tokens.len());
    }

    /// Returns `true` if the cursor has reached EOF or the end of the token slice.
    #[inline]
    pub fn is_at_end(&self) -> bool {
        self.cursor >= self.tokens.len() || self.peek().kind == TokenKind::Eof
    }

    /// Peeks at the next token without advancing.
    #[inline]
    pub fn peek(&self) -> &Token {
        if self.cursor < self.tokens.len() {
            &self.tokens[self.cursor]
        } else {
            &self.eof_token
        }
    }

    /// Advances the cursor and returns a reference to the consumed token.
    #[inline]
    pub fn advance(&mut self) -> &Token {
        if self.cursor < self.tokens.len() {
            let tok = &self.tokens[self.cursor];
            self.cursor += 1;
            tok
        } else {
            &self.eof_token
        }
    }

    /// Skips tokens until a statement-terminating semicolon or EOF is reached.
    pub fn skip_to_semicolon(&mut self) {
        while self.cursor < self.tokens.len() {
            let tok = self.advance();
            if tok.kind == TokenKind::Semicolon || tok.kind == TokenKind::Eof {
                break;
            }
        }
    }

    /// Parses an expression using Pratt operator precedence parsing.
    ///
    /// Precedence hierarchy:
    /// 1. Parenthesized subexpressions and primary literals/identifiers.
    /// 2. Unary prefix `+` and `-` (binding power 5).
    /// 3. Multiplicative operators `*` and `/` (binding powers: left 3, right 4).
    /// 4. Additive operators `+` and `-` (binding powers: left 1, right 2).
    ///
    /// Left associativity is guaranteed because left binding power is strictly less than right binding power.
    ///
    /// # Errors
    /// Returns a [`Diagnostic`] if a syntax error occurs, if literal division by zero is detected,
    /// or if the expression exceeds [`MAX_EXPRESSION_DEPTH`].
    pub fn parse_expression(&mut self, min_bp: u8) -> Result<AstExpression, Box<Diagnostic>> {
        if self.depth >= self.max_depth {
            let span = self.peek().span;
            return Err(Box::new(
                Diagnostic::error(format!(
                    "expression nesting depth exceeds maximum limit of {}",
                    self.max_depth
                ))
                .with_code("E0001")
                .with_span(span)
                .with_file_path(self.buffer.name()),
            ));
        }

        self.depth += 1;
        let res = self.parse_expression_internal(min_bp);
        self.depth -= 1;
        res
    }

    fn parse_expression_internal(&mut self, min_bp: u8) -> Result<AstExpression, Box<Diagnostic>> {
        let mut lhs = self.parse_primary_expression()?;

        loop {
            let tok = self.peek();
            let (lbp, rbp) = match tok.kind {
                TokenKind::Plus | TokenKind::Minus => (1, 2),
                TokenKind::Star | TokenKind::Slash => (3, 4),
                _ => break,
            };

            if lbp < min_bp {
                break;
            }

            let op_tok = self.advance().clone();
            let rhs = self.parse_expression(rbp)?;

            // Detect literal division by zero at parse time
            if op_tok.kind == TokenKind::Slash && is_literal_zero(&rhs) {
                let span = self
                    .buffer
                    .span(op_tok.start(), rhs.span.end())
                    .unwrap_or(rhs.span);
                return Err(Box::new(
                    Diagnostic::error("division by zero in parameter expression")
                        .with_code("E0030")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ));
            }

            let span = self
                .buffer
                .span(lhs.span.start(), rhs.span.end())
                .unwrap_or(lhs.span);

            lhs = match op_tok.kind {
                TokenKind::Plus => AstExpression {
                    kind: AstExprKind::Add(Box::new(lhs), Box::new(rhs)),
                    span,
                },
                TokenKind::Minus => AstExpression {
                    kind: AstExprKind::Sub(Box::new(lhs), Box::new(rhs)),
                    span,
                },
                TokenKind::Star => AstExpression {
                    kind: AstExprKind::Mul(Box::new(lhs), Box::new(rhs)),
                    span,
                },
                TokenKind::Slash => AstExpression {
                    kind: AstExprKind::Div(Box::new(lhs), Box::new(rhs)),
                    span,
                },
                _ => unreachable!(),
            };
        }

        Ok(lhs)
    }

    /// Parses a primary expression (constant, identifier, unary sign, or parenthesized subexpression).
    pub fn parse_primary_expression(&mut self) -> Result<AstExpression, Box<Diagnostic>> {
        let tok = self.advance().clone();
        match tok.kind {
            TokenKind::FloatLiteral(f) => Ok(AstExpression::constant(f, tok.span)),
            TokenKind::IntLiteral(i) => Ok(AstExpression::constant(i as f64, tok.span)),
            TokenKind::Ident(name) => Ok(AstExpression::identifier(name, tok.span)),
            TokenKind::Plus => {
                // Unary positive sign: identity wrapper preserving span
                let inner = self.parse_expression(5)?;
                let span = self
                    .buffer
                    .span(tok.start(), inner.span.end())
                    .unwrap_or(inner.span);
                Ok(AstExpression {
                    kind: inner.kind,
                    span,
                })
            }
            TokenKind::Minus => {
                // Unary negative sign: Neg wrapper
                let inner = self.parse_expression(5)?;
                let span = self
                    .buffer
                    .span(tok.start(), inner.span.end())
                    .unwrap_or(inner.span);
                Ok(AstExpression {
                    kind: AstExprKind::Neg(Box::new(inner)),
                    span,
                })
            }
            TokenKind::LParen => {
                let inner = self.parse_expression(0)?;
                let rparen = self.advance().clone();
                if rparen.kind != TokenKind::RParen {
                    let span = rparen.span;
                    return Err(Box::new(
                        Diagnostic::error("expected ')' closing subexpression")
                            .with_code("E0001")
                            .with_span(span)
                            .with_file_path(self.buffer.name()),
                    ));
                }
                let span = self
                    .buffer
                    .span(tok.start(), rparen.end())
                    .unwrap_or(inner.span);
                Ok(AstExpression {
                    kind: inner.kind,
                    span,
                })
            }
            _ => {
                let span = tok.span;
                Err(Box::new(
                    Diagnostic::error("expected parameter expression")
                        .with_code("E0001")
                        .with_span(span)
                        .with_file_path(self.buffer.name()),
                ))
            }
        }
    }
}
