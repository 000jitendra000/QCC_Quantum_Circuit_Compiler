//! In-place two-qubit quantum statevector simulation kernels.
//!
//! Under Little-Endian qubit indexing:
//! - Logical qubit $q_0$ is the least-significant bit (LSB) with bit weight $2^0 = 1$.
//! - Logical qubit $q_k$ has bit weight $2^k$.
//! - A two-qubit operation on ordered operands $(q_A, q_B)$ modifies amplitude quartets
//!   whose basis indices differ only at bit positions $q_A$ and $q_B$.
//!
//! ## Local Quartet Basis Ordering
//! For any base index $i$ where bits at $q_A$ and $q_B$ are zero, the four basis indices are:
//! - `i00`: $(q_A = 0, q_B = 0) = i$ (local basis index 0)
//! - `i10`: $(q_A = 1, q_B = 0) = i \mid 2^{q_A}$ (local basis index 1)
//! - `i01`: $(q_A = 0, q_B = 1) = i \mid 2^{q_B}$ (local basis index 2)
//! - `i11`: $(q_A = 1, q_B = 1) = i \mid 2^{q_A} \mid 2^{q_B}$ (local basis index 3)
//!
//! Here, operand 0 ($q_A$) corresponds to local bit weight 1, and operand 1 ($q_B$)
//! corresponds to local bit weight 2.
//!
//! All $2^n$ statevector amplitudes are partitioned into $2^{n-2}$ disjoint quartets.
//! Each quartet is visited exactly once with $\mathcal{O}(2^n)$ runtime and $\mathcal{O}(1)$ space.

use num_complex::Complex64;

use crate::quantum::gate::Gate;
use crate::quantum::gate_matrix::Matrix4x4;
use crate::simulator::error::SimulatorError;

/// The four basis indices comprising a local two-qubit amplitude quartet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmplitudeQuartet {
    /// Local index 0: operand 0 = 0, operand 1 = 0
    pub i00: usize,
    /// Local index 1: operand 0 = 1, operand 1 = 0
    pub i10: usize,
    /// Local index 2: operand 0 = 0, operand 1 = 1
    pub i01: usize,
    /// Local index 3: operand 0 = 1, operand 1 = 1
    pub i11: usize,
}

/// Zero-allocation iterator over all $2^{n-2}$ amplitude quartets for operands `(op0, op1)`.
#[derive(Clone, Debug)]
pub struct QuartetIterator {
    k: usize,
    num_quartets: usize,
    low_mask: usize,
    mid_mask: usize,
    q_high: usize,
    mask_a: usize,
    mask_b: usize,
}

impl Iterator for QuartetIterator {
    type Item = AmplitudeQuartet;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.k >= self.num_quartets {
            return None;
        }

        let k = self.k;
        self.k += 1;

        let part0 = k & self.low_mask;
        let part1 = (k & self.mid_mask) << 1;
        let part2 = (k >> (self.q_high - 1)) << (self.q_high + 1);

        let base_i = part2 | part1 | part0;

        Some(AmplitudeQuartet {
            i00: base_i,
            i10: base_i | self.mask_a,
            i01: base_i | self.mask_b,
            i11: base_i | self.mask_a | self.mask_b,
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.num_quartets.saturating_sub(self.k);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for QuartetIterator {}

/// Validates operand indices and creates a [`QuartetIterator`] over all $2^{n-2}$ amplitude quartets.
///
/// # Errors
/// - [`SimulatorError::InvalidQubitCount`] if `num_qubits < 2`.
/// - [`SimulatorError::TargetOutOfBounds`] if `op0 >= num_qubits` or `op1 >= num_qubits`.
/// - [`SimulatorError::IdenticalQubitOperands`] if `op0 == op1`.
pub fn quartet_indices(
    num_qubits: usize,
    op0: usize,
    op1: usize,
) -> Result<QuartetIterator, SimulatorError> {
    if num_qubits < 2 {
        return Err(SimulatorError::InvalidQubitCount {
            num_qubits,
            max_allowed: usize::BITS as usize - 1,
        });
    }

    if op0 >= num_qubits {
        return Err(SimulatorError::TargetOutOfBounds {
            target: op0,
            num_qubits,
        });
    }

    if op1 >= num_qubits {
        return Err(SimulatorError::TargetOutOfBounds {
            target: op1,
            num_qubits,
        });
    }

    if op0 == op1 {
        return Err(SimulatorError::IdenticalQubitOperands { qubit: op0 });
    }

    let q_low = op0.min(op1);
    let q_high = op0.max(op1);

    let num_quartets = 1usize << (num_qubits - 2);
    let low_mask = (1usize << q_low) - 1;
    let mid_mask = ((1usize << (q_high - 1)) - 1) ^ low_mask;

    Ok(QuartetIterator {
        k: 0,
        num_quartets,
        low_mask,
        mid_mask,
        q_high,
        mask_a: 1usize << op0,
        mask_b: 1usize << op1,
    })
}

/// Applies a generic $4 \times 4$ unitary matrix to two target qubits in place.
///
/// Operands `(op0, op1)` specify the ordered qubit targets:
/// - Operand 0 (`op0`) corresponds to local basis bit weight 1.
/// - Operand 1 (`op1`) corresponds to local basis bit weight 2.
///
/// # Errors
/// Returns [`SimulatorError`] if operands are invalid, identical, or out of bounds,
/// or if `amplitudes.len() != 2^num_qubits`.
pub fn apply_4x4_kernel(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    op0: usize,
    op1: usize,
    matrix: &Matrix4x4,
) -> Result<(), SimulatorError> {
    let expected_len =
        1usize
            .checked_shl(num_qubits as u32)
            .ok_or(SimulatorError::InvalidQubitCount {
                num_qubits,
                max_allowed: usize::BITS as usize - 1,
            })?;

    if amplitudes.len() != expected_len {
        return Err(SimulatorError::DimensionMismatch {
            num_qubits,
            expected: expected_len,
            actual: amplitudes.len(),
        });
    }

    let iter = quartet_indices(num_qubits, op0, op1)?;
    let m = matrix.0;

    for q in iter {
        let a00 = amplitudes[q.i00];
        let a10 = amplitudes[q.i10];
        let a01 = amplitudes[q.i01];
        let a11 = amplitudes[q.i11];

        amplitudes[q.i00] = m[0][0] * a00 + m[0][1] * a10 + m[0][2] * a01 + m[0][3] * a11;
        amplitudes[q.i10] = m[1][0] * a00 + m[1][1] * a10 + m[1][2] * a01 + m[1][3] * a11;
        amplitudes[q.i01] = m[2][0] * a00 + m[2][1] * a10 + m[2][2] * a01 + m[2][3] * a11;
        amplitudes[q.i11] = m[3][0] * a00 + m[3][1] * a10 + m[3][2] * a01 + m[3][3] * a11;
    }

    Ok(())
}

/// Applies SWAP gate to two qubits in place, exchanging their states.
#[inline]
pub fn apply_swap(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    q0: usize,
    q1: usize,
) -> Result<(), SimulatorError> {
    let expected_len =
        1usize
            .checked_shl(num_qubits as u32)
            .ok_or(SimulatorError::InvalidQubitCount {
                num_qubits,
                max_allowed: usize::BITS as usize - 1,
            })?;

    if amplitudes.len() != expected_len {
        return Err(SimulatorError::DimensionMismatch {
            num_qubits,
            expected: expected_len,
            actual: amplitudes.len(),
        });
    }

    let iter = quartet_indices(num_qubits, q0, q1)?;
    for q in iter {
        amplitudes.swap(q.i10, q.i01);
    }
    Ok(())
}

/// Dispatches a two-qubit gate to the statevector in place.
///
/// # Errors
/// Returns [`SimulatorError::UnsupportedGate`] if the gate is not a two-qubit gate.
pub fn apply_two_qubit_gate_kernel(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    op0: usize,
    op1: usize,
    gate: &Gate,
) -> Result<(), SimulatorError> {
    let matrix = gate
        .matrix4x4()
        .ok_or_else(|| SimulatorError::UnsupportedGate {
            gate: gate.to_string(),
            reason: "expected two-qubit gate, found single-qubit gate",
        })?;

    apply_4x4_kernel(amplitudes, num_qubits, op0, op1, &matrix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quartet_partition_exactness() {
        // Test for n = 4, various qubit pairs
        let pairs = [(0, 1), (1, 0), (0, 3), (3, 0), (1, 2), (2, 1), (1, 3)];
        for (q0, q1) in pairs {
            let iter = quartet_indices(4, q0, q1).unwrap();
            let mut visited = [false; 16];
            let mut count = 0;

            for q in iter {
                count += 1;
                for idx in [q.i00, q.i10, q.i01, q.i11] {
                    assert!(!visited[idx], "index {} visited more than once", idx);
                    visited[idx] = true;
                }
            }

            assert_eq!(count, 4, "expected 4 quartets for n=4");
            assert!(visited.iter().all(|&v| v), "not all indices were covered");
        }
    }

    #[test]
    fn test_quartet_identical_operands_rejected() {
        assert_eq!(
            quartet_indices(3, 1, 1).unwrap_err(),
            SimulatorError::IdenticalQubitOperands { qubit: 1 }
        );
    }

    #[test]
    fn test_quartet_out_of_bounds() {
        assert_eq!(
            quartet_indices(3, 1, 3).unwrap_err(),
            SimulatorError::TargetOutOfBounds {
                target: 3,
                num_qubits: 3
            }
        );
    }

    #[test]
    fn test_apply_swap_basis() {
        // In 2 qubits: SWAP |01> = |10>
        let mut amps = vec![Complex64::new(0.0, 0.0); 4];
        amps[1] = Complex64::new(1.0, 0.0); // |01> (q0 = 1, q1 = 0)

        apply_swap(&mut amps, 2, 0, 1).unwrap();

        assert_eq!(amps[2], Complex64::new(1.0, 0.0)); // |10> (q0 = 0, q1 = 1)
        assert_eq!(amps[1], Complex64::new(0.0, 0.0));
    }
}
