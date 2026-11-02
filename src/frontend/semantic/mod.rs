//! Semantic analysis and diagnostic validation for OpenQASM 3 programs.
//!
//! Provides the [`SemanticAnalyzer`] driver and [`SymbolTable`] model to validate
//! register declarations, gate applications, parameter expressions, non-unitary statements,
//! and ordering boundaries before lowering to QccIR.

pub mod checks;
pub mod symbols;

use std::collections::HashMap;

pub use checks::SemanticChecker;
pub use symbols::SymbolTable;

use crate::diagnostics::{Diagnostic, DiagnosticLevel};
use crate::frontend::ast::AstProgram;
use crate::frontend::errors::FrontendResult;
use crate::frontend::source::SourceBuffer;

/// Semantic analyzer driver validating an [`AstProgram`] against language rules and symbol tables.
pub struct SemanticAnalyzer<'a> {
    buffer: &'a SourceBuffer,
    parameter_bindings: HashMap<String, f64>,
}

impl<'a> SemanticAnalyzer<'a> {
    /// Constructs a new [`SemanticAnalyzer`] over the given source buffer.
    pub fn new(buffer: &'a SourceBuffer) -> Self {
        Self {
            buffer,
            parameter_bindings: HashMap::new(),
        }
    }

    /// Constructs a [`SemanticAnalyzer`] with pre-configured parameter value bindings.
    pub fn with_bindings(buffer: &'a SourceBuffer, bindings: HashMap<String, f64>) -> Self {
        Self {
            buffer,
            parameter_bindings: bindings,
        }
    }

    /// Adds or updates a parameter value binding.
    pub fn bind_parameter(&mut self, name: impl Into<String>, value: f64) -> &mut Self {
        self.parameter_bindings.insert(name.into(), value);
        self
    }

    /// Analyzes an [`AstProgram`], building symbol tables and validating all statements.
    ///
    /// # Errors
    /// Returns `Err(Vec<Diagnostic>)` if any error-level diagnostics are encountered.
    pub fn analyze(&self, program: &AstProgram) -> Result<SymbolTable, Vec<Diagnostic>> {
        let (symbols, mut diagnostics) = SymbolTable::build_from_program(
            program,
            self.buffer.name(),
            Some(self.parameter_bindings.clone()),
        );

        let checker = SemanticChecker::new(self.buffer, &symbols);
        let check_diags = checker.check_program(program);
        diagnostics.extend(check_diags);

        if diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Error)
        {
            Err(diagnostics)
        } else {
            Ok(symbols)
        }
    }

    /// Analyzes an [`AstProgram`] and returns a [`FrontendResult<SymbolTable>`].
    pub fn analyze_result(&self, program: &AstProgram) -> FrontendResult<SymbolTable> {
        match self.analyze(program) {
            Ok(table) => FrontendResult::success(table),
            Err(diagnostics) => FrontendResult::failure(diagnostics),
        }
    }
}
