//! Directionality queries and coupling orientation analysis for two-qubit hardware operations.
//!
//! Provides typed direction classification (`Legal`, `NeedsReversal`, `NoCoupling`)
//! distinguishing hardware coupling graph direction from operation-level control/target semantics.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::gate_catalog::GateCatalog;
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;

/// Status of physical operand orientation for a two-qubit operation on hardware.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DirectionStatus {
    /// Physical operand tuple $(u, v)$ is directly legal and executable on the target.
    Legal,
    /// Forward physical operand tuple $(u, v)$ is not permitted, but the reversed tuple $(v, u)$ is legal.
    NeedsReversal,
    /// Neither $(u, v)$ nor $(v, u)$ is permitted on the target (no physical coupling edge exists).
    NoCoupling,
    /// The requested operation is not supported by the target.
    UnsupportedOperation,
    /// The operation quantum arity is not 2.
    ArityMismatch,
}

impl DirectionStatus {
    /// Returns `true` if the operation is directly legal without any direction correction.
    #[inline]
    pub fn is_legal(&self) -> bool {
        matches!(self, Self::Legal)
    }

    /// Returns `true` if reversing physical operands $(v, u)$ satisfies target connectivity.
    #[inline]
    pub fn needs_reversal(&self) -> bool {
        matches!(self, Self::NeedsReversal)
    }

    /// Returns `true` if no coupling edge exists between the two physical qubits in either direction.
    #[inline]
    pub fn is_unconnected(&self) -> bool {
        matches!(self, Self::NoCoupling)
    }
}

/// Structured error for direction analysis and correction failures.
#[derive(Clone, Debug, PartialEq, Error, Serialize, Deserialize)]
pub enum DirectionError {
    /// An operation cannot be satisfied on target because no physical edge exists in either orientation.
    #[error("operation '{operation}' on physical qubits ({u}, {v}) has no viable direction on target '{target}': {reason}")]
    NoViableDirection {
        /// Operation mnemonic.
        operation: String,
        /// First physical operand.
        u: PhysicalQubitId,
        /// Second physical operand.
        v: PhysicalQubitId,
        /// Target machine identifier.
        target: String,
        /// Diagnostic reason.
        reason: String,
    },

    /// Direction correction sequence failed local unitary equivalence verification.
    #[error("direction correction for operation '{operation}' on qubits ({u}, {v}) failed unitary verification: max diff {max_diff:.3e} exceeds tolerance {tolerance:.3e}")]
    VerificationFailed {
        /// Operation mnemonic.
        operation: String,
        /// First physical operand.
        u: PhysicalQubitId,
        /// Second physical operand.
        v: PhysicalQubitId,
        /// Maximum matrix element discrepancy.
        max_diff: f64,
        /// Permitted tolerance threshold.
        tolerance: f64,
    },

    /// Emitted operation failed target legality checks after direction correction.
    #[error("target legality violation after direction correction: {0}")]
    Legality(String),
}

impl Target {
    /// Evaluates the direction status of a two-qubit operation on physical qubits `(u, v)`.
    pub fn check_direction(
        &self,
        op_name: &str,
        u: PhysicalQubitId,
        v: PhysicalQubitId,
    ) -> DirectionStatus {
        let desc = match self.operation(op_name) {
            Some(d) => d,
            None => return DirectionStatus::UnsupportedOperation,
        };

        if desc.quantum_arity != 2 {
            return DirectionStatus::ArityMismatch;
        }

        // 1. Check if forward tuple (u, v) is legal
        if self.check_operand_legality(op_name, &[u, v]).is_ok() {
            return DirectionStatus::Legal;
        }

        // 2. Check if reversed tuple (v, u) is legal
        if self.check_operand_legality(op_name, &[v, u]).is_ok() {
            return DirectionStatus::NeedsReversal;
        }

        DirectionStatus::NoCoupling
    }

    /// Returns `true` if the target operation enforces directed (asymmetric) coupling.
    #[inline]
    pub fn is_operation_directed(&self, op_name: &str) -> bool {
        self.operation(op_name)
            .map(|desc| desc.connectivity.is_directed())
            .unwrap_or(false)
    }

    /// Returns `true` if an operation is direction-sensitive due to either asymmetric gate semantics
    /// or directed target coupling.
    pub fn is_direction_sensitive(&self, op_name: &str) -> bool {
        if self.is_operation_directed(op_name) {
            return true;
        }

        // Check if gate semantics are asymmetric (e.g. CX control != target)
        if let Some(entry) = GateCatalog::lookup(op_name) {
            let sig = GateCatalog::get(entry.id);
            if !sig.is_symmetric {
                return true;
            }
        }

        // Explicit fallback for canonical asymmetric gates
        op_name.eq_ignore_ascii_case("cx")
            || op_name.eq_ignore_ascii_case("cnot")
            || op_name.eq_ignore_ascii_case("cy")
            || op_name.eq_ignore_ascii_case("ch")
    }
}
