//! AST nodes for OpenQASM program headers, version directives, and file inclusions.

use crate::diagnostics::SourceSpan;
use crate::frontend::ast::statement::AstStatement;

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

    /// Returns the source span covering the entire program.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }

    /// Returns the optional version directive.
    #[inline]
    pub fn version(&self) -> Option<&AstVersion> {
        self.version.as_ref()
    }

    /// Returns the slice of include directives.
    #[inline]
    pub fn includes(&self) -> &[AstInclude] {
        &self.includes
    }

    /// Returns the slice of program statements.
    #[inline]
    pub fn statements(&self) -> &[AstStatement] {
        &self.statements
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

impl AstVersion {
    /// Constructs a new [`AstVersion`] node.
    pub fn new(specifier: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            specifier: specifier.into(),
            span,
        }
    }

    /// Returns the source span of the version declaration.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

/// Include directive node (e.g. `include "stdgates.inc";`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstInclude {
    /// Included file path.
    pub path: String,
    /// Source span of the include statement.
    pub span: SourceSpan,
}

impl AstInclude {
    /// Constructs a new [`AstInclude`] node.
    pub fn new(path: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            path: path.into(),
            span,
        }
    }

    /// Returns the source span of the include directive.
    #[inline]
    pub fn span(&self) -> SourceSpan {
        self.span
    }
}
