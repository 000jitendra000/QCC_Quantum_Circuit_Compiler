//! Transactional rewrite infrastructure for QccIR dependency DAGs.
//!
//! Provides atomic, validated graph mutations with automatic wire reconnection,
//! classical hazard modeling, cycle detection, and complete rollback on failure.

use crate::ir::circuit::Circuit;
use crate::ir::error::{IrError, IrValidationError};
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::validation::validate_circuit;

pub mod expand;
pub mod fuse;
pub mod insert;
pub mod remove;
pub mod replace;
pub mod swap;

pub use expand::expand_operation;
pub use fuse::{check_convexity, fuse_operations};
pub use insert::{insert_after, insert_at_end, insert_at_start, insert_before};
pub use remove::remove_operation;
pub use replace::{replace_operation, replace_sequence};
pub use swap::{reorder_operations, swap_adjacent_operations};

/// Errors arising during transactional DAG rewriting.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum RewriteError {
    /// Target operation ID does not exist in the circuit graph.
    #[error("operation {id} not found in circuit")]
    OperationNotFound {
        /// The missing operation identifier.
        id: OperationId,
    },

    /// A directed cycle was detected after graph reconstruction.
    #[error("cycle detected in dependency graph")]
    CycleDetected,

    /// Gate fusion violates DAG convexity or target constraints.
    #[error("invalid fusion: {reason}")]
    InvalidFusion {
        /// Explanation of why the fusion was rejected.
        reason: String,
    },

    /// Graph or circuit semantic invariant validation failed.
    #[error("circuit validation failed: {0}")]
    Validation(#[from] IrValidationError),

    /// Lower-level IR operation or bound error.
    #[error("IR error: {0}")]
    Ir(#[from] IrError),

    /// Postcondition verification failed after candidate mutation.
    #[error("postcondition failed: {reason}")]
    PostconditionFailed {
        /// Reason for postcondition failure.
        reason: String,
    },
}

/// Summary report of operations added and removed during a committed transaction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RewriteResult {
    /// Stable semantic IDs of operations added during the rewrite.
    pub added: Vec<OperationId>,
    /// Stable semantic IDs of operations removed during the rewrite.
    pub removed: Vec<OperationId>,
}

impl RewriteResult {
    /// Creates a new empty `RewriteResult`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if no operations were added or removed.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }
}

/// Transactional wrapper providing atomic, multi-step mutations on a [`Circuit`].
///
/// All mutations are staged on an isolated candidate copy. If validation fails or
/// the transaction is dropped before calling [`commit`](Self::commit), the target
/// circuit is completely unmodified.
pub struct CircuitTransaction<'a> {
    circuit: &'a mut Circuit,
    candidate: Circuit,
    added: Vec<OperationId>,
    removed: Vec<OperationId>,
    committed: bool,
}

impl<'a> CircuitTransaction<'a> {
    /// Begins a new transaction on the target circuit.
    pub fn begin(circuit: &'a mut Circuit) -> Self {
        let candidate = circuit.clone();
        Self {
            circuit,
            candidate,
            added: Vec::new(),
            removed: Vec::new(),
            committed: false,
        }
    }

    /// Returns a reference to the staged candidate circuit.
    pub fn circuit(&self) -> &Circuit {
        &self.candidate
    }

    /// Inserts an operation immediately after the specified target operation.
    pub fn insert_after(
        &mut self,
        target: OperationId,
        new_op: Operation,
    ) -> Result<OperationId, RewriteError> {
        let id = insert::stage_insert_after(&mut self.candidate, target, new_op)?;
        self.added.push(id);
        Ok(id)
    }

    /// Inserts an operation immediately before the specified target operation.
    pub fn insert_before(
        &mut self,
        target: OperationId,
        new_op: Operation,
    ) -> Result<OperationId, RewriteError> {
        let id = insert::stage_insert_before(&mut self.candidate, target, new_op)?;
        self.added.push(id);
        Ok(id)
    }

    /// Inserts an operation at the start of the circuit execution order.
    pub fn insert_at_start(&mut self, new_op: Operation) -> Result<OperationId, RewriteError> {
        let id = insert::stage_insert_at_start(&mut self.candidate, new_op)?;
        self.added.push(id);
        Ok(id)
    }

    /// Appends an operation at the end of the circuit execution order.
    pub fn insert_at_end(&mut self, new_op: Operation) -> Result<OperationId, RewriteError> {
        let id = insert::stage_insert_at_end(&mut self.candidate, new_op)?;
        self.added.push(id);
        Ok(id)
    }

    /// Removes the specified operation from the circuit, reconnecting dependencies on affected wires.
    pub fn remove_operation(&mut self, target: OperationId) -> Result<Operation, RewriteError> {
        let op = remove::stage_remove_operation(&mut self.candidate, target)?;
        self.removed.push(target);
        Ok(op)
    }

    /// Replaces the target operation with a new operation in place.
    pub fn replace_operation(
        &mut self,
        target: OperationId,
        new_op: Operation,
    ) -> Result<OperationId, RewriteError> {
        let id = replace::stage_replace_operation(&mut self.candidate, target, new_op)?;
        self.removed.push(target);
        self.added.push(id);
        Ok(id)
    }

    /// Replaces multiple target operations with a sequence of replacement operations.
    ///
    /// Verifies DAG convexity before rewriting.
    pub fn replace_sequence(
        &mut self,
        targets: &[OperationId],
        replacement_ops: Vec<Operation>,
    ) -> Result<Vec<OperationId>, RewriteError> {
        let ids = replace::stage_replace_sequence(&mut self.candidate, targets, replacement_ops)?;
        self.removed.extend(targets.iter().copied());
        self.added.extend(ids.iter().copied());
        Ok(ids)
    }

    /// Expands a single target operation into a sequence of replacement operations.
    pub fn expand_operation(
        &mut self,
        target: OperationId,
        replacement_ops: Vec<Operation>,
    ) -> Result<Vec<OperationId>, RewriteError> {
        let ids = expand::stage_expand_operation(&mut self.candidate, target, replacement_ops)?;
        self.removed.push(target);
        self.added.extend(ids.iter().copied());
        Ok(ids)
    }

    /// Fuses multiple target operations into a single combined operation.
    ///
    /// Verifies DAG convexity before rewriting.
    pub fn fuse_operations(
        &mut self,
        targets: &[OperationId],
        fused_op: Operation,
    ) -> Result<OperationId, RewriteError> {
        let id = fuse::stage_fuse_operations(&mut self.candidate, targets, fused_op)?;
        self.removed.extend(targets.iter().copied());
        self.added.push(id);
        Ok(id)
    }

    /// Swaps the execution order of two adjacent operations in the DAG.
    pub fn swap_adjacent_operations(
        &mut self,
        first: OperationId,
        second: OperationId,
    ) -> Result<(), RewriteError> {
        swap::stage_swap_adjacent_operations(&mut self.candidate, first, second)
    }

    /// Reorders all operations in the DAG according to the specified list of operation IDs.
    pub fn reorder_operations(&mut self, new_order: &[OperationId]) -> Result<(), RewriteError> {
        swap::stage_reorder_operations(&mut self.candidate, new_order)
    }

    /// Adds a global phase adjustment to the candidate circuit.
    pub fn add_global_phase(&mut self, phase: ParameterExpr) {
        self.candidate.add_global_phase(phase);
    }

    /// Sets the global phase of the candidate circuit.
    pub fn set_global_phase(&mut self, phase: ParameterExpr) {
        self.candidate.set_global_phase(phase);
    }

    /// Returns a reference to the global phase of the candidate circuit.
    pub fn global_phase(&self) -> &ParameterExpr {
        self.candidate.global_phase()
    }

    /// Validates the staged candidate and commits changes to the underlying circuit.
    ///
    /// # Errors
    /// Returns [`RewriteError`] if the candidate contains cycles or violates semantic invariants.
    /// In case of error, the underlying circuit is completely unchanged.
    pub fn commit(mut self) -> Result<RewriteResult, RewriteError> {
        if self.candidate.graph.is_cyclic() {
            return Err(RewriteError::CycleDetected);
        }

        validate_circuit(&self.candidate)?;

        *self.circuit = self.candidate;
        self.committed = true;

        Ok(RewriteResult {
            added: self.added,
            removed: self.removed,
        })
    }

    /// Discards all staged changes without modifying the underlying circuit.
    pub fn rollback(self) {
        // Drop without setting committed = true leaves self.circuit untouched.
    }
}
