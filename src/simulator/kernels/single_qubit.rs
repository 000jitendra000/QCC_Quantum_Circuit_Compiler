//! In-place single-qubit quantum statevector simulation kernels.
//!
//! Under the Little-Endian indexing convention:
//! - Logical qubit $q_0$ is the least-significant bit (LSB) in state indices.
//! - A single-qubit gate acting on target qubit $q$ modifies pairs of amplitudes
//!   whose basis indices differ only at bit position $q$.
//! - For each pair $(i_0, i_1)$ where the $q$-th bit is 0 in $i_0$ and 1 in $i_1$:
//!   $$a'_{0} = m_{00} a_0 + m_{01} a_1$$
//!   $$a'_{1} = m_{10} a_0 + m_{11} a_1$$
//!
//! Amplitude pairs are enumerated via bit manipulation without full-system matrix allocation,
//! ensuring $\mathcal{O}(2^n)$ runtime and $\mathcal{O}(1)$ auxiliary memory.

use num_complex::Complex64;

use crate::quantum::gate::Gate;
use crate::quantum::gate_matrix::{
    matrix_h, matrix_rx, matrix_ry, matrix_rz, matrix_s, matrix_sdg, matrix_t, matrix_tdg,
    matrix_x, matrix_y, matrix_z, Matrix2x2,
};
use crate::quantum::parameter::Angle;
use crate::simulator::error::SimulatorError;

/// Applies a generic $2 \times 2$ unitary matrix to the specified target qubit
/// of an $n$-qubit statevector in place.
///
/// # Arguments
/// - `amplitudes`: Mutable slice of $2^n$ statevector amplitudes.
/// - `num_qubits`: Number of qubits $n$ in the register.
/// - `target`: Target qubit index $q \in [0, n-1]$.
/// - `matrix`: $2 \times 2$ unitary matrix to apply.
///
/// # Errors
/// Returns [`SimulatorError::TargetOutOfBounds`] if `target >= num_qubits`.
/// Returns [`SimulatorError::DimensionMismatch`] if `amplitudes.len() != 2^num_qubits`.
pub fn apply_2x2_kernel(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
    matrix: &Matrix2x2,
) -> Result<(), SimulatorError> {
    if target >= num_qubits {
        return Err(SimulatorError::TargetOutOfBounds { target, num_qubits });
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

    let m00 = matrix.get(0, 0);
    let m01 = matrix.get(0, 1);
    let m10 = matrix.get(1, 0);
    let m11 = matrix.get(1, 1);

    let num_pairs = 1usize << (num_qubits - 1);
    let low_mask = (1usize << target) - 1;
    let target_bit = 1usize << target;

    for k in 0..num_pairs {
        let high = (k >> target) << (target + 1);
        let low = k & low_mask;
        let i0 = high | low;
        let i1 = i0 | target_bit;

        let a0 = amplitudes[i0];
        let a1 = amplitudes[i1];

        amplitudes[i0] = m00 * a0 + m01 * a1;
        amplitudes[i1] = m10 * a0 + m11 * a1;
    }

    Ok(())
}

/// Applies an arbitrary gate to the specified target qubit in place.
///
/// # Errors
/// Returns [`SimulatorError::UnsupportedGate`] if the gate is not a single-qubit gate.
/// Returns other [`SimulatorError`] variants if target or dimensions are invalid.
pub fn apply_gate_kernel(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
    gate: &Gate,
) -> Result<(), SimulatorError> {
    let matrix = gate
        .matrix2x2()
        .ok_or_else(|| SimulatorError::UnsupportedGate {
            gate: gate.to_string(),
            reason: "expected single-qubit gate, found multi-qubit gate",
        })?;
    apply_2x2_kernel(amplitudes, num_qubits, target, &matrix)
}

/// Applies Pauli X (bit flip) to the target qubit in place.
#[inline]
pub fn apply_x(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
) -> Result<(), SimulatorError> {
    apply_2x2_kernel(amplitudes, num_qubits, target, &matrix_x())
}

/// Applies Pauli Y to the target qubit in place.
#[inline]
pub fn apply_y(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
) -> Result<(), SimulatorError> {
    apply_2x2_kernel(amplitudes, num_qubits, target, &matrix_y())
}

/// Applies Pauli Z (phase flip) to the target qubit in place.
#[inline]
pub fn apply_z(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
) -> Result<(), SimulatorError> {
    apply_2x2_kernel(amplitudes, num_qubits, target, &matrix_z())
}

/// Applies Hadamard gate H to the target qubit in place.
#[inline]
pub fn apply_h(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
) -> Result<(), SimulatorError> {
    apply_2x2_kernel(amplitudes, num_qubits, target, &matrix_h())
}

/// Applies phase gate S to the target qubit in place.
#[inline]
pub fn apply_s(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
) -> Result<(), SimulatorError> {
    apply_2x2_kernel(amplitudes, num_qubits, target, &matrix_s())
}

/// Applies inverse phase gate Sdg ($S^\dagger$) to the target qubit in place.
#[inline]
pub fn apply_sdg(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
) -> Result<(), SimulatorError> {
    apply_2x2_kernel(amplitudes, num_qubits, target, &matrix_sdg())
}

/// Applies T gate ($\sqrt{S}$) to the target qubit in place.
#[inline]
pub fn apply_t(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
) -> Result<(), SimulatorError> {
    apply_2x2_kernel(amplitudes, num_qubits, target, &matrix_t())
}

/// Applies inverse T gate Tdg ($T^\dagger$) to the target qubit in place.
#[inline]
pub fn apply_tdg(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
) -> Result<(), SimulatorError> {
    apply_2x2_kernel(amplitudes, num_qubits, target, &matrix_tdg())
}

/// Applies single-qubit rotation $R_x(\theta)$ to the target qubit in place.
#[inline]
pub fn apply_rx(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
    theta: Angle,
) -> Result<(), SimulatorError> {
    apply_2x2_kernel(amplitudes, num_qubits, target, &matrix_rx(theta))
}

/// Applies single-qubit rotation $R_y(\theta)$ to the target qubit in place.
#[inline]
pub fn apply_ry(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
    theta: Angle,
) -> Result<(), SimulatorError> {
    apply_2x2_kernel(amplitudes, num_qubits, target, &matrix_ry(theta))
}

/// Applies single-qubit rotation $R_z(\theta)$ to the target qubit in place.
#[inline]
pub fn apply_rz(
    amplitudes: &mut [Complex64],
    num_qubits: usize,
    target: usize,
    theta: Angle,
) -> Result<(), SimulatorError> {
    apply_2x2_kernel(amplitudes, num_qubits, target, &matrix_rz(theta))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_1_SQRT_2;

    const TOLERANCE: f64 = 1e-12;

    #[test]
    fn test_apply_h_on_zero_state() {
        // |0> -> (|0> + |1>) / sqrt(2)
        let mut amps = vec![Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)];
        apply_h(&mut amps, 1, 0).unwrap();
        assert!((amps[0] - Complex64::new(FRAC_1_SQRT_2, 0.0)).norm() < TOLERANCE);
        assert!((amps[1] - Complex64::new(FRAC_1_SQRT_2, 0.0)).norm() < TOLERANCE);
    }

    #[test]
    fn test_apply_h_squared_identity() {
        // H^2 = I
        let mut amps = vec![Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)];
        apply_h(&mut amps, 1, 0).unwrap();
        apply_h(&mut amps, 1, 0).unwrap();
        assert!((amps[0] - Complex64::new(1.0, 0.0)).norm() < TOLERANCE);
        assert!(amps[1].norm() < TOLERANCE);
    }

    #[test]
    fn test_apply_x_target_out_of_bounds() {
        let mut amps = vec![Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)];
        let err = apply_x(&mut amps, 1, 1).unwrap_err();
        assert_eq!(
            err,
            SimulatorError::TargetOutOfBounds {
                target: 1,
                num_qubits: 1
            }
        );
    }

    #[test]
    fn test_apply_gate_unsupported_two_qubit() {
        let mut amps = vec![Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)];
        let err = apply_gate_kernel(&mut amps, 1, 0, &Gate::CX).unwrap_err();
        assert!(matches!(err, SimulatorError::UnsupportedGate { .. }));
    }
}
