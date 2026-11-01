//! Insertion rewrite operations for QccIR dependency DAGs.

use crate::ir::circuit::Circuit;
use crate::ir::node::{NodeData, OperationId};
use crate::ir::operation::Operation;
use crate::ir::rewrite::{CircuitTransaction, RewriteError};

/// Inserts an operation immediately after the specified target operation.
pub fn insert_after(
    circuit: &mut Circuit,
    target: OperationId,
    new_op: Operation,
) -> Result<OperationId, RewriteError> {
    let mut tx = CircuitTransaction::begin(circuit);
    let id = tx.insert_after(target, new_op)?;
    tx.commit()?;
    Ok(id)
}

/// Inserts an operation immediately before the specified target operation.
pub fn insert_before(
    circuit: &mut Circuit,
    target: OperationId,
    new_op: Operation,
) -> Result<OperationId, RewriteError> {
    let mut tx = CircuitTransaction::begin(circuit);
    let id = tx.insert_before(target, new_op)?;
    tx.commit()?;
    Ok(id)
}

/// Inserts an operation at the start of the circuit execution order.
pub fn insert_at_start(
    circuit: &mut Circuit,
    new_op: Operation,
) -> Result<OperationId, RewriteError> {
    let mut tx = CircuitTransaction::begin(circuit);
    let id = tx.insert_at_start(new_op)?;
    tx.commit()?;
    Ok(id)
}

/// Appends an operation at the end of the circuit execution order.
pub fn insert_at_end(
    circuit: &mut Circuit,
    new_op: Operation,
) -> Result<OperationId, RewriteError> {
    let mut tx = CircuitTransaction::begin(circuit);
    let id = tx.insert_at_end(new_op)?;
    tx.commit()?;
    Ok(id)
}

pub(crate) fn stage_insert_after(
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
    nodes.insert(target_idx + 1, new_node);

    candidate.rebuild_graph(nodes)?;
    Ok(new_id)
}

pub(crate) fn stage_insert_before(
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
    nodes.insert(target_idx, new_node);

    candidate.rebuild_graph(nodes)?;
    Ok(new_id)
}

pub(crate) fn stage_insert_at_start(
    candidate: &mut Circuit,
    new_op: Operation,
) -> Result<OperationId, RewriteError> {
    candidate.check_operation_wire_bounds(&new_op)?;

    let mut nodes = candidate.graph.ordered_nodes()?;
    let new_id = candidate.graph.allocate_operation_id();
    let new_node = NodeData::new(new_id, 0, new_op);
    nodes.insert(0, new_node);

    candidate.rebuild_graph(nodes)?;
    Ok(new_id)
}

pub(crate) fn stage_insert_at_end(
    candidate: &mut Circuit,
    new_op: Operation,
) -> Result<OperationId, RewriteError> {
    candidate.check_operation_wire_bounds(&new_op)?;

    let mut nodes = candidate.graph.ordered_nodes()?;
    let new_id = candidate.graph.allocate_operation_id();
    let new_node = NodeData::new(new_id, 0, new_op);
    nodes.push(new_node);

    candidate.rebuild_graph(nodes)?;
    Ok(new_id)
}
