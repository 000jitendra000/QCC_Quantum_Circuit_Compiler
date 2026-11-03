//! Replacement rewrite operations for QccIR dependency DAGs.

use std::collections::{HashSet, VecDeque};

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

/// Replaces a sequence of operations with a new sequence of replacement operations.
///
/// Verifies DAG convexity before rewriting. Reconnects dependencies on all affected wires.
pub fn replace_sequence(
    circuit: &mut Circuit,
    targets: &[OperationId],
    replacement_ops: Vec<Operation>,
) -> Result<Vec<OperationId>, RewriteError> {
    let mut tx = CircuitTransaction::begin(circuit);
    let ids = tx.replace_sequence(targets, replacement_ops)?;
    tx.commit()?;
    Ok(ids)
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

pub(crate) fn stage_replace_sequence(
    candidate: &mut Circuit,
    targets: &[OperationId],
    replacement_ops: Vec<Operation>,
) -> Result<Vec<OperationId>, RewriteError> {
    if targets.is_empty() {
        return Err(RewriteError::InvalidFusion {
            reason: "targets list cannot be empty".to_string(),
        });
    }

    for &id in targets {
        if !candidate.graph.contains_operation(id) {
            return Err(RewriteError::OperationNotFound { id });
        }
    }

    for op in &replacement_ops {
        candidate.check_operation_wire_bounds(op)?;
    }

    // 1. Check DAG convexity
    crate::ir::rewrite::fuse::check_convexity(&candidate.graph, targets)?;

    let target_set: HashSet<OperationId> = targets.iter().copied().collect();

    // 2. Find ancestors of targets outside target_set
    let mut ancestors: HashSet<OperationId> = HashSet::new();
    let mut queue: VecDeque<OperationId> = targets.iter().copied().collect();
    let mut visited: HashSet<OperationId> = HashSet::new();
    while let Some(current) = queue.pop_front() {
        for (pred, _) in candidate.graph.predecessors(current) {
            if visited.insert(pred) {
                if !target_set.contains(&pred) {
                    ancestors.insert(pred);
                }
                queue.push_back(pred);
            }
        }
    }

    // 3. Find descendants of targets outside target_set
    let mut descendants: HashSet<OperationId> = HashSet::new();
    queue.extend(targets.iter().copied());
    visited.clear();
    while let Some(current) = queue.pop_front() {
        for (succ, _) in candidate.graph.successors(current) {
            if visited.insert(succ) {
                if !target_set.contains(&succ) {
                    descendants.insert(succ);
                }
                queue.push_back(succ);
            }
        }
    }

    let mut nodes = candidate.graph.ordered_nodes()?;

    // Find the index of the first target in original order
    let first_target_pos = nodes
        .iter()
        .position(|n| target_set.contains(&n.id))
        .unwrap_or(0);

    // Remove all targets from nodes
    nodes.retain(|n| !target_set.contains(&n.id));

    // Determine safe insertion bounds in remaining nodes
    let mut min_pos = 0;
    let mut max_pos = nodes.len();

    for (i, node) in nodes.iter().enumerate() {
        if ancestors.contains(&node.id) {
            min_pos = min_pos.max(i + 1);
        }
        if descendants.contains(&node.id) {
            max_pos = max_pos.min(i);
        }
    }

    let insert_pos = first_target_pos.clamp(min_pos, max_pos);

    let mut new_ids = Vec::with_capacity(replacement_ops.len());
    let mut new_nodes = Vec::with_capacity(replacement_ops.len());

    for op in replacement_ops {
        let new_id = candidate.graph.allocate_operation_id();
        new_ids.push(new_id);
        new_nodes.push(NodeData::with_provenance(new_id, 0, op, targets.to_vec()));
    }

    nodes.splice(insert_pos..insert_pos, new_nodes);

    candidate.rebuild_graph(nodes)?;
    Ok(new_ids)
}
