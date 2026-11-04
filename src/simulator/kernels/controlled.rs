//! In-place controlled-gate simulation kernels.
//!
//! Provides optimized kernels for single-qubit unitary operations conditioned on a control qubit.
//!
//! Under Little-Endian indexing and standard operand ordering:
//! - In $CX(q_{\text{ctrl}}, q_{\text{targ}})$, operand 0 is control and operand 1 is target.
//! - For each amplitude quartet $(i_{00}, i_{10}, i_{01}, i_{11})$ partitioned by $(q_{\text{ctrl}}, q_{\text{targ}})$:
//!   - When control is 0 ($i_{00}, i_{01}$), amplitudes remain untouched.
//!   - When control is 1 ($i_{10}, i_{11}$), target qubit amplitudes $(a_0 = \alpha_{i_{10}}, a_1 = \alpha_{i_{11}})$
//!     are transformed according to the specified operation.

use num_complex::Complex64;

use crate::quantum::gate_matrix::Matrix2x2;
use crate::simulator::error::SimulatorError;
use crate::simulator::kernels::two_qubit::quartet_indices;

/// Applies a generic $2 \times 2$ single-qubit unitary to `target` conditioned on `control == 1`.
///
/// Runs in $\mathcal{O}(2^n)$ time with zero auxiliary memory.
///
/// # Errors
/// - [`SimulatorError::IdenticalQubitOperands`] if `control == target`.
/// - [`SimulatorError::TargetOutOfBounds`] if `control >= num_qubits` or `target >= num_qubits`.
/// - [`SimulatorError::DimensionMismatch`] if `amplitudes.len() != 2^num_qubits`.
pub fn apply_controlled_2x2_kernel(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    control: usize,
    target: usize,
    matrix: &Matrix2x2,
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

    let iter = quartet_indices(num_qubits, control, target)?;
    let m = matrix.0;

    for q in iter {
        let a0 = amplitudes[q.i10];
        let a1 = amplitudes[q.i11];

        amplitudes[q.i10] = m[0][0] * a0 + m[0][1] * a1;
        amplitudes[q.i11] = m[1][0] * a0 + m[1][1] * a1;
    }

    Ok(())
}

/// Applies Controlled-NOT ($CX$) in place.
///
/// - First operand `control` is the control qubit.
/// - Second operand `target` is the target qubit.
/// - When `control` is 1, flips the target qubit state $|0\rangle \leftrightarrow |1\rangle$.
///
/// # Errors
/// Returns [`SimulatorError`] if operands are invalid, identical, or out of bounds.
#[inline]
pub fn apply_cx(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    control: usize,
    target: usize,
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

    let iter = quartet_indices(num_qubits, control, target)?;
    for q in iter {
        amplitudes.swap(q.i10, q.i11);
    }

    Ok(())
}

/// Applies Controlled-Z ($CZ$) in place.
///
/// Symmetric under operand permutation: $CZ(q_0, q_1) = CZ(q_1, q_0)$.
/// Negates the amplitude of $|11\rangle$ while preserving all other basis states.
///
/// # Errors
/// Returns [`SimulatorError`] if operands are invalid, identical, or out of bounds.
#[inline]
pub fn apply_cz(
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
        amplitudes[q.i11] = -amplitudes[q.i11];
    }

    Ok(())
}

/// Applies an in-place Toffoli (CCX) gate on controls `c1, c2` and target `target`.
///
/// Runs in $\mathcal{O}(2^n)$ time with zero auxiliary memory.
pub fn apply_ccx(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    c1: usize,
    c2: usize,
    target: usize,
) -> Result<(), SimulatorError> {
    if c1 >= num_qubits || c2 >= num_qubits || target >= num_qubits {
        return Err(SimulatorError::TargetOutOfBounds {
            target: c1.max(c2).max(target),
            num_qubits,
        });
    }
    if c1 == c2 || c1 == target {
        return Err(SimulatorError::IdenticalQubitOperands { qubit: c1 });
    }
    if c2 == target {
        return Err(SimulatorError::IdenticalQubitOperands { qubit: c2 });
    }
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

    let target_mask = 1usize << target;
    let c1_mask = 1usize << c1;
    let c2_mask = 1usize << c2;

    for i in 0..expected_len {
        if (i & target_mask) == 0 && (i & c1_mask) != 0 && (i & c2_mask) != 0 {
            amplitudes.swap(i, i | target_mask);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_cx_truth_table() {
        // Test CX(control=0, target=1) in 2-qubit system
        // |00> -> |00>
        // |01> -> |11> (q0 is 1, so q1 flips from 0 to 1)
        // |10> -> |10> (q0 is 0, so q1 stays 1)
        // |11> -> |01> (q0 is 1, so q1 flips from 1 to 0)
        let mut amps = vec![Complex64::new(0.0, 0.0); 4];
        amps[1] = Complex64::new(1.0, 0.0); // |01>

        apply_cx(&mut amps, 2, 0, 1).unwrap();
        assert_eq!(amps[3], Complex64::new(1.0, 0.0)); // |11>
        assert_eq!(amps[1], Complex64::new(0.0, 0.0));
    }

    #[test]
    fn test_apply_cz_truth_table() {
        // |11> (index 3) should be negated
        let mut amps = vec![Complex64::new(1.0, 0.0); 4];
        apply_cz(&mut amps, 2, 0, 1).unwrap();
        assert_eq!(amps[0], Complex64::new(1.0, 0.0));
        assert_eq!(amps[1], Complex64::new(1.0, 0.0));
        assert_eq!(amps[2], Complex64::new(1.0, 0.0));
        assert_eq!(amps[3], Complex64::new(-1.0, 0.0));
    }

    #[test]
    fn test_ccx_behavior() {
        // Test CCX(c1=0, c2=1, target=2) in 3-qubit system
        // Swaps |011> (index 3) and |111> (index 7)
        let mut amps = vec![Complex64::new(0.0, 0.0); 8];
        amps[3] = Complex64::new(0.3, 0.4);
        amps[7] = Complex64::new(0.5, 0.6);

        apply_ccx(&mut amps, 3, 0, 1, 2).unwrap();

        assert_eq!(amps[3], Complex64::new(0.5, 0.6));
        assert_eq!(amps[7], Complex64::new(0.3, 0.4));
        for i in [0, 1, 2, 4, 5, 6] {
            assert_eq!(amps[i], Complex64::new(0.0, 0.0));
        }
    }

    #[test]
    fn test_ccx_errors() {
        let mut amps = vec![Complex64::new(1.0, 0.0); 4];

        // Identical operands
        assert!(apply_ccx(&mut amps, 2, 0, 0, 1).is_err());
        assert!(apply_ccx(&mut amps, 2, 0, 1, 0).is_err());
        assert!(apply_ccx(&mut amps, 2, 1, 1, 0).is_err());

        // Out of bounds
        assert!(apply_ccx(&mut amps, 2, 0, 1, 2).is_err());
        assert!(apply_ccx(&mut amps, 2, 2, 1, 0).is_err());
    }
}
