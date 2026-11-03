//! Operation swap and reordering rewrite operations for QccIR dependency DAGs.

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::rewrite::{CircuitTransaction, RewriteError};

/// Swaps the execution order of two adjacent operations in the DAG.
pub fn swap_adjacent_operations(
    circuit: &mut Circuit,
    first: OperationId,
    second: OperationId,
) -> Result<(), RewriteError> {
    let mut tx = CircuitTransaction::begin(circuit);
    tx.swap_adjacent_operations(first, second)?;
    tx.commit()?;
    Ok(())
}

/// Reorders all operations in the DAG according to the specified list of operation IDs.
pub fn reorder_operations(
    circuit: &mut Circuit,
    new_order: &[OperationId],
) -> Result<(), RewriteError> {
    let mut tx = CircuitTransaction::begin(circuit);
    tx.reorder_operations(new_order)?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn stage_swap_adjacent_operations(
    candidate: &mut Circuit,
    first: OperationId,
    second: OperationId,
) -> Result<(), RewriteError> {
    if !candidate.graph.contains_operation(first) {
        return Err(RewriteError::OperationNotFound { id: first });
    }
    if !candidate.graph.contains_operation(second) {
        return Err(RewriteError::OperationNotFound { id: second });
    }

    let mut nodes = candidate
        .graph
        .ordered_nodes()
        .map_err(RewriteError::from)?;
    let idx1 = nodes
        .iter()
        .position(|n| n.id == first)
        .ok_or(RewriteError::OperationNotFound { id: first })?;
    let idx2 = nodes
        .iter()
        .position(|n| n.id == second)
        .ok_or(RewriteError::OperationNotFound { id: second })?;

    if idx2 != idx1 + 1 && idx1 != idx2 + 1 {
        return Err(RewriteError::InvalidFusion {
            reason: format!(
                "operations {:?} (index {}) and {:?} (index {}) are not adjacent in topological order",
                first, idx1, second, idx2
            ),
        });
    }

    nodes.swap(idx1, idx2);
    candidate.rebuild_graph(nodes).map_err(RewriteError::from)?;
    Ok(())
}

pub(crate) fn stage_reorder_operations(
    candidate: &mut Circuit,
    new_order: &[OperationId],
) -> Result<(), RewriteError> {
    let current_nodes = candidate
        .graph
        .ordered_nodes()
        .map_err(RewriteError::from)?;
    if new_order.len() != current_nodes.len() {
        return Err(RewriteError::InvalidFusion {
            reason: format!(
                "reorder_operations: length mismatch (expected {}, got {})",
                current_nodes.len(),
                new_order.len()
            ),
        });
    }

    let mut node_map: std::collections::HashMap<OperationId, crate::ir::node::NodeData> =
        current_nodes.into_iter().map(|n| (n.id, n)).collect();

    let mut new_nodes = Vec::with_capacity(new_order.len());
    for &id in new_order {
        let node = node_map
            .remove(&id)
            .ok_or(RewriteError::OperationNotFound { id })?;
        new_nodes.push(node);
    }

    candidate
        .rebuild_graph(new_nodes)
        .map_err(RewriteError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::CircuitBuilder;

    #[test]
    fn test_swap_adjacent_operations_success() {
        let mut builder = CircuitBuilder::new("swap_test");
        let q = builder.add_quantum_register("q", 2).unwrap();
        let pi_3 = crate::ir::parameter::ParameterExpr::from(1.0);
        let id1 = builder.rz(q[0], pi_3).unwrap();
        let id2 = builder.cx(q[0], q[1]).unwrap();
        let mut circuit = builder.finish().unwrap();

        let orig_nodes = circuit.graph.ordered_nodes().unwrap();
        assert_eq!(orig_nodes[0].id, id1);
        assert_eq!(orig_nodes[1].id, id2);

        swap_adjacent_operations(&mut circuit, id1, id2).unwrap();

        let nodes = circuit.graph.ordered_nodes().unwrap();
        assert_eq!(nodes[0].id, id2);
        assert_eq!(nodes[1].id, id1);
        assert_eq!(nodes[0].sequence_number, 1);
        assert_eq!(nodes[1].sequence_number, 2);
    }

    #[test]
    fn test_swap_non_adjacent_rejection() {
        let mut builder = CircuitBuilder::new("non_adj");
        let q = builder.add_quantum_register("q", 3).unwrap();
        let id1 = builder.h(q[0]).unwrap();
        let _id2 = builder.x(q[1]).unwrap();
        let id3 = builder.z(q[2]).unwrap();
        let mut circuit = builder.finish().unwrap();

        let res = swap_adjacent_operations(&mut circuit, id1, id3);
        assert!(matches!(res, Err(RewriteError::InvalidFusion { .. })));
    }

    #[test]
    fn test_swap_non_existent_op_rejection() {
        let mut builder = CircuitBuilder::new("non_existent");
        let q = builder.add_quantum_register("q", 1).unwrap();
        let id1 = builder.h(q[0]).unwrap();
        let mut circuit = builder.finish().unwrap();

        let res = swap_adjacent_operations(&mut circuit, id1, OperationId(9999));
        assert!(matches!(res, Err(RewriteError::OperationNotFound { .. })));
    }

    #[test]
    fn test_reorder_operations_permutation() {
        let mut builder = CircuitBuilder::new("reorder_perm");
        let q = builder.add_quantum_register("q", 2).unwrap();
        let pi = crate::ir::parameter::ParameterExpr::from(1.0);
        let id1 = builder.rz(q[0], pi.clone()).unwrap();
        let id2 = builder.cx(q[0], q[1]).unwrap();
        let id3 = builder.rz(q[0], pi).unwrap();
        let mut circuit = builder.finish().unwrap();

        // Swap id2 and id3
        reorder_operations(&mut circuit, &[id1, id3, id2]).unwrap();

        let nodes = circuit.graph.ordered_nodes().unwrap();
        assert_eq!(nodes[0].id, id1);
        assert_eq!(nodes[1].id, id3);
        assert_eq!(nodes[2].id, id2);
    }

    #[test]
    fn test_reorder_operations_length_mismatch() {
        let mut builder = CircuitBuilder::new("len_mismatch");
        let q = builder.add_quantum_register("q", 2).unwrap();
        let id1 = builder.h(q[0]).unwrap();
        let _id2 = builder.x(q[1]).unwrap();
        let mut circuit = builder.finish().unwrap();

        let res = reorder_operations(&mut circuit, &[id1]);
        assert!(matches!(res, Err(RewriteError::InvalidFusion { .. })));
    }
}
