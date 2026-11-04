//! Layout-aware unitary equivalence checker.
//!
//! Validates routed physical circuits against unrouted logical reference circuits
//! accounting for:
//! 1. Initial logical-to-physical layout ($L_{\text{initial}}$ at circuit entry $t=0$).
//! 2. Final logical-to-physical layout ($L_{\text{final}}$ at circuit exit $t=T_{\text{end}}$).
//! 3. ADR-0001 strict ancilla zero-state policy: Every physical qubit not occupied
//!    by a logical qubit starts strictly in $|0\rangle$ and must return to $|0\rangle$
//!    at circuit termination without residual leakage or entanglement.
//! 4. Unitary equivalence up to exactly one single common global phase across all test states.

use std::collections::HashMap;

use num_complex::Complex64;

use crate::ir::circuit::Circuit;
use crate::ir::wire::QubitId;
use crate::routing::{CircuitLayout, Layout, LayoutError};
use crate::simulator::executor::CircuitExecutor;
use crate::simulator::sampling::SplitMix64;
use crate::simulator::statevector::StateVector;
use crate::testkit::random_circuit::generate_random_statevector;
use crate::verification::tolerance::VerificationTolerances;
use crate::verification::unitary::{
    EquivalenceChecker, EquivalenceVerdict, MismatchDiagnostic, VerificationError,
    VerificationMethod,
};

/// Embeds a $2^m$-dimensional logical statevector into a full $2^n$-dimensional physical register
/// under the given [`Layout`], setting all unassigned physical qubits strictly to $|0\rangle$.
///
/// # Little-Endian Convention
/// Logical qubit `q[j]` (corresponding to bit `j` in the logical basis state index)
/// maps to physical qubit `p_j = layout.get_physical(q[j])` (corresponding to bit `p_j`
/// in the physical basis state index).
///
/// Any physical qubit not mapped by `layout` has bit value 0 across all basis states,
/// guaranteeing the $|0\rangle$ ancilla condition required by ADR-0001.
pub fn embed_logical_state(
    logical_state: &StateVector,
    layout: &Layout,
) -> Result<StateVector, VerificationError> {
    layout.validate()?;

    let m = layout.num_logical();
    let n = layout.num_physical();

    if logical_state.num_qubits() != m {
        return Err(VerificationError::InputQubitMismatch {
            circuit: m,
            input: logical_state.num_qubits(),
        });
    }

    if !layout.is_fully_assigned() {
        return Err(LayoutError::InvalidState(format!(
            "cannot embed state: layout has {} unassigned logical qubits",
            m - layout.num_assigned()
        ))
        .into());
    }

    if m > n {
        return Err(LayoutError::CapacityExceeded {
            required: m,
            capacity: n,
        }
        .into());
    }

    let dim_phys = 1usize << n;
    let mut physical_amplitudes = vec![Complex64::new(0.0, 0.0); dim_phys];

    // Precompute physical bit index for each logical qubit
    let shifts: Vec<usize> = (0..m)
        .map(|j| layout.get_physical(QubitId(j)).unwrap().index())
        .collect();

    for (logical_idx, &amp) in logical_state.amplitudes().iter().enumerate() {
        if amp != Complex64::new(0.0, 0.0) {
            let mut phys_idx = 0usize;
            for (logical_qubit, &shift) in shifts.iter().enumerate().take(m) {
                if (logical_idx >> logical_qubit) & 1 == 1 {
                    phys_idx |= 1usize << shift;
                }
            }
            physical_amplitudes[phys_idx] = amp;
        }
    }

    let embedded = StateVector::from_amplitudes(n, physical_amplitudes)?;
    Ok(embedded)
}

/// Layout-aware unitary equivalence checker.
///
/// Evaluates whether a physical routed circuit $C_{\text{routed}}$ correctly implements
/// the logical transformation of $C_{\text{logical}}$ under an initial layout $L_{\text{initial}}$
/// and a final layout $L_{\text{final}}$, up to a single common scalar global phase $e^{i\theta}$.
///
/// Validates ADR-0001: all unused physical qubits must start in $|0\rangle$ and return to $|0\rangle$.
#[derive(Clone, Debug, Default)]
pub struct LayoutEquivalenceChecker {
    checker: EquivalenceChecker,
}

impl LayoutEquivalenceChecker {
    /// Creates a new `LayoutEquivalenceChecker` with default tolerances and configurations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets custom numerical verification tolerances.
    pub fn with_tolerances(mut self, tolerances: VerificationTolerances) -> Self {
        self.checker = self.checker.with_tolerances(tolerances);
        self
    }

    /// Sets the maximum logical qubit count for exhaustive computational-basis verification (default: 5).
    pub fn with_max_matrix_qubits(mut self, max_qubits: usize) -> Self {
        self.checker = self.checker.with_max_matrix_qubits(max_qubits);
        self
    }

    /// Sets the number of independent random states to test for larger circuits (default: 20).
    pub fn with_random_states_count(mut self, count: usize) -> Self {
        self.checker = self.checker.with_random_states_count(count);
        self
    }

    /// Binds a named symbolic parameter to a numeric float value.
    pub fn with_parameter(mut self, name: impl Into<String>, value: f64) -> Self {
        self.checker = self.checker.with_parameter(name, value);
        self
    }

    /// Sets the entire parameter evaluation environment.
    pub fn with_environment(mut self, env: HashMap<String, f64>) -> Self {
        self.checker = self.checker.with_environment(env);
        self
    }

    /// Returns the active verification tolerances.
    pub fn tolerances(&self) -> &VerificationTolerances {
        self.checker.tolerances()
    }

    /// Checks layout-aware equivalence between a logical circuit and a routed physical circuit
    /// using a [`CircuitLayout`] containing $L_{\text{initial}}$ and $L_{\text{final}}$.
    pub fn check_layout_equivalence(
        &self,
        circ_logical: &Circuit,
        circ_routed: &Circuit,
        layout: &CircuitLayout,
    ) -> Result<EquivalenceVerdict, VerificationError> {
        self.check_layout_equivalence_with_layouts(
            circ_logical,
            circ_routed,
            layout.initial(),
            layout.final_layout(),
        )
    }

    /// Checks layout-aware equivalence with explicit initial and final layouts.
    ///
    /// For each test input state:
    /// 1. Embeds the logical input into the physical register using `initial_layout`, with unused ancillae in $|0\rangle$.
    /// 2. Simulates `circ_routed` on the physical register to produce the actual physical output state.
    /// 3. Simulates `circ_logical` on the logical input, and embeds the logical output into the physical register
    ///    using `final_layout`, with unused ancillae in $|0\rangle$, producing the expected physical output state.
    /// 4. Compares the complete $2^n$-dimensional actual and expected physical statevectors across all test inputs,
    ///    estimating a single common global phase $e^{i\theta}$.
    pub fn check_layout_equivalence_with_layouts(
        &self,
        circ_logical: &Circuit,
        circ_routed: &Circuit,
        initial_layout: &Layout,
        final_layout: &Layout,
    ) -> Result<EquivalenceVerdict, VerificationError> {
        self.checker.validate_unitary_circuit(circ_logical)?;
        self.checker.validate_unitary_circuit(circ_routed)?;

        let m = circ_logical.num_qubits();
        let n = circ_routed.num_qubits();

        if initial_layout.num_logical() != m {
            return Err(LayoutError::CapacityMismatch {
                expected: m,
                actual: initial_layout.num_logical(),
            }
            .into());
        }
        if initial_layout.num_physical() != n {
            return Err(LayoutError::CapacityMismatch {
                expected: n,
                actual: initial_layout.num_physical(),
            }
            .into());
        }
        if final_layout.num_logical() != m {
            return Err(LayoutError::CapacityMismatch {
                expected: m,
                actual: final_layout.num_logical(),
            }
            .into());
        }
        if final_layout.num_physical() != n {
            return Err(LayoutError::CapacityMismatch {
                expected: n,
                actual: final_layout.num_physical(),
            }
            .into());
        }
        if n < m {
            return Err(LayoutError::CapacityExceeded {
                required: m,
                capacity: n,
            }
            .into());
        }

        initial_layout.validate()?;
        final_layout.validate()?;

        if !initial_layout.is_fully_assigned() {
            return Err(LayoutError::InvalidState(
                "initial layout is not fully assigned for all logical qubits".to_string(),
            )
            .into());
        }
        if !final_layout.is_fully_assigned() {
            return Err(LayoutError::InvalidState(
                "final layout is not fully assigned for all logical qubits".to_string(),
            )
            .into());
        }

        let executor = CircuitExecutor::new().with_environment(self.checker.environment.clone());

        let (verdict, actual_outputs) = if m <= self.checker.max_matrix_qubits {
            let dim_logical = 1usize << m;
            let mut expected_outputs = Vec::with_capacity(dim_logical);
            let mut actual_outputs = Vec::with_capacity(dim_logical);

            for basis_idx in 0..dim_logical {
                let logical_in = StateVector::from_basis_state(m, basis_idx)?;
                let (logical_ref_out, _) =
                    executor.execute_with_initial_state(circ_logical, logical_in.clone())?;
                let exp_phys_out = embed_logical_state(&logical_ref_out, final_layout)?;

                let phys_in = embed_logical_state(&logical_in, initial_layout)?;
                let (act_phys_out, _) =
                    executor.execute_with_initial_state(circ_routed, phys_in)?;

                expected_outputs.push(exp_phys_out);
                actual_outputs.push(act_phys_out);
            }

            let pairs: Vec<(&StateVector, &StateVector)> =
                expected_outputs.iter().zip(actual_outputs.iter()).collect();

            let verdict = self.checker.evaluate_pairs_equivalence(
                &pairs,
                n,
                VerificationMethod::ComputationalBasis {
                    num_basis_states: dim_logical,
                },
                true,
            )?;
            (verdict, actual_outputs)
        } else {
            let count = self.checker.random_states_count;
            let mut rng = SplitMix64::new(0x1234_5678);
            let mut expected_outputs = Vec::with_capacity(count);
            let mut actual_outputs = Vec::with_capacity(count);

            for _ in 0..count {
                let logical_in = generate_random_statevector(m, &mut rng)?;
                let (logical_ref_out, _) =
                    executor.execute_with_initial_state(circ_logical, logical_in.clone())?;
                let exp_phys_out = embed_logical_state(&logical_ref_out, final_layout)?;

                let phys_in = embed_logical_state(&logical_in, initial_layout)?;
                let (act_phys_out, _) =
                    executor.execute_with_initial_state(circ_routed, phys_in)?;

                expected_outputs.push(exp_phys_out);
                actual_outputs.push(act_phys_out);
            }

            let pairs: Vec<(&StateVector, &StateVector)> =
                expected_outputs.iter().zip(actual_outputs.iter()).collect();

            let verdict = self.checker.evaluate_pairs_equivalence(
                &pairs,
                n,
                VerificationMethod::RandomizedStates { num_states: count },
                false,
            )?;
            (verdict, actual_outputs)
        };

        let mut final_verdict = verdict;
        let unused_qubits = final_layout.unused_physical_qubits();

        // Check for any ADR-0001 ancilla leakage across actual physical outputs
        let mut ancilla_violation = None;
        for (input_idx, act) in actual_outputs.iter().enumerate() {
            for &p in &unused_qubits {
                let p_idx = p.index();
                let mut leak_prob = 0.0;
                let mut max_leak_amp = 0.0;
                let mut leak_k = 0;
                for (k, amp) in act.amplitudes().iter().enumerate() {
                    if (k >> p_idx) & 1 == 1 {
                        let mag_sq = amp.norm_sqr();
                        leak_prob += mag_sq;
                        if amp.norm() > max_leak_amp {
                            max_leak_amp = amp.norm();
                            leak_k = k;
                        }
                    }
                }
                if leak_prob > self.tolerances().state_tolerance {
                    ancilla_violation =
                        Some((input_idx, p, leak_prob, leak_k, act.amplitudes()[leak_k]));
                    break;
                }
            }
            if ancilla_violation.is_some() {
                break;
            }
        }

        if let Some((in_idx, p, leak_prob, leak_k, cand_amp)) = ancilla_violation {
            final_verdict.is_equivalent = false;
            final_verdict.worst_mismatch = Some(MismatchDiagnostic {
                input_index: in_idx,
                output_amplitude_index: leak_k,
                reference_amplitude: Complex64::new(0.0, 0.0),
                candidate_amplitude: cand_amp,
                expected_candidate_amplitude: Complex64::new(0.0, 0.0),
                amplitude_difference: cand_amp.norm(),
                exceeded_tolerance: format!(
                    "ADR-0001 ancilla zero-state violation on {p}: expected |0>, found leakage population {leak_prob:.3e}"
                ),
            });
        }

        Ok(final_verdict)
    }
}

/// Convenience function checking layout-aware unitary equivalence with default configurations.
pub fn check_layout_equivalence(
    circ_logical: &Circuit,
    circ_routed: &Circuit,
    layout: &CircuitLayout,
) -> Result<EquivalenceVerdict, VerificationError> {
    LayoutEquivalenceChecker::new().check_layout_equivalence(circ_logical, circ_routed, layout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::CircuitBuilder;
    use crate::target::operation::PhysicalQubitId;

    #[test]
    fn test_embed_logical_state_little_endian() {
        // Logical state: |01> (q[0] = 1, q[1] = 0)
        let log_state = StateVector::from_basis_state(2, 1).unwrap();
        // Layout: q[0] -> p[2], q[1] -> p[0], p[1] unused (idle) on 3-qubit device
        let layout = Layout::from_mappings(
            2,
            3,
            [
                (QubitId(0), PhysicalQubitId(2)),
                (QubitId(1), PhysicalQubitId(0)),
            ],
        )
        .unwrap();

        let phys_state = embed_logical_state(&log_state, &layout).unwrap();
        assert_eq!(phys_state.num_qubits(), 3);
        // In physical state, p[0]=0, p[1]=0, p[2]=1 => basis state |100> (index 4)
        assert_eq!(phys_state.amplitudes()[4], Complex64::new(1.0, 0.0));
        // All other amplitudes must be 0
        for (i, &amp) in phys_state.amplitudes().iter().enumerate() {
            if i != 4 {
                assert_eq!(amp, Complex64::new(0.0, 0.0));
            }
        }
    }

    #[test]
    fn test_layout_equivalence_identity() {
        let mut b1 = CircuitBuilder::new("log");
        let q1 = b1.add_quantum_register("q", 2).unwrap();
        b1.h(q1[0]).unwrap();
        b1.cx(q1[0], q1[1]).unwrap();
        let circ_log = b1.finish().unwrap();

        let mut b2 = CircuitBuilder::new("phys");
        let p2 = b2.add_quantum_register("p", 2).unwrap();
        b2.h(p2[0]).unwrap();
        b2.cx(p2[0], p2[1]).unwrap();
        let circ_phys = b2.finish().unwrap();

        let cl = CircuitLayout::identity(2, 2).unwrap();
        let verdict = check_layout_equivalence(&circ_log, &circ_phys, &cl).unwrap();
        assert!(verdict.is_equivalent);
    }
}
