//! Measurement execution results and shot outcome representations.
//!
//! Formats classical measurement result bits according to Phase 2 semantics:
//! - `ClassicalBitOrder::MsbFirst`: standard OpenQASM format $c_{m-1} \dots c_0$
//! - `LsbFirst`: $c_0 \dots c_{m-1}$

use std::collections::BTreeMap;
use std::fmt;

use crate::quantum::semantics::{format_measurement_bits, ClassicalBitOrder};

/// Represents a single measurement outcome bit vector.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShotResult {
    bits: Vec<u8>,
}

impl ShotResult {
    /// Creates a new `ShotResult` from a slice of binary bits (`0` or `1`).
    ///
    /// # Panics
    /// Panics if any bit value is not 0 or 1.
    pub fn new(bits: Vec<u8>) -> Self {
        for &b in &bits {
            assert!(b <= 1, "bit must be 0 or 1, got {b}");
        }
        Self { bits }
    }

    /// Returns a slice of the measurement outcome bits.
    #[inline]
    pub fn bits(&self) -> &[u8] {
        &self.bits
    }

    /// Number of bits in the outcome vector.
    #[inline]
    pub fn len(&self) -> usize {
        self.bits.len()
    }

    /// Returns `true` if the bit vector is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }

    /// Formats the measurement outcome bits as a string under the specified ordering convention.
    ///
    /// - [`ClassicalBitOrder::MsbFirst`]: string is formatted as `c[m-1]...c[0]`.
    /// - [`ClassicalBitOrder::LsbFirst`]: string is formatted as `c[0]...c[m-1]`.
    #[inline]
    pub fn to_bitstring(&self, order: ClassicalBitOrder) -> String {
        format_measurement_bits(&self.bits, order)
    }

    /// Converts the measurement bits into an integer using little-endian bit weights:
    /// $$\sum_{k=0}^{m-1} \text{bit}_k \cdot 2^k$$
    pub fn to_integer(&self) -> u64 {
        let mut val = 0u64;
        for (k, &b) in self.bits.iter().enumerate() {
            if b == 1 {
                val |= 1u64 << k;
            }
        }
        val
    }
}

impl fmt::Display for ShotResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_bitstring(ClassicalBitOrder::MsbFirst))
    }
}

/// Aggregated outcome of running terminal measurement shots.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionResult {
    measured_qubits: Vec<usize>,
    num_shots: usize,
    shots: Vec<ShotResult>,
}

impl ExecutionResult {
    /// Creates a new `ExecutionResult`.
    pub fn new(measured_qubits: Vec<usize>, shots: Vec<ShotResult>) -> Self {
        let num_shots = shots.len();
        Self {
            measured_qubits,
            num_shots,
            shots,
        }
    }

    /// Returns the ordered list of measured logical qubits.
    #[inline]
    pub fn measured_qubits(&self) -> &[usize] {
        &self.measured_qubits
    }

    /// Returns the total number of requested shots.
    #[inline]
    pub fn num_shots(&self) -> usize {
        self.num_shots
    }

    /// Returns the list of individual shot outcomes.
    #[inline]
    pub fn shots(&self) -> &[ShotResult] {
        &self.shots
    }

    /// Returns a histogram of bitstring outcome counts ordered under the specified bit ordering.
    pub fn counts(&self, order: ClassicalBitOrder) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for shot in &self.shots {
            let key = shot.to_bitstring(order);
            *counts.entry(key).or_insert(0) += 1;
        }
        counts
    }

    /// Returns the empirical probability (relative frequency) of each observed bitstring.
    pub fn frequencies(&self, order: ClassicalBitOrder) -> BTreeMap<String, f64> {
        let counts = self.counts(order);
        let total = self.num_shots as f64;
        counts
            .into_iter()
            .map(|(key, count)| (key, count as f64 / total))
            .collect()
    }

    /// Returns the most frequently observed bitstring outcome and its count.
    pub fn most_frequent(&self, order: ClassicalBitOrder) -> Option<(String, usize)> {
        self.counts(order)
            .into_iter()
            .max_by_key(|&(_, count)| count)
    }
}
