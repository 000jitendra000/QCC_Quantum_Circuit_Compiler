//! Unitary equivalence verifier checking circuit equivalence up to one common global phase.

use std::collections::HashMap;

use num_complex::Complex64;
use thiserror::Error;

use crate::ir::circuit::Circuit;
use crate::ir::error::{IrError, IrValidationError};
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::routing::LayoutError;
use crate::simulator::error::{ExecutionError, SimulatorError};
use crate::simulator::executor::CircuitExecutor;
use crate::simulator::sampling::SplitMix64;
use crate::simulator::statevector::StateVector;
use crate::testkit::random_circuit::generate_random_statevector;
use crate::verification::global_phase::{GlobalPhaseEstimator, PhaseEstimationResult};
use crate::verification::tolerance::VerificationTolerances;

/// Specific error conditions encountered during unitary equivalence verification.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum VerificationError {
    /// Circuit contains a non-unitary operation (measurement, reset, or classical computation).
    #[error(
        "non-unitary operation '{operation}' (op {op_id:?}) rejected by unitary checker: {reason}"
    )]
    NonUnitaryOperation {
        /// Semantic operation ID if known.
        op_id: Option<OperationId>,
        /// Operation type name.
        operation: String,
        /// Explanation of why this operation invalidates unitary checking.
        reason: String,
    },

    /// The two circuits declare differing numbers of logical qubits.
    #[error("qubit count mismatch between circuits: circuit A has {circuit_a} qubits, circuit B has {circuit_b} qubits")]
    QubitCountMismatch {
        /// Qubit count in circuit A.
        circuit_a: usize,
        /// Qubit count in circuit B.
        circuit_b: usize,
    },

    /// Input statevector qubit count does not match circuit qubit count.
    #[error(
        "input statevector qubit count ({input}) does not match circuit qubit count ({circuit})"
    )]
    InputQubitMismatch {
        /// Qubit count declared in circuit.
        circuit: usize,
        /// Qubit count in input statevector.
        input: usize,
    },

    /// Input statevector is not normalized to unity within tolerance.
    #[error("input statevector is not normalized (norm squared = {norm_sqr}, expected 1.0 within {tolerance})")]
    UnnormalizedInput {
        /// Computed norm squared of input state.
        norm_sqr: f64,
        /// Normalization tolerance.
        tolerance: f64,
    },

    /// A circuit contains zero declared qubits.
    #[error("cannot verify circuit with 0 declared qubits")]
    ZeroQubits,

    /// Full matrix reconstruction was requested for a system exceeding the configured matrix qubit limit.
    #[error(
        "system size of {qubits} qubits exceeds matrix reconstruction limit of {max_qubits} qubits"
    )]
    SystemTooLargeForMatrix {
        /// Circuit qubit count.
        qubits: usize,
        /// Maximum allowed qubit count for matrix reconstruction.
        max_qubits: usize,
    },

    /// Circuit validation error before verification.
    #[error("circuit validation failed: {0}")]
    Validation(#[from] IrValidationError),

    /// Graph or IR traversal error.
    #[error("IR error: {0}")]
    Ir(#[from] IrError),

    /// Low-level simulator kernel or memory error.
    #[error("simulator error: {0}")]
    Simulator(#[from] SimulatorError),

    /// Simulator or kernel execution error.
    #[error("execution error: {0}")]
    Execution(#[from] ExecutionError),

    /// Layout or placement error during layout-aware verification.
    #[error("layout error: {0}")]
    Layout(#[from] LayoutError),
}

/// Verification methodology used to compare circuit unitary actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationMethod {
    /// Exhaustive computational-basis input comparison covering all $2^n$ basis columns.
    ComputationalBasis {
        /// Total number of basis columns verified ($2^n$).
        num_basis_states: usize,
    },
    /// Randomized arbitrary-superposition testing on normalized complex statevectors.
    RandomizedStates {
        /// Number of independent randomized states tested.
        num_states: usize,
    },
}

/// Detailed diagnostic information for the worst detected equivalence mismatch.
#[derive(Clone, Debug, PartialEq)]
pub struct MismatchDiagnostic {
    /// Index of the test input state where the maximum deviation occurred
    /// (computational basis index for matrix mode, or randomized state sequence index).
    pub input_index: usize,
    /// Output state amplitude index ($j \in [0, 2^n)$) where the error was maximized.
    pub output_amplitude_index: usize,
    /// Reference circuit output amplitude: $a_j$.
    pub reference_amplitude: Complex64,
    /// Candidate circuit output amplitude: $b_j$.
    pub candidate_amplitude: Complex64,
    /// Expected candidate amplitude under the candidate global phase: $e^{i\theta} a_j$.
    pub expected_candidate_amplitude: Complex64,
    /// Pointwise absolute error: $| b_j - e^{i\theta} a_j |$.
    pub amplitude_difference: f64,
    /// Name and threshold of the specific tolerance that was violated.
    pub exceeded_tolerance: String,
}

/// Comprehensive outcome of a unitary equivalence check between two circuits.
#[derive(Clone, Debug, PartialEq)]
pub struct EquivalenceVerdict {
    /// `true` if candidate circuit $B$ is unitarily equivalent to reference circuit $A$
    /// up to a single common global phase ($B \approx e^{i\theta} A$).
    pub is_equivalent: bool,
    /// Logical qubit count of the compared circuits.
    pub qubits: usize,
    /// Method utilized for comparison (computational basis vs randomized states).
    pub method: VerificationMethod,
    /// Estimated common global phase $\theta \in (-\pi, \pi]$ when available.
    pub estimated_phase: Option<f64>,
    /// Unit-modulus phase factor $e^{i\theta}$.
    pub phase_factor: Option<Complex64>,
    /// Maximum pointwise amplitude difference across all test inputs and amplitude components.
    pub max_amplitude_difference: f64,
    /// Reconstructed matrix Frobenius norm error, if matrix mode was used.
    pub matrix_frobenius_error: Option<f64>,
    /// Diagnostic description of the worst amplitude or phase mismatch, if any.
    pub worst_mismatch: Option<MismatchDiagnostic>,
    /// Tolerances applied during this verification check.
    pub tolerances: VerificationTolerances,
}

/// High-precision unitary equivalence checker.
///
/// Compares two quantum circuits representing operations $U_A$ and $U_B$ to verify
/// whether $U_B = e^{i\theta} U_A$ holds under **one single common global phase** $\theta$.
#[derive(Clone, Debug)]
pub struct EquivalenceChecker {
    pub(crate) tolerances: VerificationTolerances,
    pub(crate) max_matrix_qubits: usize,
    pub(crate) random_states_count: usize,
    pub(crate) environment: HashMap<String, f64>,
}

impl Default for EquivalenceChecker {
    fn default() -> Self {
        Self {
            tolerances: VerificationTolerances::default(),
            max_matrix_qubits: 5,
            random_states_count: 20,
            environment: HashMap::new(),
        }
    }
}

impl EquivalenceChecker {
    /// Creates a new `EquivalenceChecker` with default tolerances and configurations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets custom numerical verification tolerances.
    pub fn with_tolerances(mut self, tolerances: VerificationTolerances) -> Self {
        self.tolerances = tolerances;
        self
    }

    /// Sets the maximum qubit count for full computational-basis matrix reconstruction (default: 5).
    pub fn with_max_matrix_qubits(mut self, max_qubits: usize) -> Self {
        self.max_matrix_qubits = max_qubits;
        self
    }

    /// Sets the number of independent random states to test for broader systems (default: 20).
    pub fn with_random_states_count(mut self, count: usize) -> Self {
        self.random_states_count = count;
        self
    }

    /// Binds a named symbolic parameter to a numeric float value.
    pub fn with_parameter(mut self, name: impl Into<String>, value: f64) -> Self {
        self.environment.insert(name.into(), value);
        self
    }

    /// Sets the entire parameter evaluation environment.
    pub fn with_environment(mut self, env: HashMap<String, f64>) -> Self {
        self.environment = env;
        self
    }

    /// Returns the active verification tolerances.
    pub fn tolerances(&self) -> &VerificationTolerances {
        &self.tolerances
    }

    /// Validates that a circuit is strictly unitary:
    /// - Structural/semantic IR validation succeeds.
    /// - Declares at least 1 qubit.
    /// - Contains zero non-unitary operations (measurements, resets, or classical computation).
    ///
    /// # Semantic Note on Barriers
    /// An [`Operation::Barrier`] is a compiler scheduling and reordering constraint.
    /// Because a barrier performs no physical interaction on qubit states and acts as the
    /// identity operator $I$ on quantum statevectors ($I^\dagger I = I$), it is mathematically
    /// unitary and safely treated as a semantic identity no-op during statevector evolution.
    pub fn validate_unitary_circuit(&self, circuit: &Circuit) -> Result<(), VerificationError> {
        if circuit.num_qubits() == 0 {
            return Err(VerificationError::ZeroQubits);
        }

        circuit.validate()?;

        for node in circuit.graph.nodes() {
            match &node.operation {
                Operation::Measure { .. } => {
                    return Err(VerificationError::NonUnitaryOperation {
                        op_id: Some(node.id),
                        operation: "measure".to_string(),
                        reason: "circuit contains quantum measurement, which collapses statevectors and is non-unitary".to_string(),
                    });
                }
                Operation::Reset { .. } => {
                    return Err(VerificationError::NonUnitaryOperation {
                        op_id: Some(node.id),
                        operation: "reset".to_string(),
                        reason: "circuit contains active reset, which is an irreversible non-unitary projection".to_string(),
                    });
                }
                Operation::Classical { name, .. } => {
                    return Err(VerificationError::NonUnitaryOperation {
                        op_id: Some(node.id),
                        operation: "classical".to_string(),
                        reason: format!("circuit contains classical computation '{name}', which cannot be verified unitarily"),
                    });
                }
                Operation::Gate { .. } | Operation::Barrier { .. } => {}
            }
        }

        Ok(())
    }

    /// Evolve an arbitrary normalized complex input statevector through the unitary circuit.
    pub fn evolve_state(
        &self,
        circuit: &Circuit,
        input: &StateVector,
    ) -> Result<StateVector, VerificationError> {
        self.validate_unitary_circuit(circuit)?;

        let num_qubits = circuit.num_qubits();
        if input.num_qubits() != num_qubits {
            return Err(VerificationError::InputQubitMismatch {
                circuit: num_qubits,
                input: input.num_qubits(),
            });
        }

        if !input.is_normalized(self.tolerances.norm_tolerance) {
            let norm_sqr = input.norm().powi(2);
            return Err(VerificationError::UnnormalizedInput {
                norm_sqr,
                tolerance: self.tolerances.norm_tolerance,
            });
        }

        let executor = CircuitExecutor::new().with_environment(self.environment.clone());
        let (output_state, _trace) = executor.execute_with_initial_state(circuit, input.clone())?;
        Ok(output_state)
    }

    /// Compares two circuits by exhaustive computational-basis matrix reconstruction.
    ///
    /// Evaluates both circuits on all $2^n$ basis states $|0\rangle, \dots, |2^n - 1\rangle$,
    /// reconstructs the unitary operators column-by-column, estimates one common phase $\theta$,
    /// and checks element-wise matrix and pointwise amplitude differences against tolerances.
    pub fn check_computational_basis(
        &self,
        circ_a: &Circuit,
        circ_b: &Circuit,
    ) -> Result<EquivalenceVerdict, VerificationError> {
        self.validate_unitary_circuit(circ_a)?;
        self.validate_unitary_circuit(circ_b)?;

        let n = circ_a.num_qubits();
        if n != circ_b.num_qubits() {
            return Err(VerificationError::QubitCountMismatch {
                circuit_a: n,
                circuit_b: circ_b.num_qubits(),
            });
        }

        if n > self.max_matrix_qubits {
            return Err(VerificationError::SystemTooLargeForMatrix {
                qubits: n,
                max_qubits: self.max_matrix_qubits,
            });
        }

        let dim = 1usize << n;
        let mut outputs_a = Vec::with_capacity(dim);
        let mut outputs_b = Vec::with_capacity(dim);

        for basis_idx in 0..dim {
            let basis_state = StateVector::from_basis_state(n, basis_idx)?;
            let out_a = self.evolve_state(circ_a, &basis_state)?;
            let out_b = self.evolve_state(circ_b, &basis_state)?;
            outputs_a.push(out_a);
            outputs_b.push(out_b);
        }

        let pairs: Vec<(&StateVector, &StateVector)> =
            outputs_a.iter().zip(outputs_b.iter()).collect();

        self.evaluate_pairs_equivalence(
            &pairs,
            n,
            VerificationMethod::ComputationalBasis {
                num_basis_states: dim,
            },
            true,
        )
    }

    /// Compares two circuits across an arbitrary collection of normalized complex test statevectors.
    pub fn check_randomized_states(
        &self,
        circ_a: &Circuit,
        circ_b: &Circuit,
        test_states: &[StateVector],
    ) -> Result<EquivalenceVerdict, VerificationError> {
        self.validate_unitary_circuit(circ_a)?;
        self.validate_unitary_circuit(circ_b)?;

        let n = circ_a.num_qubits();
        if n != circ_b.num_qubits() {
            return Err(VerificationError::QubitCountMismatch {
                circuit_a: n,
                circuit_b: circ_b.num_qubits(),
            });
        }

        let mut outputs_a = Vec::with_capacity(test_states.len());
        let mut outputs_b = Vec::with_capacity(test_states.len());

        for state in test_states {
            let out_a = self.evolve_state(circ_a, state)?;
            let out_b = self.evolve_state(circ_b, state)?;
            outputs_a.push(out_a);
            outputs_b.push(out_b);
        }

        let pairs: Vec<(&StateVector, &StateVector)> =
            outputs_a.iter().zip(outputs_b.iter()).collect();

        self.evaluate_pairs_equivalence(
            &pairs,
            n,
            VerificationMethod::RandomizedStates {
                num_states: test_states.len(),
            },
            false,
        )
    }

    /// Compares two circuits using pseudorandom normalized complex statevectors seeded deterministically.
    pub fn check_randomized_with_seed(
        &self,
        circ_a: &Circuit,
        circ_b: &Circuit,
        num_states: usize,
        seed: u64,
    ) -> Result<EquivalenceVerdict, VerificationError> {
        let n = circ_a.num_qubits();
        let mut rng = SplitMix64::new(seed);
        let mut states = Vec::with_capacity(num_states);

        for _ in 0..num_states {
            states.push(generate_random_statevector(n, &mut rng)?);
        }

        self.check_randomized_states(circ_a, circ_b, &states)
    }

    /// Automatically checks equivalence between two circuits:
    /// - For systems with $n \le \text{max\_matrix\_qubits}$, uses exhaustive computational basis matrix reconstruction.
    /// - For systems with $n > \text{max\_matrix\_qubits}$, uses randomized arbitrary complex statevectors.
    pub fn check_equivalence(
        &self,
        circ_a: &Circuit,
        circ_b: &Circuit,
    ) -> Result<EquivalenceVerdict, VerificationError> {
        let n = circ_a.num_qubits();
        if n <= self.max_matrix_qubits {
            self.check_computational_basis(circ_a, circ_b)
        } else {
            self.check_randomized_with_seed(circ_a, circ_b, self.random_states_count, 0x1234_5678)
        }
    }

    pub(crate) fn evaluate_pairs_equivalence(
        &self,
        pairs: &[(&StateVector, &StateVector)],
        qubits: usize,
        method: VerificationMethod,
        is_matrix_mode: bool,
    ) -> Result<EquivalenceVerdict, VerificationError> {
        let phase_res = GlobalPhaseEstimator::estimate_common_phase(pairs, &self.tolerances);

        let (theta, phase_factor, max_phase_dev) = match phase_res {
            PhaseEstimationResult::Success {
                estimated_phase,
                phase_factor,
                max_phase_deviation,
                ..
            } => (estimated_phase, phase_factor, max_phase_deviation),
            PhaseEstimationResult::IncoherentOrOrthogonal { mean_overlap } => {
                // States have near-zero aggregate overlap; cannot be equivalent up to any common phase
                let (first_ref, first_cand) = pairs[0];
                let worst = MismatchDiagnostic {
                    input_index: 0,
                    output_amplitude_index: 0,
                    reference_amplitude: first_ref.amplitudes()[0],
                    candidate_amplitude: first_cand.amplitudes()[0],
                    expected_candidate_amplitude: Complex64::new(0.0, 0.0),
                    amplitude_difference: 1.0,
                    exceeded_tolerance: format!(
                        "incoherent / near-zero aggregate overlap: {mean_overlap:.3e}"
                    ),
                };

                return Ok(EquivalenceVerdict {
                    is_equivalent: false,
                    qubits,
                    method,
                    estimated_phase: None,
                    phase_factor: None,
                    max_amplitude_difference: 1.0,
                    matrix_frobenius_error: None,
                    worst_mismatch: Some(worst),
                    tolerances: self.tolerances,
                });
            }
            PhaseEstimationResult::EmptyInputs => {
                return Ok(EquivalenceVerdict {
                    is_equivalent: true,
                    qubits,
                    method,
                    estimated_phase: Some(0.0),
                    phase_factor: Some(Complex64::new(1.0, 0.0)),
                    max_amplitude_difference: 0.0,
                    matrix_frobenius_error: Some(0.0),
                    worst_mismatch: None,
                    tolerances: self.tolerances,
                });
            }
        };

        let mut max_diff = 0.0;
        let mut frobenius_sqr = 0.0;
        let mut worst_mismatch = None;

        for (k, &(ref_state, cand_state)) in pairs.iter().enumerate() {
            let ref_amps = ref_state.amplitudes();
            let cand_amps = cand_state.amplitudes();
            let dim = ref_amps.len();

            for j in 0..dim {
                let a = ref_amps[j];
                let b = cand_amps[j];
                let expected_b = phase_factor * a;
                let diff = (b - expected_b).norm();

                frobenius_sqr += diff * diff;

                if diff > max_diff {
                    max_diff = diff;
                    worst_mismatch = Some(MismatchDiagnostic {
                        input_index: k,
                        output_amplitude_index: j,
                        reference_amplitude: a,
                        candidate_amplitude: b,
                        expected_candidate_amplitude: expected_b,
                        amplitude_difference: diff,
                        exceeded_tolerance: if is_matrix_mode
                            && diff > self.tolerances.matrix_tolerance
                        {
                            format!(
                                "matrix_tolerance ({:.1e})",
                                self.tolerances.matrix_tolerance
                            )
                        } else {
                            format!("state_tolerance ({:.1e})", self.tolerances.state_tolerance)
                        },
                    });
                }
            }
        }

        let matrix_frobenius_error = if is_matrix_mode {
            Some(frobenius_sqr.sqrt())
        } else {
            None
        };

        // Determine equivalence
        let mut is_equivalent = true;

        // 1. Pointwise statevector amplitude check
        if max_diff > self.tolerances.state_tolerance {
            is_equivalent = false;
        }

        // 2. Matrix reconstruction element check (if matrix mode)
        if is_matrix_mode && max_diff > self.tolerances.matrix_tolerance {
            is_equivalent = false;
        }

        // 3. Global phase coherence check
        if max_phase_dev > self.tolerances.phase_tolerance {
            is_equivalent = false;
        }

        Ok(EquivalenceVerdict {
            is_equivalent,
            qubits,
            method,
            estimated_phase: Some(theta),
            phase_factor: Some(phase_factor),
            max_amplitude_difference: max_diff,
            matrix_frobenius_error,
            worst_mismatch: if is_equivalent { None } else { worst_mismatch },
            tolerances: self.tolerances,
        })
    }
}

/// Convenience function checking unitary equivalence between two circuits with default options.
pub fn check_equivalence(
    circ_a: &Circuit,
    circ_b: &Circuit,
) -> Result<EquivalenceVerdict, VerificationError> {
    EquivalenceChecker::new().check_equivalence(circ_a, circ_b)
}
