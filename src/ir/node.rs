//! Node structures and stable semantic operation identifiers.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::operation::Operation;

/// Stable semantic identifier for operations, strictly decoupled from petgraph indices.
///
/// Guaranteed to remain unchanged across graph traversals and mutations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OperationId(pub u64);

impl fmt::Display for OperationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "op#{}", self.0)
    }
}

/// Payload data stored inside each QccIR graph node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeData {
    /// QCC-owned stable operation identifier.
    pub id: OperationId,
    /// Strictly increasing sequence number for deterministic sorting.
    pub sequence_number: u64,
    /// Concrete quantum or classical operation.
    pub operation: Operation,
    /// Provenance tracking: original operation IDs that were replaced, expanded, or fused into this node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<OperationId>,
}

impl NodeData {
    /// Creates a new `NodeData` without provenance.
    pub fn new(id: OperationId, sequence_number: u64, operation: Operation) -> Self {
        Self {
            id,
            sequence_number,
            operation,
            provenance: Vec::new(),
        }
    }

    /// Creates a new `NodeData` with explicit provenance tracking.
    pub fn with_provenance(
        id: OperationId,
        sequence_number: u64,
        operation: Operation,
        provenance: Vec<OperationId>,
    ) -> Self {
        Self {
            id,
            sequence_number,
            operation,
            provenance,
        }
    }
}
