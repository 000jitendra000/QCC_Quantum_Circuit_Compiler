//! Target legality diagnostics and standardized diagnostic codes.
//!
//! Provides diagnostic classification and actionable diagnostic construction for
//! target legality violations encountered during physical circuit verification.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::diagnostics::diagnostic::Diagnostic;
use crate::ir::node::OperationId;
use crate::target::direction::DirectionStatus;
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;
use crate::target::query::LegalityError;

/// Standardized diagnostic codes for target legality failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetDiagnosticCode {
    /// Operation mnemonic is not supported by target (`T0001`).
    UnsupportedOperation,
    /// Quantum arity mismatch (`T0002`).
    ArityMismatch,
    /// Physical qubit index exceeds device capacity (`T0003`).
    QubitOutOfBounds,
    /// Duplicate physical qubit operand in multi-qubit gate (`T0004`).
    DuplicateOperandQubit,
    /// Coupling graph edge or operand directionality violation (`T0005`).
    IllegalCoupling,
    /// Gate parameter count mismatch (`T0006`).
    ParameterCountMismatch,
    /// Non-finite continuous parameter value (NaN or infinite) (`T0007`).
    NonFiniteParameter,
    /// Continuous parameter violates declared domain bounds (`T0008`).
    ParameterDomainViolation,
}

impl TargetDiagnosticCode {
    /// Returns the standardized code string (e.g. `"T0001"`).
    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnsupportedOperation => "T0001",
            Self::ArityMismatch => "T0002",
            Self::QubitOutOfBounds => "T0003",
            Self::DuplicateOperandQubit => "T0004",
            Self::IllegalCoupling => "T0005",
            Self::ParameterCountMismatch => "T0006",
            Self::NonFiniteParameter => "T0007",
            Self::ParameterDomainViolation => "T0008",
        }
    }

    /// Returns a short human-readable description of the diagnostic code.
    pub fn description(&self) -> &'static str {
        match self {
            Self::UnsupportedOperation => "target does not support operation",
            Self::ArityMismatch => "operation quantum arity mismatch",
            Self::QubitOutOfBounds => "physical qubit ID exceeds target capacity",
            Self::DuplicateOperandQubit => "duplicate physical qubit in operand tuple",
            Self::IllegalCoupling => "unsupported coupling edge or operand directionality",
            Self::ParameterCountMismatch => "parameter count does not match target descriptor",
            Self::NonFiniteParameter => "parameter value is non-finite (NaN or infinite)",
            Self::ParameterDomainViolation => "parameter value violates domain constraints",
        }
    }
}

impl fmt::Display for TargetDiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Constructs a rich compiler [`Diagnostic`] from a [`LegalityError`] in the context of a target and node.
pub fn create_target_diagnostic(
    node_id: Option<OperationId>,
    op_name: &str,
    _operands: &[PhysicalQubitId],
    err: &LegalityError,
    target: &Target,
) -> Diagnostic {
    let node_prefix = match node_id {
        Some(id) => format!("{id}: "),
        None => String::new(),
    };

    match err {
        LegalityError::UnsupportedOperation(name) => {
            let mut supported_ops: Vec<String> =
                target.operations.keys().map(|k| k.to_string()).collect();
            supported_ops.sort();

            let code = TargetDiagnosticCode::UnsupportedOperation;
            let mut diag = Diagnostic::error(format!(
                "{node_prefix}operation '{name}' is not supported by hardware target '{}'",
                target.name
            ))
            .with_code(code.as_str());

            if !supported_ops.is_empty() {
                diag = diag.with_note(format!(
                    "target '{}' natively supports operations: [{}]",
                    target.name,
                    supported_ops.join(", ")
                ));
            } else {
                diag = diag.with_note(format!(
                    "target '{}' declares zero native operations",
                    target.name
                ));
            }
            diag
        }

        LegalityError::ArityMismatch {
            operation,
            expected,
            actual,
        } => {
            let code = TargetDiagnosticCode::ArityMismatch;
            Diagnostic::error(format!(
                "{node_prefix}arity mismatch for operation '{operation}': expected {expected} physical operands, got {actual}"
            ))
            .with_code(code.as_str())
            .with_note(format!(
                "target operation '{operation}' declares quantum arity {expected}"
            ))
        }

        LegalityError::QubitOutOfBounds { qubit, capacity } => {
            let code = TargetDiagnosticCode::QubitOutOfBounds;
            let valid_range = if *capacity > 0 {
                format!("0..={}", capacity - 1)
            } else {
                "empty".to_string()
            };

            Diagnostic::error(format!(
                "{node_prefix}physical qubit {qubit} is out of bounds for target '{}'",
                target.name
            ))
            .with_code(code.as_str())
            .with_note(format!(
                "target '{}' physical capacity is {capacity} qubits (valid indices: {valid_range})",
                target.name
            ))
        }

        LegalityError::DuplicateOperandQubit { qubit, operands } => {
            let code = TargetDiagnosticCode::DuplicateOperandQubit;
            Diagnostic::error(format!(
                "{node_prefix}duplicate physical qubit {qubit} in operand tuple {operands:?}"
            ))
            .with_code(code.as_str())
            .with_note(
                "multi-qubit operations require mutually distinct physical qubit operands"
                    .to_string(),
            )
        }

        LegalityError::IllegalCoupling {
            operation,
            operands,
        } => {
            let code = TargetDiagnosticCode::IllegalCoupling;
            let mut diag = Diagnostic::error(format!(
                "{node_prefix}illegal coupling or directionality for operation '{operation}' on physical operands {operands:?}"
            ))
            .with_code(code.as_str());

            if operands.len() == 2 {
                let u = operands[0];
                let v = operands[1];
                let status = target.check_direction(operation, u, v);
                match status {
                    DirectionStatus::NeedsReversal => {
                        diag = diag.with_note(format!(
                            "coupling edge ({v}, {u}) is supported by target '{}'; direction correction or reversing operands is required",
                            target.name
                        ));
                    }
                    DirectionStatus::NoCoupling => {
                        diag = diag.with_note(format!(
                            "no coupling edge exists between physical qubits ({u}, {v}) on target '{}' in either direction",
                            target.name
                        ));
                    }
                    _ => {
                        diag = diag.with_note(format!(
                            "operands {operands:?} violate target '{}' connectivity constraints",
                            target.name
                        ));
                    }
                }
            } else {
                diag = diag.with_note(format!(
                    "operands {operands:?} violate target '{}' connectivity constraints for '{operation}'",
                    target.name
                ));
            }
            diag
        }

        LegalityError::ParameterCountMismatch {
            operation,
            expected,
            actual,
        } => {
            let code = TargetDiagnosticCode::ParameterCountMismatch;
            Diagnostic::error(format!(
                "{node_prefix}parameter count mismatch for operation '{operation}': expected {expected}, got {actual}"
            ))
            .with_code(code.as_str())
            .with_note(format!(
                "target operation '{operation}' expects {expected} parameters"
            ))
        }

        LegalityError::NonFiniteParameter { index, value } => {
            let code = TargetDiagnosticCode::NonFiniteParameter;
            Diagnostic::error(format!(
                "{node_prefix}non-finite parameter value '{value}' at index {index} for operation '{op_name}'"
            ))
            .with_code(code.as_str())
            .with_note("floating-point parameters must be finite real numbers (not NaN or infinite)".to_string())
        }

        LegalityError::ParameterDomainViolation {
            index,
            domain,
            value,
        } => {
            let code = TargetDiagnosticCode::ParameterDomainViolation;
            Diagnostic::error(format!(
                "{node_prefix}parameter value '{value}' at index {index} for operation '{op_name}' violates domain {domain}"
            ))
            .with_code(code.as_str())
            .with_note(format!("expected parameter in domain {domain}"))
        }
    }
}
