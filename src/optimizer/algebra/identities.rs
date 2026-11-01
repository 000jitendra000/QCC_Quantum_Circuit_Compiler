//! Symbolic identity registry and parameter normalization rules.
//!
//! Categorizes and documents canonical gate identities, distinguishes exact matrix
//! equality from global-phase equivalence, and validates identities against concrete matrices.

use std::f64::consts::PI;

use num_complex::Complex64;

use crate::ir::parameter::ParameterExpr;
use crate::quantum::gate_matrix::{
    kron_2x2, matrix_cx_c0_t1, matrix_cx_c1_t0, matrix_cz, matrix_h, matrix_rx, matrix_ry,
    matrix_rz, matrix_s, matrix_sdg, matrix_swap, matrix_t, matrix_tdg, matrix_x, matrix_y,
    matrix_z, Matrix2x2, Matrix4x4,
};
use crate::quantum::parameter::Angle;

/// Classification of identity relationships.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IdentityCategory {
    /// Cancellation of identical self-inverse gates ($U \cdot U = I$).
    SelfInverseCancellation,
    /// Cancellation of discrete inverse pairs ($U \cdot U^\dagger = I$).
    InversePairCancellation,
    /// Discrete gate power identities (e.g. $S^2 = Z, T^2 = S$).
    PowerEquivalence,
    /// Clifford conjugation relations (e.g. $H X H = Z, S X S^\dagger = Y$).
    PauliConjugation,
    /// Rotation composition along a shared axis ($R_k(\alpha) \cdot R_k(\beta) = R_k(\alpha + \beta)$).
    RotationComposition,
    /// Periodicity relations under $2\pi$ or $4\pi$ rotation parameter shifts.
    RotationPeriodicity,
    /// Multi-qubit decomposition or equivalence (e.g. SWAP into 3 CX).
    TwoQubitDecomposition,
}

/// Nature of equivalence between two operator expressions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IdentityKind {
    /// Exact mathematical matrix equality ($U_{\text{LHS}} = U_{\text{RHS}}$).
    Exact,
    /// Equality up to a scalar global phase factor: $U_{\text{LHS}} = e^{i\phi} U_{\text{RHS}}$.
    GlobalPhaseEquivalent {
        /// Explicit scalar phase difference $\phi$ in radians.
        phase: f64,
    },
}

/// Uniform representation of single-qubit or two-qubit unitary matrices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GateMatrix {
    /// Single-qubit operator matrix ($2 \times 2$).
    Single(Matrix2x2),
    /// Two-qubit operator matrix ($4 \times 4$).
    Two(Matrix4x4),
}

impl GateMatrix {
    /// Checks whether the matrix is unitary ($U^\dagger U = I$) within tolerance.
    pub fn is_unitary(&self, tolerance: f64) -> bool {
        match self {
            Self::Single(m) => m.is_unitary(tolerance),
            Self::Two(m) => m.is_unitary(tolerance),
        }
    }

    /// Checks approximate element-wise equality within tolerance.
    pub fn approx_eq(&self, other: &Self, tolerance: f64) -> bool {
        match (self, other) {
            (Self::Single(a), Self::Single(b)) => a.approx_eq(b, tolerance),
            (Self::Two(a), Self::Two(b)) => a.approx_eq(b, tolerance),
            _ => false,
        }
    }

    /// Multiplies the matrix by a scalar phase factor $e^{i\phi}$.
    pub fn scale_phase(&self, phase: f64) -> Self {
        let factor = Complex64::from_polar(1.0, phase);
        match self {
            Self::Single(m) => {
                let mut res = m.0;
                for row in &mut res {
                    for cell in row {
                        *cell *= factor;
                    }
                }
                Self::Single(Matrix2x2(res))
            }
            Self::Two(m) => {
                let mut res = m.0;
                for row in &mut res {
                    for cell in row {
                        *cell *= factor;
                    }
                }
                Self::Two(Matrix4x4(res))
            }
        }
    }

    /// Computes the maximum absolute difference between corresponding matrix elements.
    pub fn max_element_difference(&self, other: &Self) -> f64 {
        match (self, other) {
            (Self::Single(a), Self::Single(b)) => {
                let mut max_diff = 0.0f64;
                for r in 0..2 {
                    for c in 0..2 {
                        let diff = (a.0[r][c] - b.0[r][c]).norm();
                        if diff > max_diff {
                            max_diff = diff;
                        }
                    }
                }
                max_diff
            }
            (Self::Two(a), Self::Two(b)) => {
                let mut max_diff = 0.0f64;
                for r in 0..4 {
                    for c in 0..4 {
                        let diff = (a.0[r][c] - b.0[r][c]).norm();
                        if diff > max_diff {
                            max_diff = diff;
                        }
                    }
                }
                max_diff
            }
            _ => f64::INFINITY,
        }
    }
}

/// A structured, typed symbolic gate identity.
#[derive(Clone, Copy)]
pub struct GateIdentity {
    /// Unique stable identifier (e.g. `"h_x_h"`).
    pub id: &'static str,
    /// Human-readable identity name.
    pub name: &'static str,
    /// Detailed description and mathematical expression.
    pub description: &'static str,
    /// Semantic category.
    pub category: IdentityCategory,
    /// Exact vs global-phase equivalent.
    pub kind: IdentityKind,
    /// Formal mathematical preconditions.
    pub preconditions: &'static [&'static str],
    /// Qubit arity (1 for single-qubit, 2 for two-qubit).
    pub arity: usize,
    /// Matrix generator returning `(LHS_matrix, RHS_matrix)` given test parameter $\theta$.
    pub matrix_eval: fn(f64) -> (GateMatrix, GateMatrix),
}

impl GateIdentity {
    /// Creates a new custom [`GateIdentity`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: &'static str,
        name: &'static str,
        description: &'static str,
        category: IdentityCategory,
        kind: IdentityKind,
        preconditions: &'static [&'static str],
        arity: usize,
        matrix_eval: fn(f64) -> (GateMatrix, GateMatrix),
    ) -> Self {
        Self {
            id,
            name,
            description,
            category,
            kind,
            preconditions,
            arity,
            matrix_eval,
        }
    }
    /// Evaluates the LHS and RHS matrices for this identity at the given parameter value.
    pub fn evaluate_matrices(&self, param: f64) -> (GateMatrix, GateMatrix) {
        (self.matrix_eval)(param)
    }

    /// Verifies this identity against concrete unitary matrices within the specified tolerance.
    ///
    /// # Errors
    /// Returns `Err` with a diagnostic message if either side is not unitary or if
    /// the matrix equivalence relation fails.
    pub fn verify(&self, param: f64, tolerance: f64) -> Result<(), String> {
        let (lhs, rhs) = self.evaluate_matrices(param);

        if !lhs.is_unitary(tolerance) {
            return Err(format!("identity '{}': LHS matrix is not unitary", self.id));
        }
        if !rhs.is_unitary(tolerance) {
            return Err(format!("identity '{}': RHS matrix is not unitary", self.id));
        }

        match self.kind {
            IdentityKind::Exact => {
                let diff = lhs.max_element_difference(&rhs);
                if diff > tolerance {
                    return Err(format!(
                        "identity '{}': exact matrix mismatch (max element diff = {:.2e} > {:.2e})",
                        self.id, diff, tolerance
                    ));
                }
            }
            IdentityKind::GlobalPhaseEquivalent { phase } => {
                let scaled_rhs = rhs.scale_phase(phase);
                let diff = lhs.max_element_difference(&scaled_rhs);
                if diff > tolerance {
                    return Err(format!(
                        "identity '{}': global-phase-scaled matrix mismatch with phase {:.4} (max element diff = {:.2e} > {:.2e})",
                        self.id, phase, diff, tolerance
                    ));
                }
            }
        }

        Ok(())
    }
}

/// Registry containing all canonical, mathematically proven optimizer identities.
pub struct IdentityRegistry {
    identities: Vec<GateIdentity>,
}

impl Default for IdentityRegistry {
    fn default() -> Self {
        Self::canonical()
    }
}

impl IdentityRegistry {
    /// Returns the canonical registry containing all verified standard identities.
    pub fn canonical() -> Self {
        let identities = vec![
            // 1. Self-Inverse Cancellations
            GateIdentity {
                id: "x_x_identity",
                name: "Pauli X self-inverse",
                description: "X · X = I",
                category: IdentityCategory::SelfInverseCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    let m = matrix_x();
                    (
                        GateMatrix::Single(m.matmul(&m)),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            GateIdentity {
                id: "y_y_identity",
                name: "Pauli Y self-inverse",
                description: "Y · Y = I",
                category: IdentityCategory::SelfInverseCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    let m = matrix_y();
                    (
                        GateMatrix::Single(m.matmul(&m)),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            GateIdentity {
                id: "z_z_identity",
                name: "Pauli Z self-inverse",
                description: "Z · Z = I",
                category: IdentityCategory::SelfInverseCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    let m = matrix_z();
                    (
                        GateMatrix::Single(m.matmul(&m)),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            GateIdentity {
                id: "h_h_identity",
                name: "Hadamard self-inverse",
                description: "H · H = I",
                category: IdentityCategory::SelfInverseCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    let m = matrix_h();
                    (
                        GateMatrix::Single(m.matmul(&m)),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            GateIdentity {
                id: "cx_cx_identity",
                name: "CX self-inverse",
                description: "CX · CX = I",
                category: IdentityCategory::SelfInverseCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same control and target qubits"],
                arity: 2,
                matrix_eval: |_| {
                    let m = matrix_cx_c0_t1();
                    (
                        GateMatrix::Two(m.matmul(&m)),
                        GateMatrix::Two(Matrix4x4::identity()),
                    )
                },
            },
            GateIdentity {
                id: "cz_cz_identity",
                name: "CZ self-inverse",
                description: "CZ · CZ = I",
                category: IdentityCategory::SelfInverseCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same operand qubits (symmetric)"],
                arity: 2,
                matrix_eval: |_| {
                    let m = matrix_cz();
                    (
                        GateMatrix::Two(m.matmul(&m)),
                        GateMatrix::Two(Matrix4x4::identity()),
                    )
                },
            },
            GateIdentity {
                id: "swap_swap_identity",
                name: "SWAP self-inverse",
                description: "SWAP · SWAP = I",
                category: IdentityCategory::SelfInverseCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same operand qubits (symmetric)"],
                arity: 2,
                matrix_eval: |_| {
                    let m = matrix_swap();
                    (
                        GateMatrix::Two(m.matmul(&m)),
                        GateMatrix::Two(Matrix4x4::identity()),
                    )
                },
            },
            // 2. Inverse Pair Cancellations
            GateIdentity {
                id: "s_sdg_identity",
                name: "S and Sdg inverse cancellation",
                description: "S · Sdg = I",
                category: IdentityCategory::InversePairCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_s().matmul(&matrix_sdg())),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            GateIdentity {
                id: "sdg_s_identity",
                name: "Sdg and S inverse cancellation",
                description: "Sdg · S = I",
                category: IdentityCategory::InversePairCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_sdg().matmul(&matrix_s())),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            GateIdentity {
                id: "t_tdg_identity",
                name: "T and Tdg inverse cancellation",
                description: "T · Tdg = I",
                category: IdentityCategory::InversePairCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_t().matmul(&matrix_tdg())),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            GateIdentity {
                id: "tdg_t_identity",
                name: "Tdg and T inverse cancellation",
                description: "Tdg · T = I",
                category: IdentityCategory::InversePairCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_tdg().matmul(&matrix_t())),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            GateIdentity {
                id: "rx_inv_identity",
                name: "Rx inverse cancellation",
                description: "Rx(θ) · Rx(-θ) = I",
                category: IdentityCategory::InversePairCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    let m1 = matrix_rx(Angle::from_radians(theta));
                    let m2 = matrix_rx(Angle::from_radians(-theta));
                    (
                        GateMatrix::Single(m1.matmul(&m2)),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            GateIdentity {
                id: "ry_inv_identity",
                name: "Ry inverse cancellation",
                description: "Ry(θ) · Ry(-θ) = I",
                category: IdentityCategory::InversePairCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    let m1 = matrix_ry(Angle::from_radians(theta));
                    let m2 = matrix_ry(Angle::from_radians(-theta));
                    (
                        GateMatrix::Single(m1.matmul(&m2)),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            GateIdentity {
                id: "rz_inv_identity",
                name: "Rz inverse cancellation",
                description: "Rz(θ) · Rz(-θ) = I",
                category: IdentityCategory::InversePairCancellation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    let m1 = matrix_rz(Angle::from_radians(theta));
                    let m2 = matrix_rz(Angle::from_radians(-theta));
                    (
                        GateMatrix::Single(m1.matmul(&m2)),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            // 3. Power Equivalences
            GateIdentity {
                id: "s_squared_is_z",
                name: "S squared is Z",
                description: "S · S = Z",
                category: IdentityCategory::PowerEquivalence,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_s().matmul(&matrix_s())),
                        GateMatrix::Single(matrix_z()),
                    )
                },
            },
            GateIdentity {
                id: "sdg_squared_is_z",
                name: "Sdg squared is Z",
                description: "Sdg · Sdg = Z",
                category: IdentityCategory::PowerEquivalence,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_sdg().matmul(&matrix_sdg())),
                        GateMatrix::Single(matrix_z()),
                    )
                },
            },
            GateIdentity {
                id: "t_squared_is_s",
                name: "T squared is S",
                description: "T · T = S",
                category: IdentityCategory::PowerEquivalence,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_t().matmul(&matrix_t())),
                        GateMatrix::Single(matrix_s()),
                    )
                },
            },
            GateIdentity {
                id: "t_fourth_is_z",
                name: "T fourth power is Z",
                description: "T^4 = Z",
                category: IdentityCategory::PowerEquivalence,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    let t = matrix_t();
                    let t2 = t.matmul(&t);
                    let t4 = t2.matmul(&t2);
                    (GateMatrix::Single(t4), GateMatrix::Single(matrix_z()))
                },
            },
            GateIdentity {
                id: "tdg_squared_is_sdg",
                name: "Tdg squared is Sdg",
                description: "Tdg · Tdg = Sdg",
                category: IdentityCategory::PowerEquivalence,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_tdg().matmul(&matrix_tdg())),
                        GateMatrix::Single(matrix_sdg()),
                    )
                },
            },
            GateIdentity {
                id: "s_fourth_is_identity",
                name: "S fourth power is identity",
                description: "S^4 = I",
                category: IdentityCategory::PowerEquivalence,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    let s = matrix_s();
                    let s2 = s.matmul(&s);
                    let s4 = s2.matmul(&s2);
                    (
                        GateMatrix::Single(s4),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            GateIdentity {
                id: "t_eighth_is_identity",
                name: "T eighth power is identity",
                description: "T^8 = I",
                category: IdentityCategory::PowerEquivalence,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    let t = matrix_t();
                    let t2 = t.matmul(&t);
                    let t4 = t2.matmul(&t2);
                    let t8 = t4.matmul(&t4);
                    (
                        GateMatrix::Single(t8),
                        GateMatrix::Single(Matrix2x2::identity()),
                    )
                },
            },
            // 4. Pauli & Clifford Conjugations
            GateIdentity {
                id: "h_x_h_is_z",
                name: "Hadamard conjugation of X",
                description: "H · X · H = Z",
                category: IdentityCategory::PauliConjugation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    let h = matrix_h();
                    let x = matrix_x();
                    (
                        GateMatrix::Single(h.matmul(&x).matmul(&h)),
                        GateMatrix::Single(matrix_z()),
                    )
                },
            },
            GateIdentity {
                id: "h_z_h_is_x",
                name: "Hadamard conjugation of Z",
                description: "H · Z · H = X",
                category: IdentityCategory::PauliConjugation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    let h = matrix_h();
                    let z = matrix_z();
                    (
                        GateMatrix::Single(h.matmul(&z).matmul(&h)),
                        GateMatrix::Single(matrix_x()),
                    )
                },
            },
            GateIdentity {
                id: "h_y_h_is_neg_y",
                name: "Hadamard conjugation of Y carries phase π",
                description: "H · Y · H = -Y = e^{iπ} Y",
                category: IdentityCategory::PauliConjugation,
                kind: IdentityKind::GlobalPhaseEquivalent { phase: PI },
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    let h = matrix_h();
                    let y = matrix_y();
                    (
                        GateMatrix::Single(h.matmul(&y).matmul(&h)),
                        GateMatrix::Single(matrix_y()),
                    )
                },
            },
            GateIdentity {
                id: "s_x_sdg_is_y",
                name: "S conjugation of X is Y",
                description: "S · X · Sdg = Y",
                category: IdentityCategory::PauliConjugation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_s().matmul(&matrix_x()).matmul(&matrix_sdg())),
                        GateMatrix::Single(matrix_y()),
                    )
                },
            },
            GateIdentity {
                id: "s_z_sdg_is_z",
                name: "S commutes with Z",
                description: "S · Z · Sdg = Z",
                category: IdentityCategory::PauliConjugation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_s().matmul(&matrix_z()).matmul(&matrix_sdg())),
                        GateMatrix::Single(matrix_z()),
                    )
                },
            },
            // 5. Rotation Relations and Periodicity
            GateIdentity {
                id: "rx_composition",
                name: "Rx composition",
                description: "Rx(θ₁) · Rx(θ₂) = Rx(θ₁ + θ₂)",
                category: IdentityCategory::RotationComposition,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit", "same axis"],
                arity: 1,
                matrix_eval: |theta| {
                    let t1 = theta;
                    let t2 = theta * 0.7;
                    let m1 = matrix_rx(Angle::from_radians(t1));
                    let m2 = matrix_rx(Angle::from_radians(t2));
                    let sum = matrix_rx(Angle::from_radians(t1 + t2));
                    (GateMatrix::Single(m1.matmul(&m2)), GateMatrix::Single(sum))
                },
            },
            GateIdentity {
                id: "ry_composition",
                name: "Ry composition",
                description: "Ry(θ₁) · Ry(θ₂) = Ry(θ₁ + θ₂)",
                category: IdentityCategory::RotationComposition,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit", "same axis"],
                arity: 1,
                matrix_eval: |theta| {
                    let t1 = theta;
                    let t2 = theta * 0.7;
                    let m1 = matrix_ry(Angle::from_radians(t1));
                    let m2 = matrix_ry(Angle::from_radians(t2));
                    let sum = matrix_ry(Angle::from_radians(t1 + t2));
                    (GateMatrix::Single(m1.matmul(&m2)), GateMatrix::Single(sum))
                },
            },
            GateIdentity {
                id: "rz_composition",
                name: "Rz composition",
                description: "Rz(θ₁) · Rz(θ₂) = Rz(θ₁ + θ₂)",
                category: IdentityCategory::RotationComposition,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit", "same axis"],
                arity: 1,
                matrix_eval: |theta| {
                    let t1 = theta;
                    let t2 = theta * 0.7;
                    let m1 = matrix_rz(Angle::from_radians(t1));
                    let m2 = matrix_rz(Angle::from_radians(t2));
                    let sum = matrix_rz(Angle::from_radians(t1 + t2));
                    (GateMatrix::Single(m1.matmul(&m2)), GateMatrix::Single(sum))
                },
            },
            GateIdentity {
                id: "rx_four_pi_periodicity",
                name: "Rx 4π exact periodicity",
                description: "Rx(θ + 4π) = Rx(θ)",
                category: IdentityCategory::RotationPeriodicity,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    (
                        GateMatrix::Single(matrix_rx(Angle::from_radians(theta + 4.0 * PI))),
                        GateMatrix::Single(matrix_rx(Angle::from_radians(theta))),
                    )
                },
            },
            GateIdentity {
                id: "ry_four_pi_periodicity",
                name: "Ry 4π exact periodicity",
                description: "Ry(θ + 4π) = Ry(θ)",
                category: IdentityCategory::RotationPeriodicity,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    (
                        GateMatrix::Single(matrix_ry(Angle::from_radians(theta + 4.0 * PI))),
                        GateMatrix::Single(matrix_ry(Angle::from_radians(theta))),
                    )
                },
            },
            GateIdentity {
                id: "rz_four_pi_periodicity",
                name: "Rz 4π exact periodicity",
                description: "Rz(θ + 4π) = Rz(θ)",
                category: IdentityCategory::RotationPeriodicity,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    (
                        GateMatrix::Single(matrix_rz(Angle::from_radians(theta + 4.0 * PI))),
                        GateMatrix::Single(matrix_rz(Angle::from_radians(theta))),
                    )
                },
            },
            GateIdentity {
                id: "rx_two_pi_periodicity",
                name: "Rx 2π periodicity carries phase π",
                description: "Rx(θ + 2π) = -Rx(θ) = e^{iπ} Rx(θ)",
                category: IdentityCategory::RotationPeriodicity,
                kind: IdentityKind::GlobalPhaseEquivalent { phase: PI },
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    (
                        GateMatrix::Single(matrix_rx(Angle::from_radians(theta + 2.0 * PI))),
                        GateMatrix::Single(matrix_rx(Angle::from_radians(theta))),
                    )
                },
            },
            GateIdentity {
                id: "ry_two_pi_periodicity",
                name: "Ry 2π periodicity carries phase π",
                description: "Ry(θ + 2π) = -Ry(θ) = e^{iπ} Ry(θ)",
                category: IdentityCategory::RotationPeriodicity,
                kind: IdentityKind::GlobalPhaseEquivalent { phase: PI },
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    (
                        GateMatrix::Single(matrix_ry(Angle::from_radians(theta + 2.0 * PI))),
                        GateMatrix::Single(matrix_ry(Angle::from_radians(theta))),
                    )
                },
            },
            GateIdentity {
                id: "rz_two_pi_periodicity",
                name: "Rz 2π periodicity carries phase π",
                description: "Rz(θ + 2π) = -Rz(θ) = e^{iπ} Rz(θ)",
                category: IdentityCategory::RotationPeriodicity,
                kind: IdentityKind::GlobalPhaseEquivalent { phase: PI },
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    (
                        GateMatrix::Single(matrix_rz(Angle::from_radians(theta + 2.0 * PI))),
                        GateMatrix::Single(matrix_rz(Angle::from_radians(theta))),
                    )
                },
            },
            GateIdentity {
                id: "rz_pi_is_phase_z",
                name: "Rz(π) equals Z with global phase -π/2",
                description: "Rz(π) = e^{-iπ/2} Z = -i Z",
                category: IdentityCategory::PowerEquivalence,
                kind: IdentityKind::GlobalPhaseEquivalent { phase: -PI / 2.0 },
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_rz(Angle::pi())),
                        GateMatrix::Single(matrix_z()),
                    )
                },
            },
            GateIdentity {
                id: "rx_pi_is_phase_x",
                name: "Rx(π) equals X with global phase -π/2",
                description: "Rx(π) = e^{-iπ/2} X = -i X",
                category: IdentityCategory::PowerEquivalence,
                kind: IdentityKind::GlobalPhaseEquivalent { phase: -PI / 2.0 },
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_rx(Angle::pi())),
                        GateMatrix::Single(matrix_x()),
                    )
                },
            },
            GateIdentity {
                id: "ry_pi_is_phase_y",
                name: "Ry(π) equals Y with global phase -π/2",
                description: "Ry(π) = e^{-iπ/2} Y = -i Y",
                category: IdentityCategory::PowerEquivalence,
                kind: IdentityKind::GlobalPhaseEquivalent { phase: -PI / 2.0 },
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_ry(Angle::pi())),
                        GateMatrix::Single(matrix_y()),
                    )
                },
            },
            GateIdentity {
                id: "rz_half_pi_is_phase_s",
                name: "Rz(π/2) equals S with global phase -π/4",
                description: "Rz(π/2) = e^{-iπ/4} S",
                category: IdentityCategory::PowerEquivalence,
                kind: IdentityKind::GlobalPhaseEquivalent { phase: -PI / 4.0 },
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_rz(Angle::pi_by_2())),
                        GateMatrix::Single(matrix_s()),
                    )
                },
            },
            GateIdentity {
                id: "rz_fourth_pi_is_phase_t",
                name: "Rz(π/4) equals T with global phase -π/8",
                description: "Rz(π/4) = e^{-iπ/8} T",
                category: IdentityCategory::PowerEquivalence,
                kind: IdentityKind::GlobalPhaseEquivalent { phase: -PI / 8.0 },
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |_| {
                    (
                        GateMatrix::Single(matrix_rz(Angle::pi_by_4())),
                        GateMatrix::Single(matrix_t()),
                    )
                },
            },
            GateIdentity {
                id: "h_rx_h_is_rz",
                name: "Hadamard basis change from Rx to Rz",
                description: "H · Rx(θ) · H = Rz(θ)",
                category: IdentityCategory::PauliConjugation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    let h = matrix_h();
                    let rx = matrix_rx(Angle::from_radians(theta));
                    let rz = matrix_rz(Angle::from_radians(theta));
                    (
                        GateMatrix::Single(h.matmul(&rx).matmul(&h)),
                        GateMatrix::Single(rz),
                    )
                },
            },
            GateIdentity {
                id: "h_rz_h_is_rx",
                name: "Hadamard basis change from Rz to Rx",
                description: "H · Rz(θ) · H = Rx(θ)",
                category: IdentityCategory::PauliConjugation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    let h = matrix_h();
                    let rz = matrix_rz(Angle::from_radians(theta));
                    let rx = matrix_rx(Angle::from_radians(theta));
                    (
                        GateMatrix::Single(h.matmul(&rz).matmul(&h)),
                        GateMatrix::Single(rx),
                    )
                },
            },
            GateIdentity {
                id: "h_ry_h_is_ry_neg",
                name: "Hadamard conjugation of Ry negates angle",
                description: "H · Ry(θ) · H = Ry(-θ)",
                category: IdentityCategory::PauliConjugation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    let h = matrix_h();
                    let ry = matrix_ry(Angle::from_radians(theta));
                    let ry_neg = matrix_ry(Angle::from_radians(-theta));
                    (
                        GateMatrix::Single(h.matmul(&ry).matmul(&h)),
                        GateMatrix::Single(ry_neg),
                    )
                },
            },
            GateIdentity {
                id: "x_rz_x_is_rz_neg",
                name: "X conjugation of Rz negates angle",
                description: "X · Rz(θ) · X = Rz(-θ)",
                category: IdentityCategory::PauliConjugation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    let x = matrix_x();
                    let rz = matrix_rz(Angle::from_radians(theta));
                    let rz_neg = matrix_rz(Angle::from_radians(-theta));
                    (
                        GateMatrix::Single(x.matmul(&rz).matmul(&x)),
                        GateMatrix::Single(rz_neg),
                    )
                },
            },
            GateIdentity {
                id: "z_rx_z_is_rx_neg",
                name: "Z conjugation of Rx negates angle",
                description: "Z · Rx(θ) · Z = Rx(-θ)",
                category: IdentityCategory::PauliConjugation,
                kind: IdentityKind::Exact,
                preconditions: &["same target qubit"],
                arity: 1,
                matrix_eval: |theta| {
                    let z = matrix_z();
                    let rx = matrix_rx(Angle::from_radians(theta));
                    let rx_neg = matrix_rx(Angle::from_radians(-theta));
                    (
                        GateMatrix::Single(z.matmul(&rx).matmul(&z)),
                        GateMatrix::Single(rx_neg),
                    )
                },
            },
            // 6. Two-Qubit Identities
            GateIdentity {
                id: "swap_via_three_cx",
                name: "SWAP synthesized from 3 CX gates",
                description: "SWAP(0, 1) = CX(0, 1) · CX(1, 0) · CX(0, 1)",
                category: IdentityCategory::TwoQubitDecomposition,
                kind: IdentityKind::Exact,
                preconditions: &["qubit 0 and qubit 1 distinct"],
                arity: 2,
                matrix_eval: |_| {
                    let cx01 = matrix_cx_c0_t1();
                    let cx10 = matrix_cx_c1_t0();
                    // Sequential execution: cx01 first, cx10 second, cx01 third
                    // Matrix composition: cx01 * cx10 * cx01
                    let prod = cx01.matmul(&cx10).matmul(&cx01);
                    (GateMatrix::Two(prod), GateMatrix::Two(matrix_swap()))
                },
            },
            GateIdentity {
                id: "cx_direction_reversal",
                name: "CX direction reversal via 4 Hadamards",
                description: "CX(1, 0) = (H ⊗ H) · CX(0, 1) · (H ⊗ H)",
                category: IdentityCategory::TwoQubitDecomposition,
                kind: IdentityKind::Exact,
                preconditions: &["qubit 0 and qubit 1 distinct"],
                arity: 2,
                matrix_eval: |_| {
                    let h = matrix_h();
                    let h_h = kron_2x2(&h, &h);
                    let cx01 = matrix_cx_c0_t1();
                    let prod = h_h.matmul(&cx01).matmul(&h_h);
                    (GateMatrix::Two(prod), GateMatrix::Two(matrix_cx_c1_t0()))
                },
            },
            GateIdentity {
                id: "cz_via_cx_h",
                name: "CZ synthesized from CX and target Hadamards",
                description: "CZ(0, 1) = (H ⊗ I) · CX(0, 1) · (H ⊗ I)",
                category: IdentityCategory::TwoQubitDecomposition,
                kind: IdentityKind::Exact,
                preconditions: &[
                    "qubit 0 is control (inner factor), qubit 1 is target (outer factor)",
                ],
                arity: 2,
                matrix_eval: |_| {
                    let h1_i0 = kron_2x2(&matrix_h(), &Matrix2x2::identity());
                    let cx01 = matrix_cx_c0_t1();
                    let prod = h1_i0.matmul(&cx01).matmul(&h1_i0);
                    (GateMatrix::Two(prod), GateMatrix::Two(matrix_cz()))
                },
            },
            GateIdentity {
                id: "cz_symmetry",
                name: "CZ operand symmetry",
                description: "CZ(0, 1) = CZ(1, 0)",
                category: IdentityCategory::TwoQubitDecomposition,
                kind: IdentityKind::Exact,
                preconditions: &["qubits distinct"],
                arity: 2,
                matrix_eval: |_| (GateMatrix::Two(matrix_cz()), GateMatrix::Two(matrix_cz())),
            },
            GateIdentity {
                id: "swap_symmetry",
                name: "SWAP operand symmetry",
                description: "SWAP(0, 1) = SWAP(1, 0)",
                category: IdentityCategory::TwoQubitDecomposition,
                kind: IdentityKind::Exact,
                preconditions: &["qubits distinct"],
                arity: 2,
                matrix_eval: |_| {
                    (
                        GateMatrix::Two(matrix_swap()),
                        GateMatrix::Two(matrix_swap()),
                    )
                },
            },
        ];

        Self { identities }
    }

    /// Returns the number of registered identities.
    #[inline]
    pub fn len(&self) -> usize {
        self.identities.len()
    }

    /// Returns `true` if the registry contains no identities.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.identities.is_empty()
    }

    /// Looks up a registered identity by its unique ID.
    pub fn get(&self, id: &str) -> Option<&GateIdentity> {
        self.identities.iter().find(|i| i.id == id)
    }

    /// Looks up a registered identity by its unique ID (alias for [`Self::get`]).
    #[inline]
    pub fn by_id(&self, id: &str) -> Option<&GateIdentity> {
        self.get(id)
    }

    /// Returns an iterator over all registered identities.
    pub fn all(&self) -> &[GateIdentity] {
        &self.identities
    }

    /// Returns an iterator over identities that represent exact matrix equalities.
    pub fn exact_identities(&self) -> impl Iterator<Item = &GateIdentity> {
        self.identities
            .iter()
            .filter(|i| matches!(i.kind, IdentityKind::Exact))
    }

    /// Returns an iterator over identities valid up to a global phase.
    pub fn phase_equivalent_identities(&self) -> impl Iterator<Item = &GateIdentity> {
        self.identities
            .iter()
            .filter(|i| matches!(i.kind, IdentityKind::GlobalPhaseEquivalent { .. }))
    }

    /// Returns an iterator over identities belonging to a specific category.
    pub fn by_category(&self, category: IdentityCategory) -> impl Iterator<Item = &GateIdentity> {
        self.identities
            .iter()
            .filter(move |i| i.category == category)
    }

    /// Verifies all registered identities against concrete unitary matrices.
    ///
    /// # Errors
    /// Returns the first error diagnostic encountered if any identity check fails.
    pub fn verify_all(&self, param: f64, tolerance: f64) -> Result<(), String> {
        for identity in &self.identities {
            identity.verify(param, tolerance)?;
        }
        Ok(())
    }
}

// ============================================================================
// Parameter Normalization & Simplification
// ============================================================================

/// Normalizes a continuous rotation angle into $[0, 4\pi)$ with ZERO global phase difference.
///
/// Because $SU(2)$ double-covers $SO(3)$, $R_k(\theta + 4\pi) = R_k(\theta)$ is an exact matrix
/// equality, while shifts by $2\pi$ introduce an overall minus sign ($e^{i\pi} = -1$).
pub fn normalize_rotation_angle_exact(angle: f64) -> f64 {
    let four_pi = 4.0 * PI;
    let mut rad = angle % four_pi;
    if rad < 0.0 {
        rad += four_pi;
    }
    if rad >= four_pi {
        rad = 0.0;
    }
    rad
}

/// Normalizes a continuous rotation angle into $[0, 2\pi)$ and returns the required scalar global phase.
///
/// Returns `(normalized_angle, global_phase_correction)` such that:
/// $$R_k(\theta) = e^{i \cdot \text{global\_phase\_correction}} R_k(\text{normalized\_angle})$$
pub fn normalize_rotation_angle_2pi(angle: f64) -> (f64, f64) {
    let two_pi = 2.0 * PI;
    let four_pi = 4.0 * PI;

    let mut rad4 = angle % four_pi;
    if rad4 < 0.0 {
        rad4 += four_pi;
    }
    if rad4 >= four_pi {
        rad4 = 0.0;
    }

    if rad4 >= two_pi {
        // Shifted by an odd multiple of 2π: introduces phase factor of π (-1)
        (rad4 - two_pi, PI)
    } else {
        (rad4, 0.0)
    }
}

/// Exactly simplifies a parameter expression without loss of mathematical precision.
///
/// Applies canonical constant folding and identity elimination ($x + 0 = x, -(-x) = x$).
pub fn simplify_parameter_expr(expr: &ParameterExpr) -> ParameterExpr {
    match expr {
        ParameterExpr::Constant(c) => ParameterExpr::Constant(*c),
        ParameterExpr::Symbolic(s) => ParameterExpr::Symbolic(s.clone()),
        ParameterExpr::Neg(inner) => match simplify_parameter_expr(inner) {
            ParameterExpr::Constant(c) => ParameterExpr::Constant(-c),
            ParameterExpr::Neg(nested) => *nested,
            simplified => ParameterExpr::Neg(Box::new(simplified)),
        },
        ParameterExpr::Add(a, b) => {
            let sim_a = simplify_parameter_expr(a);
            let sim_b = simplify_parameter_expr(b);
            match (&sim_a, &sim_b) {
                (ParameterExpr::Constant(ca), ParameterExpr::Constant(cb)) => {
                    ParameterExpr::Constant(ca + cb)
                }
                (ParameterExpr::Constant(ca), _) if ca.abs() < 1e-15 => sim_b,
                (_, ParameterExpr::Constant(cb)) if cb.abs() < 1e-15 => sim_a,
                _ => ParameterExpr::Add(Box::new(sim_a), Box::new(sim_b)),
            }
        }
        ParameterExpr::Sub(a, b) => {
            let sim_a = simplify_parameter_expr(a);
            let sim_b = simplify_parameter_expr(b);
            match (&sim_a, &sim_b) {
                (ParameterExpr::Constant(ca), ParameterExpr::Constant(cb)) => {
                    ParameterExpr::Constant(ca - cb)
                }
                (_, ParameterExpr::Constant(cb)) if cb.abs() < 1e-15 => sim_a,
                (ParameterExpr::Constant(ca), _) if ca.abs() < 1e-15 => {
                    ParameterExpr::Neg(Box::new(sim_b))
                }
                _ if sim_a == sim_b => ParameterExpr::Constant(0.0),
                _ => ParameterExpr::Sub(Box::new(sim_a), Box::new(sim_b)),
            }
        }
        ParameterExpr::Mul(a, b) => {
            let sim_a = simplify_parameter_expr(a);
            let sim_b = simplify_parameter_expr(b);
            match (&sim_a, &sim_b) {
                (ParameterExpr::Constant(ca), ParameterExpr::Constant(cb)) => {
                    ParameterExpr::Constant(ca * cb)
                }
                (ParameterExpr::Constant(ca), _) if (ca - 1.0).abs() < 1e-15 => sim_b,
                (_, ParameterExpr::Constant(cb)) if (cb - 1.0).abs() < 1e-15 => sim_a,
                (ParameterExpr::Constant(ca), _) if ca.abs() < 1e-15 => {
                    ParameterExpr::Constant(0.0)
                }
                (_, ParameterExpr::Constant(cb)) if cb.abs() < 1e-15 => {
                    ParameterExpr::Constant(0.0)
                }
                _ => ParameterExpr::Mul(Box::new(sim_a), Box::new(sim_b)),
            }
        }
        ParameterExpr::Div(a, b) => {
            let sim_a = simplify_parameter_expr(a);
            let sim_b = simplify_parameter_expr(b);
            match (&sim_a, &sim_b) {
                (ParameterExpr::Constant(ca), ParameterExpr::Constant(cb)) if cb.abs() > 1e-15 => {
                    ParameterExpr::Constant(ca / cb)
                }
                (_, ParameterExpr::Constant(cb)) if (cb - 1.0).abs() < 1e-15 => sim_a,
                _ => ParameterExpr::Div(Box::new(sim_a), Box::new(sim_b)),
            }
        }
    }
}

/// Composes two rotation angle expressions along the same axis into a single simplified sum.
pub fn compose_rotation_angles(theta1: &ParameterExpr, theta2: &ParameterExpr) -> ParameterExpr {
    simplify_parameter_expr(&ParameterExpr::Add(
        Box::new(theta1.clone()),
        Box::new(theta2.clone()),
    ))
}
