use std::fmt;

use crate::quantum::gate_matrix::{
    matrix_cx, matrix_cz, matrix_h, matrix_rx, matrix_ry, matrix_rz, matrix_s, matrix_sdg,
    matrix_swap, matrix_t, matrix_tdg, matrix_x, matrix_y, matrix_z, Matrix2x2, Matrix4x4,
};
use crate::quantum::parameter::Angle;

/// Represents a quantum gate within QCC's canonical initial gate set.
///
/// Gates in this set are classified into:
/// - **Fixed single-qubit gates**: `X`, `Y`, `Z`, `H`, `S`, `Sdg`, `T`, `Tdg`.
/// - **Parameterized single-qubit rotations**: `Rx(Angle)`, `Ry(Angle)`, `Rz(Angle)`.
/// - **Two-qubit entanglers**: `CX`, `CZ`, `SWAP`.
///
/// In controlled operations (specifically $CX$), **Operand 0 is control** and **Operand 1 is target**.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gate {
    /// Pauli X (bit flip): $\begin{pmatrix} 0 & 1 \\ 1 & 0 \end{pmatrix}$.
    X,
    /// Pauli Y: $\begin{pmatrix} 0 & -i \\ i & 0 \end{pmatrix}$.
    Y,
    /// Pauli Z (phase flip): $\begin{pmatrix} 1 & 0 \\ 0 & -1 \end{pmatrix}$.
    Z,
    /// Hadamard: $\frac{1}{\sqrt{2}} \begin{pmatrix} 1 & 1 \\ 1 & -1 \end{pmatrix}$.
    H,
    /// Phase S gate ($\sqrt{Z}$): $\begin{pmatrix} 1 & 0 \\ 0 & i \end{pmatrix}$.
    S,
    /// Phase Sdg gate ($S^\dagger$): $\begin{pmatrix} 1 & 0 \\ 0 & -i \end{pmatrix}$.
    Sdg,
    /// Phase T gate ($\sqrt{S}$): $\begin{pmatrix} 1 & 0 \\ 0 & e^{i\pi/4} \end{pmatrix}$.
    T,
    /// Phase Tdg gate ($T^\dagger$): $\begin{pmatrix} 1 & 0 \\ 0 & e^{-i\pi/4} \end{pmatrix}$.
    Tdg,
    /// Rotation around X-axis: $R_x(\theta) = \exp(-i \frac{\theta}{2} X)$.
    Rx(Angle),
    /// Rotation around Y-axis: $R_y(\theta) = \exp(-i \frac{\theta}{2} Y)$.
    Ry(Angle),
    /// Rotation around Z-axis: $R_z(\theta) = \exp(-i \frac{\theta}{2} Z)$.
    Rz(Angle),
    /// Controlled-NOT: Operand 0 is control, Operand 1 is target.
    CX,
    /// Controlled-Z: Operand 0 and Operand 1 are symmetric.
    CZ,
    /// SWAP: exchanges state of Operand 0 and Operand 1.
    SWAP,
}

impl Gate {
    /// Returns the canonical lowercase mnemonic name for the gate.
    #[inline]
    pub fn name(&self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
            Self::H => "h",
            Self::S => "s",
            Self::Sdg => "sdg",
            Self::T => "t",
            Self::Tdg => "tdg",
            Self::Rx(_) => "rx",
            Self::Ry(_) => "ry",
            Self::Rz(_) => "rz",
            Self::CX => "cx",
            Self::CZ => "cz",
            Self::SWAP => "swap",
        }
    }

    /// Returns the canonical [`GateId`](crate::gate_signature::GateId) for this gate.
    #[inline]
    pub fn gate_id(&self) -> crate::gate_signature::GateId {
        match self {
            Self::X => crate::gate_signature::GateId::X,
            Self::Y => crate::gate_signature::GateId::Y,
            Self::Z => crate::gate_signature::GateId::Z,
            Self::H => crate::gate_signature::GateId::H,
            Self::S => crate::gate_signature::GateId::S,
            Self::Sdg => crate::gate_signature::GateId::Sdg,
            Self::T => crate::gate_signature::GateId::T,
            Self::Tdg => crate::gate_signature::GateId::Tdg,
            Self::Rx(_) => crate::gate_signature::GateId::Rx,
            Self::Ry(_) => crate::gate_signature::GateId::Ry,
            Self::Rz(_) => crate::gate_signature::GateId::Rz,
            Self::CX => crate::gate_signature::GateId::CX,
            Self::CZ => crate::gate_signature::GateId::CZ,
            Self::SWAP => crate::gate_signature::GateId::SWAP,
        }
    }

    /// Returns the number of qubit operands required by this gate (arity).
    #[inline]
    pub fn arity(&self) -> usize {
        match self {
            Self::X
            | Self::Y
            | Self::Z
            | Self::H
            | Self::S
            | Self::Sdg
            | Self::T
            | Self::Tdg
            | Self::Rx(_)
            | Self::Ry(_)
            | Self::Rz(_) => 1,
            Self::CX | Self::CZ | Self::SWAP => 2,
        }
    }

    /// Returns the number of continuous parameters required by this gate.
    #[inline]
    pub fn num_parameters(&self) -> usize {
        match self {
            Self::Rx(_) | Self::Ry(_) | Self::Rz(_) => 1,
            _ => 0,
        }
    }

    /// Returns `true` if this is a single-qubit gate.
    #[inline]
    pub fn is_single_qubit(&self) -> bool {
        self.arity() == 1
    }

    /// Returns `true` if this is a two-qubit gate.
    #[inline]
    pub fn is_two_qubit(&self) -> bool {
        self.arity() == 2
    }

    /// Returns `true` if the gate is parameterized.
    #[inline]
    pub fn is_parameterized(&self) -> bool {
        self.num_parameters() > 0
    }

    /// Returns `true` if the gate action is symmetric under operand permutation.
    ///
    /// For two-qubit gates:
    /// - $CZ(q_0, q_1) = CZ(q_1, q_0)$ is symmetric.
    /// - $SWAP(q_0, q_1) = SWAP(q_1, q_0)$ is symmetric.
    /// - $CX(q_0, q_1) \neq CX(q_1, q_0)$ is asymmetric (direction-sensitive).
    #[inline]
    pub fn is_symmetric(&self) -> bool {
        matches!(self, Self::CZ | Self::SWAP)
    }

    /// Computes the exact mathematical inverse $U^\dagger$ of the gate.
    ///
    /// - Self-inverse gates ($X, Y, Z, H, CX, CZ, SWAP$) return themselves.
    /// - S and Sdg invert each other.
    /// - T and Tdg invert each other.
    /// - Rotation gates negate their rotation angle: $R(\theta)^{-1} = R(-\theta)$.
    pub fn inverse(&self) -> Self {
        match self {
            Self::X => Self::X,
            Self::Y => Self::Y,
            Self::Z => Self::Z,
            Self::H => Self::H,
            Self::S => Self::Sdg,
            Self::Sdg => Self::S,
            Self::T => Self::Tdg,
            Self::Tdg => Self::T,
            Self::Rx(theta) => Self::Rx(-*theta),
            Self::Ry(theta) => Self::Ry(-*theta),
            Self::Rz(theta) => Self::Rz(-*theta),
            Self::CX => Self::CX,
            Self::CZ => Self::CZ,
            Self::SWAP => Self::SWAP,
        }
    }

    /// Returns the $2 \times 2$ unitary matrix for single-qubit gates, or `None` if two-qubit.
    pub fn matrix2x2(&self) -> Option<Matrix2x2> {
        match self {
            Self::X => Some(matrix_x()),
            Self::Y => Some(matrix_y()),
            Self::Z => Some(matrix_z()),
            Self::H => Some(matrix_h()),
            Self::S => Some(matrix_s()),
            Self::Sdg => Some(matrix_sdg()),
            Self::T => Some(matrix_t()),
            Self::Tdg => Some(matrix_tdg()),
            Self::Rx(theta) => Some(matrix_rx(*theta)),
            Self::Ry(theta) => Some(matrix_ry(*theta)),
            Self::Rz(theta) => Some(matrix_rz(*theta)),
            Self::CX | Self::CZ | Self::SWAP => None,
        }
    }

    /// Returns the $4 \times 4$ unitary matrix for two-qubit gates, or `None` if single-qubit.
    pub fn matrix4x4(&self) -> Option<Matrix4x4> {
        match self {
            Self::CX => Some(matrix_cx()),
            Self::CZ => Some(matrix_cz()),
            Self::SWAP => Some(matrix_swap()),
            _ => None,
        }
    }
}

impl fmt::Display for Gate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rx(theta) => write!(f, "rx({:.6})", theta.radians()),
            Self::Ry(theta) => write!(f, "ry({:.6})", theta.radians()),
            Self::Rz(theta) => write!(f, "rz({:.6})", theta.radians()),
            other => write!(f, "{}", other.name()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOLERANCE: f64 = 1e-12;

    #[test]
    fn test_gate_metadata() {
        assert_eq!(Gate::H.arity(), 1);
        assert_eq!(Gate::H.num_parameters(), 0);
        assert_eq!(Gate::CX.arity(), 2);
        assert_eq!(Gate::CX.num_parameters(), 0);
        assert_eq!(Gate::Rx(Angle::zero()).arity(), 1);
        assert_eq!(Gate::Rx(Angle::zero()).num_parameters(), 1);
    }

    #[test]
    fn test_gate_symmetry() {
        assert!(Gate::CZ.is_symmetric());
        assert!(Gate::SWAP.is_symmetric());
        assert!(!Gate::CX.is_symmetric());
    }

    #[test]
    fn test_gate_inverse_definitions() {
        assert_eq!(Gate::X.inverse(), Gate::X);
        assert_eq!(Gate::S.inverse(), Gate::Sdg);
        assert_eq!(Gate::Sdg.inverse(), Gate::S);
        assert_eq!(Gate::T.inverse(), Gate::Tdg);
        assert_eq!(Gate::Tdg.inverse(), Gate::T);
        let theta = Angle::from_radians(1.234);
        assert_eq!(Gate::Rx(theta).inverse(), Gate::Rx(-theta));
        assert_eq!(Gate::Ry(theta).inverse(), Gate::Ry(-theta));
        assert_eq!(Gate::Rz(theta).inverse(), Gate::Rz(-theta));
        assert_eq!(Gate::CX.inverse(), Gate::CX);
        assert_eq!(Gate::CZ.inverse(), Gate::CZ);
        assert_eq!(Gate::SWAP.inverse(), Gate::SWAP);
    }

    #[test]
    fn test_gate_matrix_inverse_composition() {
        // Test all 1-qubit gates: matrix * matrix_inverse == Identity
        let gates = [
            Gate::X,
            Gate::Y,
            Gate::Z,
            Gate::H,
            Gate::S,
            Gate::Sdg,
            Gate::T,
            Gate::Tdg,
            Gate::Rx(Angle::from_radians(0.7)),
            Gate::Ry(Angle::from_radians(1.3)),
            Gate::Rz(Angle::from_radians(-2.5)),
        ];

        for g in gates {
            let m = g.matrix2x2().unwrap();
            let inv_m = g.inverse().matrix2x2().unwrap();
            let prod = m.matmul(&inv_m);
            assert!(
                prod.approx_eq(&Matrix2x2::identity(), TOLERANCE),
                "Gate {} * inverse failed",
                g
            );
        }

        // Test 2-qubit gates
        for g in [Gate::CX, Gate::CZ, Gate::SWAP] {
            let m = g.matrix4x4().unwrap();
            let inv_m = g.inverse().matrix4x4().unwrap();
            let prod = m.matmul(&inv_m);
            assert!(
                prod.approx_eq(&Matrix4x4::identity(), TOLERANCE),
                "Gate {} * inverse failed",
                g
            );
        }
    }
}
