//! N-to-1 gate fusion with DAG convexity verification for QccIR dependency DAGs.

use std::collections::{HashSet, VecDeque};

use crate::ir::circuit::Circuit;
use crate::ir::graph::CircuitGraph;
use crate::ir::node::{NodeData, OperationId};
use crate::ir::operation::Operation;
use crate::ir::rewrite::{CircuitTransaction, RewriteError};

/// Fuses multiple target operations into a single combined operation.
///
/// Verifies DAG convexity before rewriting. Rejects non-convex target sets with
/// [`RewriteError::InvalidFusion`].
pub fn fuse_operations(
    circuit: &mut Circuit,
    targets: &[OperationId],
    fused_op: Operation,
) -> Result<OperationId, RewriteError> {
    let mut tx = CircuitTransaction::begin(circuit);
    let id = tx.fuse_operations(targets, fused_op)?;
    tx.commit()?;
    Ok(id)
}

/// Verifies that a set of target nodes forms a convex subgraph within the dependency DAG.
///
/// A node subset $S \subseteq V$ is convex if and only if for all $u, v \in S$, every directed
/// path from $u$ to $v$ in the DAG consists exclusively of nodes in $S$.
///
/// Equivalently, no intermediate node $w \notin S$ exists such that $u \to^+ w \to^+ v$.
///
/// # Errors
/// Returns [`RewriteError::InvalidFusion`] if the targets are empty, contain duplicates,
/// or violate DAG convexity.
pub fn check_convexity(graph: &CircuitGraph, targets: &[OperationId]) -> Result<(), RewriteError> {
    if targets.is_empty() {
        return Err(RewriteError::InvalidFusion {
            reason: "targets list cannot be empty".to_string(),
        });
    }

    if targets.len() == 1 {
        return Ok(());
    }

    let target_set: HashSet<OperationId> = targets.iter().copied().collect();
    if target_set.len() != targets.len() {
        return Err(RewriteError::InvalidFusion {
            reason: "duplicate target operation in fusion list".to_string(),
        });
    }

    // Collect all descendants of targets outside target_set
    let mut descendants: HashSet<OperationId> = HashSet::new();
    let mut queue: VecDeque<OperationId> = targets.iter().copied().collect();
    let mut visited: HashSet<OperationId> = HashSet::new();

    while let Some(current) = queue.pop_front() {
        for (succ, _) in graph.successors(current) {
            if visited.insert(succ) {
                if !target_set.contains(&succ) {
                    descendants.insert(succ);
                }
                queue.push_back(succ);
            }
        }
    }

    // Collect all ancestors of targets outside target_set
    let mut ancestors: HashSet<OperationId> = HashSet::new();
    queue.extend(targets.iter().copied());
    visited.clear();

    while let Some(current) = queue.pop_front() {
        for (pred, _) in graph.predecessors(current) {
            if visited.insert(pred) {
                if !target_set.contains(&pred) {
                    ancestors.insert(pred);
                }
                queue.push_back(pred);
            }
        }
    }

    // Any intermediate node in (descendants ∩ ancestors) violates convexity
    if let Some(&violating) = descendants.intersection(&ancestors).next() {
        return Err(RewriteError::InvalidFusion {
            reason: format!(
                "targets are not convex in DAG: intermediate node {} is not in fusion set",
                violating
            ),
        });
    }

    Ok(())
}

pub(crate) fn stage_fuse_operations(
    candidate: &mut Circuit,
    targets: &[OperationId],
    fused_op: Operation,
) -> Result<OperationId, RewriteError> {
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

    candidate.check_operation_wire_bounds(&fused_op)?;

    // 1. Check DAG convexity
    check_convexity(&candidate.graph, targets)?;

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

    let fused_id = candidate.graph.allocate_operation_id();
    let fused_node = NodeData::with_provenance(fused_id, 0, fused_op, targets.to_vec());
    nodes.insert(insert_pos, fused_node);

    candidate.rebuild_graph(nodes)?;
    Ok(fused_id)
}
