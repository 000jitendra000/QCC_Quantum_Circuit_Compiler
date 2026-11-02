//! AST nodes for arithmetic parameter expressions and operations.

use std::collections::HashMap;
use std::fmt;

use crate::diagnostics::SourceSpan;

/// Arithmetic parameter expression node with exact source provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct AstExpression {
    /// Expression tree variant.
    pub kind: AstExprKind,
    /// Source span covering the expression.
    pub span: SourceSpan,
}

/// Tree variants of an OpenQASM parameter expression.
#[derive(Clone, Debug, PartialEq)]
pub enum AstExprKind {
    /// Constant floating-point literal.
    Constant(f64),
    /// Variable identifier or named mathematical constant (e.g. `"pi"`, `"theta"`).
    Identifier(String),
    /// Addition of two subexpressions.
    Add(Box<AstExpression>, Box<AstExpression>),
    /// Subtraction of two subexpressions.
    Sub(Box<AstExpression>, Box<AstExpression>),
    /// Multiplication of two subexpressions.
    Mul(Box<AstExpression>, Box<AstExpression>),
    /// Division of two subexpressions.
    Div(Box<AstExpression>, Box<AstExpression>),
    /// Unary negation of a subexpression.
    Neg(Box<AstExpression>),
    /// Exponentiation / power operation (`base ^ exponent`).
    Pow(Box<AstExpression>, Box<AstExpression>),
    /// Function call invocation (e.g. `sin(x)`, `sqrt(2.0)`).
    Call(String, Vec<AstExpression>),
}

impl AstExpression {
    /// Constructs a new expression node from a kind and source span.
    #[inline]
    pub fn new(kind: AstExprKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }

    /// Constructs a constant literal expression.
    #[inline]
    pub fn constant(val: f64, span: SourceSpan) -> Self {
        Self {
            kind: AstExprKind::Constant(val),
            span,
        }
    }

    /// Constructs an identifier or symbol expression.
    #[inline]
    pub fn identifier(name: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            kind: AstExprKind::Identifier(name.into()),
            span,
        }
    }

    /// Constructs an addition expression.
    #[inline]
    pub fn add(lhs: AstExpression, rhs: AstExpression, span: SourceSpan) -> Self {
        Self {
            kind: AstExprKind::Add(Box::new(lhs), Box::new(rhs)),
            span,
        }
    }

    /// Constructs a subtraction expression.
    #[inline]
    pub fn sub(lhs: AstExpression, rhs: AstExpression, span: SourceSpan) -> Self {
        Self {
            kind: AstExprKind::Sub(Box::new(lhs), Box::new(rhs)),
            span,
        }
    }

    /// Constructs a multiplication expression.
    #[inline]
    pub fn mul(lhs: AstExpression, rhs: AstExpression, span: SourceSpan) -> Self {
        Self {
            kind: AstExprKind::Mul(Box::new(lhs), Box::new(rhs)),
            span,
        }
    }

    /// Constructs a division expression.
    #[inline]
    pub fn div(lhs: AstExpression, rhs: AstExpression, span: SourceSpan) -> Self {
        Self {
            kind: AstExprKind::Div(Box::new(lhs), Box::new(rhs)),
            span,
        }
    }

    /// Constructs a unary negation expression.
    #[inline]
    pub fn neg(inner: AstExpression, span: SourceSpan) -> Self {
        Self {
            kind: AstExprKind::Neg(Box::new(inner)),
            span,
        }
    }

    /// Constructs an exponentiation expression.
    #[inline]
    pub fn pow(lhs: AstExpression, rhs: AstExpression, span: SourceSpan) -> Self {
        Self {
            kind: AstExprKind::Pow(Box::new(lhs), Box::new(rhs)),
            span,
        }
    }

    /// Constructs a function call expression.
    #[inline]
    pub fn call(name: impl Into<String>, args: Vec<AstExpression>, span: SourceSpan) -> Self {
        Self {
            kind: AstExprKind::Call(name.into(), args),
            span,
        }
    }

    /// Returns the source span of the expression.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }

    /// Returns `true` if this expression is a constant literal.
    #[inline]
    pub fn is_constant(&self) -> bool {
        matches!(self.kind, AstExprKind::Constant(_))
    }

    /// Returns `true` if this expression is an identifier.
    #[inline]
    pub fn is_identifier(&self) -> bool {
        matches!(self.kind, AstExprKind::Identifier(_))
    }

    /// Returns the constant float value if this expression is a constant literal.
    #[inline]
    pub fn as_constant(&self) -> Option<f64> {
        match self.kind {
            AstExprKind::Constant(val) => Some(val),
            _ => None,
        }
    }

    /// Returns the identifier name if this expression is an identifier.
    #[inline]
    pub fn as_identifier(&self) -> Option<&str> {
        match &self.kind {
            AstExprKind::Identifier(name) => Some(name.as_str()),
            _ => None,
        }
    }

    /// Evaluates this AST expression numerically, resolving `"pi"` and `"π"`
    /// to mathematical $\pi$, looking up symbols in `env`, and computing arithmetic.
    ///
    /// This evaluation is purely frontend AST-level arithmetic and does not
    /// construct or depend upon any QccIR types.
    pub fn eval(&self, env: &HashMap<String, f64>) -> Result<f64, String> {
        match &self.kind {
            AstExprKind::Constant(val) => Ok(*val),
            AstExprKind::Identifier(name) => {
                if name == "pi" || name == "π" {
                    Ok(std::f64::consts::PI)
                } else if let Some(&v) = env.get(name) {
                    Ok(v)
                } else {
                    Err(format!("unbound parameter symbol: '{name}'"))
                }
            }
            AstExprKind::Add(lhs, rhs) => Ok(lhs.eval(env)? + rhs.eval(env)?),
            AstExprKind::Sub(lhs, rhs) => Ok(lhs.eval(env)? - rhs.eval(env)?),
            AstExprKind::Mul(lhs, rhs) => Ok(lhs.eval(env)? * rhs.eval(env)?),
            AstExprKind::Div(lhs, rhs) => {
                let divisor = rhs.eval(env)?;
                if divisor == 0.0 {
                    return Err("division by zero in parameter expression".to_string());
                }
                let res = lhs.eval(env)? / divisor;
                if !res.is_finite() {
                    return Err("division resulted in non-finite value (overflow)".to_string());
                }
                Ok(res)
            }
            AstExprKind::Neg(inner) => Ok(-inner.eval(env)?),
            AstExprKind::Pow(base, exp) => Ok(base.eval(env)?.powf(exp.eval(env)?)),
            AstExprKind::Call(name, args) => match name.as_str() {
                "sin" => {
                    if args.len() != 1 {
                        return Err("sin expects 1 argument".to_string());
                    }
                    Ok(args[0].eval(env)?.sin())
                }
                "cos" => {
                    if args.len() != 1 {
                        return Err("cos expects 1 argument".to_string());
                    }
                    Ok(args[0].eval(env)?.cos())
                }
                "tan" => {
                    if args.len() != 1 {
                        return Err("tan expects 1 argument".to_string());
                    }
                    Ok(args[0].eval(env)?.tan())
                }
                "sqrt" => {
                    if args.len() != 1 {
                        return Err("sqrt expects 1 argument".to_string());
                    }
                    let val = args[0].eval(env)?;
                    if val < 0.0 {
                        return Err("cannot take square root of negative number".to_string());
                    }
                    Ok(val.sqrt())
                }
                "exp" => {
                    if args.len() != 1 {
                        return Err("exp expects 1 argument".to_string());
                    }
                    Ok(args[0].eval(env)?.exp())
                }
                "ln" => {
                    if args.len() != 1 {
                        return Err("ln expects 1 argument".to_string());
                    }
                    let val = args[0].eval(env)?;
                    if val <= 0.0 {
                        return Err("cannot take logarithm of non-positive number".to_string());
                    }
                    Ok(val.ln())
                }
                _ => Err(format!("unsupported function in AST evaluation: '{name}'")),
            },
        }
    }
}

impl fmt::Display for AstExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            AstExprKind::Constant(val) => write!(f, "{val}"),
            AstExprKind::Identifier(name) => write!(f, "{name}"),
            AstExprKind::Add(lhs, rhs) => write!(f, "({lhs} + {rhs})"),
            AstExprKind::Sub(lhs, rhs) => write!(f, "({lhs} - {rhs})"),
            AstExprKind::Mul(lhs, rhs) => write!(f, "({lhs} * {rhs})"),
            AstExprKind::Div(lhs, rhs) => write!(f, "({lhs} / {rhs})"),
            AstExprKind::Neg(inner) => write!(f, "-{inner}"),
            AstExprKind::Pow(lhs, rhs) => write!(f, "({lhs} ^ {rhs})"),
            AstExprKind::Call(name, args) => {
                write!(f, "{name}(")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ")")
            }
        }
    }
}
