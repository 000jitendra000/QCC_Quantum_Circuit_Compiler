//! Decomposition templates for CX <-> CZ transformations using Hadamards.
//!
//! Conjugating the target qubit of a CX gate with Hadamards transforms it into a symmetric CZ gate,
//! and vice-versa:
//!
//! $$CX(c, t) = (I_c \otimes H_t) \cdot CZ(c, t) \cdot (I_c \otimes H_t)$$
//! $$CZ(c, t) = (I_c \otimes H_t) \cdot CX(c, t) \cdot (I_c \otimes H_t)$$

use crate::ir::operation::Operation;
use crate::ir::wire::QubitId;
use crate::quantum::gate_matrix::{
    kron_2x2, matrix_cx_c0_t1, matrix_cx_c1_t0, matrix_cz, matrix_h, Matrix2x2, Matrix4x4,
};
use crate::synthesis::templates::{DecompositionTemplate, Precondition, TemplateError, TemplateId};

const PRECONDITIONS: &[Precondition] = &[
    Precondition::DistinctOperands,
    Precondition::QuantumArity(2),
    Precondition::ZeroParameters,
    Precondition::UnitaryTarget,
];

/// Exact decomposition of two-qubit `CX(c, t)` into `H(t); CZ(c, t); H(t)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CxToCzTemplate;

impl CxToCzTemplate {
    /// Numerical tolerance for local matrix verification.
    pub const TOLERANCE: f64 = 1e-12;

    /// Creates a new instance of [`CxToCzTemplate`].
    pub fn new() -> Self {
        Self
    }

    /// Computes the 4x4 matrix for $c = q_0$ (ctrl) and $t = q_1$ (targ).
    ///
    /// Evaluates $(H \otimes I) \cdot CZ \cdot (H \otimes I)$ where $H$ is on $q_1$.
    pub fn decomposed_matrix_c0_t1() -> Matrix4x4 {
        let h1 = kron_2x2(&matrix_h(), &Matrix2x2::identity()); // H on q1, I on q0
        let cz = matrix_cz();
        // Combined in left-multiplication order: H1 * CZ * H1
        h1.matmul(&cz).matmul(&h1)
    }

    /// Computes the 4x4 matrix for $c = q_1$ (ctrl) and $t = q_0$ (targ).
    ///
    /// Evaluates $(I \otimes H) \cdot CZ \cdot (I \otimes H)$ where $H$ is on $q_0$.
    pub fn decomposed_matrix_c1_t0() -> Matrix4x4 {
        let h0 = kron_2x2(&Matrix2x2::identity(), &matrix_h()); // I on q1, H on q0
        let cz = matrix_cz();
        h0.matmul(&cz).matmul(&h0)
    }
}

impl DecompositionTemplate for CxToCzTemplate {
    fn id(&self) -> TemplateId {
        TemplateId::CxToCz
    }

    fn name(&self) -> &'static str {
        "cx_to_cz"
    }

    fn target_gate(&self) -> &'static str {
        "cx"
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
        let ctrl = operands[0];
        let targ = operands[1];

        // Conjugate the target qubit with Hadamards
        Ok(vec![
            Operation::custom_gate("h", vec![targ], vec![]),
            Operation::custom_gate("cz", vec![ctrl, targ], vec![]),
            Operation::custom_gate("h", vec![targ], vec![]),
        ])
    }

    fn verify_matrix(&self) -> Result<(), TemplateError> {
        // Verify orientation c=0, t=1
        let expected_01 = matrix_cx_c0_t1();
        let actual_01 = Self::decomposed_matrix_c0_t1();
        let diff_01 = actual_01.max_difference(&expected_01);
        if diff_01 > Self::TOLERANCE {
            return Err(TemplateError::VerificationFailed {
                template: format!("{}(c0, t1)", self.name()),
                max_diff: diff_01,
                tolerance: Self::TOLERANCE,
            });
        }

        // Verify orientation c=1, t=0
        let expected_10 = matrix_cx_c1_t0();
        let actual_10 = Self::decomposed_matrix_c1_t0();
        let diff_10 = actual_10.max_difference(&expected_10);
        if diff_10 > Self::TOLERANCE {
            return Err(TemplateError::VerificationFailed {
                template: format!("{}(c1, t0)", self.name()),
                max_diff: diff_10,
                tolerance: Self::TOLERANCE,
            });
        }

        Ok(())
    }
}

/// Exact decomposition of two-qubit `CZ(a, b)` into `H(b); CX(a, b); H(b)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CzToCxTemplate;

impl CzToCxTemplate {
    /// Numerical tolerance for local matrix verification.
    pub const TOLERANCE: f64 = 1e-12;

    /// Creates a new instance of [`CzToCxTemplate`].
    pub fn new() -> Self {
        Self
    }

    /// Evaluates $(H \otimes I) \cdot CX_{01} \cdot (H \otimes I)$ with $H$ on $q_1$.
    pub fn decomposed_matrix_target_q1() -> Matrix4x4 {
        let h1 = kron_2x2(&matrix_h(), &Matrix2x2::identity()); // H on q1
        let cx01 = matrix_cx_c0_t1(); // ctrl q0, targ q1
        h1.matmul(&cx01).matmul(&h1)
    }

    /// Evaluates $(I \otimes H) \cdot CX_{10} \cdot (I \otimes H)$ with $H$ on $q_0$.
    pub fn decomposed_matrix_target_q0() -> Matrix4x4 {
        let h0 = kron_2x2(&Matrix2x2::identity(), &matrix_h()); // H on q0
        let cx10 = matrix_cx_c1_t0(); // ctrl q1, targ q0
        h0.matmul(&cx10).matmul(&h0)
    }
}

impl DecompositionTemplate for CzToCxTemplate {
    fn id(&self) -> TemplateId {
        TemplateId::CzToCx
    }

    fn name(&self) -> &'static str {
        "cz_to_cx"
    }

    fn target_gate(&self) -> &'static str {
        "cz"
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

        // Standard convention: conjugate operands[1] with Hadamards and use operands[0] as control
        Ok(vec![
            Operation::custom_gate("h", vec![b], vec![]),
            Operation::custom_gate("cx", vec![a, b], vec![]),
            Operation::custom_gate("h", vec![b], vec![]),
        ])
    }

    fn verify_matrix(&self) -> Result<(), TemplateError> {
        let expected = matrix_cz();

        let actual_q1 = Self::decomposed_matrix_target_q1();
        let diff_q1 = actual_q1.max_difference(&expected);
        if diff_q1 > Self::TOLERANCE {
            return Err(TemplateError::VerificationFailed {
                template: format!("{}(target=q1)", self.name()),
                max_diff: diff_q1,
                tolerance: Self::TOLERANCE,
            });
        }

        let actual_q0 = Self::decomposed_matrix_target_q0();
        let diff_q0 = actual_q0.max_difference(&expected);
        if diff_q0 > Self::TOLERANCE {
            return Err(TemplateError::VerificationFailed {
                template: format!("{}(target=q0)", self.name()),
                max_diff: diff_q0,
                tolerance: Self::TOLERANCE,
            });
        }

        Ok(())
    }
}
