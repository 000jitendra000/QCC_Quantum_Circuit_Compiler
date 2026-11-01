//! Parameter expressions supporting constant and symbolic circuit parameters.

use std::collections::HashMap;
use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

use serde::{Deserialize, Serialize};

use crate::quantum::parameter::Angle;

/// Represents a constant or symbolic parameter expression in QccIR.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParameterExpr {
    /// A constant numeric value.
    Constant(f64),
    /// A named symbolic parameter variable.
    Symbolic(String),
    /// Addition of two subexpressions.
    Add(Box<ParameterExpr>, Box<ParameterExpr>),
    /// Subtraction of two subexpressions.
    Sub(Box<ParameterExpr>, Box<ParameterExpr>),
    /// Multiplication of two subexpressions.
    Mul(Box<ParameterExpr>, Box<ParameterExpr>),
    /// Division of two subexpressions.
    Div(Box<ParameterExpr>, Box<ParameterExpr>),
    /// Negation of a subexpression.
    Neg(Box<ParameterExpr>),
}

impl ParameterExpr {
    /// Creates a constant parameter expression.
    #[inline]
    pub fn constant(val: f64) -> Self {
        Self::Constant(val)
    }

    /// Creates a symbolic variable parameter expression.
    #[inline]
    pub fn symbol(name: impl Into<String>) -> Self {
        Self::Symbolic(name.into())
    }

    /// Creates a zero parameter expression.
    #[inline]
    pub fn zero() -> Self {
        Self::Constant(0.0)
    }

    /// Returns `true` if this expression is a constant literal.
    pub fn is_constant(&self) -> bool {
        matches!(self, Self::Constant(_))
    }

    /// Evaluates the expression numerically using the provided symbol environment.
    ///
    /// # Errors
    /// Returns `Err` if a referenced symbol is not found in `env`, or if division by zero occurs.
    pub fn eval(&self, env: &HashMap<String, f64>) -> Result<f64, String> {
        match self {
            Self::Constant(v) => Ok(*v),
            Self::Symbolic(name) => env
                .get(name)
                .copied()
                .ok_or_else(|| format!("unbound parameter symbol: '{name}'")),
            Self::Add(a, b) => Ok(a.eval(env)? + b.eval(env)?),
            Self::Sub(a, b) => Ok(a.eval(env)? - b.eval(env)?),
            Self::Mul(a, b) => Ok(a.eval(env)? * b.eval(env)?),
            Self::Div(a, b) => {
                let divisor = b.eval(env)?;
                if divisor.abs() < 1e-15 {
                    return Err("division by zero in parameter expression".to_string());
                }
                Ok(a.eval(env)? / divisor)
            }
            Self::Neg(a) => Ok(-a.eval(env)?),
        }
    }

    /// Returns the constant float value if this expression is a constant literal.
    pub fn as_constant(&self) -> Option<f64> {
        match self {
            Self::Constant(v) => Some(*v),
            _ => None,
        }
    }
}

impl From<f64> for ParameterExpr {
    #[inline]
    fn from(v: f64) -> Self {
        Self::Constant(v)
    }
}

impl From<Angle> for ParameterExpr {
    #[inline]
    fn from(angle: Angle) -> Self {
        Self::Constant(angle.radians())
    }
}

impl Add for ParameterExpr {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        match (&self, &rhs) {
            (Self::Constant(a), Self::Constant(b)) => Self::Constant(a + b),
            _ => Self::Add(Box::new(self), Box::new(rhs)),
        }
    }
}

impl Sub for ParameterExpr {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        match (&self, &rhs) {
            (Self::Constant(a), Self::Constant(b)) => Self::Constant(a - b),
            _ => Self::Sub(Box::new(self), Box::new(rhs)),
        }
    }
}

impl Mul for ParameterExpr {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        match (&self, &rhs) {
            (Self::Constant(a), Self::Constant(b)) => Self::Constant(a * b),
            _ => Self::Mul(Box::new(self), Box::new(rhs)),
        }
    }
}

impl Neg for ParameterExpr {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        match self {
            Self::Constant(v) => Self::Constant(-v),
            _ => Self::Neg(Box::new(self)),
        }
    }
}

impl fmt::Display for ParameterExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Constant(v) => write!(f, "{v}"),
            Self::Symbolic(s) => write!(f, "{s}"),
            Self::Add(a, b) => write!(f, "({a} + {b})"),
            Self::Sub(a, b) => write!(f, "({a} - {b})"),
            Self::Mul(a, b) => write!(f, "({a} * {b})"),
            Self::Div(a, b) => write!(f, "({a} / {b})"),
            Self::Neg(a) => write!(f, "(-{a})"),
        }
    }
}

impl From<&str> for ParameterExpr {
    #[inline]
    fn from(s: &str) -> Self {
        Self::Symbolic(s.to_string())
    }
}

impl From<String> for ParameterExpr {
    #[inline]
    fn from(s: String) -> Self {
        Self::Symbolic(s)
    }
}
