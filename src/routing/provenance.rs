//! Operation provenance tracking and identity preservation for routing rewrites.
//!
//! Preserves the identity of original logical operations through physical routing
//! transformations and explicitly attributes inserted physical SWAP and direction-correction
//! operations to their causal logical interactions.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;

/// Categorization of an operation appearing in a routed physical circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouteOperationKind {
    /// Direct physical execution of an original logical operation.
    Logical {
        /// Stable semantic identifier of the original operation in the logical circuit.
        original_id: OperationId,
    },
    /// Physical SWAP operation inserted by the router to move operands into adjacency.
    RoutingSwap {
        /// The logical operation whose routing necessitated this SWAP (if applicable).
        caused_by: Option<OperationId>,
        /// 0-indexed hop sequence along the routing path.
        step: usize,
    },
    /// Single-qubit operation inserted for direction correction (e.g. Hadamard reversal).
    DirectionCorrection {
        /// The logical operation whose direction constraints necessitated this gate.
        caused_by: OperationId,
    },
}

impl fmt::Display for RouteOperationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Logical { original_id } => write!(f, "logical({original_id})"),
            Self::RoutingSwap { caused_by, step } => match caused_by {
                Some(cause) => write!(f, "routing_swap(step={step}, cause={cause})"),
                None => write!(f, "routing_swap(step={step})"),
            },
            Self::DirectionCorrection { caused_by } => {
                write!(f, "direction_correction(cause={caused_by})")
            }
        }
    }
}

/// A provenance record linking a physical circuit node to its logical origin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteProvenanceRecord {
    /// Stable semantic identifier of the node in the physical circuit.
    pub physical_id: OperationId,
    /// Detailed classification of the physical operation.
    pub kind: RouteOperationKind,
    /// Human-readable description or rule name.
    pub description: String,
}

impl RouteProvenanceRecord {
    /// Returns the original logical operation ID if this is a mapped logical operation.
    #[inline]
    pub fn original_id(&self) -> Option<OperationId> {
        match self.kind {
            RouteOperationKind::Logical { original_id } => Some(original_id),
            _ => None,
        }
    }

    /// Returns the logical operation that caused this routing operation, if any.
    #[inline]
    pub fn caused_by(&self) -> Option<OperationId> {
        match self.kind {
            RouteOperationKind::Logical { original_id } => Some(original_id),
            RouteOperationKind::RoutingSwap { caused_by, .. } => caused_by,
            RouteOperationKind::DirectionCorrection { caused_by } => Some(caused_by),
        }
    }

    /// Returns `true` if this record represents an inserted routing SWAP.
    #[inline]
    pub fn is_swap(&self) -> bool {
        matches!(self.kind, RouteOperationKind::RoutingSwap { .. })
    }
}

/// Bidirectional provenance database tracking physical circuit operations back to logical operations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteProvenanceTracker {
    /// Map from physical `OperationId` to its provenance record.
    records: BTreeMap<OperationId, RouteProvenanceRecord>,
    /// Forward map: logical `OperationId` -> all physical `OperationId`s created for it.
    logical_to_physical: BTreeMap<OperationId, Vec<OperationId>>,
}

impl RouteProvenanceTracker {
    /// Creates a new, empty provenance tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a physical operation as the direct implementation of an original logical operation.
    pub fn record_logical_operation(
        &mut self,
        physical_id: OperationId,
        original_id: OperationId,
        description: impl Into<String>,
    ) {
        let record = RouteProvenanceRecord {
            physical_id,
            kind: RouteOperationKind::Logical { original_id },
            description: description.into(),
        };
        self.records.insert(physical_id, record);
        self.logical_to_physical
            .entry(original_id)
            .or_default()
            .push(physical_id);
    }

    /// Records a physical SWAP operation inserted along a routing path.
    pub fn record_routing_swap(
        &mut self,
        physical_id: OperationId,
        caused_by: Option<OperationId>,
        step: usize,
        description: impl Into<String>,
    ) {
        let record = RouteProvenanceRecord {
            physical_id,
            kind: RouteOperationKind::RoutingSwap { caused_by, step },
            description: description.into(),
        };
        self.records.insert(physical_id, record);
        if let Some(cause) = caused_by {
            self.logical_to_physical
                .entry(cause)
                .or_default()
                .push(physical_id);
        }
    }

    /// Records a physical gate inserted for direction correction.
    pub fn record_direction_correction(
        &mut self,
        physical_id: OperationId,
        caused_by: OperationId,
        description: impl Into<String>,
    ) {
        let record = RouteProvenanceRecord {
            physical_id,
            kind: RouteOperationKind::DirectionCorrection { caused_by },
            description: description.into(),
        };
        self.records.insert(physical_id, record);
        self.logical_to_physical
            .entry(caused_by)
            .or_default()
            .push(physical_id);
    }

    /// Returns the provenance record for a given physical operation, if registered.
    pub fn get(&self, physical_id: OperationId) -> Option<&RouteProvenanceRecord> {
        self.records.get(&physical_id)
    }

    /// Returns the kind of the given physical operation, if registered.
    pub fn get_kind(&self, physical_id: OperationId) -> Option<&RouteOperationKind> {
        self.records.get(&physical_id).map(|r| &r.kind)
    }

    /// Returns all physical operation IDs associated with a given logical operation.
    pub fn physical_operations_for(&self, logical_id: OperationId) -> &[OperationId] {
        self.logical_to_physical
            .get(&logical_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns all physical SWAP operation IDs inserted on behalf of a logical operation.
    pub fn swaps_for(&self, logical_id: OperationId) -> Vec<OperationId> {
        self.physical_operations_for(logical_id)
            .iter()
            .copied()
            .filter(|&pid| self.get(pid).map(|r| r.is_swap()).unwrap_or(false))
            .collect()
    }

    /// Returns all physical SWAP operation IDs across the entire circuit.
    pub fn all_swaps(&self) -> Vec<OperationId> {
        self.records
            .values()
            .filter(|r| r.is_swap())
            .map(|r| r.physical_id)
            .collect()
    }

    /// Returns total count of registered physical operations.
    #[inline]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` if no records are registered.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns an immutable map of all records.
    #[inline]
    pub fn records(&self) -> &BTreeMap<OperationId, RouteProvenanceRecord> {
        &self.records
    }

    /// Validates that every node in `circuit` has a registered provenance record.
    pub fn validate_coverage(&self, circuit: &Circuit) -> Result<(), String> {
        let ordered_nodes = circuit
            .graph
            .ordered_nodes()
            .map_err(|e| format!("cannot order nodes: {e}"))?;

        for node in ordered_nodes {
            if !self.records.contains_key(&node.id) {
                return Err(format!(
                    "physical node {} ('{}') has no registered routing provenance",
                    node.id,
                    node.operation.name()
                ));
            }
        }
        Ok(())
    }
}
