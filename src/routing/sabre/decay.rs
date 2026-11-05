//! Decay factor tracking for physical qubits in SABRE routing.
//!
//! Tracks repeated usage of physical qubits across SWAP operations to discourage
//! oscillation (repeatedly swapping between the same two physical qubits without
//! making forward progress through the circuit).
//!
//! # Mathematical Formulation (ASPLOS 2019, Li et al.)
//! - Initial decay factor for each physical qubit $p$:
//!   $$\text{decay}(p) = 1.0$$
//! - When a physical SWAP $(u, v)$ is selected, the decay factor of both involved
//!   physical qubits is incremented by $\delta$:
//!   $$\text{decay}(u) \leftarrow \text{decay}(u) + \delta$$
//!   $$\text{decay}(v) \leftarrow \text{decay}(v) + \delta$$
//! - For a candidate SWAP $(u, v)$, the decay multiplier applied to the distance cost is:
//!   $$\text{decay\_factor}(u, v) = \max(\text{decay}(u), \text{decay}(v))$$
//! - When any gate in the front layer is satisfied and executed, decay factors
//!   are reset back to $1.0$ across the device.

use serde::{Deserialize, Serialize};

use crate::routing::pathfinding::RoutingError;
use crate::routing::sabre::candidate::CandidateSwap;
use crate::target::operation::PhysicalQubitId;

/// Default decay increment $\delta$ applied to physical qubits per SWAP.
pub const DEFAULT_DECAY_DELTA: f64 = 0.001;

/// Default baseline decay factor.
pub const BASE_DECAY: f64 = 1.0;

/// Table of decay factors for physical qubits on the target device.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecayTable {
    num_qubits: usize,
    decay_factors: Vec<f64>,
    delta: f64,
}

impl DecayTable {
    /// Constructs a new decay table for `num_qubits` physical qubits using the default delta.
    pub fn new(num_qubits: usize) -> Self {
        Self {
            num_qubits,
            decay_factors: vec![BASE_DECAY; num_qubits],
            delta: DEFAULT_DECAY_DELTA,
        }
    }

    /// Constructs a new decay table with a custom increment $\delta$.
    ///
    /// # Errors
    /// Returns [`RoutingError::InvalidConfig`] if `delta < 0.0` or is non-finite.
    pub fn with_delta(num_qubits: usize, delta: f64) -> Result<Self, RoutingError> {
        if !delta.is_finite() || delta < 0.0 {
            return Err(RoutingError::InvalidConfig(format!(
                "decay delta must be finite and non-negative, got {delta}"
            )));
        }
        Ok(Self {
            num_qubits,
            decay_factors: vec![BASE_DECAY; num_qubits],
            delta,
        })
    }

    /// Returns the decay factor for physical qubit `qubit`.
    ///
    /// If `qubit` is out of bounds, returns the baseline decay factor ($1.0$).
    #[inline]
    pub fn get(&self, qubit: PhysicalQubitId) -> f64 {
        let idx = qubit.index();
        if idx < self.num_qubits {
            self.decay_factors[idx]
        } else {
            BASE_DECAY
        }
    }

    /// Returns the effective decay factor multiplier for candidate SWAP $(u, v)$:
    ///
    /// $$\text{decay\_factor}(u, v) = \max(\text{decay}(u), \text{decay}(v))$$
    #[inline]
    pub fn decay_factor(&self, candidate: CandidateSwap) -> f64 {
        let d_u = self.get(candidate.u);
        let d_v = self.get(candidate.v);
        d_u.max(d_v)
    }

    /// Records the execution of candidate SWAP $(u, v)$, incrementing the decay factor
    /// of both involved physical qubits by $\delta$.
    ///
    /// # Transactional Guarantee
    /// Validates that both physical qubits are within bounds before modifying any state.
    /// If either qubit index is out of bounds, returns [`RoutingError::QubitOutOfBounds`]
    /// without modifying either qubit's decay factor.
    ///
    /// # Errors
    /// Returns [`RoutingError::QubitOutOfBounds`] if `u` or `v` exceeds device physical capacity.
    pub fn record_swap(&mut self, candidate: CandidateSwap) -> Result<(), RoutingError> {
        let u_idx = candidate.u.index();
        let v_idx = candidate.v.index();
        if u_idx >= self.num_qubits {
            return Err(RoutingError::QubitOutOfBounds {
                qubit: candidate.u,
                capacity: self.num_qubits,
            });
        }
        if v_idx >= self.num_qubits {
            return Err(RoutingError::QubitOutOfBounds {
                qubit: candidate.v,
                capacity: self.num_qubits,
            });
        }
        self.decay_factors[u_idx] += self.delta;
        self.decay_factors[v_idx] += self.delta;
        Ok(())
    }

    /// Resets all physical qubit decay factors back to baseline ($1.0$).
    ///
    /// Called when the front layer advances, indicating forward routing progress.
    pub fn reset(&mut self) {
        self.decay_factors.fill(BASE_DECAY);
    }

    /// Resets the decay factor for a specific physical qubit back to $1.0$.
    ///
    /// # Errors
    /// Returns [`RoutingError::QubitOutOfBounds`] if `qubit` exceeds device capacity.
    pub fn reset_qubit(&mut self, qubit: PhysicalQubitId) -> Result<(), RoutingError> {
        let idx = qubit.index();
        if idx >= self.num_qubits {
            return Err(RoutingError::QubitOutOfBounds {
                qubit,
                capacity: self.num_qubits,
            });
        }
        self.decay_factors[idx] = BASE_DECAY;
        Ok(())
    }

    /// Returns the number of physical qubits tracked in this table.
    #[inline]
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Returns the decay increment $\delta$.
    #[inline]
    pub fn delta(&self) -> f64 {
        self.delta
    }

    /// Returns a slice of all decay factors.
    #[inline]
    pub fn factors(&self) -> &[f64] {
        &self.decay_factors
    }
}
