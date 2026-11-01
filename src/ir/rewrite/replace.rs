//! Replacement rewrite operations for QccIR dependency DAGs.

use crate::ir::circuit::Circuit;
use crate::ir::node::{NodeData, OperationId};
use crate::ir::operation::Operation;
use crate::ir::rewrite::{CircuitTransaction, RewriteError};

/// Replaces the target operation with a new operation in place.
pub fn replace_operation(
    circuit: &mut Circuit,
    target: OperationId,
    new_op: Operation,
) -> Result<OperationId, RewriteError> {
    let mut tx = CircuitTransaction::begin(circuit);
    let id = tx.replace_operation(target, new_op)?;
    tx.commit()?;
    Ok(id)
}

pub(crate) fn stage_replace_operation(
    candidate: &mut Circuit,
    target: OperationId,
    new_op: Operation,
) -> Result<OperationId, RewriteError> {
    if !candidate.graph.contains_operation(target) {
        return Err(RewriteError::OperationNotFound { id: target });
    }
    candidate.check_operation_wire_bounds(&new_op)?;

    let mut nodes = candidate.graph.ordered_nodes()?;
    let target_idx = nodes
        .iter()
        .position(|n| n.id == target)
        .ok_or(RewriteError::OperationNotFound { id: target })?;

    let new_id = candidate.graph.allocate_operation_id();
    let new_node = NodeData::with_provenance(new_id, 0, new_op, vec![target]);
    nodes[target_idx] = new_node;

    candidate.rebuild_graph(nodes)?;
    Ok(new_id)
}
