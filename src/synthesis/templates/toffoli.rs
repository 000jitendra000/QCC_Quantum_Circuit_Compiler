//! Exact Toffoli (CCX) decomposition into single-qubit gates (H, T, Tdg) and six CX gates.
//!
//! Implements the exact canonical 15-gate decomposition from Nielsen & Chuang (Figure 4.9, p. 182).
//!
//! Unlike relative-phase Toffoli approximations (such as the Margolus gate), this decomposition
//! implements the exact Toffoli unitary with **zero relative-phase errors** and **zero global-phase delta**
//! across all $2^3 = 8$ computational basis states.

use crate::ir::operation::Operation;
use crate::ir::wire::QubitId;
use crate::quantum::gate_matrix::{matrix_h, matrix_t, matrix_tdg};
use crate::synthesis::templates::matrix::Matrix8x8;
use crate::synthesis::templates::{DecompositionTemplate, Precondition, TemplateError, TemplateId};

const PRECONDITIONS: &[Precondition] = &[
    Precondition::DistinctOperands,
    Precondition::QuantumArity(3),
    Precondition::ZeroParameters,
    Precondition::UnitaryTarget,
];

/// Exact decomposition of three-qubit `Toffoli(c1, c2, t)` into 6 CX, 2 H, 4 T, and 3 Tdg gates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToffoliToCxTemplate;

impl ToffoliToCxTemplate {
    /// Numerical tolerance for local matrix verification.
    pub const TOLERANCE: f64 = 1e-12;

    /// Creates a new instance of [`ToffoliToCxTemplate`].
    pub fn new() -> Self {
        Self
    }

    /// Evaluates the complete 8x8 unitary operator matrix for the decomposed 15-gate sequence.
    ///
    /// Preserves little-endian indexing where basis index $i = q_0 + 2 q_1 + 4 q_2$.
    pub fn decomposed_matrix(c1: usize, c2: usize, t: usize) -> Matrix8x8 {
        let h = matrix_h();
        let t_gate = matrix_t();
        let tdg = matrix_tdg();

        // 15 constituent gates in sequential execution order:
        // 1. H(t)
        let m1 = Matrix8x8::embed_single_qubit(&h, t);
        // 2. CX(c2, t)
        let m2 = Matrix8x8::embed_cx(c2, t);
        // 3. Tdg(t)
        let m3 = Matrix8x8::embed_single_qubit(&tdg, t);
        // 4. CX(c1, t)
        let m4 = Matrix8x8::embed_cx(c1, t);
        // 5. T(t)
        let m5 = Matrix8x8::embed_single_qubit(&t_gate, t);
        // 6. CX(c2, t)
        let m6 = Matrix8x8::embed_cx(c2, t);
        // 7. Tdg(t)
        let m7 = Matrix8x8::embed_single_qubit(&tdg, t);
        // 8. CX(c1, t)
        let m8 = Matrix8x8::embed_cx(c1, t);
        // 9. T(c2)
        let m9 = Matrix8x8::embed_single_qubit(&t_gate, c2);
        // 10. T(t)
        let m10 = Matrix8x8::embed_single_qubit(&t_gate, t);
        // 11. CX(c1, c2)
        let m11 = Matrix8x8::embed_cx(c1, c2);
        // 12. H(t)
        let m12 = Matrix8x8::embed_single_qubit(&h, t);
        // 13. T(c1)
        let m13 = Matrix8x8::embed_single_qubit(&t_gate, c1);
        // 14. Tdg(c2)
        let m14 = Matrix8x8::embed_single_qubit(&tdg, c2);
        // 15. CX(c1, c2)
        let m15 = Matrix8x8::embed_cx(c1, c2);

        // Combined in left-multiplication order: m15 * m14 * ... * m1
        let mut u = Matrix8x8::identity();
        let gates = [
            m1, m2, m3, m4, m5, m6, m7, m8, m9, m10, m11, m12, m13, m14, m15,
        ];
        for g in gates {
            u = g.matmul(&u);
        }
        u
    }
}

impl DecompositionTemplate for ToffoliToCxTemplate {
    fn id(&self) -> TemplateId {
        TemplateId::ToffoliToCx
    }

    fn name(&self) -> &'static str {
        "toffoli_to_cx"
    }

    fn target_gate(&self) -> &'static str {
        "ccx"
    }

    fn quantum_arity(&self) -> usize {
        3
    }

    fn phase_contribution(&self) -> f64 {
        0.0
    }

    fn preconditions(&self) -> &'static [Precondition] {
        PRECONDITIONS
    }

    fn expand(&self, operands: &[QubitId]) -> Result<Vec<Operation>, TemplateError> {
        self.check_preconditions(operands)?;
        let c1 = operands[0];
        let c2 = operands[1];
        let t = operands[2];

        // 15 constituent gates in Nielsen & Chuang Fig 4.9 order:
        Ok(vec![
            Operation::custom_gate("h", vec![t], vec![]),
            Operation::custom_gate("cx", vec![c2, t], vec![]),
            Operation::custom_gate("tdg", vec![t], vec![]),
            Operation::custom_gate("cx", vec![c1, t], vec![]),
            Operation::custom_gate("t", vec![t], vec![]),
            Operation::custom_gate("cx", vec![c2, t], vec![]),
            Operation::custom_gate("tdg", vec![t], vec![]),
            Operation::custom_gate("cx", vec![c1, t], vec![]),
            Operation::custom_gate("t", vec![c2], vec![]),
            Operation::custom_gate("t", vec![t], vec![]),
            Operation::custom_gate("cx", vec![c1, c2], vec![]),
            Operation::custom_gate("h", vec![t], vec![]),
            Operation::custom_gate("t", vec![c1], vec![]),
            Operation::custom_gate("tdg", vec![c2], vec![]),
            Operation::custom_gate("cx", vec![c1, c2], vec![]),
        ])
    }

    fn verify_matrix(&self) -> Result<(), TemplateError> {
        // Test standard orientation c1 = 0, c2 = 1, t = 2
        let expected = Matrix8x8::exact_toffoli(0, 1, 2);
        let actual = Self::decomposed_matrix(0, 1, 2);
        let diff = actual.max_difference(&expected);
        if diff > Self::TOLERANCE {
            return Err(TemplateError::VerificationFailed {
                template: format!("{}(c1=0, c2=1, t=2)", self.name()),
                max_diff: diff,
                tolerance: Self::TOLERANCE,
            });
        }

        // Test control-swapped orientation c1 = 1, c2 = 0, t = 2
        let expected_swapped = Matrix8x8::exact_toffoli(1, 0, 2);
        let actual_swapped = Self::decomposed_matrix(1, 0, 2);
        let diff_swapped = actual_swapped.max_difference(&expected_swapped);
        if diff_swapped > Self::TOLERANCE {
            return Err(TemplateError::VerificationFailed {
                template: format!("{}(c1=1, c2=0, t=2)", self.name()),
                max_diff: diff_swapped,
                tolerance: Self::TOLERANCE,
            });
        }

        // Test target-at-0 orientation c1 = 1, c2 = 2, t = 0
        let expected_t0 = Matrix8x8::exact_toffoli(1, 2, 0);
        let actual_t0 = Self::decomposed_matrix(1, 2, 0);
        let diff_t0 = actual_t0.max_difference(&expected_t0);
        if diff_t0 > Self::TOLERANCE {
            return Err(TemplateError::VerificationFailed {
                template: format!("{}(c1=1, c2=2, t=0)", self.name()),
                max_diff: diff_t0,
                tolerance: Self::TOLERANCE,
            });
        }

        Ok(())
    }
}
