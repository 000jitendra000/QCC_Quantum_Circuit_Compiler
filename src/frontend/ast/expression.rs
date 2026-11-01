//! AST nodes for arithmetic parameter expressions and operations.

use std::fmt;

use crate::diagnostics::SourceSpan;
use crate::ir::parameter::ParameterExpr;

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

    /// Converts this AST expression into QccIR's [`ParameterExpr`], mapping
    /// recognized constants (`"pi"`, `"π"`) to numerical $\pi$.
    ///
    /// # Errors
    /// Returns an error string if this expression contains operations not supported
    /// by QccIR's basic parameter model (such as function calls or powers).
    pub fn to_parameter_expr(&self) -> Result<ParameterExpr, String> {
        match &self.kind {
            AstExprKind::Constant(val) => Ok(ParameterExpr::Constant(*val)),
            AstExprKind::Identifier(name) => {
                if name == "pi" || name == "π" {
                    Ok(ParameterExpr::Constant(std::f64::consts::PI))
                } else {
                    Ok(ParameterExpr::Symbolic(name.clone()))
                }
            }
            AstExprKind::Add(lhs, rhs) => {
                let l = lhs.to_parameter_expr()?;
                let r = rhs.to_parameter_expr()?;
                Ok(ParameterExpr::Add(Box::new(l), Box::new(r)))
            }
            AstExprKind::Sub(lhs, rhs) => {
                let l = lhs.to_parameter_expr()?;
                let r = rhs.to_parameter_expr()?;
                Ok(ParameterExpr::Sub(Box::new(l), Box::new(r)))
            }
            AstExprKind::Mul(lhs, rhs) => {
                let l = lhs.to_parameter_expr()?;
                let r = rhs.to_parameter_expr()?;
                Ok(ParameterExpr::Mul(Box::new(l), Box::new(r)))
            }
            AstExprKind::Div(lhs, rhs) => {
                let l = lhs.to_parameter_expr()?;
                let r = rhs.to_parameter_expr()?;
                Ok(ParameterExpr::Div(Box::new(l), Box::new(r)))
            }
            AstExprKind::Neg(inner) => {
                let val = inner.to_parameter_expr()?;
                Ok(ParameterExpr::Neg(Box::new(val)))
            }
            AstExprKind::Pow(_, _) => Err(
                "exponentiation (^) is not supported in QccIR parameter expressions".to_string(),
            ),
            AstExprKind::Call(name, _) => Err(format!(
                "function call '{name}' is not supported in QccIR parameter expressions"
            )),
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
