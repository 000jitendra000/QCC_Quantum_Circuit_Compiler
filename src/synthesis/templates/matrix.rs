//! Complex $8 \times 8$ matrix representation and operator embeddings for 3-qubit systems.
//!
//! Preserves QCC's little-endian qubit indexing convention where qubit $q_0$ is the least-significant bit (LSB).

use num_complex::Complex64;

use crate::quantum::gate_matrix::Matrix2x2;

/// A $8 \times 8$ complex matrix for 3-qubit unitary operators.
#[derive(Clone, Copy, PartialEq)]
pub struct Matrix8x8(pub [[Complex64; 8]; 8]);

impl Matrix8x8 {
    /// Identity matrix $I_8$.
    #[inline]
    pub const fn identity() -> Self {
        let mut rows = [[Complex64::new(0.0, 0.0); 8]; 8];
        let mut i = 0;
        while i < 8 {
            rows[i][i] = Complex64::new(1.0, 0.0);
            i += 1;
        }
        Self(rows)
    }

    /// Creates a matrix from rows.
    #[inline]
    pub const fn from_rows(rows: [[Complex64; 8]; 8]) -> Self {
        Self(rows)
    }

    /// Returns the element at `(row, col)`.
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> Complex64 {
        self.0[row][col]
    }

    /// Matrix product $A \cdot B$.
    pub fn matmul(&self, other: &Self) -> Self {
        let mut res = [[Complex64::new(0.0, 0.0); 8]; 8];
        for (r, row) in res.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                for k in 0..8 {
                    *cell += self.0[r][k] * other.0[k][c];
                }
            }
        }
        Self(res)
    }

    /// Conjugate transpose (Hermitian adjoint) $U^\dagger = (U^*)^T$.
    pub fn adjoint(&self) -> Self {
        let mut res = [[Complex64::new(0.0, 0.0); 8]; 8];
        for (r, row) in res.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                *cell = self.0[c][r].conj();
            }
        }
        Self(res)
    }

    /// Maximum pointwise absolute difference between matrix elements:
    /// $\max_{i, j} |A_{ij} - B_{ij}|$.
    pub fn max_difference(&self, other: &Self) -> f64 {
        let mut max_diff = 0.0f64;
        for r in 0..8 {
            for c in 0..8 {
                let diff = (self.0[r][c] - other.0[r][c]).norm();
                if diff > max_diff {
                    max_diff = diff;
                }
            }
        }
        max_diff
    }

    /// Frobenius norm difference: $\|A - B\|_F = \sqrt{\sum |A_{ij} - B_{ij}|^2}$.
    pub fn frobenius_norm_diff(&self, other: &Self) -> f64 {
        let mut sum_sq = 0.0f64;
        for r in 0..8 {
            for c in 0..8 {
                let diff = (self.0[r][c] - other.0[r][c]).norm();
                sum_sq += diff * diff;
            }
        }
        sum_sq.sqrt()
    }

    /// Returns `true` if this matrix is approximately equal to `other` within `tolerance`.
    pub fn approx_eq(&self, other: &Self, tolerance: f64) -> bool {
        self.max_difference(other) <= tolerance
    }

    /// Checks if this matrix is unitary within `tolerance`: $U^\dagger U \approx I$.
    pub fn is_unitary(&self, tolerance: f64) -> bool {
        let prod = self.adjoint().matmul(self);
        prod.approx_eq(&Self::identity(), tolerance)
    }

    /// Embeds a single-qubit unitary gate into 3-qubit state space.
    ///
    /// Preserves little-endian indexing where basis index $i = q_0 + 2 q_1 + 4 q_2$.
    #[allow(clippy::needless_range_loop)]
    pub fn embed_single_qubit(gate: &Matrix2x2, target_qubit: usize) -> Self {
        assert!(
            target_qubit < 3,
            "target qubit index must be in [0, 2] for 3-qubit space"
        );
        let mut rows = [[Complex64::new(0.0, 0.0); 8]; 8];

        for col in 0..8 {
            let target_bit = (col >> target_qubit) & 1;
            let col0 = col & !(1 << target_qubit);
            let col1 = col | (1 << target_qubit);

            if target_bit == 0 {
                // col == col0: maps to u00 * |col0> + u10 * |col1>
                rows[col0][col] += gate.0[0][0];
                rows[col1][col] += gate.0[1][0];
            } else {
                // col == col1: maps to u01 * |col0> + u11 * |col1>
                rows[col0][col] += gate.0[0][1];
                rows[col1][col] += gate.0[1][1];
            }
        }

        Self(rows)
    }

    /// Embeds a 2-qubit controlled-NOT (CX) gate into 3-qubit state space.
    ///
    /// Flips `target_qubit` when `control_qubit` is 1.
    #[allow(clippy::needless_range_loop)]
    pub fn embed_cx(control_qubit: usize, target_qubit: usize) -> Self {
        assert!(
            control_qubit < 3 && target_qubit < 3 && control_qubit != target_qubit,
            "control and target must be distinct qubits in [0, 2]"
        );
        let mut rows = [[Complex64::new(0.0, 0.0); 8]; 8];

        for col in 0..8 {
            let ctrl_bit = (col >> control_qubit) & 1;
            let row = if ctrl_bit == 1 {
                col ^ (1 << target_qubit)
            } else {
                col
            };
            rows[row][col] = Complex64::new(1.0, 0.0);
        }

        Self(rows)
    }

    /// Constructs the exact standard reference Toffoli (CCX) matrix on 3 qubits.
    ///
    /// Flips `target_qubit` if and only if both `ctrl1` and `ctrl2` are 1.
    #[allow(clippy::needless_range_loop)]
    pub fn exact_toffoli(ctrl1: usize, ctrl2: usize, target_qubit: usize) -> Self {
        assert!(
            ctrl1 < 3 && ctrl2 < 3 && target_qubit < 3,
            "qubit indices must be in [0, 2]"
        );
        assert!(
            ctrl1 != ctrl2 && ctrl1 != target_qubit && ctrl2 != target_qubit,
            "all three qubits must be distinct"
        );
        let mut rows = [[Complex64::new(0.0, 0.0); 8]; 8];

        for col in 0..8 {
            let c1 = (col >> ctrl1) & 1;
            let c2 = (col >> ctrl2) & 1;
            let row = if c1 == 1 && c2 == 1 {
                col ^ (1 << target_qubit)
            } else {
                col
            };
            rows[row][col] = Complex64::new(1.0, 0.0);
        }

        Self(rows)
    }
}

impl std::fmt::Debug for Matrix8x8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Matrix8x8 [")?;
        for r in 0..8 {
            write!(f, "  [")?;
            for c in 0..8 {
                let elem = self.0[r][c];
                if c > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "({:.3}+{:.3}i)", elem.re, elem.im)?;
            }
            writeln!(f, "]")?;
        }
        write!(f, "]")
    }
}
