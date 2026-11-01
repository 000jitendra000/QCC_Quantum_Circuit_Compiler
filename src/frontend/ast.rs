//! Typed Abstract Syntax Tree (AST) representing the supported OpenQASM 3 subset.
//!
//! All AST nodes are owned exclusively by the frontend boundary and retain exact
//! [`SourceSpan`] coordinates for diagnostic provenance without leaking parser-specific types.

use std::fmt;

use crate::diagnostics::SourceSpan;
use crate::ir::parameter::ParameterExpr;

/// Root AST node representing a complete parsed OpenQASM 3 compilation unit.
#[derive(Clone, Debug, PartialEq)]
pub struct AstProgram {
    /// Optional OpenQASM version header (e.g. `OPENQASM 3.0;`).
    pub version: Option<AstVersion>,
    /// Included header files (e.g. `include "stdgates.inc";`).
    pub includes: Vec<AstInclude>,
    /// Top-level circuit statements in source order.
    pub statements: Vec<AstStatement>,
    /// Source span covering the full program text.
    pub span: SourceSpan,
}

impl AstProgram {
    /// Constructs a new [`AstProgram`].
    pub fn new(
        version: Option<AstVersion>,
        includes: Vec<AstInclude>,
        statements: Vec<AstStatement>,
        span: SourceSpan,
    ) -> Self {
        Self {
            version,
            includes,
            statements,
            span,
        }
    }
}

/// Version specification header node (e.g. `OPENQASM 3.0;`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstVersion {
    /// Version string literal (e.g. `"3.0"` or `"3"`).
    pub specifier: String,
    /// Source span of the version declaration.
    pub span: SourceSpan,
}

/// Include directive node (e.g. `include "stdgates.inc";`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstInclude {
    /// Included file path.
    pub path: String,
    /// Source span of the include statement.
    pub span: SourceSpan,
}

/// Top-level AST statement variant.
#[derive(Clone, Debug, PartialEq)]
pub enum AstStatement {
    /// Register declaration statement (`qubit[n] q;`, `bit[n] c;`).
    RegisterDecl(AstRegisterDecl),
    /// Standard or parameterized gate application statement (`h q[0];`, `cx q[0], q[1];`).
    GateCall(AstGateCall),
    /// Measurement operation (`c[0] = measure q[0];` or `measure q[0] -> c[0];`).
    Measure(AstMeasure),
    /// Active state reset (`reset q[0];`).
    Reset(AstReset),
    /// Synchronization barrier (`barrier;` or `barrier q[0], q[1];`).
    Barrier(AstBarrier),
    /// Explicit circuit global phase directive (`gphase(expr);`).
    GlobalPhase(AstGlobalPhase),
    /// An unsupported construct preserved for diagnostics and error recovery.
    Unsupported(AstUnsupported),
}

impl AstStatement {
    /// Returns the [`SourceSpan`] associated with this statement.
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::RegisterDecl(s) => s.span,
            Self::GateCall(s) => s.span,
            Self::Measure(s) => s.span,
            Self::Reset(s) => s.span,
            Self::Barrier(s) => s.span,
            Self::GlobalPhase(s) => s.span,
            Self::Unsupported(s) => s.span,
        }
    }
}

/// Quantum or classical register declaration kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AstRegisterKind {
    /// Quantum register (`qubit`).
    Qubit,
    /// Classical bit register (`bit`).
    Bit,
}

impl fmt::Display for AstRegisterKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Qubit => write!(f, "qubit"),
            Self::Bit => write!(f, "bit"),
        }
    }
}

/// Register declaration statement node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstRegisterDecl {
    /// Register type (`qubit` or `bit`).
    pub kind: AstRegisterKind,
    /// Declared register identifier.
    pub name: String,
    /// Declared register size (`None` for scalar declarations such as `qubit q;`).
    pub size: Option<usize>,
    /// Source span of the full declaration statement.
    pub span: SourceSpan,
}

/// Gate call invocation statement node.
#[derive(Clone, Debug, PartialEq)]
pub struct AstGateCall {
    /// Case-sensitive gate name (e.g. `"h"`, `"cx"`, `"rx"`).
    pub name: String,
    /// Parameter expressions supplied to the gate (e.g. `[pi / 2]`).
    pub parameters: Vec<AstExpression>,
    /// Ordered qubit operands supplied to the gate.
    pub qubits: Vec<AstQubitRef>,
    /// Source span of the gate call statement.
    pub span: SourceSpan,
}

/// Qubit reference operand node.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AstQubitRef {
    /// Register name.
    pub name: String,
    /// Optional 0-indexed element within the register (`None` for scalar register references).
    pub index: Option<usize>,
    /// Source span of the qubit reference.
    pub span: SourceSpan,
}

impl fmt::Display for AstQubitRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(idx) = self.index {
            write!(f, "{}[{idx}]", self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

/// Classical bit reference operand node.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AstBitRef {
    /// Register name.
    pub name: String,
    /// Optional 0-indexed element within the register (`None` for scalar register references).
    pub index: Option<usize>,
    /// Source span of the bit reference.
    pub span: SourceSpan,
}

impl fmt::Display for AstBitRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(idx) = self.index {
            write!(f, "{}[{idx}]", self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

/// Measurement statement node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstMeasure {
    /// Source quantum qubit operand being measured.
    pub qubit: AstQubitRef,
    /// Target classical bit receiving the measurement outcome.
    pub bit: AstBitRef,
    /// Source span of the measurement statement.
    pub span: SourceSpan,
}

/// State reset statement node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstReset {
    /// Target qubit operand being reset to |0>.
    pub qubit: AstQubitRef,
    /// Source span of the reset statement.
    pub span: SourceSpan,
}

/// Barrier synchronization statement node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstBarrier {
    /// Specific qubit operands bounded by the barrier (empty for global barrier).
    pub qubits: Vec<AstQubitRef>,
    /// Source span of the barrier statement.
    pub span: SourceSpan,
}

/// Circuit global phase statement node.
#[derive(Clone, Debug, PartialEq)]
pub struct AstGlobalPhase {
    /// Explicit global phase angle expression.
    pub angle: AstExpression,
    /// Source span of the gphase statement.
    pub span: SourceSpan,
}

/// An unsupported or unrecognized construct captured for diagnostic reporting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstUnsupported {
    /// Category or kind of unsupported feature (e.g. `"while"`, `"defcal"`, `"ccx"`).
    pub kind: String,
    /// Raw source text of the construct.
    pub raw: String,
    /// Source span of the unsupported construct.
    pub span: SourceSpan,
}

/// Arithmetic parameter expression node.
#[derive(Clone, Debug, PartialEq)]
pub struct AstExpression {
    /// Expression tree variant.
    pub kind: AstExprKind,
    /// Source span of the expression.
    pub span: SourceSpan,
}

/// Tree variants of an arithmetic parameter expression.
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
    /// Unary negation.
    Neg(Box<AstExpression>),
}

impl AstExpression {
    /// Constructs a constant literal expression with a source span.
    pub fn constant(val: f64, span: SourceSpan) -> Self {
        Self {
            kind: AstExprKind::Constant(val),
            span,
        }
    }

    /// Constructs an identifier or symbol expression with a source span.
    pub fn identifier(name: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            kind: AstExprKind::Identifier(name.into()),
            span,
        }
    }

    /// Converts this AST expression into QccIR's [`ParameterExpr`], mapping
    /// recognized constants (`"pi"`, `"π"`) to numerical $\pi$.
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
        }
    }
}
