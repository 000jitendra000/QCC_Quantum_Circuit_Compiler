//! Deterministic shortest-path finding on physical hardware coupling graphs.
//!
//! Provides breadth-first search (BFS) on the undirected projection of a target's
//! [`CouplingGraph`], producing minimal-hop routing paths between physical qubits.

use std::collections::{BTreeSet, HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ir::error::{IrError, IrValidationError};
use crate::routing::layout_map::LayoutError;
use crate::routing::placement::PlacementError;
use crate::target::connectivity::CouplingGraph;
use crate::target::operation::PhysicalQubitId;

/// Structured errors encountered during circuit routing, pathfinding, and layout mutation.
#[derive(Clone, Debug, PartialEq, Error, Serialize, Deserialize)]
pub enum RoutingError {
    /// Two physical qubits belong to disconnected components of the coupling graph.
    #[error(
        "physical qubits {src} and {dst} are in disconnected topology components (no path exists)"
    )]
    DisconnectedTopology {
        /// Source physical qubit.
        src: PhysicalQubitId,
        /// Destination physical qubit.
        dst: PhysicalQubitId,
    },

    /// Physical qubit operand index exceeds the physical qubit capacity of the target device.
    #[error("physical qubit {qubit} is out of bounds for target with capacity {capacity}")]
    QubitOutOfBounds {
        /// Out-of-bounds physical qubit.
        qubit: PhysicalQubitId,
        /// Target device physical capacity.
        capacity: usize,
    },

    /// Target device physical capacity is insufficient for the logical circuit.
    #[error("insufficient target physical qubit capacity: circuit requires {required}, target provides {capacity}")]
    InsufficientCapacity {
        /// Logical qubits required.
        required: usize,
        /// Physical qubits available.
        capacity: usize,
    },

    /// An operation specifies duplicate operands on the same physical qubit.
    #[error("duplicate operand qubit {qubit} specified for operation")]
    DuplicateOperand {
        /// Duplicate physical qubit.
        qubit: PhysicalQubitId,
    },

    /// An operation cannot be satisfied on target because no valid orientation or coupling exists.
    #[error("operation '{operation}' on physical qubits ({u}, {v}) has no viable direction on target '{target}'")]
    NoViableDirection {
        /// Operation mnemonic.
        operation: String,
        /// First physical operand.
        u: PhysicalQubitId,
        /// Second physical operand.
        v: PhysicalQubitId,
        /// Target device name.
        target: String,
    },

    /// Routing exceeded the configured maximum iteration or SWAP limit.
    #[error("routing exceeded limit of {limit} iterations: {details}")]
    IterationLimitExceeded {
        /// Limit that was exceeded.
        limit: usize,
        /// Context details.
        details: String,
    },

    /// An operation is unsupported by the target device.
    #[error("target does not support operation '{operation}'")]
    UnsupportedOperation {
        /// Unsupported operation mnemonic.
        operation: String,
    },

    /// Target legality violation during routing.
    #[error("target legality violation during routing: {0}")]
    TargetLegality(String),

    /// Layout mutation or validation failure.
    #[error("layout error during routing: {0}")]
    Layout(String),

    /// Intermediate representation DAG or wire dependency failure.
    #[error("circuit IR error during routing: {0}")]
    Ir(String),
}

impl From<LayoutError> for RoutingError {
    fn from(err: LayoutError) -> Self {
        Self::Layout(err.to_string())
    }
}

impl From<PlacementError> for RoutingError {
    fn from(err: PlacementError) -> Self {
        Self::Layout(err.to_string())
    }
}

impl From<IrError> for RoutingError {
    fn from(err: IrError) -> Self {
        Self::Ir(err.to_string())
    }
}

impl From<IrValidationError> for RoutingError {
    fn from(err: IrValidationError) -> Self {
        Self::Ir(err.to_string())
    }
}

/// Checks if two physical qubits are directly connected by an edge in either direction.
pub fn are_physical_neighbors(
    coupling: &CouplingGraph,
    u: PhysicalQubitId,
    v: PhysicalQubitId,
) -> bool {
    coupling.has_edge(u, v) || coupling.has_edge(v, u)
}

/// Returns the hop count of a physical path.
#[inline]
pub fn path_length(path: &[PhysicalQubitId]) -> usize {
    path.len().saturating_sub(1)
}

/// Finds the shortest physical path between `src` and `dst` on the target's coupling graph.
///
/// Uses breadth-first search (BFS) on the undirected projection of the coupling graph,
/// exploring neighbors in strictly deterministic sorted order.
///
/// Returns:
/// - `Ok(path)`: A sequence of physical qubits `[v_0, v_1, ..., v_k]` where `v_0 == src`,
///   `v_k == dst`, and each `(v_i, v_{i+1})` is adjacent in `coupling`.
/// - `Err(RoutingError::DisconnectedTopology)`: If `src` and `dst` belong to disconnected components.
/// - `Err(RoutingError::QubitOutOfBounds)`: If `src` or `dst` is out of bounds.
pub fn find_shortest_path(
    coupling: &CouplingGraph,
    src: PhysicalQubitId,
    dst: PhysicalQubitId,
) -> Result<Vec<PhysicalQubitId>, RoutingError> {
    let n = coupling.num_qubits;
    if src.index() >= n {
        return Err(RoutingError::QubitOutOfBounds {
            qubit: src,
            capacity: n,
        });
    }
    if dst.index() >= n {
        return Err(RoutingError::QubitOutOfBounds {
            qubit: dst,
            capacity: n,
        });
    }

    if src == dst {
        return Ok(vec![src]);
    }

    // Build deterministic undirected adjacency list: sorted neighbors per physical qubit
    let mut adj: Vec<BTreeSet<PhysicalQubitId>> = vec![BTreeSet::new(); n];
    for &(u, v) in &coupling.edges {
        if u.index() < n && v.index() < n && u != v {
            adj[u.index()].insert(v);
            adj[v.index()].insert(u);
        }
    }

    // BFS tracking visited and predecessor pointers
    let mut visited = vec![false; n];
    let mut came_from: HashMap<PhysicalQubitId, PhysicalQubitId> = HashMap::new();
    let mut queue = VecDeque::new();

    visited[src.index()] = true;
    queue.push_back(src);

    let mut found = false;
    while let Some(curr) = queue.pop_front() {
        if curr == dst {
            found = true;
            break;
        }

        for &next in &adj[curr.index()] {
            if !visited[next.index()] {
                visited[next.index()] = true;
                came_from.insert(next, curr);
                queue.push_back(next);
            }
        }
    }

    if !found {
        return Err(RoutingError::DisconnectedTopology { src, dst });
    }

    // Reconstruct path from dst back to src
    let mut path = Vec::new();
    let mut curr = dst;
    path.push(curr);
    while curr != src {
        let prev = *came_from
            .get(&curr)
            .expect("predecessor must exist along BFS path");
        path.push(prev);
        curr = prev;
    }
    path.reverse();

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::connectivity::Directionality;

    #[test]
    fn test_shortest_path_same_qubit() {
        let coupling = CouplingGraph::linear_chain(4, Directionality::Undirected);
        let path = find_shortest_path(&coupling, PhysicalQubitId(2), PhysicalQubitId(2)).unwrap();
        assert_eq!(path, vec![PhysicalQubitId(2)]);
        assert_eq!(path_length(&path), 0);
    }

    #[test]
    fn test_shortest_path_adjacent() {
        let coupling = CouplingGraph::linear_chain(4, Directionality::Undirected);
        let path = find_shortest_path(&coupling, PhysicalQubitId(1), PhysicalQubitId(2)).unwrap();
        assert_eq!(path, vec![PhysicalQubitId(1), PhysicalQubitId(2)]);
        assert_eq!(path_length(&path), 1);
    }

    #[test]
    fn test_shortest_path_linear_chain_multi_hop() {
        let coupling = CouplingGraph::linear_chain(5, Directionality::Directed);
        let path = find_shortest_path(&coupling, PhysicalQubitId(0), PhysicalQubitId(4)).unwrap();
        assert_eq!(
            path,
            vec![
                PhysicalQubitId(0),
                PhysicalQubitId(1),
                PhysicalQubitId(2),
                PhysicalQubitId(3),
                PhysicalQubitId(4),
            ]
        );
        assert_eq!(path_length(&path), 4);
    }

    #[test]
    fn test_shortest_path_ring_wraparound() {
        let coupling = CouplingGraph::ring(6, Directionality::Undirected);
        // On a 6-qubit ring: 0 - 1 - 2 - 3 - 4 - 5 - 0
        // Path between 0 and 5 should be direct: [0, 5], not [0, 1, 2, 3, 4, 5]
        let path = find_shortest_path(&coupling, PhysicalQubitId(0), PhysicalQubitId(5)).unwrap();
        assert_eq!(path, vec![PhysicalQubitId(0), PhysicalQubitId(5)]);
        assert_eq!(path_length(&path), 1);
    }

    #[test]
    fn test_shortest_path_disconnected_topology() {
        let mut coupling = CouplingGraph::new(4, Directionality::Undirected);
        // Component 1: 0 - 1
        coupling.add_edge(0u32, 1u32).unwrap();
        // Component 2: 2 - 3
        coupling.add_edge(2u32, 3u32).unwrap();

        let err =
            find_shortest_path(&coupling, PhysicalQubitId(0), PhysicalQubitId(3)).unwrap_err();
        match err {
            RoutingError::DisconnectedTopology { src, dst } => {
                assert_eq!(src, PhysicalQubitId(0));
                assert_eq!(dst, PhysicalQubitId(3));
            }
            other => panic!("expected DisconnectedTopology, got {other:?}"),
        }
    }

    #[test]
    fn test_shortest_path_out_of_bounds() {
        let coupling = CouplingGraph::linear_chain(4, Directionality::Undirected);
        let err =
            find_shortest_path(&coupling, PhysicalQubitId(0), PhysicalQubitId(10)).unwrap_err();
        assert!(matches!(err, RoutingError::QubitOutOfBounds { .. }));
    }
}
