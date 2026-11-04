//! Trivial placement strategies: Identity and Random-seeded.
//!
//! Provides baseline placement strategies for capacity checking, comparative benchmarking,
//! and deterministic randomized initial layout exploration.

use crate::ir::wire::QubitId;
use crate::routing::layout_map::Layout;
use crate::routing::placement::distance::TopologyDistances;
use crate::routing::placement::interaction_graph::InteractionGraph;
use crate::routing::placement::score::PlacementScore;
use crate::routing::placement::strategy::{PlacementError, PlacementResult, PlacementStrategy};
use crate::simulator::sampling::{SeededRng, SplitMix64};
use crate::target::operation::PhysicalQubitId;

/// Straightforward identity placement: maps logical qubit $q_i \to p_i$.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IdentityPlacer;

impl IdentityPlacer {
    /// Creates a new `IdentityPlacer`.
    pub fn new() -> Self {
        Self
    }

    /// Computes the identity placement mapping logical qubits to identical physical qubit indices.
    pub fn place(
        &self,
        interactions: &InteractionGraph,
        distances: &TopologyDistances,
    ) -> Result<PlacementResult, PlacementError> {
        let m = interactions.num_qubits();
        let n = distances.num_qubits();

        if m > n {
            return Err(PlacementError::InsufficientCapacity {
                required: m,
                available: n,
            });
        }

        let mut mappings = Vec::with_capacity(m);
        for i in 0..m {
            mappings.push((QubitId(i), PhysicalQubitId::new(i as u32)));
        }

        let layout = Layout::from_mappings(m, n, mappings)?;
        let score = PlacementScore::evaluate(&layout, interactions, distances);

        Ok(PlacementResult {
            layout,
            strategy: PlacementStrategy::Identity,
            score,
            logical_qubits: m,
            physical_qubits: n,
        })
    }
}

/// Deterministic randomized initial placement parameterized by an explicit 64-bit seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RandomPlacer {
    seed: u64,
}

impl RandomPlacer {
    /// Creates a new `RandomPlacer` with the specified PRNG seed.
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Returns the configured seed.
    #[inline]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Computes a reproducible randomized placement using the explicit seed.
    pub fn place(
        &self,
        interactions: &InteractionGraph,
        distances: &TopologyDistances,
    ) -> Result<PlacementResult, PlacementError> {
        let m = interactions.num_qubits();
        let n = distances.num_qubits();

        if m > n {
            return Err(PlacementError::InsufficientCapacity {
                required: m,
                available: n,
            });
        }

        // Deterministic Fisher-Yates shuffle of available physical qubits
        let mut physical_pool: Vec<PhysicalQubitId> =
            (0..n as u32).map(PhysicalQubitId::new).collect();
        let mut rng = SplitMix64::new(self.seed);

        for i in (1..n).rev() {
            let j = (rng.next_u64() % ((i + 1) as u64)) as usize;
            physical_pool.swap(i, j);
        }

        let mut mappings = Vec::with_capacity(m);
        for (i, &p) in physical_pool.iter().enumerate().take(m) {
            mappings.push((QubitId(i), p));
        }

        let layout = Layout::from_mappings(m, n, mappings)?;
        let score = PlacementScore::evaluate(&layout, interactions, distances);

        Ok(PlacementResult {
            layout,
            strategy: PlacementStrategy::Random { seed: self.seed },
            score,
            logical_qubits: m,
            physical_qubits: n,
        })
    }
}
