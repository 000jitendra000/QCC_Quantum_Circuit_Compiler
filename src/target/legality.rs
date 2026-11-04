//! Independent final target legality validator and compiler pass.
//!
//! Validates physical quantum circuit operations against a hardware [`Target`] model,
//! enforcing operation identity, quantum arity, parameter domains, physical qubit capacity,
//! and coupling directionality with rich compiler diagnostics.

use std::fmt;

use crate::diagnostics::diagnostic::Diagnostic;
use crate::diagnostics::target::create_target_diagnostic;
use crate::gate_signature::ParameterDomain;
use crate::ir::circuit::Circuit;
use crate::ir::node::{NodeData, OperationId};
use crate::opt::context::PassContext;
use crate::opt::errors::PassError;
use crate::opt::pass::{CompilerPass, PassId, PassMutation, PassResult, SemanticDomain};
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;
use crate::target::query::LegalityError;

/// Details of a single target legality violation on a specific circuit node.
#[derive(Clone, Debug, PartialEq)]
pub struct TargetViolation {
    /// Stable semantic identifier of the offending node (`op#N`).
    pub node_id: OperationId,
    /// Topological sequence number of the node.
    pub sequence_number: u64,
    /// Operation mnemonic (e.g. `"cx"`, `"rz"`).
    pub operation_name: String,
    /// Physical qubit operands supplied to the operation.
    pub operands: Vec<PhysicalQubitId>,
    /// Underlying structured legality error.
    pub error: LegalityError,
    /// Rich compiler diagnostic with standardized code and target notes.
    pub diagnostic: Diagnostic,
}

impl fmt::Display for TargetViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.diagnostic)
    }
}

/// Comprehensive report produced by validating a circuit against a target model.
#[derive(Clone, Debug, PartialEq)]
pub struct TargetLegalityReport {
    /// Name of the validated circuit.
    pub circuit_name: String,
    /// Name of the hardware target machine.
    pub target_name: String,
    /// Total count of operations examined.
    pub operations_examined: usize,
    /// True if the circuit strictly satisfies all target legality constraints.
    pub is_legal: bool,
    /// Detailed list of all legality violations encountered.
    pub violations: Vec<TargetViolation>,
}

impl TargetLegalityReport {
    /// Returns `true` if zero legality violations were found.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.is_legal && self.violations.is_empty()
    }

    /// Returns the total count of violations.
    #[inline]
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }

    /// Returns references to all generated compiler diagnostics.
    pub fn diagnostics(&self) -> Vec<&Diagnostic> {
        self.violations.iter().map(|v| &v.diagnostic).collect()
    }

    /// Returns a multiline formatted diagnostic summary of all violations.
    pub fn summary(&self) -> String {
        if self.is_valid() {
            format!(
                "Circuit '{}' is fully legal on target '{}' ({} operations verified).",
                self.circuit_name, self.target_name, self.operations_examined
            )
        } else {
            let mut lines = Vec::new();
            lines.push(format!(
                "Circuit '{}' failed target legality on '{}' ({} violations across {} operations):",
                self.circuit_name,
                self.target_name,
                self.violations.len(),
                self.operations_examined
            ));
            for v in &self.violations {
                lines.push(format!("  - {v}"));
            }
            lines.join("\n")
        }
    }
}

/// Structured error returned when target legality validation fails.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum TargetLegalityError {
    /// The circuit contains one or more illegal target operations or coupling violations.
    #[error("circuit '{circuit}' failed target legality validation against '{target}' ({violation_count} violations):\n{details}")]
    ValidationFailed {
        /// Name of the invalid circuit.
        circuit: String,
        /// Name of the hardware target.
        target: String,
        /// Number of violations detected.
        violation_count: usize,
        /// Formatted details string.
        details: String,
        /// All individual target violations.
        violations: Vec<TargetViolation>,
    },

    /// DAG traversal or topological ordering failure during validation.
    #[error("circuit DAG traversal error during legality validation: {0}")]
    DagError(String),
}

/// Independent validator for verifying circuit operations against hardware target constraints.
#[derive(Clone, Debug)]
pub struct TargetLegalityValidator {
    target: Target,
    allow_barriers: bool,
}

impl TargetLegalityValidator {
    /// Creates a new validator for the given hardware `Target`.
    pub fn new(target: Target) -> Self {
        Self {
            target,
            allow_barriers: true,
        }
    }

    /// Configures whether compiler barriers are permitted as synchronization pseudo-ops.
    ///
    /// When `true` (default), barriers on valid physical qubits are allowed even if the target
    /// does not declare an explicit `"barrier"` hardware instruction.
    pub fn with_allow_barriers(mut self, allow: bool) -> Self {
        self.allow_barriers = allow;
        self
    }

    /// Returns a reference to the compilation target being validated against.
    #[inline]
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// Validates an entire circuit, returning a detailed [`TargetLegalityReport`].
    pub fn check_circuit(
        &self,
        circuit: &Circuit,
    ) -> Result<TargetLegalityReport, TargetLegalityError> {
        let ordered_nodes = circuit.graph.ordered_nodes().map_err(|e| {
            TargetLegalityError::DagError(format!("failed to order DAG nodes: {e}"))
        })?;

        let mut operations_examined = 0;
        let mut violations = Vec::new();

        for node in &ordered_nodes {
            operations_examined += 1;
            if let Err(violation) = self.validate_node(node) {
                violations.push(*violation);
            }
        }

        let is_legal = violations.is_empty();
        Ok(TargetLegalityReport {
            circuit_name: circuit.name.clone(),
            target_name: self.target.name.clone(),
            operations_examined,
            is_legal,
            violations,
        })
    }

    /// Validates a circuit, returning `Ok(report)` if legal or `Err(TargetLegalityError)` if any violation exists.
    pub fn validate_circuit(
        &self,
        circuit: &Circuit,
    ) -> Result<TargetLegalityReport, TargetLegalityError> {
        let report = self.check_circuit(circuit)?;
        if report.is_valid() {
            Ok(report)
        } else {
            let details = report.summary();
            Err(TargetLegalityError::ValidationFailed {
                circuit: circuit.name.clone(),
                target: self.target.name.clone(),
                violation_count: report.violations.len(),
                details,
                violations: report.violations,
            })
        }
    }

    /// Convenience predicate returning `true` if the circuit is strictly legal on the target.
    pub fn is_circuit_legal(&self, circuit: &Circuit) -> bool {
        self.check_circuit(circuit)
            .map(|r| r.is_valid())
            .unwrap_or(false)
    }

    /// Helper creating a boxed [`TargetViolation`] with rich diagnostics.
    fn make_violation(
        &self,
        id: OperationId,
        seq: u64,
        op_name: &str,
        operands: Vec<PhysicalQubitId>,
        err: LegalityError,
    ) -> Box<TargetViolation> {
        let diagnostic = create_target_diagnostic(Some(id), op_name, &operands, &err, &self.target);
        Box::new(TargetViolation {
            node_id: id,
            sequence_number: seq,
            operation_name: op_name.to_string(),
            operands,
            error: err,
            diagnostic,
        })
    }

    /// Validates a single DAG node against the target model.
    pub fn validate_node(&self, node: &NodeData) -> Result<(), Box<TargetViolation>> {
        let op = &node.operation;
        let id = node.id;
        let seq = node.sequence_number;
        let op_name = op.name();
        let operands: Vec<PhysicalQubitId> = op
            .qubits()
            .iter()
            .map(|q| PhysicalQubitId(q.0 as u32))
            .collect();

        // 1. Check for compiler barriers
        if op_name.eq_ignore_ascii_case("barrier") {
            if self.allow_barriers {
                // Check physical qubit bounds for barrier operands
                for &q in &operands {
                    if q.index() >= self.target.num_qubits {
                        let err = LegalityError::QubitOutOfBounds {
                            qubit: q,
                            capacity: self.target.num_qubits,
                        };
                        return Err(self.make_violation(id, seq, op_name, operands, err));
                    }
                }
                // Check duplicate qubits in barrier
                for i in 0..operands.len() {
                    for j in (i + 1)..operands.len() {
                        if operands[i] == operands[j] {
                            let err = LegalityError::DuplicateOperandQubit {
                                qubit: operands[i],
                                operands: operands.clone(),
                            };
                            return Err(self.make_violation(id, seq, op_name, operands, err));
                        }
                    }
                }
                return Ok(());
            } else if !self.target.supports_operation("barrier") {
                let err = LegalityError::UnsupportedOperation("barrier".to_string());
                return Err(self.make_violation(id, seq, op_name, operands, err));
            }
        }

        // 2. Classical operations without quantum wires
        if matches!(op, crate::ir::operation::Operation::Classical { .. }) {
            return Ok(());
        }

        // 3. Validate against target operation descriptor
        let desc = match self.target.operation(op_name) {
            Some(d) => d,
            None => {
                let err = LegalityError::UnsupportedOperation(op_name.to_string());
                return Err(self.make_violation(id, seq, op_name, operands, err));
            }
        };

        // 4. Validate quantum arity
        if operands.len() != desc.quantum_arity {
            let err = LegalityError::ArityMismatch {
                operation: op_name.to_string(),
                expected: desc.quantum_arity,
                actual: operands.len(),
            };
            return Err(self.make_violation(id, seq, op_name, operands, err));
        }

        // 5. Validate physical qubit bounds
        for &q in &operands {
            if q.index() >= self.target.num_qubits {
                let err = LegalityError::QubitOutOfBounds {
                    qubit: q,
                    capacity: self.target.num_qubits,
                };
                return Err(self.make_violation(id, seq, op_name, operands, err));
            }
        }

        // 6. Validate distinct physical operands (no duplicates)
        for i in 0..operands.len() {
            for j in (i + 1)..operands.len() {
                if operands[i] == operands[j] {
                    let err = LegalityError::DuplicateOperandQubit {
                        qubit: operands[i],
                        operands: operands.clone(),
                    };
                    return Err(self.make_violation(id, seq, op_name, operands, err));
                }
            }
        }

        // 7. Validate physical coupling and directionality
        if !desc.connectivity.is_valid_operands(&operands) {
            let err = LegalityError::IllegalCoupling {
                operation: op_name.to_string(),
                operands: operands.clone(),
            };
            return Err(self.make_violation(id, seq, op_name, operands, err));
        }

        // 8. Validate parameter count and domain constraints
        let params = op.parameters();
        if params.len() != desc.parameter_count() {
            let err = LegalityError::ParameterCountMismatch {
                operation: op_name.to_string(),
                expected: desc.parameter_count(),
                actual: params.len(),
            };
            return Err(self.make_violation(id, seq, op_name, operands, err));
        }

        for (i, (param_expr, &domain)) in params.iter().zip(&desc.parameter_domains).enumerate() {
            if let Some(val) = param_expr.as_constant() {
                if !val.is_finite() {
                    let err = LegalityError::NonFiniteParameter {
                        index: i,
                        value: val,
                    };
                    return Err(self.make_violation(id, seq, op_name, operands, err));
                }

                match domain {
                    ParameterDomain::Angle4Pi
                    | ParameterDomain::Angle2Pi
                    | ParameterDomain::UnconstrainedReal => {
                        // All finite real numbers are mathematically valid in continuous angular/unconstrained domains
                    }
                }
            }
        }

        Ok(())
    }
}

// ============================================================================
// TargetLegalityPass (CompilerPass Integration)
// ============================================================================

/// Compiler pass validating target legality within a [`crate::opt::pass_manager::PassManager`].
pub struct TargetLegalityPass {
    validator: TargetLegalityValidator,
    domain: SemanticDomain,
    last_report: Option<TargetLegalityReport>,
}

impl TargetLegalityPass {
    /// Creates a new target legality pass for `target`.
    pub fn new(target: Target) -> Self {
        Self {
            validator: TargetLegalityValidator::new(target),
            domain: SemanticDomain::GeneralStructural,
            last_report: None,
        }
    }

    /// Wraps an existing [`TargetLegalityValidator`].
    pub fn with_validator(validator: TargetLegalityValidator) -> Self {
        Self {
            validator,
            domain: SemanticDomain::GeneralStructural,
            last_report: None,
        }
    }

    /// Returns the legality report from the most recent pass run, if any.
    pub fn last_report(&self) -> Option<&TargetLegalityReport> {
        self.last_report.as_ref()
    }
}

impl CompilerPass for TargetLegalityPass {
    fn id(&self) -> PassId {
        PassId::new("target_legality_validator")
    }

    fn description(&self) -> &'static str {
        "Independently validates physical circuit operations against target coupling, arity, and parameter constraints."
    }

    fn semantic_domain(&self) -> SemanticDomain {
        self.domain
    }

    fn mutation_scope(&self) -> PassMutation {
        PassMutation::None
    }

    fn run(
        &mut self,
        circuit: &mut Circuit,
        _context: &mut PassContext,
    ) -> Result<PassResult, PassError> {
        let report =
            self.validator
                .validate_circuit(circuit)
                .map_err(|err| PassError::ExecutionFailed {
                    pass_id: self.id(),
                    reason: format!("{err}"),
                })?;

        self.last_report = Some(report);
        Ok(PassResult::unmodified())
    }
}
