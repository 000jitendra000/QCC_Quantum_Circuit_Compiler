//! Decomposition template for SWAP into three alternating CX gates.
//!
//! $$\text{SWAP}(a, b) = CX(a, b) \cdot CX(b, a) \cdot CX(a, b)$$

use crate::ir::operation::Operation;
use crate::ir::wire::QubitId;
use crate::quantum::gate_matrix::{matrix_cx_c0_t1, matrix_cx_c1_t0, matrix_swap, Matrix4x4};
use crate::synthesis::templates::{DecompositionTemplate, Precondition, TemplateError, TemplateId};

const PRECONDITIONS: &[Precondition] = &[
    Precondition::DistinctOperands,
    Precondition::QuantumArity(2),
    Precondition::ZeroParameters,
    Precondition::UnitaryTarget,
];

/// Exact decomposition of two-qubit `SWAP` into three alternating `CX` gates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwapToCxTemplate;

impl SwapToCxTemplate {
    /// Numerical tolerance for local matrix verification.
    pub const TOLERANCE: f64 = 1e-12;

    /// Creates a new instance of [`SwapToCxTemplate`].
    pub fn new() -> Self {
        Self
    }

    /// Computes the exact 4x4 unitary operator matrix for the decomposed CX sequence.
    ///
    /// Evaluates $CX(a, b) \cdot CX(b, a) \cdot CX(a, b)$ with $a = q_0$ and $b = q_1$.
    pub fn decomposed_matrix() -> Matrix4x4 {
        let cx01 = matrix_cx_c0_t1();
        let cx10 = matrix_cx_c1_t0();
        // Combined in left-multiplication execution order: Op3 * Op2 * Op1
        cx01.matmul(&cx10).matmul(&cx01)
    }

    /// Evaluates the decomposed matrix for the reverse orientation: $a = q_1, b = q_0$.
    pub fn decomposed_matrix_reversed() -> Matrix4x4 {
        let cx01 = matrix_cx_c0_t1();
        let cx10 = matrix_cx_c1_t0();
        cx10.matmul(&cx01).matmul(&cx10)
    }
}

impl DecompositionTemplate for SwapToCxTemplate {
    fn id(&self) -> TemplateId {
        TemplateId::SwapToCx
    }

    fn name(&self) -> &'static str {
        "swap_to_cx"
    }

    fn target_gate(&self) -> &'static str {
        "swap"
    }

    fn quantum_arity(&self) -> usize {
        2
    }

    fn phase_contribution(&self) -> f64 {
        0.0
    }

    fn preconditions(&self) -> &'static [Precondition] {
        PRECONDITIONS
    }

    fn expand(&self, operands: &[QubitId]) -> Result<Vec<Operation>, TemplateError> {
        self.check_preconditions(operands)?;
        let a = operands[0];
        let b = operands[1];

        Ok(vec![
            Operation::custom_gate("cx", vec![a, b], vec![]),
            Operation::custom_gate("cx", vec![b, a], vec![]),
            Operation::custom_gate("cx", vec![a, b], vec![]),
        ])
    }

    fn verify_matrix(&self) -> Result<(), TemplateError> {
        let expected = matrix_swap();
        let actual_forward = Self::decomposed_matrix();
        let diff_fwd = actual_forward.max_difference(&expected);
        if diff_fwd > Self::TOLERANCE {
            return Err(TemplateError::VerificationFailed {
                template: self.name().to_string(),
                max_diff: diff_fwd,
                tolerance: Self::TOLERANCE,
            });
        }

        let actual_rev = Self::decomposed_matrix_reversed();
        let diff_rev = actual_rev.max_difference(&expected);
        if diff_rev > Self::TOLERANCE {
            return Err(TemplateError::VerificationFailed {
                template: self.name().to_string(),
                max_diff: diff_rev,
                tolerance: Self::TOLERANCE,
            });
        }

        Ok(())
    }
}
