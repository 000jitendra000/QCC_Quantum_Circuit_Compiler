use std::f64::consts::FRAC_1_SQRT_2;
use std::fmt;

use num_complex::Complex64;

use crate::quantum::parameter::Angle;

/// A $2 \times 2$ complex matrix for single-qubit unitary operators.
#[derive(Clone, Copy, PartialEq)]
pub struct Matrix2x2(pub [[Complex64; 2]; 2]);

impl Matrix2x2 {
    /// Identity matrix $I_2$.
    #[inline]
    pub const fn identity() -> Self {
        Self([
            [Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)],
            [Complex64::new(0.0, 0.0), Complex64::new(1.0, 0.0)],
        ])
    }

    /// Creates a new matrix from 4 elements in row-major order: `[[m00, m01], [m10, m11]]`.
    #[inline]
    pub const fn from_rows(rows: [[Complex64; 2]; 2]) -> Self {
        Self(rows)
    }

    /// Returns element at `(row, col)`.
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> Complex64 {
        self.0[row][col]
    }

    /// Matrix product $A \cdot B$.
    pub fn matmul(&self, other: &Self) -> Self {
        let mut res = [[Complex64::new(0.0, 0.0); 2]; 2];
        for (r, row) in res.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                for k in 0..2 {
                    *cell += self.0[r][k] * other.0[k][c];
                }
            }
        }
        Self(res)
    }

    /// Conjugate transpose (Hermitian adjoint) $U^\dagger = (U^*)^T$.
    pub fn adjoint(&self) -> Self {
        Self([
            [self.0[0][0].conj(), self.0[1][0].conj()],
            [self.0[0][1].conj(), self.0[1][1].conj()],
        ])
    }

    /// Trace of the matrix: $\text{Tr}(M) = \sum M_{ii}$.
    pub fn trace(&self) -> Complex64 {
        self.0[0][0] + self.0[1][1]
    }

    /// Frobenius norm: $\|M\|_F = \sqrt{\sum |M_{ij}|^2}$.
    pub fn frobenius_norm(&self) -> f64 {
        let mut sum = 0.0;
        for row in &self.0 {
            for elem in row {
                sum += elem.norm_sqr();
            }
        }
        sum.sqrt()
    }

    /// Returns `true` if all elements equal `other` within the specified tolerance.
    pub fn approx_eq(&self, other: &Self, tolerance: f64) -> bool {
        for r in 0..2 {
            for c in 0..2 {
                if (self.0[r][c] - other.0[r][c]).norm() > tolerance {
                    return false;
                }
            }
        }
        true
    }

    /// Verifies that the matrix is unitary: $U^\dagger U = I$ within tolerance.
    pub fn is_unitary(&self, tolerance: f64) -> bool {
        let prod = self.adjoint().matmul(self);
        prod.approx_eq(&Self::identity(), tolerance)
    }
}

impl fmt::Debug for Matrix2x2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[[{:.4}+{:.4}i, {:.4}+{:.4}i], [{:.4}+{:.4}i, {:.4}+{:.4}i]]",
            self.0[0][0].re,
            self.0[0][0].im,
            self.0[0][1].re,
            self.0[0][1].im,
            self.0[1][0].re,
            self.0[1][0].im,
            self.0[1][0].re,
            self.0[1][0].im,
        )
    }
}

/// A $4 \times 4$ complex matrix for two-qubit unitary operators.
#[derive(Clone, Copy, PartialEq)]
pub struct Matrix4x4(pub [[Complex64; 4]; 4]);

impl Matrix4x4 {
    /// Identity matrix $I_4$.
    #[inline]
    pub const fn identity() -> Self {
        Self([
            [
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            [
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            [
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            [
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
            ],
        ])
    }

    /// Creates a matrix from rows.
    #[inline]
    pub const fn from_rows(rows: [[Complex64; 4]; 4]) -> Self {
        Self(rows)
    }

    /// Returns element at `(row, col)`.
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> Complex64 {
        self.0[row][col]
    }

    /// Matrix product $A \cdot B$.
    pub fn matmul(&self, other: &Self) -> Self {
        let mut res = [[Complex64::new(0.0, 0.0); 4]; 4];
        for (r, row) in res.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                for k in 0..4 {
                    *cell += self.0[r][k] * other.0[k][c];
                }
            }
        }
        Self(res)
    }

    /// Conjugate transpose (Hermitian adjoint) $U^\dagger = (U^*)^T$.
    pub fn adjoint(&self) -> Self {
        let mut res = [[Complex64::new(0.0, 0.0); 4]; 4];
        for (r, row) in res.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                *cell = self.0[c][r].conj();
            }
        }
        Self(res)
    }

    /// Trace of the matrix: $\text{Tr}(M) = \sum M_{ii}$.
    pub fn trace(&self) -> Complex64 {
        self.0[0][0] + self.0[1][1] + self.0[2][2] + self.0[3][3]
    }

    /// Frobenius norm: $\|M\|_F = \sqrt{\sum |M_{ij}|^2}$.
    pub fn frobenius_norm(&self) -> f64 {
        let mut sum = 0.0;
        for row in &self.0 {
            for elem in row {
                sum += elem.norm_sqr();
            }
        }
        sum.sqrt()
    }

    /// Returns `true` if all elements equal `other` within the specified tolerance.
    pub fn approx_eq(&self, other: &Self, tolerance: f64) -> bool {
        for r in 0..4 {
            for c in 0..4 {
                if (self.0[r][c] - other.0[r][c]).norm() > tolerance {
                    return false;
                }
            }
        }
        true
    }

    /// Verifies that the matrix is unitary: $U^\dagger U = I$ within tolerance.
    pub fn is_unitary(&self, tolerance: f64) -> bool {
        let prod = self.adjoint().matmul(self);
        prod.approx_eq(&Self::identity(), tolerance)
    }
}

impl fmt::Debug for Matrix4x4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "[")?;
        for row in 0..4 {
            write!(f, "  [")?;
            for col in 0..4 {
                write!(
                    f,
                    "{:.4}+{:.4}i, ",
                    self.0[row][col].re, self.0[row][col].im
                )?;
            }
            writeln!(f, "],")?;
        }
        write!(f, "]")
    }
}

/// Computes the Kronecker tensor product $A \otimes B$ of two $2 \times 2$ matrices.
///
/// In standard matrix Kronecker product ordering:
/// $$(A \otimes B)_{2i+k, 2j+l} = A_{i,j} \cdot B_{k,l}$$
///
/// Under little-endian statevector indexing where computational basis vectors are $|q_1 q_0\rangle$:
/// - $q_0$ (LSB) corresponds to the second (inner) tensor factor.
/// - $q_1$ (MSB) corresponds to the first (outer) tensor factor.
///
/// Thus, if operator $U_0$ acts on $q_0$ and $U_1$ acts on $q_1$, the composite matrix on basis
/// $\{|00\rangle, |01\rangle, |10\rangle, |11\rangle\}$ is $U_1 \otimes U_0$.
pub fn kron_2x2(a: &Matrix2x2, b: &Matrix2x2) -> Matrix4x4 {
    let mut res = [[Complex64::new(0.0, 0.0); 4]; 4];
    for r1 in 0..2 {
        for c1 in 0..2 {
            let a_elem = a.get(r1, c1);
            for r2 in 0..2 {
                for c2 in 0..2 {
                    let b_elem = b.get(r2, c2);
                    res[2 * r1 + r2][2 * c1 + c2] = a_elem * b_elem;
                }
            }
        }
    }
    Matrix4x4(res)
}

// ============================================================================
// Single-Qubit Gate Matrices
// ============================================================================

/// Pauli X gate matrix: $\begin{pmatrix} 0 & 1 \\ 1 & 0 \end{pmatrix}$.
#[inline]
pub fn matrix_x() -> Matrix2x2 {
    Matrix2x2([
        [Complex64::new(0.0, 0.0), Complex64::new(1.0, 0.0)],
        [Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)],
    ])
}

/// Pauli Y gate matrix: $\begin{pmatrix} 0 & -i \\ i & 0 \end{pmatrix}$.
#[inline]
pub fn matrix_y() -> Matrix2x2 {
    Matrix2x2([
        [Complex64::new(0.0, 0.0), Complex64::new(0.0, -1.0)],
        [Complex64::new(0.0, 1.0), Complex64::new(0.0, 0.0)],
    ])
}

/// Pauli Z gate matrix: $\begin{pmatrix} 1 & 0 \\ 0 & -1 \end{pmatrix}$.
#[inline]
pub fn matrix_z() -> Matrix2x2 {
    Matrix2x2([
        [Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)],
        [Complex64::new(0.0, 0.0), Complex64::new(-1.0, 0.0)],
    ])
}

/// Hadamard gate matrix: $\frac{1}{\sqrt{2}} \begin{pmatrix} 1 & 1 \\ 1 & -1 \end{pmatrix}$.
#[inline]
pub fn matrix_h() -> Matrix2x2 {
    Matrix2x2([
        [
            Complex64::new(FRAC_1_SQRT_2, 0.0),
            Complex64::new(FRAC_1_SQRT_2, 0.0),
        ],
        [
            Complex64::new(FRAC_1_SQRT_2, 0.0),
            Complex64::new(-FRAC_1_SQRT_2, 0.0),
        ],
    ])
}

/// Phase S gate matrix: $\begin{pmatrix} 1 & 0 \\ 0 & i \end{pmatrix}$.
#[inline]
pub fn matrix_s() -> Matrix2x2 {
    Matrix2x2([
        [Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)],
        [Complex64::new(0.0, 0.0), Complex64::new(0.0, 1.0)],
    ])
}

/// Phase Sdg ($S^\dagger$) gate matrix: $\begin{pmatrix} 1 & 0 \\ 0 & -i \end{pmatrix}$.
#[inline]
pub fn matrix_sdg() -> Matrix2x2 {
    Matrix2x2([
        [Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)],
        [Complex64::new(0.0, 0.0), Complex64::new(0.0, -1.0)],
    ])
}

/// T gate matrix: $\begin{pmatrix} 1 & 0 \\ 0 & e^{i\pi/4} \end{pmatrix} = \begin{pmatrix} 1 & 0 \\ 0 & \frac{1+i}{\sqrt{2}} \end{pmatrix}$.
#[inline]
pub fn matrix_t() -> Matrix2x2 {
    Matrix2x2([
        [Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2),
        ],
    ])
}

/// Tdg ($T^\dagger$) gate matrix: $\begin{pmatrix} 1 & 0 \\ 0 & e^{-i\pi/4} \end{pmatrix} = \begin{pmatrix} 1 & 0 \\ 0 & \frac{1-i}{\sqrt{2}} \end{pmatrix}$.
#[inline]
pub fn matrix_tdg() -> Matrix2x2 {
    Matrix2x2([
        [Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
        ],
    ])
}

/// Rotation around X-axis:
/// $$R_x(\theta) = \exp\left(-i \frac{\theta}{2} X\right) = \begin{pmatrix} \cos(\theta/2) & -i \sin(\theta/2) \\ -i \sin(\theta/2) & \cos(\theta/2) \end{pmatrix}$$
pub fn matrix_rx(theta: Angle) -> Matrix2x2 {
    let half = theta.radians() / 2.0;
    let cos_half = half.cos();
    let sin_half = half.sin();
    Matrix2x2([
        [
            Complex64::new(cos_half, 0.0),
            Complex64::new(0.0, -sin_half),
        ],
        [
            Complex64::new(0.0, -sin_half),
            Complex64::new(cos_half, 0.0),
        ],
    ])
}

/// Rotation around Y-axis:
/// $$R_y(\theta) = \exp\left(-i \frac{\theta}{2} Y\right) = \begin{pmatrix} \cos(\theta/2) & -\sin(\theta/2) \\ \sin(\theta/2) & \cos(\theta/2) \end{pmatrix}$$
pub fn matrix_ry(theta: Angle) -> Matrix2x2 {
    let half = theta.radians() / 2.0;
    let cos_half = half.cos();
    let sin_half = half.sin();
    Matrix2x2([
        [
            Complex64::new(cos_half, 0.0),
            Complex64::new(-sin_half, 0.0),
        ],
        [Complex64::new(sin_half, 0.0), Complex64::new(cos_half, 0.0)],
    ])
}

/// Rotation around Z-axis:
/// $$R_z(\theta) = \exp\left(-i \frac{\theta}{2} Z\right) = \begin{pmatrix} e^{-i \theta/2} & 0 \\ 0 & e^{i \theta/2} \end{pmatrix}$$
pub fn matrix_rz(theta: Angle) -> Matrix2x2 {
    let half = theta.radians() / 2.0;
    let cos_half = half.cos();
    let sin_half = half.sin();
    Matrix2x2([
        [
            Complex64::new(cos_half, -sin_half),
            Complex64::new(0.0, 0.0),
        ],
        [Complex64::new(0.0, 0.0), Complex64::new(cos_half, sin_half)],
    ])
}

// ============================================================================
// Two-Qubit Gate Matrices (Basis: {|00>, |01>, |10>, |11>} where q0 is LSB)
// ============================================================================

/// Controlled-NOT ($CX$) matrix where $q_0$ is control and $q_1$ is target:
/// - $|00\rangle \to |00\rangle$ (row 0: 1)
/// - $|01\rangle \to |11\rangle$ (row 3: 1)
/// - $|10\rangle \to |10\rangle$ (row 2: 1)
/// - $|11\rangle \to |01\rangle$ (row 1: 1)
///
/// $$\begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 0 & 0 & 1 \\ 0 & 0 & 1 & 0 \\ 0 & 1 & 0 & 0 \end{pmatrix}$$
pub fn matrix_cx_c0_t1() -> Matrix4x4 {
    Matrix4x4([
        [
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
    ])
}

/// Controlled-NOT ($CX$) matrix where $q_1$ is control and $q_0$ is target:
/// - $|00\rangle \to |00\rangle$
/// - $|01\rangle \to |01\rangle$
/// - $|10\rangle \to |11\rangle$
/// - $|11\rangle \to |10\rangle$
///
/// $$\begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 1 & 0 & 0 \\ 0 & 0 & 0 & 1 \\ 0 & 0 & 1 & 0 \end{pmatrix}$$
pub fn matrix_cx_c1_t0() -> Matrix4x4 {
    Matrix4x4([
        [
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
    ])
}

/// Default canonical Controlled-NOT ($CX$) matrix where operand 0 ($q_0$) is control
/// and operand 1 ($q_1$) is target.
#[inline]
pub fn matrix_cx() -> Matrix4x4 {
    matrix_cx_c0_t1()
}

/// Controlled-Z ($CZ$) gate matrix: $\text{diag}(1, 1, 1, -1)$.
///
/// Symmetric under operand interchange: $CZ(q_0, q_1) = CZ(q_1, q_0)$.
pub fn matrix_cz() -> Matrix4x4 {
    Matrix4x4([
        [
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(-1.0, 0.0),
        ],
    ])
}

/// SWAP gate matrix: exchanges states of two qubits:
/// - $|00\rangle \to |00\rangle$
/// - $|01\rangle \to |10\rangle$
/// - $|10\rangle \to |01\rangle$
/// - $|11\rangle \to |11\rangle$
///
/// $$\begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 0 & 1 & 0 \\ 0 & 1 & 0 & 0 \\ 0 & 0 & 0 & 1 \end{pmatrix}$$
pub fn matrix_swap() -> Matrix4x4 {
    Matrix4x4([
        [
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
        [
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOLERANCE: f64 = 1e-12;

    #[test]
    fn test_pauli_unitarity() {
        assert!(matrix_x().is_unitary(TOLERANCE));
        assert!(matrix_y().is_unitary(TOLERANCE));
        assert!(matrix_z().is_unitary(TOLERANCE));
    }

    #[test]
    fn test_hadamard_unitarity_and_self_inverse() {
        let h = matrix_h();
        assert!(h.is_unitary(TOLERANCE));
        let h2 = h.matmul(&h);
        assert!(h2.approx_eq(&Matrix2x2::identity(), TOLERANCE));
    }

    #[test]
    fn test_s_sdg_inverse() {
        let s = matrix_s();
        let sdg = matrix_sdg();
        assert!(s.is_unitary(TOLERANCE));
        assert!(sdg.is_unitary(TOLERANCE));
        assert!(s.matmul(&sdg).approx_eq(&Matrix2x2::identity(), TOLERANCE));
        assert!(sdg.matmul(&s).approx_eq(&Matrix2x2::identity(), TOLERANCE));
    }

    #[test]
    fn test_t_tdg_inverse() {
        let t = matrix_t();
        let tdg = matrix_tdg();
        assert!(t.is_unitary(TOLERANCE));
        assert!(tdg.is_unitary(TOLERANCE));
        assert!(t.matmul(&tdg).approx_eq(&Matrix2x2::identity(), TOLERANCE));
        assert!(tdg.matmul(&t).approx_eq(&Matrix2x2::identity(), TOLERANCE));
    }

    #[test]
    fn test_two_qubit_gates_unitarity() {
        assert!(matrix_cx().is_unitary(TOLERANCE));
        assert!(matrix_cx_c1_t0().is_unitary(TOLERANCE));
        assert!(matrix_cz().is_unitary(TOLERANCE));
        assert!(matrix_swap().is_unitary(TOLERANCE));
    }

    #[test]
    fn test_cx_orientation_difference() {
        let cx01 = matrix_cx_c0_t1();
        let cx10 = matrix_cx_c1_t0();
        assert!(!cx01.approx_eq(&cx10, TOLERANCE));
    }

    #[test]
    fn test_swap_cx_decomposition() {
        // SWAP = CX(a, b) * CX(b, a) * CX(a, b) in circuit execution order.
        // In left-multiplication order (B * A for A before B):
        // Op 1: CX(q0, q1) => matrix cx01
        // Op 2: CX(q1, q0) => matrix cx10
        // Op 3: CX(q0, q1) => matrix cx01
        // Combined = Op3 * Op2 * Op1 = cx01 * cx10 * cx01
        let cx01 = matrix_cx_c0_t1();
        let cx10 = matrix_cx_c1_t0();
        let swap_decomp = cx01.matmul(&cx10).matmul(&cx01);
        assert!(swap_decomp.approx_eq(&matrix_swap(), TOLERANCE));
    }

    #[test]
    fn test_kronecker_product_little_endian() {
        // Test U1 on q1 and U0 on q0 => kron(U1, U0)
        let x = matrix_x();
        let z = matrix_z();
        let joint = kron_2x2(&z, &x); // Z on q1, X on q0
        assert!(joint.is_unitary(TOLERANCE));
    }
}
