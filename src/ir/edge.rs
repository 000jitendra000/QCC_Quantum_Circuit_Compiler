//! Dependency edges and classifications in QccIR.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ir::wire::WireId;

/// Classifies the causal relationship enforced by a dependency edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DependencyKind {
    /// Sequential quantum state evolution on a qubit wire.
    QuantumOrder,
    /// Read-After-Write (RAW): classical operation reads from preceding writer.
    ClassicalRaw,
    /// Write-After-Read (WAR): classical write must follow all prior outstanding readers.
    ClassicalWar,
    /// Write-After-Write (WAW): sequential write order on a classical wire.
    ClassicalWaw,
    /// Synchronization barrier boundary across designated wires.
    BarrierBoundary,
}

impl fmt::Display for DependencyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QuantumOrder => write!(f, "quantum-order"),
            Self::ClassicalRaw => write!(f, "raw"),
            Self::ClassicalWar => write!(f, "war"),
            Self::ClassicalWaw => write!(f, "waw"),
            Self::BarrierBoundary => write!(f, "barrier"),
        }
    }
}

/// A directed dependency edge in QccIR carrying wire and hazard metadata.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WireDependency {
    /// Wire whose ordering is enforced by this edge.
    pub wire: WireId,
    /// Specific hazard or ordering relationship.
    pub kind: DependencyKind,
}

impl WireDependency {
    /// Creates a new `WireDependency`.
    pub fn new(wire: WireId, kind: DependencyKind) -> Self {
        Self { wire, kind }
    }
}

impl fmt::Display for WireDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.wire, self.kind)
    }
}
