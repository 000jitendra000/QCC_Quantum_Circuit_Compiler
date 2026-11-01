//! AST nodes for quantum and classical register declarations.

use std::fmt;

use crate::diagnostics::SourceSpan;

/// Quantum or classical register declaration kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AstRegisterKind {
    /// Quantum register (`qubit`).
    Qubit,
    /// Classical bit register (`bit`).
    Bit,
}

impl AstRegisterKind {
    /// Returns the static string representation of this register kind.
    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Qubit => "qubit",
            Self::Bit => "bit",
        }
    }
}

impl fmt::Display for AstRegisterKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Register declaration statement node (e.g. `qubit[2] q;` or `bit c;`).
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

impl AstRegisterDecl {
    /// Constructs a new [`AstRegisterDecl`].
    pub fn new(
        kind: AstRegisterKind,
        name: impl Into<String>,
        size: Option<usize>,
        span: SourceSpan,
    ) -> Self {
        Self {
            kind,
            name: name.into(),
            size,
            span,
        }
    }

    /// Constructs a quantum register declaration node.
    pub fn qubit(name: impl Into<String>, size: Option<usize>, span: SourceSpan) -> Self {
        Self::new(AstRegisterKind::Qubit, name, size, span)
    }

    /// Constructs a classical register declaration node.
    pub fn bit(name: impl Into<String>, size: Option<usize>, span: SourceSpan) -> Self {
        Self::new(AstRegisterKind::Bit, name, size, span)
    }

    /// Returns `true` if this is a quantum register declaration.
    #[inline]
    pub fn is_qubit(&self) -> bool {
        self.kind == AstRegisterKind::Qubit
    }

    /// Returns `true` if this is a classical bit register declaration.
    #[inline]
    pub fn is_bit(&self) -> bool {
        self.kind == AstRegisterKind::Bit
    }

    /// Returns `true` if this is a scalar declaration without array sizing (`size.is_none()`).
    #[inline]
    pub fn is_scalar(&self) -> bool {
        self.size.is_none()
    }

    /// Returns the source span of the declaration.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

impl fmt::Display for AstRegisterDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(size) = self.size {
            write!(f, "{}[{size}] {};", self.kind, self.name)
        } else {
            write!(f, "{} {};", self.kind, self.name)
        }
    }
}
