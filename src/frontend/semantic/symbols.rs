//! Semantic symbol tables and identifier resolution for OpenQASM 3.
//!
//! Enforces the rule that register identity and typing delegate to the authoritative
//! [`RegisterTable`] model, tracking declared quantum registers, classical registers,
//! and parameter symbol bindings.

use std::collections::HashMap;

use crate::diagnostics::{Diagnostic, SourceSpan};
use crate::frontend::ast::{
    AstBitRef, AstProgram, AstQubitRef, AstRegisterDecl, AstRegisterKind, AstStatement,
};
use crate::frontend::parser::registers::{
    ClassicalRegisterEntry, ClassicalRegisterId, QuantumRegisterEntry, QuantumRegisterId,
    RegisterEntry, RegisterError, RegisterTable, ResolvedBit, ResolvedQubit,
};

/// Symbol table tracking all declared quantum registers, classical registers,
/// and runtime parameter value bindings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SymbolTable {
    /// Authoritative symbol table managing quantum and classical registers.
    registers: RegisterTable,
    /// Parameter symbol environment mapping identifier names to evaluated numeric values.
    parameter_bindings: HashMap<String, f64>,
}

impl SymbolTable {
    /// Constructs a new, empty [`SymbolTable`].
    pub fn new() -> Self {
        Self {
            registers: RegisterTable::new(),
            parameter_bindings: HashMap::new(),
        }
    }

    /// Constructs a [`SymbolTable`] with pre-configured parameter value bindings.
    pub fn with_bindings(bindings: HashMap<String, f64>) -> Self {
        Self {
            registers: RegisterTable::new(),
            parameter_bindings: bindings,
        }
    }

    /// Constructs a [`SymbolTable`] wrapping an existing [`RegisterTable`].
    pub fn from_register_table(registers: RegisterTable) -> Self {
        Self {
            registers,
            parameter_bindings: HashMap::new(),
        }
    }

    /// Builds a [`SymbolTable`] from an AST program's register declarations.
    ///
    /// Iterates through top-level statements, registers all `qubit` and `bit` declarations,
    /// and collects any diagnostic errors (e.g. duplicate names, zero sizes).
    pub fn build_from_program(
        program: &AstProgram,
        file_path: &str,
        bindings: Option<HashMap<String, f64>>,
    ) -> (Self, Vec<Diagnostic>) {
        let mut table = if let Some(b) = bindings {
            Self::with_bindings(b)
        } else {
            Self::new()
        };

        let mut diagnostics = Vec::new();

        for stmt in &program.statements {
            if let AstStatement::RegisterDecl(decl) = stmt {
                if let Err(diag) = table.declare_ast_register(decl, file_path) {
                    diagnostics.push(*diag);
                }
            }
        }

        (table, diagnostics)
    }

    /// Declares a register from an AST declaration node.
    pub fn declare_ast_register(
        &mut self,
        decl: &AstRegisterDecl,
        file_path: &str,
    ) -> Result<(), Box<Diagnostic>> {
        match decl.kind {
            AstRegisterKind::Qubit => self
                .registers
                .declare_quantum(&decl.name, decl.size, decl.span)
                .map(|_| ())
                .map_err(|err| Box::new(err.to_diagnostic(file_path))),
            AstRegisterKind::Bit => self
                .registers
                .declare_classical(&decl.name, decl.size, decl.span)
                .map(|_| ())
                .map_err(|err| Box::new(err.to_diagnostic(file_path))),
        }
    }

    /// Declares a new quantum register.
    pub fn declare_quantum(
        &mut self,
        name: impl Into<String>,
        size: Option<usize>,
        span: SourceSpan,
    ) -> Result<QuantumRegisterId, RegisterError> {
        self.registers.declare_quantum(name, size, span)
    }

    /// Declares a new classical register.
    pub fn declare_classical(
        &mut self,
        name: impl Into<String>,
        size: Option<usize>,
        span: SourceSpan,
    ) -> Result<ClassicalRegisterId, RegisterError> {
        self.registers.declare_classical(name, size, span)
    }

    /// Resolves an AST qubit operand reference against declared quantum registers.
    pub fn resolve_qubit(&self, qubit_ref: &AstQubitRef) -> Result<ResolvedQubit, RegisterError> {
        self.registers.resolve_qubit(qubit_ref)
    }

    /// Resolves an AST classical bit operand reference against declared classical registers.
    pub fn resolve_bit(&self, bit_ref: &AstBitRef) -> Result<ResolvedBit, RegisterError> {
        self.registers.resolve_bit(bit_ref)
    }

    /// Validates an AST qubit reference, optionally permitting unindexed arrays.
    pub fn validate_qubit_ref(
        &self,
        qubit_ref: &AstQubitRef,
        allow_unindexed_array: bool,
    ) -> Result<(), RegisterError> {
        self.registers
            .validate_qubit_ref(qubit_ref, allow_unindexed_array)
    }

    /// Validates an AST bit reference, optionally permitting unindexed arrays.
    pub fn validate_bit_ref(
        &self,
        bit_ref: &AstBitRef,
        allow_unindexed_array: bool,
    ) -> Result<(), RegisterError> {
        self.registers
            .validate_bit_ref(bit_ref, allow_unindexed_array)
    }

    /// Validates a quantum-to-classical measurement pair.
    pub fn validate_measurement(
        &self,
        qubit_ref: &AstQubitRef,
        bit_ref: &AstBitRef,
        statement_span: SourceSpan,
    ) -> Result<(), RegisterError> {
        self.registers
            .validate_measurement(qubit_ref, bit_ref, statement_span)
    }

    /// Returns a reference to the underlying authoritative [`RegisterTable`].
    #[inline]
    pub fn registers(&self) -> &RegisterTable {
        &self.registers
    }

    /// Returns a mutable reference to the underlying authoritative [`RegisterTable`].
    #[inline]
    pub fn registers_mut(&mut self) -> &mut RegisterTable {
        &mut self.registers
    }

    /// Binds a runtime parameter identifier to a concrete numeric value.
    pub fn bind_parameter(&mut self, name: impl Into<String>, value: f64) {
        self.parameter_bindings.insert(name.into(), value);
    }

    /// Queries the bound numeric value of a parameter, if present.
    pub fn parameter_value(&self, name: &str) -> Option<f64> {
        self.parameter_bindings.get(name).copied()
    }

    /// Returns a reference to the parameter bindings map.
    #[inline]
    pub fn parameter_bindings(&self) -> &HashMap<String, f64> {
        &self.parameter_bindings
    }

    /// Returns the total number of flattened physical qubits across all quantum registers.
    #[inline]
    pub fn total_qubits(&self) -> usize {
        self.registers.total_qubits()
    }

    /// Returns the total number of flattened classical bits across all classical registers.
    #[inline]
    pub fn total_bits(&self) -> usize {
        self.registers.total_bits()
    }

    /// Returns all declared quantum register entries in declaration order.
    #[inline]
    pub fn quantum_registers(&self) -> &[QuantumRegisterEntry] {
        self.registers.quantum_registers()
    }

    /// Returns all declared classical register entries in declaration order.
    #[inline]
    pub fn classical_registers(&self) -> &[ClassicalRegisterEntry] {
        self.registers.classical_registers()
    }

    /// Returns all register entries in source declaration order.
    #[inline]
    pub fn declaration_order(&self) -> &[RegisterEntry] {
        self.registers.declaration_order()
    }
}
