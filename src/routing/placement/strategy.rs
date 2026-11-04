//! Placement strategies, results, and structured error types.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::routing::layout_map::{Layout, LayoutError};
use crate::routing::placement::score::PlacementScore;
use crate::target::error::TargetError;

/// Placement strategy configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementStrategy {
    /// Direct sequential identity mapping: $q_i \to p_i$.
    Identity,
    /// Deterministic randomized mapping seeded by an explicit 64-bit integer.
    Random {
        /// PRNG seed.
        seed: u64,
    },
    /// Degree-based mapping matching logical interaction degrees to physical topology degrees.
    Degree,
    /// Interaction-weighted mapping prioritizing frequently interacting qubit pairs.
    #[default]
    Weighted,
}

/// Output result produced by a placement algorithm.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementResult {
    /// Injective initial layout mapping logical qubits to physical sites.
    pub layout: Layout,
    /// Selected placement strategy used to generate this layout.
    pub strategy: PlacementStrategy,
    /// Evaluated placement score.
    pub score: PlacementScore,
    /// Number of logical qubits mapped.
    pub logical_qubits: usize,
    /// Total physical qubit capacity of the target device.
    pub physical_qubits: usize,
}

/// Structured errors encountered during placement.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum PlacementError {
    /// Circuit requires more qubits than the target device provides.
    #[error("insufficient physical qubit capacity: circuit requires {required} qubits, but target only has {available}")]
    InsufficientCapacity {
        /// Number of logical qubits required by the circuit.
        required: usize,
        /// Number of physical qubits available on the target.
        available: usize,
    },

    /// Target declares zero physical qubits.
    #[error("target declares zero physical qubits")]
    ZeroPhysicalQubits,

    /// Error constructing or validating the underlying [`Layout`].
    #[error("layout error during placement: {0}")]
    Layout(#[from] LayoutError),

    /// Error querying or validating the target topology.
    #[error("target error during placement: {0}")]
    Target(#[from] TargetError),
}
