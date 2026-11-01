//! Error definitions for the QCC compiler pass framework and pass manager.

use thiserror::Error;

use crate::ir::error::{IrError, IrValidationError};
use crate::opt::context::AnalysisId;
use crate::opt::pass::PassId;
use crate::verification::unitary::VerificationError;

/// Errors arising during the execution of an individual compiler pass.
#[derive(Debug, Error)]
pub enum PassError {
    /// A required precondition was violated prior to pass execution.
    #[error("pass '{pass_id}' precondition failed: {reason}")]
    PreconditionViolation {
        /// The pass whose precondition failed.
        pass_id: PassId,
        /// Description of the violation.
        reason: String,
    },

    /// A required postcondition was violated after pass execution.
    #[error("pass '{pass_id}' postcondition failed: {reason}")]
    PostconditionViolation {
        /// The pass whose postcondition failed.
        pass_id: PassId,
        /// Description of the violation.
        reason: String,
    },

    /// Pass execution failed internally.
    #[error("pass '{pass_id}' execution failed: {reason}")]
    ExecutionFailed {
        /// The pass that failed.
        pass_id: PassId,
        /// Description of the failure.
        reason: String,
    },

    /// QccIR graph or wire access manipulation error.
    #[error("IR error during pass '{pass_id}': {source}")]
    Ir {
        /// The pass during which the error occurred.
        pass_id: PassId,
        /// Underlying IR error.
        #[source]
        source: IrError,
    },

    /// QccIR structural validation error.
    #[error("IR validation error during pass '{pass_id}': {source}")]
    IrValidation {
        /// The pass during which the error occurred.
        pass_id: PassId,
        /// Underlying IR validation error.
        #[source]
        source: IrValidationError,
    },

    /// Unitary verification error during debug checks.
    #[error("verification error during pass '{pass_id}': {source}")]
    Verification {
        /// The pass during which the error occurred.
        pass_id: PassId,
        /// Underlying verification error.
        #[source]
        source: VerificationError,
    },
}

/// Errors arising during pass pipeline orchestration in the [`PassManager`](crate::opt::pass_manager::PassManager).
#[derive(Debug, Error)]
pub enum PassManagerError {
    /// A pass required an analysis that was either missing or previously invalidated.
    #[error(
        "pass '{pass_id}' requires analysis '{analysis_id}', which is missing or invalidated in context"
    )]
    MissingRequiredAnalysis {
        /// Pass that declared the requirement.
        pass_id: PassId,
        /// Missing or invalidated analysis.
        analysis_id: AnalysisId,
    },

    /// A pass's preconditions were not met.
    #[error(
        "pass pipeline halted at step {step_index}: pass '{pass_id}' precondition failed: {error}"
    )]
    PreconditionFailed {
        /// Pipeline execution index (0-based).
        step_index: usize,
        /// Pass that failed precondition checking.
        pass_id: PassId,
        /// Specific precondition error.
        #[source]
        error: PassError,
    },

    /// An individual pass failed during execution.
    #[error(
        "pass pipeline halted at step {step_index}: pass '{pass_id}' execution failed: {error}"
    )]
    PassFailed {
        /// Pipeline execution index (0-based).
        step_index: usize,
        /// Pass that failed.
        pass_id: PassId,
        /// Specific pass execution error.
        #[source]
        error: PassError,
    },

    /// A pass's declared postconditions were not met.
    #[error(
        "pass pipeline halted at step {step_index}: pass '{pass_id}' postcondition failed: {error}"
    )]
    PostconditionFailed {
        /// Pipeline execution index (0-based).
        step_index: usize,
        /// Pass that failed postcondition checking.
        pass_id: PassId,
        /// Specific postcondition error.
        #[source]
        error: PassError,
    },

    /// Debug verification detected an invalid circuit state after a pass.
    #[error("pass pipeline halted at step {step_index}: debug verification failed after pass '{pass_id}': {error}")]
    DebugVerificationFailed {
        /// Pipeline execution index (0-based).
        step_index: usize,
        /// Pass that produced the invalid state.
        pass_id: PassId,
        /// Underlying verification or validation error.
        #[source]
        error: PassError,
    },
}
