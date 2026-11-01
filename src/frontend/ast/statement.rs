//! AST nodes for quantum and classical statements and operations.

use std::fmt;

use crate::diagnostics::SourceSpan;
use crate::frontend::ast::declaration::AstRegisterDecl;
use crate::frontend::ast::expression::AstExpression;

/// Top-level AST statement variant representing any statement within a program body.
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
            Self::RegisterDecl(s) => s.span(),
            Self::GateCall(s) => s.span(),
            Self::Measure(s) => s.span(),
            Self::Reset(s) => s.span(),
            Self::Barrier(s) => s.span(),
            Self::GlobalPhase(s) => s.span(),
            Self::Unsupported(s) => s.span(),
        }
    }

    /// Returns `true` if this statement is a register declaration.
    #[inline]
    pub fn is_register_decl(&self) -> bool {
        matches!(self, Self::RegisterDecl(_))
    }

    /// Returns `true` if this statement is a gate call.
    #[inline]
    pub fn is_gate_call(&self) -> bool {
        matches!(self, Self::GateCall(_))
    }

    /// Returns `true` if this statement is a measurement.
    #[inline]
    pub fn is_measure(&self) -> bool {
        matches!(self, Self::Measure(_))
    }

    /// Returns `true` if this statement is a reset.
    #[inline]
    pub fn is_reset(&self) -> bool {
        matches!(self, Self::Reset(_))
    }

    /// Returns `true` if this statement is a barrier.
    #[inline]
    pub fn is_barrier(&self) -> bool {
        matches!(self, Self::Barrier(_))
    }

    /// Returns `true` if this statement is a global phase directive.
    #[inline]
    pub fn is_global_phase(&self) -> bool {
        matches!(self, Self::GlobalPhase(_))
    }

    /// Returns `true` if this statement is an unsupported construct.
    #[inline]
    pub fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported(_))
    }
}

impl From<AstRegisterDecl> for AstStatement {
    #[inline]
    fn from(s: AstRegisterDecl) -> Self {
        Self::RegisterDecl(s)
    }
}

impl From<AstGateCall> for AstStatement {
    #[inline]
    fn from(s: AstGateCall) -> Self {
        Self::GateCall(s)
    }
}

impl From<AstMeasure> for AstStatement {
    #[inline]
    fn from(s: AstMeasure) -> Self {
        Self::Measure(s)
    }
}

impl From<AstReset> for AstStatement {
    #[inline]
    fn from(s: AstReset) -> Self {
        Self::Reset(s)
    }
}

impl From<AstBarrier> for AstStatement {
    #[inline]
    fn from(s: AstBarrier) -> Self {
        Self::Barrier(s)
    }
}

impl From<AstGlobalPhase> for AstStatement {
    #[inline]
    fn from(s: AstGlobalPhase) -> Self {
        Self::GlobalPhase(s)
    }
}

impl From<AstUnsupported> for AstStatement {
    #[inline]
    fn from(s: AstUnsupported) -> Self {
        Self::Unsupported(s)
    }
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

impl AstGateCall {
    /// Constructs a new [`AstGateCall`] node.
    pub fn new(
        name: impl Into<String>,
        parameters: Vec<AstExpression>,
        qubits: Vec<AstQubitRef>,
        span: SourceSpan,
    ) -> Self {
        Self {
            name: name.into(),
            parameters,
            qubits,
            span,
        }
    }

    /// Returns the source span of the gate call statement.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

/// Qubit reference operand node.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AstQubitRef {
    /// Register identifier name.
    pub name: String,
    /// Optional 0-indexed element within the register (`None` for scalar register references).
    pub index: Option<usize>,
    /// Source span of the qubit reference.
    pub span: SourceSpan,
}

impl AstQubitRef {
    /// Constructs a new [`AstQubitRef`].
    pub fn new(name: impl Into<String>, index: Option<usize>, span: SourceSpan) -> Self {
        Self {
            name: name.into(),
            index,
            span,
        }
    }

    /// Constructs an indexed qubit reference (e.g. `q[0]`).
    pub fn indexed(name: impl Into<String>, index: usize, span: SourceSpan) -> Self {
        Self::new(name, Some(index), span)
    }

    /// Constructs a scalar qubit reference (e.g. `q`).
    pub fn scalar(name: impl Into<String>, span: SourceSpan) -> Self {
        Self::new(name, None, span)
    }

    /// Returns `true` if this reference specifies an element index.
    #[inline]
    pub fn is_indexed(&self) -> bool {
        self.index.is_some()
    }

    /// Returns `true` if this reference is scalar (unindexed).
    #[inline]
    pub fn is_scalar(&self) -> bool {
        self.index.is_none()
    }

    /// Returns the source span of the qubit reference.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }
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
    /// Register identifier name.
    pub name: String,
    /// Optional 0-indexed element within the register (`None` for scalar register references).
    pub index: Option<usize>,
    /// Source span of the bit reference.
    pub span: SourceSpan,
}

impl AstBitRef {
    /// Constructs a new [`AstBitRef`].
    pub fn new(name: impl Into<String>, index: Option<usize>, span: SourceSpan) -> Self {
        Self {
            name: name.into(),
            index,
            span,
        }
    }

    /// Constructs an indexed classical bit reference (e.g. `c[0]`).
    pub fn indexed(name: impl Into<String>, index: usize, span: SourceSpan) -> Self {
        Self::new(name, Some(index), span)
    }

    /// Constructs a scalar classical bit reference (e.g. `c`).
    pub fn scalar(name: impl Into<String>, span: SourceSpan) -> Self {
        Self::new(name, None, span)
    }

    /// Returns `true` if this reference specifies an element index.
    #[inline]
    pub fn is_indexed(&self) -> bool {
        self.index.is_some()
    }

    /// Returns `true` if this reference is scalar (unindexed).
    #[inline]
    pub fn is_scalar(&self) -> bool {
        self.index.is_none()
    }

    /// Returns the source span of the bit reference.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }
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

/// Measurement statement node (`c = measure q;` or `measure q -> c;`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstMeasure {
    /// Source quantum qubit operand being measured.
    pub qubit: AstQubitRef,
    /// Target classical bit receiving the measurement outcome.
    pub bit: AstBitRef,
    /// Source span of the measurement statement.
    pub span: SourceSpan,
}

impl AstMeasure {
    /// Constructs a new [`AstMeasure`] statement.
    pub fn new(qubit: AstQubitRef, bit: AstBitRef, span: SourceSpan) -> Self {
        Self { qubit, bit, span }
    }

    /// Returns the source span of the measurement statement.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

/// State reset statement node (`reset q;`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstReset {
    /// Target qubit operand being reset to |0>.
    pub qubit: AstQubitRef,
    /// Source span of the reset statement.
    pub span: SourceSpan,
}

impl AstReset {
    /// Constructs a new [`AstReset`] statement.
    pub fn new(qubit: AstQubitRef, span: SourceSpan) -> Self {
        Self { qubit, span }
    }

    /// Returns the source span of the reset statement.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

/// Barrier synchronization statement node (`barrier;` or `barrier q0, q1;`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstBarrier {
    /// Specific qubit operands bounded by the barrier (empty for global barrier).
    pub qubits: Vec<AstQubitRef>,
    /// Source span of the barrier statement.
    pub span: SourceSpan,
}

impl AstBarrier {
    /// Constructs a new [`AstBarrier`] statement.
    pub fn new(qubits: Vec<AstQubitRef>, span: SourceSpan) -> Self {
        Self { qubits, span }
    }

    /// Returns `true` if this barrier applies globally to all qubits.
    #[inline]
    pub fn is_global(&self) -> bool {
        self.qubits.is_empty()
    }

    /// Returns the source span of the barrier statement.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

/// Circuit global phase statement node (`gphase(angle);`).
#[derive(Clone, Debug, PartialEq)]
pub struct AstGlobalPhase {
    /// Explicit global phase angle expression.
    pub angle: AstExpression,
    /// Source span of the gphase statement.
    pub span: SourceSpan,
}

impl AstGlobalPhase {
    /// Constructs a new [`AstGlobalPhase`] statement.
    pub fn new(angle: AstExpression, span: SourceSpan) -> Self {
        Self { angle, span }
    }

    /// Returns the source span of the global phase statement.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }
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

impl AstUnsupported {
    /// Constructs a new [`AstUnsupported`] statement node.
    pub fn new(kind: impl Into<String>, raw: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            kind: kind.into(),
            raw: raw.into(),
            span,
        }
    }

    /// Returns the source span of the unsupported construct.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }
}
