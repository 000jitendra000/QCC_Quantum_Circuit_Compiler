//! Seeded random sampling and cumulative distribution sampling engine.
//!
//! # Algorithmic Architecture
//! 1. **Single State Evolution**: The quantum circuit evolves **exactly once** to produce
//!    the terminal statevector.
//! 2. **Cumulative Distribution (CDF)**: Born-rule probabilities $P(i) = |\alpha_i|^2$
//!    are computed once and accumulated into a monotonically increasing CDF:
//!    $$C_k = \sum_{j=0}^k P(j), \quad C_{2^n - 1} = 1.0$$
//! 3. **Binary Search Sampling**: Each shot draws a pseudorandom float $u \in [0.0, 1.0)$
//!    and locates the basis outcome via binary search (`partition_point`) in $\mathcal{O}(n)$ time.
//! 4. **No Circuit Re-execution**: The shot sampling loop performs solely CDF lookups and bit
//!    extraction. Quantum gates are never re-simulated per shot.

use crate::simulator::error::SimulatorError;
use crate::simulator::measurement::{MeasurementSelection, ProbabilityDistribution};
use crate::simulator::result::{ExecutionResult, ShotResult};
use crate::simulator::statevector::StateVector;

/// Trait for deterministic pseudo-random number generators used in quantum sampling.
pub trait SeededRng {
    /// Generates the next pseudorandom 64-bit unsigned integer.
    fn next_u64(&mut self) -> u64;

    /// Generates the next pseudorandom float in the half-open interval $[0.0, 1.0)$.
    fn next_f64(&mut self) -> f64;
}

impl<R: SeededRng + ?Sized> SeededRng for &mut R {
    #[inline]
    fn next_u64(&mut self) -> u64 {
        (**self).next_u64()
    }

    #[inline]
    fn next_f64(&mut self) -> f64 {
        (**self).next_f64()
    }
}

/// Canonical deterministic pseudorandom number generator using the SplitMix64 algorithm.
///
/// Fully portable, dependency-free, and guaranteed to produce identical sequences across
/// all architectures and compiler versions for a given seed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Creates a new generator initialized with the given 64-bit seed.
    #[inline]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}

impl SeededRng for SplitMix64 {
    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    #[inline]
    fn next_f64(&mut self) -> f64 {
        // Extract 53 random bits to populate the IEEE 754 double-precision mantissa
        let val = self.next_u64() >> 11;
        (val as f64) * (1.0 / (1u64 << 53) as f64)
    }
}

/// Cumulative distribution function (CDF) for $\mathcal{O}(\log(2^n))$ binary-search sampling.
#[derive(Clone, Debug, PartialEq)]
pub struct CumulativeDistribution {
    cdf: Vec<f64>,
}

impl CumulativeDistribution {
    /// Constructs a cumulative distribution function from a validated probability distribution.
    ///
    /// Clamps the terminal cumulative tail to exactly $1.0$ to guarantee safe binary search.
    pub fn from_distribution(dist: &ProbabilityDistribution) -> Self {
        let probs = dist.probabilities();
        let mut cdf = Vec::with_capacity(probs.len());
        let mut acc = 0.0;

        for &p in probs {
            acc += p;
            cdf.push(acc);
        }

        // Clamp floating-point tail
        if let Some(last) = cdf.last_mut() {
            *last = 1.0;
        }

        Self { cdf }
    }

    /// Number of outcomes in the cumulative distribution.
    #[inline]
    pub fn len(&self) -> usize {
        self.cdf.len()
    }

    /// Returns `true` if the cumulative distribution is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cdf.is_empty()
    }

    /// Draws a random basis outcome index $k \in [0, 2^n - 1]$ by binary searching the CDF.
    pub fn sample<R: SeededRng>(&self, rng: &mut R) -> usize {
        let u = rng.next_f64();
        // Locate first index where cdf[k] > u
        let idx = self.cdf.partition_point(|&c| c <= u);
        idx.min(self.cdf.len().saturating_sub(1))
    }
}

/// Terminal measurement sampling engine.
///
/// Pre-computes the statevector probability distribution and CDF **once**, then draws
/// all requested shots via binary search without re-executing circuit gates.
pub struct TerminalSampler<R> {
    selection: MeasurementSelection,
    cdf: CumulativeDistribution,
    rng: R,
    shots_drawn: usize,
}

impl<R: SeededRng> TerminalSampler<R> {
    /// Creates a new terminal sampler from a pre-computed probability distribution.
    pub fn new(
        distribution: &ProbabilityDistribution,
        selection: MeasurementSelection,
        rng: R,
    ) -> Self {
        let cdf = CumulativeDistribution::from_distribution(distribution);
        Self {
            selection,
            cdf,
            rng,
            shots_drawn: 0,
        }
    }

    /// Creates a new terminal sampler directly from an evolved statevector.
    ///
    /// Evaluates the Born-rule probabilities and builds the CDF once.
    pub fn from_statevector(
        state: &StateVector,
        selection: MeasurementSelection,
        rng: R,
    ) -> Result<Self, SimulatorError> {
        let dist = ProbabilityDistribution::from_statevector(state)?;
        Ok(Self::new(&dist, selection, rng))
    }

    /// Generates `num_shots` measurement outcomes from the pre-computed distribution.
    ///
    /// # Errors
    /// Returns [`SimulatorError::InvalidShotCount`] if `num_shots == 0`.
    pub fn sample_shots(&mut self, num_shots: usize) -> Result<ExecutionResult, SimulatorError> {
        if num_shots == 0 {
            return Err(SimulatorError::InvalidShotCount { count: 0 });
        }

        let mut shots = Vec::with_capacity(num_shots);
        for _ in 0..num_shots {
            let basis_index = self.cdf.sample(&mut self.rng);
            let bits = self.selection.extract_bits(basis_index);
            shots.push(ShotResult::new(bits));
        }

        self.shots_drawn += num_shots;
        Ok(ExecutionResult::new(
            self.selection.qubits().to_vec(),
            shots,
        ))
    }

    /// Total number of shots drawn by this sampler.
    #[inline]
    pub fn shots_drawn(&self) -> usize {
        self.shots_drawn
    }
}
