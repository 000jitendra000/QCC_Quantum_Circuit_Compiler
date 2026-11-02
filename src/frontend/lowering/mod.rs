//! AST-to-QccIR lowering pipeline.
//!
//! Provides the canonical [`CircuitLowerer`] driver and convenience functions to convert
//! semantically validated OpenQASM 3 AST compilation units into valid, verified QccIR
//! [`Circuit`] representations.

pub mod operations;
pub mod registers;

pub use operations::*;
pub use registers::*;

use std::collections::HashMap;

use crate::diagnostics::{qasm, Diagnostic, DiagnosticLevel};
use crate::frontend::adapter::{Frontend, OpenQasmFrontend};
use crate::frontend::ast::{AstProgram, AstStatement};
use crate::frontend::errors::FrontendResult;
use crate::frontend::semantic::{SemanticAnalyzer, SymbolTable};
use crate::frontend::source::SourceBuffer;
use crate::ir::circuit::Circuit;

/// Driver lowering a validated [`AstProgram`] into a canonical QccIR [`Circuit`].
#[derive(Clone, Debug)]
pub struct CircuitLowerer<'a> {
    buffer: &'a SourceBuffer,
    circuit_name: Option<String>,
    parameter_bindings: HashMap<String, f64>,
}

impl<'a> CircuitLowerer<'a> {
    /// Creates a new [`CircuitLowerer`] over the given source buffer.
    pub fn new(buffer: &'a SourceBuffer) -> Self {
        Self {
            buffer,
            circuit_name: None,
            parameter_bindings: HashMap::new(),
        }
    }

    /// Configures an explicit circuit name for the lowered QccIR circuit.
    pub fn with_circuit_name(mut self, name: impl Into<String>) -> Self {
        self.circuit_name = Some(name.into());
        self
    }

    /// Configures pre-defined parameter bindings for symbolic parameter evaluation.
    pub fn with_bindings(mut self, bindings: HashMap<String, f64>) -> Self {
        self.parameter_bindings = bindings;
        self
    }

    /// Adds or updates a single parameter binding.
    pub fn bind_parameter(&mut self, name: impl Into<String>, value: f64) -> &mut Self {
        self.parameter_bindings.insert(name.into(), value);
        self
    }

    /// Lowers an [`AstProgram`] into a validated QccIR [`Circuit`].
    ///
    /// Automatically executes semantic analysis before lowering. If any errors are found,
    /// lowering halts immediately and returns the structured diagnostics.
    ///
    /// # Errors
    /// Returns `Err(Vec<Diagnostic>)` if semantic analysis fails, if unsupported statements
    /// are encountered, or if QccIR graph invariant validation detects a cycle or invalid wire bounds.
    pub fn lower(&self, program: &AstProgram) -> Result<Circuit, Vec<Diagnostic>> {
        let analyzer =
            SemanticAnalyzer::with_bindings(self.buffer, self.parameter_bindings.clone());
        let symbols = analyzer.analyze(program)?;
        self.lower_with_symbols(program, &symbols)
    }

    /// Lowers an [`AstProgram`] that has already been validated into a QccIR [`Circuit`],
    /// using the provided pre-computed [`SymbolTable`].
    pub fn lower_with_symbols(
        &self,
        program: &AstProgram,
        symbols: &SymbolTable,
    ) -> Result<Circuit, Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let file_path = self.buffer.name();

        // 1. Initialize Circuit
        let c_name = self.circuit_name.clone().unwrap_or_else(|| {
            let p = std::path::Path::new(file_path);
            p.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("qasm_circuit")
                .to_string()
        });
        let mut circuit = Circuit::new(c_name);

        // Record source provenance in metadata
        circuit
            .metadata
            .insert("source.file".to_string(), file_path.to_string());
        circuit.metadata.insert(
            "source.program_span".to_string(),
            format!("{}", program.span()),
        );

        // 2. Allocate Registers
        for stmt in &program.statements {
            if let AstStatement::RegisterDecl(decl) = stmt {
                let res = if decl.is_qubit() {
                    lower_quantum_register_decl(&mut circuit, decl, file_path).map(|_| ())
                } else {
                    lower_classical_register_decl(&mut circuit, decl, file_path).map(|_| ())
                };
                if let Err(diag) = res {
                    diagnostics.push(*diag);
                }
            }
        }

        if diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Error)
        {
            return Err(diagnostics);
        }

        // 3. Lower Statements
        for stmt in &program.statements {
            match stmt {
                AstStatement::RegisterDecl(_) => {
                    // Handled in allocation pass above
                }
                AstStatement::GateCall(call) => {
                    match lower_gate_statement(
                        &mut circuit,
                        call,
                        symbols,
                        &self.parameter_bindings,
                        file_path,
                    ) {
                        Ok(_) => {}
                        Err(diag) => diagnostics.push(*diag),
                    }
                }
                AstStatement::Measure(measure) => {
                    match lower_measure_statement(&mut circuit, measure, symbols, file_path) {
                        Ok(_) => {}
                        Err(diag) => diagnostics.push(*diag),
                    }
                }
                AstStatement::Reset(reset) => {
                    match lower_reset_statement(&mut circuit, reset, symbols, file_path) {
                        Ok(_) => {}
                        Err(diag) => diagnostics.push(*diag),
                    }
                }
                AstStatement::Barrier(barrier) => {
                    match lower_barrier_statement(&mut circuit, barrier, symbols, file_path) {
                        Ok(_) => {}
                        Err(diag) => diagnostics.push(*diag),
                    }
                }
                AstStatement::GlobalPhase(gphase) => {
                    match lower_global_phase_statement(
                        &mut circuit,
                        gphase,
                        &self.parameter_bindings,
                        file_path,
                    ) {
                        Ok(()) => {}
                        Err(diag) => diagnostics.push(*diag),
                    }
                }
                AstStatement::Unsupported(unsupported) => {
                    let code = match unsupported.kind.as_str() {
                        "legacy_keyword" => qasm::QASM_RESERVED_KEYWORD,
                        "three_qubit_gate" => qasm::QASM_UNSUPPORTED_3Q_GATE,
                        "unsupported_gate" => qasm::QASM_UNSUPPORTED_GATE,
                        _ => qasm::QASM_UNSUPPORTED_FEATURE,
                    };
                    let diag = Diagnostic::error(format!(
                        "unsupported construct '{}' cannot be lowered to QccIR",
                        unsupported.kind
                    ))
                    .with_code(code)
                    .with_span(unsupported.span)
                    .with_file_path(file_path);
                    diagnostics.push(diag);
                }
            }
        }

        if diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Error)
        {
            return Err(diagnostics);
        }

        // 4. Validate QccIR Graph Invariants
        if let Err(val_err) = circuit.validate() {
            diagnostics.push(qasm::lowering_error(
                format!("lowered circuit failed QccIR graph validation: {val_err}"),
                program.span(),
                file_path,
            ));
            return Err(diagnostics);
        }

        Ok(circuit)
    }

    /// Lowers an [`AstProgram`], returning a [`FrontendResult<Circuit>`].
    pub fn lower_result(&self, program: &AstProgram) -> FrontendResult<Circuit> {
        match self.lower(program) {
            Ok(circuit) => FrontendResult::success(circuit),
            Err(diagnostics) => FrontendResult::failure(diagnostics),
        }
    }

    /// Parses, semantically analyzes, and lowers the entire underlying [`SourceBuffer`].
    pub fn lower_source_buffer(&self) -> FrontendResult<Circuit> {
        let frontend = OpenQasmFrontend::new();
        let parse_res = frontend.parse(self.buffer);
        let mut diagnostics = parse_res.diagnostics;

        if let Some(program) = parse_res.output {
            match self.lower(&program) {
                Ok(circuit) => FrontendResult::with_diagnostics(circuit, diagnostics),
                Err(lower_diags) => {
                    diagnostics.extend(lower_diags);
                    FrontendResult::failure(diagnostics)
                }
            }
        } else {
            FrontendResult::failure(diagnostics)
        }
    }
}

/// Convenience standalone function to parse, validate, and lower an OpenQASM 3 source string.
pub fn lower_qasm_str(
    source: &str,
    circuit_name: Option<&str>,
    bindings: Option<HashMap<String, f64>>,
) -> FrontendResult<Circuit> {
    let buffer = SourceBuffer::new("<input>", source);
    let mut lowerer = CircuitLowerer::new(&buffer);
    if let Some(name) = circuit_name {
        lowerer = lowerer.with_circuit_name(name);
    }
    if let Some(b) = bindings {
        lowerer = lowerer.with_bindings(b);
    }
    lowerer.lower_source_buffer()
}
