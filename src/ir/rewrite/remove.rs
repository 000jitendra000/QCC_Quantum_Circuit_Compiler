//! Deletion rewrite operations for QccIR dependency DAGs.

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::rewrite::{CircuitTransaction, RewriteError};

/// Removes the specified operation from the circuit, reconnecting dependencies on all affected wires.
pub fn remove_operation(
    circuit: &mut Circuit,
    target: OperationId,
) -> Result<Operation, RewriteError> {
    let mut tx = CircuitTransaction::begin(circuit);
    let op = tx.remove_operation(target)?;
    tx.commit()?;
    Ok(op)
}

pub(crate) fn stage_remove_operation(
    candidate: &mut Circuit,
    target: OperationId,
) -> Result<Operation, RewriteError> {
    let op = candidate
        .graph
        .get_operation(target)
        .cloned()
        .ok_or(RewriteError::OperationNotFound { id: target })?;

    let mut nodes = candidate.graph.ordered_nodes()?;
    nodes.retain(|n| n.id != target);

    candidate.rebuild_graph(nodes)?;
    Ok(op)
}
