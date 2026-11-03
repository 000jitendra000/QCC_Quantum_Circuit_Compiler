//! Single-qubit Euler angle decomposition for arbitrary $2 \times 2$ unitary matrices.
//!
//! # Mathematical Conventions
//!
//! Any single-qubit unitary operator $U \in U(2)$ satisfies $U^\dagger U = I$ and can be
//! factored into a scalar global phase $e^{i\alpha}$ and a special unitary matrix $V \in SU(2)$:
//!
//! $$U = e^{i\alpha} V, \quad \det(V) = 1$$
//!
//! where $\det(U) = e^{i\Phi}$ with $\Phi \in (-\pi, \pi]$, and $\alpha = \Phi / 2$.
//!
//! ## Euler Rotation Bases
//!
//! In QCC, the canonical single-qubit Euler basis is **$Z\text{-}Y\text{-}Z$** (with optional
//! support for **$Z\text{-}X\text{-}Z$**):
//!
//! - **$Z\text{-}Y\text{-}Z$**:
//!   $$V = R_z(\phi) \cdot R_y(\theta) \cdot R_z(\lambda)$$
//!   $$U = e^{i\alpha} \left( R_z(\phi) \cdot R_y(\theta) \cdot R_z(\lambda) \right)$$
//!
//! - **$Z\text{-}X\text{-}Z$**:
//!   $$V = R_z(\phi) \cdot R_x(\theta) \cdot R_z(\lambda)$$
//!   $$U = e^{i\alpha} \left( R_z(\phi) \cdot R_x(\theta) \cdot R_z(\lambda) \right)$$
//!
//! ## Multiplication Order & Circuit Execution
//!
//! QCC strictly adheres to the **column-vector convention** where operations act on quantum
//! states by left multiplication. If an operation $A$ executes before operation $B$, their
//! combined matrix operator is $B \cdot A$.
//!
//! Therefore, the matrix factorization:
//! $$V = R_z(\phi) \cdot R_y(\theta) \cdot R_z(\lambda)$$
//! corresponds to sequential circuit execution on a qubit:
//! 1. Apply $R_z(\lambda)$ first
//! 2. Apply $R_y(\theta)$ second
//! 3. Apply $R_z(\phi)$ third
//!
//! with an explicit circuit global phase factor $e^{i\alpha}$.
//!
//! ## Canonical Angle Ranges
//!
//! - Polar angle $\theta \in [0, \pi]$
//! - Azimuthal angles $\phi, \lambda \in [0, 2\pi)$
//! - Global phase $\alpha \in [0, 2\pi)$
//!
//! ## Singularity & Gimbal Lock Handling
//!
//! When $\theta = 0$ or $\theta = \pi$, the two rotation axes become collinear (gimbal lock).
//! QCC resolves the gauge freedom deterministically by convention:
//! - When $\theta = 0$ ($|v_{10}| \le \epsilon$): $\theta = 0$, $\lambda = 0$, $\phi = 2 \cdot \text{Arg}(v_{11})$.
//! - When $\theta = \pi$ ($|v_{00}| \le \epsilon$): $\theta = \pi$, $\lambda = 0$, $\phi = 2 \cdot \text{Arg}(v_{10})$ (for $Z\text{-}Y\text{-}Z$).

use std::f64::consts::PI;

use num_complex::Complex64;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::builder::CircuitBuilder;
use crate::ir::circuit::Circuit;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::wire::QubitId;
use crate::quantum::gate_matrix::{matrix_rx, matrix_ry, matrix_rz, Matrix2x2};
use crate::quantum::parameter::{Angle, GlobalPhase};

/// Default tolerance for verifying matrix unitarity and reconstruction ($\epsilon_{\text{matrix}}$).
pub const DEFAULT_EULER_TOLERANCE: f64 = 1.0e-10;

/// Default tolerance for identifying gimbal-lock degenerate boundaries ($\theta \approx 0$ or $\pi$).
pub const DEFAULT_GIMBAL_TOLERANCE: f64 = 1.0e-12;

/// Supported Euler decomposition bases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EulerBasis {
    /// Canonical $Z\text{-}Y\text{-}Z$ basis: $R_z(\phi) \cdot R_y(\theta) \cdot R_z(\lambda)$.
    Zyz,
    /// $Z\text{-}X\text{-}Z$ basis: $R_z(\phi) \cdot R_x(\theta) \cdot R_z(\lambda)$.
    Zxz,
}

impl Default for EulerBasis {
    #[inline]
    fn default() -> Self {
        Self::Zyz
    }
}

/// Errors occurring during single-qubit Euler decomposition.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum DecompositionError {
    /// Matrix contains non-finite elements (NaN or infinite values).
    #[error("Matrix contains non-finite elements (NaN or infinite values)")]
    NonFiniteElement,
    /// Matrix is not unitary within the specified tolerance.
    #[error(
        "Matrix is not unitary: ||U†U - I||_F = {residual:.6e} exceeds tolerance {tolerance:.6e}"
    )]
    NonUnitaryMatrix {
        /// Measured Frobenius norm difference from identity.
        residual: f64,
        /// Maximum allowed residual tolerance.
        tolerance: f64,
    },
}

/// Result of decomposing a single-qubit unitary into Euler angles and a scalar global phase.
///
/// Represents the factorization:
/// $$U = e^{i\alpha} \cdot \left( R_{\text{axis1}}(\phi) \cdot R_{\text{axis2}}(\theta) \cdot R_{\text{axis3}}(\lambda) \right)$$
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EulerDecomposition {
    /// The decomposition basis convention (e.g. $Z\text{-}Y\text{-}Z$).
    pub basis: EulerBasis,
    /// Third rotation angle in execution order ($\phi$, around the first axis in the basis name).
    pub phi: Angle,
    /// Second rotation angle in execution order ($\theta$, around the middle axis), in $[0, \pi]$.
    pub theta: Angle,
    /// First rotation angle in execution order ($\lambda$, around the last axis in the basis name).
    pub lambda: Angle,
    /// Extracted scalar global phase: $U = e^{i\alpha} V$.
    pub global_phase: GlobalPhase,
}

impl EulerDecomposition {
    /// Returns the Euler angles $(\phi, \theta, \lambda)$ as [`Angle`] instances.
    #[inline]
    pub fn angles(&self) -> (Angle, Angle, Angle) {
        (self.phi, self.theta, self.lambda)
    }

    /// Returns the Euler angles $(\phi, \theta, \lambda)$ in radians.
    #[inline]
    pub fn angles_rad(&self) -> (f64, f64, f64) {
        (
            self.phi.radians(),
            self.theta.radians(),
            self.lambda.radians(),
        )
    }

    /// Reconstructs the normalized $SU(2)$ matrix $V$ without the global phase.
    ///
    /// Under QCC column-vector conventions, this computes:
    /// $$V = R_{\text{axis1}}(\phi) \cdot (R_{\text{axis2}}(\theta) \cdot R_{\text{axis3}}(\lambda))$$
    pub fn to_su2_matrix(&self) -> Matrix2x2 {
        let (r1, r2, r3) = match self.basis {
            EulerBasis::Zyz => (
                matrix_rz(self.phi),
                matrix_ry(self.theta),
                matrix_rz(self.lambda),
            ),
            EulerBasis::Zxz => (
                matrix_rz(self.phi),
                matrix_rx(self.theta),
                matrix_rz(self.lambda),
            ),
        };
        // Left multiplication: r1 * (r2 * r3)
        r1.matmul(&r2.matmul(&r3))
    }

    /// Reconstructs the original $U(2)$ matrix including the scalar global phase:
    /// $$U_{\text{rec}} = e^{i\alpha} \cdot V$$
    pub fn reconstruct(&self) -> Matrix2x2 {
        let v = self.to_su2_matrix();
        let phase_factor = self.global_phase.factor();
        v.scale(phase_factor)
    }

    /// Generates the sequence of circuit operations in execution order for a target qubit.
    ///
    /// Execution order:
    /// 1. $R_z(\lambda)$
    /// 2. $R_y(\theta)$ (or $R_x(\theta)$ for $Z\text{-}X\text{-}Z$)
    /// 3. $R_z(\phi)$
    pub fn to_operations(&self, qubit: QubitId) -> Vec<Operation> {
        let middle_gate = match self.basis {
            EulerBasis::Zyz => "ry",
            EulerBasis::Zxz => "rx",
        };
        vec![
            Operation::custom_gate(
                "rz",
                vec![qubit],
                vec![ParameterExpr::constant(self.lambda.radians())],
            ),
            Operation::custom_gate(
                middle_gate,
                vec![qubit],
                vec![ParameterExpr::constant(self.theta.radians())],
            ),
            Operation::custom_gate(
                "rz",
                vec![qubit],
                vec![ParameterExpr::constant(self.phi.radians())],
            ),
        ]
    }

    /// Builds a single-qubit [`Circuit`] executing the Euler rotation sequence with the
    /// extracted circuit-level global phase.
    pub fn to_circuit(&self, name: impl Into<String>, qubit: QubitId) -> Circuit {
        let mut builder = CircuitBuilder::new(name);
        let num_qubits = qubit.0 + 1;
        builder
            .add_quantum_register("q", num_qubits)
            .expect("valid register");
        let mut circuit = builder.finish().expect("valid circuit");
        circuit.set_global_phase(ParameterExpr::constant(self.global_phase.angle().radians()));
        for op in self.to_operations(qubit) {
            let _ = circuit.add_operation(op);
        }
        circuit
    }
}

/// Decomposes an arbitrary $2 \times 2$ unitary matrix into canonical Euler angles
/// and an explicit scalar global phase.
///
/// # Arguments
/// - `matrix`: The input $2 \times 2$ matrix to decompose.
/// - `basis`: Euler rotation basis ($Z\text{-}Y\text{-}Z$ or $Z\text{-}X\text{-}Z$).
/// - `tolerance`: Maximum Frobenius norm residual $\|U^\dagger U - I\|_F$ allowed.
///
/// # Returns
/// - `Ok(EulerDecomposition)` if the matrix is unitary within tolerance.
/// - `Err(DecompositionError)` if the matrix is non-finite or non-unitary.
pub fn decompose_euler(
    matrix: &Matrix2x2,
    basis: EulerBasis,
    tolerance: f64,
) -> Result<EulerDecomposition, DecompositionError> {
    // 1. Guard against non-finite elements
    if !matrix.is_finite() {
        return Err(DecompositionError::NonFiniteElement);
    }

    // 2. Validate unitarity: ||U†U - I||_F <= tolerance
    let residual = matrix.unitarity_residual();
    if residual > tolerance {
        return Err(DecompositionError::NonUnitaryMatrix {
            residual,
            tolerance,
        });
    }

    // 3. Separate determinant & global phase
    // det(U) = e^(i * Phi)
    let det = matrix.det();
    let phi_det = det.im.atan2(det.re);
    let alpha_raw = phi_det / 2.0;

    // Normalize matrix to SU(2): V = exp(-i * alpha_raw) * U
    let phase_inv = Complex64::from_polar(1.0, -alpha_raw);
    let v_raw = matrix.scale(phase_inv);

    // Enforce exact SU(2) symmetry: V = [[a, b], [-b*, a*]] with |a|^2 + |b|^2 = 1.
    // Average off-diagonal and diagonal elements to eliminate numerical noise:
    let a = (v_raw.0[0][0] + v_raw.0[1][1].conj()) / 2.0;
    let b = (v_raw.0[0][1] - v_raw.0[1][0].conj()) / 2.0;
    let norm = (a.norm_sqr() + b.norm_sqr()).sqrt();
    let (a, b) = if norm > 0.0 {
        (a / norm, b / norm)
    } else {
        (Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0))
    };

    // Magnitudes clamped to [0.0, 1.0]
    let mag_a = a.norm().clamp(0.0, 1.0);
    let mag_b = b.norm().clamp(0.0, 1.0);

    // Polar angle theta in [0, pi]
    let theta_val = (2.0 * mag_b.atan2(mag_a)).clamp(0.0, PI);

    // 4. Extract raw angles according to Euler basis
    let (phi_raw, theta_raw, lambda_raw) = match basis {
        EulerBasis::Zyz => {
            if mag_b <= DEFAULT_GIMBAL_TOLERANCE {
                // Gimbal lock Case A: theta ~ 0 (Ry(0) = I)
                // v00 = a = exp(-i * (phi + lambda)/2)
                let s = -a.im.atan2(a.re);
                (2.0 * s, 0.0, 0.0)
            } else if mag_a <= DEFAULT_GIMBAL_TOLERANCE {
                // Gimbal lock Case B: theta ~ pi (Ry(pi))
                // v10 = -b* = exp(i * (phi - lambda)/2)
                let neg_b_conj = -b.conj();
                let d = neg_b_conj.im.atan2(neg_b_conj.re);
                (2.0 * d, PI, 0.0)
            } else {
                // General non-singular case
                // a = exp(-i*S) * cos(theta/2) => S = -Arg(a)
                // -b* = exp(i*D) * sin(theta/2) => D = Arg(-b*)
                let s = -a.im.atan2(a.re);
                let neg_b_conj = -b.conj();
                let d = neg_b_conj.im.atan2(neg_b_conj.re);
                (s + d, theta_val, s - d)
            }
        }
        EulerBasis::Zxz => {
            if mag_b <= DEFAULT_GIMBAL_TOLERANCE {
                // Gimbal lock Case A: theta ~ 0 (Rx(0) = I)
                let s = -a.im.atan2(a.re);
                (2.0 * s, 0.0, 0.0)
            } else if mag_a <= DEFAULT_GIMBAL_TOLERANCE {
                // Gimbal lock Case B: theta ~ pi (Rx(pi))
                let neg_b_conj = -b.conj();
                let d = neg_b_conj.im.atan2(neg_b_conj.re) + PI / 2.0;
                (2.0 * d, PI, 0.0)
            } else {
                // General non-singular case for Z-X-Z:
                // -b* = -i * exp(i*D) * sin(theta/2) = exp(i*(D - pi/2)) * sin(theta/2)
                // => D = Arg(-b*) + pi/2
                let s = -a.im.atan2(a.re);
                let neg_b_conj = -b.conj();
                let d = neg_b_conj.im.atan2(neg_b_conj.re) + PI / 2.0;
                (s + d, theta_val, s - d)
            }
        }
    };

    // 5. Normalize angles into canonical ranges
    let phi_norm = Angle::from_radians(phi_raw).normalize();
    let theta_norm = Angle::from_radians(theta_raw);
    let lambda_norm = Angle::from_radians(lambda_raw).normalize();

    // 6. Double-Cover Spinor Sign Detection:
    // When angles are normalized modulo 2pi, each 2pi shift on Rz flips its sign by -1.
    // We construct the candidate SU(2) matrix from normalized angles and compare its trace
    // with the target SU(2) matrix V. Since both are in SU(2), Tr(V_target† * V_cand) is either +2 or -2.
    // If negative, an odd number of sign flips occurred, so we adjust alpha by +pi to preserve
    // exact U(2) matrix equality without error.
    let candidate_decomp = EulerDecomposition {
        basis,
        phi: phi_norm,
        theta: theta_norm,
        lambda: lambda_norm,
        global_phase: GlobalPhase::from_radians(alpha_raw),
    };
    let v_cand = candidate_decomp.to_su2_matrix();

    // Trace of V_target† * V_cand:
    let v_target = Matrix2x2([[a, b], [-b.conj(), a.conj()]]);
    let alignment = v_target.adjoint().matmul(&v_cand).trace();
    let final_alpha = if alignment.re < 0.0 {
        alpha_raw + PI
    } else {
        alpha_raw
    };

    let global_phase = GlobalPhase::from_radians(final_alpha).angle().normalize();

    Ok(EulerDecomposition {
        basis,
        phi: phi_norm,
        theta: theta_norm,
        lambda: lambda_norm,
        global_phase: GlobalPhase::from_angle(global_phase),
    })
}

/// Convenience decomposition into the canonical $Z\text{-}Y\text{-}Z$ Euler basis.
#[inline]
pub fn decompose_euler_zyz(matrix: &Matrix2x2) -> Result<EulerDecomposition, DecompositionError> {
    decompose_euler(matrix, EulerBasis::Zyz, DEFAULT_EULER_TOLERANCE)
}

/// Convenience decomposition into the $Z\text{-}X\text{-}Z$ Euler basis.
#[inline]
pub fn decompose_euler_zxz(matrix: &Matrix2x2) -> Result<EulerDecomposition, DecompositionError> {
    decompose_euler(matrix, EulerBasis::Zxz, DEFAULT_EULER_TOLERANCE)
}
