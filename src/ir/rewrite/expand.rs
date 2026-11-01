//! 1-to-N gate decomposition and expansion for QccIR dependency DAGs.

use crate::ir::circuit::Circuit;
use crate::ir::node::{NodeData, OperationId};
use crate::ir::operation::Operation;
use crate::ir::rewrite::{CircuitTransaction, RewriteError};

/// Expands a single target operation into a sequence of replacement operations.
pub fn expand_operation(
    circuit: &mut Circuit,
    target: OperationId,
    replacement_ops: Vec<Operation>,
) -> Result<Vec<OperationId>, RewriteError> {
    let mut tx = CircuitTransaction::begin(circuit);
    let ids = tx.expand_operation(target, replacement_ops)?;
    tx.commit()?;
    Ok(ids)
}

pub(crate) fn stage_expand_operation(
    candidate: &mut Circuit,
    target: OperationId,
    replacement_ops: Vec<Operation>,
) -> Result<Vec<OperationId>, RewriteError> {
    if !candidate.graph.contains_operation(target) {
        return Err(RewriteError::OperationNotFound { id: target });
    }
    for op in &replacement_ops {
        candidate.check_operation_wire_bounds(op)?;
    }

    let mut nodes = candidate.graph.ordered_nodes()?;
    let target_idx = nodes
        .iter()
        .position(|n| n.id == target)
        .ok_or(RewriteError::OperationNotFound { id: target })?;

    let mut new_ids = Vec::with_capacity(replacement_ops.len());
    let mut new_nodes = Vec::with_capacity(replacement_ops.len());

    for op in replacement_ops {
        let new_id = candidate.graph.allocate_operation_id();
        new_ids.push(new_id);
        new_nodes.push(NodeData::with_provenance(new_id, 0, op, vec![target]));
    }

    nodes.splice(target_idx..=target_idx, new_nodes);

    candidate.rebuild_graph(nodes)?;
    Ok(new_ids)
}
