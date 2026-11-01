//! Measurement probability distributions and qubit selection specifications.
//!
//! Under Little-Endian indexing:
//! - Computational basis states $|q_{n-1} \dots q_0\rangle$ have integer index $i = \sum q_k 2^k$.
//! - Outcome probability according to the Born rule is $P(i) = |\alpha_i|^2$.
//! - Selected-qubit measurements project the full $2^n$-state distribution onto marginal subsets.

use std::collections::HashSet;

use crate::simulator::error::SimulatorError;
use crate::simulator::statevector::StateVector;

/// Default tolerance for probability normalization validation ($\pm 10^{-5}$).
pub const PROBABILITY_NORM_TOLERANCE: f64 = 1e-5;

/// Validated specification of qubits selected for terminal measurement.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MeasurementSelection {
    qubits: Vec<usize>,
}

impl MeasurementSelection {
    /// Creates and validates a measurement selection for an $n$-qubit system.
    ///
    /// # Errors
    /// - [`SimulatorError::EmptyMeasurementSelection`] if `qubits` is empty.
    /// - [`SimulatorError::TargetOutOfBounds`] if any qubit index $\ge \text{num\_qubits}$.
    /// - [`SimulatorError::DuplicateMeasurementQubit`] if any qubit is specified more than once.
    pub fn new(num_qubits: usize, qubits: Vec<usize>) -> Result<Self, SimulatorError> {
        if qubits.is_empty() {
            return Err(SimulatorError::EmptyMeasurementSelection);
        }

        let mut seen = HashSet::with_capacity(qubits.len());
        for &q in &qubits {
            if q >= num_qubits {
                return Err(SimulatorError::TargetOutOfBounds {
                    target: q,
                    num_qubits,
                });
            }
            if !seen.insert(q) {
                return Err(SimulatorError::DuplicateMeasurementQubit { qubit: q });
            }
        }

        Ok(Self { qubits })
    }

    /// Creates a measurement selection that measures all $n$ qubits in standard order `[0, 1, ..., n-1]`.
    pub fn all(num_qubits: usize) -> Result<Self, SimulatorError> {
        if num_qubits == 0 {
            return Err(SimulatorError::EmptyMeasurementSelection);
        }
        let qubits = (0..num_qubits).collect();
        Ok(Self { qubits })
    }

    /// Returns the ordered slice of measured qubit indices.
    #[inline]
    pub fn qubits(&self) -> &[usize] {
        &self.qubits
    }

    /// Number of measured qubits.
    #[inline]
    pub fn len(&self) -> usize {
        self.qubits.len()
    }

    /// Returns `true` if no qubits are selected (never true for valid selections).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.qubits.is_empty()
    }

    /// Extracts the binary measurement outcome bits from a full-register basis state index.
    ///
    /// For each selected qubit $q_k$, bit $k$ is $((i \gg q_k) \& 1)$.
    #[inline]
    pub fn extract_bits(&self, full_basis_index: usize) -> Vec<u8> {
        self.qubits
            .iter()
            .map(|&q| ((full_basis_index >> q) & 1) as u8)
            .collect()
    }
}

/// A validated probability distribution over computational basis states.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbabilityDistribution {
    num_qubits: usize,
    probabilities: Vec<f64>,
}

impl ProbabilityDistribution {
    /// Computes the computational basis probability distribution from an evolved quantum statevector.
    ///
    /// Evaluates $P(i) = |\alpha_i|^2$ for all $2^n$ amplitudes according to the Born rule.
    pub fn from_statevector(state: &StateVector) -> Result<Self, SimulatorError> {
        let num_qubits = state.num_qubits();
        let amplitudes = state.amplitudes();
        let mut probs = Vec::with_capacity(amplitudes.len());

        let mut sum = 0.0;
        for (idx, amp) in amplitudes.iter().enumerate() {
            let p = amp.norm_sqr();
            if p.is_nan() || p.is_infinite() {
                return Err(SimulatorError::InvalidAmplitude {
                    index: idx,
                    reason: "non-finite probability computed from amplitude",
                });
            }
            probs.push(p);
            sum += p;
        }

        if (sum - 1.0).abs() > PROBABILITY_NORM_TOLERANCE {
            return Err(SimulatorError::InvalidProbabilityDistribution {
                reason: "sum of probabilities does not equal 1.0 within tolerance",
            });
        }

        Ok(Self {
            num_qubits,
            probabilities: probs,
        })
    }

    /// Creates a probability distribution from raw probabilities with strict normalization validation.
    ///
    /// # Errors
    /// Returns [`SimulatorError::InvalidProbabilityDistribution`] if:
    /// - Vector length does not equal $2^{\text{num\_qubits}}$
    /// - Any probability is negative, NaN, or infinite
    /// - Sum of probabilities deviates from 1.0 by more than [`PROBABILITY_NORM_TOLERANCE`].
    pub fn from_probabilities(
        num_qubits: usize,
        probabilities: Vec<f64>,
    ) -> Result<Self, SimulatorError> {
        if num_qubits == 0 {
            return Err(SimulatorError::InvalidQubitCount {
                num_qubits,
                max_allowed: usize::BITS as usize - 1,
            });
        }

        let expected_dim =
            1usize
                .checked_shl(num_qubits as u32)
                .ok_or(SimulatorError::InvalidQubitCount {
                    num_qubits,
                    max_allowed: usize::BITS as usize - 1,
                })?;

        if probabilities.len() != expected_dim {
            return Err(SimulatorError::DimensionMismatch {
                num_qubits,
                expected: expected_dim,
                actual: probabilities.len(),
            });
        }

        let mut sum = 0.0;
        for &p in &probabilities {
            if p.is_nan() || p.is_infinite() || p < 0.0 {
                return Err(SimulatorError::InvalidProbabilityDistribution {
                    reason: "probability value must be finite and non-negative",
                });
            }
            sum += p;
        }

        if (sum - 1.0).abs() > PROBABILITY_NORM_TOLERANCE {
            return Err(SimulatorError::InvalidProbabilityDistribution {
                reason: "sum of probabilities does not equal 1.0 within tolerance",
            });
        }

        Ok(Self {
            num_qubits,
            probabilities,
        })
    }

    /// Number of qubits in the distribution.
    #[inline]
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Number of basis outcomes ($2^n$).
    #[inline]
    pub fn len(&self) -> usize {
        self.probabilities.len()
    }

    /// Returns `true` if the distribution has length 0 (never true for valid distributions).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.probabilities.is_empty()
    }

    /// Returns an immutable slice of all basis state probabilities.
    #[inline]
    pub fn probabilities(&self) -> &[f64] {
        &self.probabilities
    }

    /// Returns the probability of basis state `index`, or `None` if out of bounds.
    #[inline]
    pub fn get(&self, index: usize) -> Option<f64> {
        self.probabilities.get(index).copied()
    }

    /// Computes the marginal probability distribution over a subset of measured qubits.
    pub fn marginal(&self, selection: &MeasurementSelection) -> Result<Self, SimulatorError> {
        let m = selection.len();
        let marginal_dim = 1usize << m;
        let mut marginal_probs = vec![0.0; marginal_dim];

        for (full_idx, &p) in self.probabilities.iter().enumerate() {
            let mut marginal_idx = 0usize;
            for (bit_pos, &q) in selection.qubits().iter().enumerate() {
                if ((full_idx >> q) & 1) == 1 {
                    marginal_idx |= 1usize << bit_pos;
                }
            }
            marginal_probs[marginal_idx] += p;
        }

        Self::from_probabilities(m, marginal_probs)
    }
}
