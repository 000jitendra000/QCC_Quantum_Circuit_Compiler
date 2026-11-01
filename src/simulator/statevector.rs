use num_complex::Complex64;

use crate::quantum::gate::Gate;
use crate::quantum::gate_matrix::{Matrix2x2, Matrix4x4};
use crate::quantum::parameter::Angle;
use crate::simulator::error::SimulatorError;
use crate::simulator::kernels::{controlled, single_qubit, two_qubit};

/// Default maximum allowable qubit count for in-memory statevector allocation (28 qubits = 4 GiB).
pub const DEFAULT_MAX_QUBITS: usize = 28;

/// Default numerical tolerance for statevector normalization validation ($10^{-6}$).
pub const DEFAULT_NORM_TOLERANCE: f64 = 1e-6;

/// In-memory pure quantum statevector representation storing $2^n$ complex double-precision amplitudes.
///
/// Under little-endian statevector indexing:
/// - Logical qubit $q_0$ is the least-significant bit (LSB).
/// - Basis index $i = \sum_{k=0}^{n-1} q_k \cdot 2^k$.
/// - Computational basis states correspond to Dirac vectors $|q_{n-1} \dots q_0\rangle$.
#[derive(Clone, Debug, PartialEq)]
pub struct StateVector {
    num_qubits: usize,
    amplitudes: Vec<Complex64>,
}

impl StateVector {
    /// Creates a new statevector initialized to $|0\dots 0\rangle$ on $n$ qubits.
    ///
    /// Amplitude at basis index 0 is initialized to $1.0 + 0.0i$; all other $2^n - 1$
    /// amplitudes are initialized to $0.0 + 0.0i$.
    pub fn new(num_qubits: usize) -> Result<Self, SimulatorError> {
        Self::with_max_qubits(num_qubits, DEFAULT_MAX_QUBITS)
    }

    /// Creates a new statevector initialized to $|0\dots 0\rangle$ with a custom maximum qubit limit.
    pub fn with_max_qubits(num_qubits: usize, max_allowed: usize) -> Result<Self, SimulatorError> {
        if num_qubits == 0 || num_qubits > max_allowed {
            return Err(SimulatorError::InvalidQubitCount {
                num_qubits,
                max_allowed,
            });
        }

        let dim =
            1usize
                .checked_shl(num_qubits as u32)
                .ok_or(SimulatorError::InvalidQubitCount {
                    num_qubits,
                    max_allowed,
                })?;

        let requested_bytes = dim.saturating_mul(std::mem::size_of::<Complex64>());
        let max_allowed_bytes =
            (1usize << max_allowed).saturating_mul(std::mem::size_of::<Complex64>());
        if requested_bytes > max_allowed_bytes {
            return Err(SimulatorError::ResourceLimitExceeded {
                num_qubits,
                requested_bytes,
                max_allowed_bytes,
            });
        }

        let mut amplitudes = vec![Complex64::new(0.0, 0.0); dim];
        amplitudes[0] = Complex64::new(1.0, 0.0);

        Ok(Self {
            num_qubits,
            amplitudes,
        })
    }

    /// Creates a statevector initialized to a specific computational basis state $|index\rangle$.
    pub fn from_basis_state(num_qubits: usize, basis_index: usize) -> Result<Self, SimulatorError> {
        let mut state = Self::new(num_qubits)?;
        let dim = state.len();

        if basis_index >= dim {
            return Err(SimulatorError::BasisIndexOutOfBounds {
                index: basis_index,
                num_qubits,
                max_index: dim - 1,
            });
        }

        state.amplitudes[0] = Complex64::new(0.0, 0.0);
        state.amplitudes[basis_index] = Complex64::new(1.0, 0.0);
        Ok(state)
    }

    /// Creates a statevector from user-provided amplitudes.
    ///
    /// # Validation Policy
    /// - Qubit count must be between 1 and `DEFAULT_MAX_QUBITS`.
    /// - Vector length must exactly equal $2^n$.
    /// - No amplitude may be NaN or infinite.
    /// - State must be normalized such that $|(\sum |\alpha_i|^2) - 1.0| \le \text{tolerance}$.
    /// - **QCC does not silently renormalize user amplitudes.** Non-normalized states are rejected.
    pub fn from_amplitudes(
        num_qubits: usize,
        amplitudes: Vec<Complex64>,
    ) -> Result<Self, SimulatorError> {
        Self::from_amplitudes_with_tolerance(num_qubits, amplitudes, DEFAULT_NORM_TOLERANCE)
    }

    /// Creates a statevector from user-provided amplitudes with an explicit normalization tolerance.
    pub fn from_amplitudes_with_tolerance(
        num_qubits: usize,
        amplitudes: Vec<Complex64>,
        tolerance: f64,
    ) -> Result<Self, SimulatorError> {
        if num_qubits == 0 || num_qubits > DEFAULT_MAX_QUBITS {
            return Err(SimulatorError::InvalidQubitCount {
                num_qubits,
                max_allowed: DEFAULT_MAX_QUBITS,
            });
        }

        let expected_dim = 1usize << num_qubits;
        if amplitudes.len() != expected_dim {
            return Err(SimulatorError::DimensionMismatch {
                num_qubits,
                expected: expected_dim,
                actual: amplitudes.len(),
            });
        }

        // Validate values and compute norm squared
        let mut norm_sqr = 0.0;
        for (idx, amp) in amplitudes.iter().enumerate() {
            if amp.is_nan() {
                return Err(SimulatorError::InvalidAmplitude {
                    index: idx,
                    reason: "amplitude contains NaN",
                });
            }
            if amp.is_infinite() {
                return Err(SimulatorError::InvalidAmplitude {
                    index: idx,
                    reason: "amplitude is infinite",
                });
            }
            norm_sqr += amp.norm_sqr();
        }

        if (norm_sqr - 1.0).abs() > tolerance {
            return Err(SimulatorError::NonNormalizedState {
                norm_sqr,
                tolerance,
            });
        }

        Ok(Self {
            num_qubits,
            amplitudes,
        })
    }

    /// Returns the number of qubits in the statevector register.
    #[inline]
    pub fn num_qubits(&self) -> usize {
        self.num_qubits
    }

    /// Returns the dimension ($2^n$) of the statevector.
    #[inline]
    pub fn len(&self) -> usize {
        self.amplitudes.len()
    }

    /// Returns `true` if the statevector has dimension 0 (never true for valid states).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.amplitudes.is_empty()
    }

    /// Returns an immutable slice of all $2^n$ statevector amplitudes.
    #[inline]
    pub fn amplitudes(&self) -> &[Complex64] {
        &self.amplitudes
    }

    /// Returns a mutable slice of all $2^n$ statevector amplitudes.
    #[inline]
    pub fn amplitudes_mut(&mut self) -> &mut [Complex64] {
        &mut self.amplitudes
    }

    /// Returns the amplitude at the given computational basis index, or `None` if out of bounds.
    #[inline]
    pub fn get(&self, index: usize) -> Option<Complex64> {
        self.amplitudes.get(index).copied()
    }

    /// Computes the sum of squared magnitudes of all amplitudes: $\sum_{i=0}^{2^n - 1} |\alpha_i|^2$.
    pub fn norm_squared(&self) -> f64 {
        self.amplitudes.iter().map(|a| a.norm_sqr()).sum()
    }

    /// Computes the $\ell_2$ state norm: $\sqrt{\sum |\alpha_i|^2}$.
    #[inline]
    pub fn norm(&self) -> f64 {
        self.norm_squared().sqrt()
    }

    /// Returns `true` if the state norm squared is approximately 1.0 within tolerance.
    #[inline]
    pub fn is_normalized(&self, tolerance: f64) -> bool {
        (self.norm_squared() - 1.0).abs() <= tolerance
    }

    /// Computes the measurement probability for a specific computational basis outcome index: $P(i) = |\alpha_i|^2$.
    pub fn probability(&self, index: usize) -> Result<f64, SimulatorError> {
        let amp = self
            .get(index)
            .ok_or(SimulatorError::BasisIndexOutOfBounds {
                index,
                num_qubits: self.num_qubits,
                max_index: self.len() - 1,
            })?;
        Ok(amp.norm_sqr())
    }

    /// Computes the full computational basis probability distribution $P(i) = |\alpha_i|^2$ for all $2^n$ basis states.
    pub fn probabilities(
        &self,
    ) -> Result<crate::simulator::measurement::ProbabilityDistribution, SimulatorError> {
        crate::simulator::measurement::ProbabilityDistribution::from_statevector(self)
    }

    /// Executes terminal measurement sampling of `num_shots` outcomes on the selected qubits.
    ///
    /// Samples all requested shots from the pre-computed state distribution without re-executing gates.
    pub fn sample_terminal<R: crate::simulator::sampling::SeededRng>(
        &self,
        selection: crate::simulator::measurement::MeasurementSelection,
        num_shots: usize,
        rng: R,
    ) -> Result<crate::simulator::result::ExecutionResult, SimulatorError> {
        let mut sampler =
            crate::simulator::sampling::TerminalSampler::from_statevector(self, selection, rng)?;
        sampler.sample_shots(num_shots)
    }

    /// Returns the allocated state memory in bytes ($16 \cdot 2^n$ bytes).
    #[inline]
    pub fn memory_bytes(&self) -> usize {
        self.amplitudes.len() * std::mem::size_of::<Complex64>()
    }

    /// Compares two statevectors for pointwise amplitude equality within tolerance.
    pub fn approx_eq(&self, other: &Self, tolerance: f64) -> bool {
        if self.num_qubits != other.num_qubits || self.len() != other.len() {
            return false;
        }
        for (a, b) in self.amplitudes.iter().zip(other.amplitudes.iter()) {
            if (a - b).norm() > tolerance {
                return false;
            }
        }
        true
    }

    /// Consumes the statevector and returns its inner amplitude vector.
    #[inline]
    pub fn into_amplitudes(self) -> Vec<Complex64> {
        self.amplitudes
    }

    /// Applies an arbitrary $2 \times 2$ unitary matrix to the target qubit in place.
    pub fn apply_matrix2x2(
        &mut self,
        target: usize,
        matrix: &Matrix2x2,
    ) -> Result<(), SimulatorError> {
        single_qubit::apply_2x2_kernel(&mut self.amplitudes, self.num_qubits, target, matrix)?;
        debug_assert!(
            self.is_normalized(1e-5),
            "state drifted from normalization after applying 2x2 matrix"
        );
        Ok(())
    }

    /// Applies a single-qubit gate to the target qubit in place.
    pub fn apply_gate(&mut self, gate: &Gate, target: usize) -> Result<(), SimulatorError> {
        single_qubit::apply_gate_kernel(&mut self.amplitudes, self.num_qubits, target, gate)?;
        debug_assert!(
            self.is_normalized(1e-5),
            "state drifted from normalization after applying gate"
        );
        Ok(())
    }

    /// Applies Pauli X to the target qubit in place.
    #[inline]
    pub fn apply_x(&mut self, target: usize) -> Result<(), SimulatorError> {
        single_qubit::apply_x(&mut self.amplitudes, self.num_qubits, target)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies Pauli Y to the target qubit in place.
    #[inline]
    pub fn apply_y(&mut self, target: usize) -> Result<(), SimulatorError> {
        single_qubit::apply_y(&mut self.amplitudes, self.num_qubits, target)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies Pauli Z to the target qubit in place.
    #[inline]
    pub fn apply_z(&mut self, target: usize) -> Result<(), SimulatorError> {
        single_qubit::apply_z(&mut self.amplitudes, self.num_qubits, target)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies Hadamard H to the target qubit in place.
    #[inline]
    pub fn apply_h(&mut self, target: usize) -> Result<(), SimulatorError> {
        single_qubit::apply_h(&mut self.amplitudes, self.num_qubits, target)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies phase S gate to the target qubit in place.
    #[inline]
    pub fn apply_s(&mut self, target: usize) -> Result<(), SimulatorError> {
        single_qubit::apply_s(&mut self.amplitudes, self.num_qubits, target)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies inverse phase Sdg ($S^\dagger$) gate to the target qubit in place.
    #[inline]
    pub fn apply_sdg(&mut self, target: usize) -> Result<(), SimulatorError> {
        single_qubit::apply_sdg(&mut self.amplitudes, self.num_qubits, target)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies T gate to the target qubit in place.
    #[inline]
    pub fn apply_t(&mut self, target: usize) -> Result<(), SimulatorError> {
        single_qubit::apply_t(&mut self.amplitudes, self.num_qubits, target)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies inverse Tdg ($T^\dagger$) gate to the target qubit in place.
    #[inline]
    pub fn apply_tdg(&mut self, target: usize) -> Result<(), SimulatorError> {
        single_qubit::apply_tdg(&mut self.amplitudes, self.num_qubits, target)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies rotation $R_x(\theta)$ to the target qubit in place.
    #[inline]
    pub fn apply_rx(&mut self, target: usize, theta: Angle) -> Result<(), SimulatorError> {
        single_qubit::apply_rx(&mut self.amplitudes, self.num_qubits, target, theta)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies rotation $R_y(\theta)$ to the target qubit in place.
    #[inline]
    pub fn apply_ry(&mut self, target: usize, theta: Angle) -> Result<(), SimulatorError> {
        single_qubit::apply_ry(&mut self.amplitudes, self.num_qubits, target, theta)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies rotation $R_z(\theta)$ to the target qubit in place.
    #[inline]
    pub fn apply_rz(&mut self, target: usize, theta: Angle) -> Result<(), SimulatorError> {
        single_qubit::apply_rz(&mut self.amplitudes, self.num_qubits, target, theta)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies a generic $4 \times 4$ unitary matrix to two target qubits in place.
    pub fn apply_matrix4x4(
        &mut self,
        op0: usize,
        op1: usize,
        matrix: &Matrix4x4,
    ) -> Result<(), SimulatorError> {
        two_qubit::apply_4x4_kernel(&mut self.amplitudes, self.num_qubits, op0, op1, matrix)?;
        debug_assert!(
            self.is_normalized(1e-5),
            "state drifted from normalization after applying 4x4 matrix"
        );
        Ok(())
    }

    /// Applies a two-qubit gate to the target qubits in place.
    pub fn apply_gate_2q(
        &mut self,
        gate: &Gate,
        op0: usize,
        op1: usize,
    ) -> Result<(), SimulatorError> {
        two_qubit::apply_two_qubit_gate_kernel(
            &mut self.amplitudes,
            self.num_qubits,
            op0,
            op1,
            gate,
        )?;
        debug_assert!(
            self.is_normalized(1e-5),
            "state drifted from normalization after applying two-qubit gate"
        );
        Ok(())
    }

    /// Applies Controlled-NOT ($CX$) in place (operand 0 = control, operand 1 = target).
    #[inline]
    pub fn apply_cx(&mut self, control: usize, target: usize) -> Result<(), SimulatorError> {
        controlled::apply_cx(&mut self.amplitudes, self.num_qubits, control, target)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies Controlled-Z ($CZ$) in place.
    #[inline]
    pub fn apply_cz(&mut self, q0: usize, q1: usize) -> Result<(), SimulatorError> {
        controlled::apply_cz(&mut self.amplitudes, self.num_qubits, q0, q1)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies SWAP gate in place, exchanging the states of `q0` and `q1`.
    #[inline]
    pub fn apply_swap(&mut self, q0: usize, q1: usize) -> Result<(), SimulatorError> {
        two_qubit::apply_swap(&mut self.amplitudes, self.num_qubits, q0, q1)?;
        debug_assert!(self.is_normalized(1e-5));
        Ok(())
    }

    /// Applies a controlled single-qubit $2 \times 2$ unitary matrix in place.
    pub fn apply_controlled_2x2(
        &mut self,
        control: usize,
        target: usize,
        matrix: &Matrix2x2,
    ) -> Result<(), SimulatorError> {
        controlled::apply_controlled_2x2_kernel(
            &mut self.amplitudes,
            self.num_qubits,
            control,
            target,
            matrix,
        )?;
        debug_assert!(
            self.is_normalized(1e-5),
            "state drifted from normalization after applying controlled 2x2 matrix"
        );
        Ok(())
    }

    /// Applies a global phase factor $e^{i\phi}$ to all amplitudes in place: $\alpha_j \leftarrow \alpha_j \cdot e^{i\phi}$.
    ///
    /// Preserves the state norm since $|e^{i\phi}| = 1$.
    pub fn apply_global_phase(&mut self, angle_radians: f64) {
        if angle_radians.abs() < 1e-15 {
            return;
        }
        let phase_factor = Complex64::new(angle_radians.cos(), angle_radians.sin());
        for amp in &mut self.amplitudes {
            *amp *= phase_factor;
        }
        debug_assert!(
            self.is_normalized(1e-5),
            "state drifted from normalization after applying global phase"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statevector_initialization_zero() {
        let sv = StateVector::new(3).unwrap();
        assert_eq!(sv.num_qubits(), 3);
        assert_eq!(sv.len(), 8);
        assert_eq!(sv.get(0).unwrap(), Complex64::new(1.0, 0.0));
        for i in 1..8 {
            assert_eq!(sv.get(i).unwrap(), Complex64::new(0.0, 0.0));
        }
        assert!((sv.norm() - 1.0).abs() < 1e-12);
        assert!(sv.is_normalized(1e-12));
    }

    #[test]
    fn test_statevector_from_basis_state() {
        let sv = StateVector::from_basis_state(2, 2).unwrap(); // |10>
        assert_eq!(sv.get(0).unwrap(), Complex64::new(0.0, 0.0));
        assert_eq!(sv.get(1).unwrap(), Complex64::new(0.0, 0.0));
        assert_eq!(sv.get(2).unwrap(), Complex64::new(1.0, 0.0));
        assert_eq!(sv.get(3).unwrap(), Complex64::new(0.0, 0.0));
        assert!(sv.is_normalized(1e-12));
    }

    #[test]
    fn test_statevector_out_of_bounds_basis() {
        assert!(StateVector::from_basis_state(2, 4).is_err());
    }

    #[test]
    fn test_statevector_unnormalized_rejection() {
        let amps = vec![
            Complex64::new(0.5, 0.0),
            Complex64::new(0.5, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ]; // norm squared = 0.25 + 0.25 = 0.5 != 1.0
        assert!(StateVector::from_amplitudes(2, amps).is_err());
    }

    #[test]
    fn test_statevector_invalid_dimension_rejection() {
        let amps = vec![Complex64::new(1.0, 0.0); 3]; // Not power of 2
        assert!(StateVector::from_amplitudes(2, amps).is_err());
    }

    #[test]
    fn test_statevector_zero_qubit_rejection() {
        assert!(StateVector::new(0).is_err());
    }

    #[test]
    fn test_memory_bytes_calculation() {
        let sv = StateVector::new(4).unwrap(); // 16 amplitudes * 16 bytes = 256 bytes
        assert_eq!(sv.memory_bytes(), 256);
    }

    #[test]
    fn test_statevector_gate_application() {
        let mut sv = StateVector::new(2).unwrap();
        // Initially |00>
        sv.apply_x(0).unwrap(); // |01> (LSB q0 = 1)
        assert_eq!(sv.get(1).unwrap(), Complex64::new(1.0, 0.0));
        assert_eq!(sv.get(0).unwrap(), Complex64::new(0.0, 0.0));

        sv.apply_x(1).unwrap(); // |11> (q1 = 1, q0 = 1, index = 3)
        assert_eq!(sv.get(3).unwrap(), Complex64::new(1.0, 0.0));
        assert_eq!(sv.get(1).unwrap(), Complex64::new(0.0, 0.0));

        // Apply H to q0: (|10> - |11>) / sqrt(2)
        sv.apply_h(0).unwrap();
        let expected_plus = Complex64::new(std::f64::consts::FRAC_1_SQRT_2, 0.0);
        let expected_minus = Complex64::new(-std::f64::consts::FRAC_1_SQRT_2, 0.0);
        assert!((sv.get(2).unwrap() - expected_plus).norm() < 1e-12);
        assert!((sv.get(3).unwrap() - expected_minus).norm() < 1e-12);

        // Apply H again to q0: should restore |11>
        sv.apply_gate(&Gate::H, 0).unwrap();
        assert!((sv.get(3).unwrap() - Complex64::new(1.0, 0.0)).norm() < 1e-12);
    }

    #[test]
    fn test_statevector_bell_state_preparation() {
        let mut sv = StateVector::new(2).unwrap();
        // |00> -> H(q0) -> (|00> + |01>) / sqrt(2)
        sv.apply_h(0).unwrap();
        // CX(q0, q1): control = q0, target = q1 -> (|00> + |11>) / sqrt(2)
        sv.apply_cx(0, 1).unwrap();

        let expected_amp = Complex64::new(std::f64::consts::FRAC_1_SQRT_2, 0.0);
        assert!((sv.get(0).unwrap() - expected_amp).norm() < 1e-12);
        assert!(sv.get(1).unwrap().norm() < 1e-12);
        assert!(sv.get(2).unwrap().norm() < 1e-12);
        assert!((sv.get(3).unwrap() - expected_amp).norm() < 1e-12);
        assert!(sv.is_normalized(1e-12));
    }
}
