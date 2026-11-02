//! Fine-grained semantic checks for OpenQASM 3 AST programs.
//!
//! Enforces:
//! - Register domain and declaration integrity.
//! - Gate existence, case-sensitivity, and parameter/qubit arities.
//! - Parameter expression validity, symbol resolution, and domain checks.
//! - Multi-qubit gate operand distinctness (no duplicate qubits in 2-qubit gates).
//! - Measurement source (quantum) and destination (classical) typing and size matching.
//! - Reset and barrier operand domain typing.

use crate::diagnostics::qasm;
use crate::diagnostics::Diagnostic;
use crate::frontend::ast::{
    AstBarrier, AstGateCall, AstGlobalPhase, AstMeasure, AstProgram, AstReset, AstStatement,
};
use crate::frontend::parser::expressions::{evaluate_expression, ExpressionEvaluationError};
use crate::frontend::parser::gates::{find_similar_gate, lookup_gate_signature};
use crate::frontend::parser::registers::RegisterError;
use crate::frontend::semantic::symbols::SymbolTable;
use crate::frontend::source::SourceBuffer;

/// Semantic checker responsible for validating AST statements against a [`SymbolTable`].
pub struct SemanticChecker<'a> {
    buffer: &'a SourceBuffer,
    symbols: &'a SymbolTable,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticChecker<'a> {
    /// Constructs a new [`SemanticChecker`].
    pub fn new(buffer: &'a SourceBuffer, symbols: &'a SymbolTable) -> Self {
        Self {
            buffer,
            symbols,
            diagnostics: Vec::new(),
        }
    }

    /// Validates all statements in an [`AstProgram`], accumulating structured diagnostics.
    pub fn check_program(mut self, program: &AstProgram) -> Vec<Diagnostic> {
        for stmt in &program.statements {
            self.check_statement(stmt);
        }
        self.diagnostics
    }

    /// Dispatches validation for an individual AST statement.
    pub fn check_statement(&mut self, stmt: &AstStatement) {
        match stmt {
            AstStatement::RegisterDecl(_) => {
                // Register declarations are checked when building the SymbolTable.
            }
            AstStatement::GateCall(call) => self.check_gate_call(call),
            AstStatement::Measure(measure) => self.check_measurement(measure),
            AstStatement::Reset(reset) => self.check_reset(reset),
            AstStatement::Barrier(barrier) => self.check_barrier(barrier),
            AstStatement::GlobalPhase(gphase) => self.check_global_phase(gphase),
            AstStatement::Unsupported(unsupported) => {
                self.diagnostics.push(
                    Diagnostic::error(unsupported.raw.clone())
                        .with_code(qasm::QASM_UNSUPPORTED_FEATURE)
                        .with_span(unsupported.span)
                        .with_file_path(self.buffer.name()),
                );
            }
        }
    }

    /// Checks a gate call statement for signature match, arities, operand validity,
    /// duplicate operands, and parameter expression evaluation.
    pub fn check_gate_call(&mut self, call: &AstGateCall) {
        let sig = match lookup_gate_signature(&call.name) {
            Some(s) => s,
            None => {
                let suggestion = find_similar_gate(&call.name);
                self.diagnostics.push(qasm::unknown_gate(
                    &call.name,
                    call.span,
                    suggestion,
                    self.buffer.name(),
                ));
                return;
            }
        };

        // 1. Check parameter count and presence
        if !sig.is_parameterized {
            if !call.parameters.is_empty() {
                self.diagnostics.push(qasm::gate_param_arity(
                    &call.name,
                    0,
                    call.parameters.len(),
                    call.span,
                    self.buffer.name(),
                ));
            }
        } else if call.parameters.len() != sig.num_params {
            self.diagnostics.push(qasm::gate_param_arity(
                &call.name,
                sig.num_params,
                call.parameters.len(),
                call.span,
                self.buffer.name(),
            ));
        }

        // 2. Validate and evaluate parameter expressions
        for param in &call.parameters {
            match evaluate_expression(param, self.symbols.parameter_bindings()) {
                Ok(val) => {
                    if !val.is_finite() {
                        self.diagnostics.push(qasm::non_finite_parameter(
                            val,
                            param.span,
                            self.buffer.name(),
                        ));
                    }
                }
                Err(err) => match err {
                    ExpressionEvaluationError::UnresolvedSymbol { name, span } => {
                        self.diagnostics.push(qasm::unresolved_symbol(
                            &name,
                            &format!("gate '{}'", call.name),
                            span,
                            self.buffer.name(),
                        ));
                    }
                    ExpressionEvaluationError::DivisionByZero { span } => {
                        self.diagnostics
                            .push(qasm::division_by_zero(span, self.buffer.name()));
                    }
                    ExpressionEvaluationError::EvaluationFailed { message, span } => {
                        self.diagnostics.push(
                            Diagnostic::error(message)
                                .with_code(qasm::QASM_NON_FINITE_PARAM)
                                .with_span(span)
                                .with_file_path(self.buffer.name()),
                        );
                    }
                },
            }
        }

        // 3. Check qubit operand count (arity)
        if call.qubits.len() != sig.num_qubits {
            self.diagnostics.push(qasm::gate_qubit_arity(
                &call.name,
                sig.num_qubits,
                call.qubits.len(),
                call.span,
                self.buffer.name(),
            ));
            return;
        }

        // 4. Resolve each qubit operand against declared quantum registers
        let mut resolved_qubits = Vec::with_capacity(call.qubits.len());
        let mut resolution_succeeded = true;

        for qubit_ref in &call.qubits {
            match self.symbols.resolve_qubit(qubit_ref) {
                Ok(resolved) => resolved_qubits.push(resolved),
                Err(err) => {
                    resolution_succeeded = false;
                    self.diagnostics.push(self.convert_register_error(&err));
                }
            }
        }

        // 5. Check duplicate qubit operands in multi-qubit gates
        if resolution_succeeded && sig.num_qubits > 1 {
            for i in 0..resolved_qubits.len() {
                for j in (i + 1)..resolved_qubits.len() {
                    if resolved_qubits[i].flat_qubit_id == resolved_qubits[j].flat_qubit_id {
                        let qubit_str = format!("{}", call.qubits[j]);
                        self.diagnostics.push(qasm::duplicate_qubit_operand(
                            &call.name,
                            &qubit_str,
                            call.qubits[j].span,
                            self.buffer.name(),
                        ));
                    }
                }
            }
        }
    }

    /// Checks a measurement statement for quantum source, classical target, and size matching.
    pub fn check_measurement(&mut self, measure: &AstMeasure) {
        // 1. Validate quantum source
        if let Err(err) = self.symbols.validate_qubit_ref(&measure.qubit, true) {
            self.diagnostics.push(self.convert_register_error(&err));
        }

        // 2. Validate classical destination
        if let Err(err) = self.symbols.validate_bit_ref(&measure.bit, true) {
            self.diagnostics.push(self.convert_register_error(&err));
        }

        // 3. Validate measurement sizing and alignment
        if let Err(err) =
            self.symbols
                .validate_measurement(&measure.qubit, &measure.bit, measure.span)
        {
            // Avoid duplicate error if either operand reference already failed validation
            if !matches!(
                err,
                RegisterError::UndeclaredRegister { .. } | RegisterError::TypeMismatch { .. }
            ) {
                self.diagnostics.push(self.convert_register_error(&err));
            }
        }
    }

    /// Checks an active reset statement.
    pub fn check_reset(&mut self, reset: &AstReset) {
        if let Err(err) = self.symbols.validate_qubit_ref(&reset.qubit, true) {
            self.diagnostics.push(self.convert_register_error(&err));
        }
    }

    /// Checks a synchronization barrier statement.
    pub fn check_barrier(&mut self, barrier: &AstBarrier) {
        if barrier.is_global() {
            return;
        }

        for q in &barrier.qubits {
            if let Err(err) = self.symbols.validate_qubit_ref(q, true) {
                self.diagnostics.push(self.convert_register_error(&err));
            }
        }
    }

    /// Checks a global phase statement.
    pub fn check_global_phase(&mut self, gphase: &AstGlobalPhase) {
        match evaluate_expression(&gphase.angle, self.symbols.parameter_bindings()) {
            Ok(val) => {
                if !val.is_finite() {
                    self.diagnostics.push(qasm::non_finite_parameter(
                        val,
                        gphase.angle.span,
                        self.buffer.name(),
                    ));
                }
            }
            Err(err) => match err {
                ExpressionEvaluationError::UnresolvedSymbol { name, span } => {
                    self.diagnostics.push(qasm::unresolved_symbol(
                        &name,
                        "global phase directive 'gphase'",
                        span,
                        self.buffer.name(),
                    ));
                }
                ExpressionEvaluationError::DivisionByZero { span } => {
                    self.diagnostics
                        .push(qasm::division_by_zero(span, self.buffer.name()));
                }
                ExpressionEvaluationError::EvaluationFailed { message, span } => {
                    self.diagnostics.push(
                        Diagnostic::error(message)
                            .with_code(qasm::QASM_NON_FINITE_PARAM)
                            .with_span(span)
                            .with_file_path(self.buffer.name()),
                    );
                }
            },
        }
    }

    /// Converts a [`RegisterError`] into a source-located compiler [`Diagnostic`].
    fn convert_register_error(&self, err: &RegisterError) -> Diagnostic {
        err.to_diagnostic(self.buffer.name())
    }
}
