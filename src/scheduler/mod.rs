//! Quantum circuit instruction scheduling subsystem.
//!
//! Milestone M6 (Phases 54–59): Binds physical quantum circuit operations to concrete execution
//! timelines and hardware channels under target duration, calibration, and concurrency constraints.

pub mod alap;
pub mod asap;
pub mod schedule;
pub mod validator;

use thiserror::Error;

use crate::ir::node::OperationId;
use crate::target::error::TargetError;
use crate::target::operation::PhysicalQubitId;
use crate::target::resources::ResourceConflict;

pub use alap::{AlapMakespanPolicy, AlapScheduler};
pub use asap::{AsapScheduler, ScheduleConfig};
pub use schedule::{Schedule, ScheduledOperation};
pub use validator::{ScheduleValidationError, ScheduleValidationReport, ScheduleValidator};

/// Structured error produced during schedule generation.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum ScheduleError {
    /// Failure during DAG traversal (e.g. cycle or invalid dependencies).
    #[error("circuit DAG traversal error: {0}")]
    DagError(String),

    /// Operation has unknown or uncalibrated duration.
    #[error(
        "operation '{op_name}' (op#{}) on operands {operands:?} has unknown duration: cannot schedule without valid timing",
        node_id.0
    )]
    UnknownDuration {
        /// Stable node identifier.
        node_id: OperationId,
        /// Operation mnemonic.
        op_name: String,
        /// Physical operand qubits.
        operands: Vec<PhysicalQubitId>,
    },

    /// Target has no declared resource model.
    #[error(
        "target '{target}' has no declared resource model: cannot assume unrestricted concurrency"
    )]
    MissingResourceModel {
        /// Target machine name.
        target: String,
    },

    /// Operation lacks declared resource requirements or feedline mappings.
    #[error("operation '{op_name}' on operands {operands:?} lacks declared hardware resource metadata: cannot assume concurrency")]
    MissingOperationResources {
        /// Operation mnemonic.
        op_name: String,
        /// Physical operand qubits.
        operands: Vec<PhysicalQubitId>,
    },

    /// Requested schedule makespan window is infeasible.
    #[error("requested schedule makespan {requested_ns:.2}ns is infeasible (minimum makespan is {minimum_ns:.2}ns)")]
    InfeasibleMakespan {
        /// Requested schedule makespan in nanoseconds.
        requested_ns: f64,
        /// Minimum feasible makespan derived from ASAP baseline in nanoseconds.
        minimum_ns: f64,
    },

    /// Negative slack detected for an operation.
    #[error("negative slack ({slack_ns:.2}ns) detected for operation '{op_name}' (op#{}) on operands {operands:?}: latest start {latest_start_ns:.2}ns precedes earliest ASAP start", op_id.0)]
    NegativeSlack {
        /// Stable node identifier.
        op_id: OperationId,
        /// Operation mnemonic.
        op_name: String,
        /// Physical operand qubits.
        operands: Vec<PhysicalQubitId>,
        /// Calculated negative slack in nanoseconds.
        slack_ns: f64,
        /// Calculated latest start time in nanoseconds.
        latest_start_ns: f64,
    },

    /// Invalid duration encountered (negative, non-finite, or inconsistent clock period).
    #[error("target error during scheduling: {0}")]
    Target(#[from] TargetError),

    /// Concurrency or resource conflict during schedule creation.
    #[error("resource conflict during scheduling: {0}")]
    Conflict(#[from] ResourceConflict),

    /// Schedule validation failure.
    #[error("schedule validation error: {0}")]
    Validation(String),
}
