//! Native single-qubit unitary synthesis into target instruction families.
//!
//! Synthesizes arbitrary one-qubit unitaries $U \in U(2)$ into declared hardware-native
//! instruction sets ($Z\text{-}Y\text{-}Z$, $Z\text{-}X\text{-}Z$, $RZ\text{-}SX$, and $U3$),
//! strictly adhering to target capabilities, angle normalization, and global phase preservation.
//!
//! # Core Invariants
//! - **No Assumed Target Operations**: Synthesizer never assumes a target supports any particular
//!   rotation or gate family. All paths are gated on explicit target capability verification.
//! - **Exact Global Phase**: Returns scalar global phase separately ($U = e^{i\alpha} \prod U_k$)
//!   with spinor sign compensation for $2\pi$ periodic shifts.
//! - **Target Legality Checked**: Every emitted operation is verified against the target's
//!   declared parameter domains, physical qubit capacity, and connectivity.
//! - **Deterministic Singularity Resolution**: Collinear gimbal-lock angles ($\theta \approx 0$
//!   and $\theta \approx \pi$) resolve deterministically via canonical Euler gauge conventions.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use crate::builder::CircuitBuilder;
use crate::decomposition::euler::{
    decompose_euler, DecompositionError, EulerBasis, DEFAULT_EULER_TOLERANCE,
};
use crate::ir::circuit::Circuit;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::wire::QubitId;
use crate::quantum::gate_matrix::{
    matrix_h, matrix_rx, matrix_ry, matrix_rz, matrix_sx, matrix_u3, matrix_x, Matrix2x2,
};
use crate::quantum::parameter::{Angle, GlobalPhase};
use crate::target::model::Target;
use crate::target::operation::PhysicalQubitId;
use crate::target::query::LegalityError;
use crate::target::synthesis_basis::SingleQubitBasis;

/// Two-pi mathematical constant.
const TWO_PI: f64 = 2.0 * PI;

/// Normalizes an angle in radians into the canonical half-open range `[0, 2π)`.
///
/// Returns `(normalized_radians, shift_count)` such that:
/// $$\theta = \theta_{\text{norm}} + 2\pi k$$
///
/// Because single-qubit spinor rotation operators ($R_x, R_y, R_z$) satisfy
/// $R(\theta + 2\pi) = -R(\theta) = e^{i\pi} R(\theta)$, an odd number of $2\pi$ shifts
/// induces a sign inversion in the rotation operator, which must be compensated in
/// the circuit-level global phase factor:
/// $$\Delta \phi = (k \bmod 2) \cdot \pi$$
pub fn normalize_angle_with_spinor_shift(rad: f64) -> (f64, i64) {
    let mut k = (rad / TWO_PI).floor() as i64;
    let mut norm = rad - (k as f64) * TWO_PI;
    if norm >= TWO_PI {
        norm = 0.0;
        k += 1;
    }
    if norm < 0.0 {
        norm += TWO_PI;
        k -= 1;
    }
    (norm, k)
}

/// Errors occurring during native single-qubit unitary synthesis.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SynthesisError {
    /// Unitary decomposition failed (e.g. non-finite or non-unitary input matrix).
    #[error("Euler decomposition failed: {0}")]
    Decomposition(#[from] DecompositionError),

    /// Target does not support the requested single-qubit synthesis basis on the target qubit.
    #[error("target does not support single-qubit basis '{basis}' on physical qubit {qubit}")]
    UnsupportedBasis {
        /// Requested synthesis basis.
        basis: SingleQubitBasis,
        /// Physical hardware qubit.
        qubit: PhysicalQubitId,
    },

    /// No supported single-qubit synthesis basis is available on the target device for this qubit.
    #[error("no supported single-qubit synthesis basis for physical qubit {qubit}; available operations: {available:?}")]
    NoSupportedBasis {
        /// Target physical qubit.
        qubit: PhysicalQubitId,
        /// List of available 1Q operations on this qubit.
        available: Vec<String>,
    },

    /// Target physical qubit index exceeds device capacity.
    #[error("physical qubit {qubit} is out of bounds for target with capacity {capacity}")]
    QubitOutOfBounds {
        /// Out-of-bounds physical qubit.
        qubit: PhysicalQubitId,
        /// Target capacity.
        capacity: usize,
    },

    /// An emitted operation violates target legality constraints.
    #[error("synthesized operation violates target legality: {0}")]
    Legality(#[from] LegalityError),

    /// Synthesized sequence does not reconstruct the target unitary within tolerance.
    #[error("reconstructed unitary discrepancy {max_diff:.6e} exceeds tolerance {tolerance:.6e}")]
    VerificationFailed {
        /// Measured element-wise difference.
        max_diff: f64,
        /// Allowed numerical tolerance.
        tolerance: f64,
    },
}

/// Result of synthesizing a single-qubit unitary into target-native hardware operations.
#[derive(Clone, Debug, PartialEq)]
pub struct SynthesizedOneQubitSequence {
    /// The synthesized physical operations in sequential circuit execution order (first operation executes first).
    operations: Vec<Operation>,
    /// Target physical qubit.
    physical_qubit: PhysicalQubitId,
    /// Extracted circuit-level global phase factor $e^{i\alpha}$ such that:
    /// $$U_{\text{target}} = e^{i\alpha} \cdot \prod_{k=1}^m U_k$$
    global_phase: GlobalPhase,
    /// Hardware instruction basis used for synthesis.
    basis: SingleQubitBasis,
}

impl SynthesizedOneQubitSequence {
    /// Creates a new synthesized single-qubit sequence.
    pub fn new(
        operations: Vec<Operation>,
        physical_qubit: PhysicalQubitId,
        global_phase: GlobalPhase,
        basis: SingleQubitBasis,
    ) -> Self {
        Self {
            operations,
            physical_qubit,
            global_phase,
            basis,
        }
    }

    /// Returns the synthesized operations in sequential circuit execution order.
    #[inline]
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    /// Consumes the result and returns the constituent operations.
    #[inline]
    pub fn into_operations(self) -> Vec<Operation> {
        self.operations
    }

    /// Returns the physical qubit targeted by this sequence.
    #[inline]
    pub fn physical_qubit(&self) -> PhysicalQubitId {
        self.physical_qubit
    }

    /// Returns the extracted circuit-level global phase factor.
    #[inline]
    pub fn global_phase(&self) -> GlobalPhase {
        self.global_phase
    }

    /// Returns the instruction basis family used for synthesis.
    #[inline]
    pub fn basis(&self) -> SingleQubitBasis {
        self.basis
    }

    /// Returns the number of physical operations emitted.
    #[inline]
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Returns `true` if zero operations were emitted (exact scalar identity).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Reconstructs the full $2 \times 2$ unitary matrix including the scalar global phase.
    ///
    /// Under QCC column-vector conventions, operations act by left multiplication. If the
    /// sequence contains operations $[O_1, O_2, \dots, O_m]$ where $O_1$ executes first,
    /// this evaluates:
    /// $$U_{\text{rec}} = e^{i\alpha} \cdot \left( O_m \cdot \dots \cdot O_2 \cdot O_1 \right)$$
    pub fn reconstruct_matrix(&self) -> Matrix2x2 {
        let mut v = Matrix2x2::identity();

        for op in &self.operations {
            let op_mat = match op.name() {
                "rz" => {
                    let rad = op.parameters()[0].as_constant().unwrap_or(0.0);
                    matrix_rz(Angle::from_radians(rad))
                }
                "rx" => {
                    let rad = op.parameters()[0].as_constant().unwrap_or(0.0);
                    matrix_rx(Angle::from_radians(rad))
                }
                "ry" => {
                    let rad = op.parameters()[0].as_constant().unwrap_or(0.0);
                    matrix_ry(Angle::from_radians(rad))
                }
                "sx" => matrix_sx(),
                "x" => matrix_x(),
                "u3" | "u" => {
                    let theta =
                        Angle::from_radians(op.parameters()[0].as_constant().unwrap_or(0.0));
                    let phi = Angle::from_radians(op.parameters()[1].as_constant().unwrap_or(0.0));
                    let lam = Angle::from_radians(op.parameters()[2].as_constant().unwrap_or(0.0));
                    matrix_u3(theta, phi, lam)
                }
                other => panic!("Unsupported synthesized operation mnemonic: {}", other),
            };
            // Left multiplication: new_v = op_mat * v
            v = op_mat.matmul(&v);
        }

        // Apply scalar global phase
        v.scale(self.global_phase.factor())
    }

    /// Verifies that the reconstructed matrix matches `target_matrix` within `tolerance`.
    pub fn verify_equivalence(
        &self,
        target_matrix: &Matrix2x2,
        tolerance: f64,
    ) -> Result<(), SynthesisError> {
        let rec = self.reconstruct_matrix();
        let max_diff = rec.max_difference(target_matrix);
        if max_diff > tolerance {
            return Err(SynthesisError::VerificationFailed {
                max_diff,
                tolerance,
            });
        }
        Ok(())
    }

    /// Verifies the legality of every emitted operation against `target`.
    pub fn verify_legality(&self, target: &Target) -> Result<(), LegalityError> {
        for op in &self.operations {
            let mut params = Vec::with_capacity(op.parameters().len());
            for p in op.parameters() {
                let val = p.as_constant().unwrap_or(f64::NAN);
                params.push(val);
            }
            target.check_operation_legality(op.name(), &[self.physical_qubit], &params)?;
        }
        Ok(())
    }

    /// Builds a standalone QccIR [`Circuit`] containing the synthesized operations and global phase.
    pub fn into_circuit(&self, name: impl Into<String>) -> Circuit {
        let mut builder = CircuitBuilder::new(name);
        let num_qubits = self.physical_qubit.index() + 1;
        builder
            .add_quantum_register("p", num_qubits)
            .expect("valid register");
        let mut circuit = builder.finish().expect("valid circuit");
        circuit.set_global_phase(ParameterExpr::constant(self.global_phase.angle().radians()));
        for op in &self.operations {
            let _ = circuit.add_operation(op.clone());
        }
        circuit
    }
}

/// Target-driven single-qubit unitary synthesis interface.
pub trait SingleQubitSynthesis {
    /// Synthesizes a one-qubit unitary matrix into the preferred native target instruction family for `qubit`.
    fn synthesize_single_qubit(
        &self,
        matrix: &Matrix2x2,
        qubit: PhysicalQubitId,
        target: &Target,
    ) -> Result<SynthesizedOneQubitSequence, SynthesisError>;

    /// Synthesizes a one-qubit unitary matrix into a specified target instruction family on `qubit`.
    fn synthesize_single_qubit_with_basis(
        &self,
        matrix: &Matrix2x2,
        qubit: PhysicalQubitId,
        target: &Target,
        basis: SingleQubitBasis,
    ) -> Result<SynthesizedOneQubitSequence, SynthesisError>;
}

/// Single-qubit unitary synthesizer.
#[derive(Clone, Copy, Debug)]
pub struct SingleQubitSynthesizer {
    /// Numerical tolerance for unitarity and matrix reconstruction verification.
    tolerance: f64,
}

impl Default for SingleQubitSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}

impl SingleQubitSynthesizer {
    /// Creates a synthesizer with default numerical tolerance ($10^{-10}$).
    pub fn new() -> Self {
        Self {
            tolerance: DEFAULT_EULER_TOLERANCE,
        }
    }

    /// Sets a custom numerical tolerance for matrix verification.
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Returns the configured numerical tolerance.
    #[inline]
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// Inherent convenience method for synthesizing a single-qubit unitary.
    #[inline]
    pub fn synthesize(
        &self,
        matrix: &Matrix2x2,
        qubit: PhysicalQubitId,
        target: &Target,
    ) -> Result<SynthesizedOneQubitSequence, SynthesisError> {
        self.synthesize_single_qubit(matrix, qubit, target)
    }

    /// Inherent convenience method for synthesizing into a specific target basis.
    #[inline]
    pub fn synthesize_with_basis(
        &self,
        matrix: &Matrix2x2,
        qubit: PhysicalQubitId,
        target: &Target,
        basis: SingleQubitBasis,
    ) -> Result<SynthesizedOneQubitSequence, SynthesisError> {
        self.synthesize_single_qubit_with_basis(matrix, qubit, target, basis)
    }
}

impl SingleQubitSynthesis for SingleQubitSynthesizer {
    fn synthesize_single_qubit(
        &self,
        matrix: &Matrix2x2,
        qubit: PhysicalQubitId,
        target: &Target,
    ) -> Result<SynthesizedOneQubitSequence, SynthesisError> {
        if qubit.index() >= target.num_qubits() {
            return Err(SynthesisError::QubitOutOfBounds {
                qubit,
                capacity: target.num_qubits(),
            });
        }

        let preferred_basis = target.preferred_single_qubit_basis(qubit).ok_or_else(|| {
            let available = target
                .operations()
                .values()
                .filter(|desc| desc.quantum_arity == 1 && desc.is_valid_operands(&[qubit]))
                .map(|desc| desc.name.as_str().to_string())
                .collect();
            SynthesisError::NoSupportedBasis { qubit, available }
        })?;

        self.synthesize_single_qubit_with_basis(matrix, qubit, target, preferred_basis)
    }

    fn synthesize_single_qubit_with_basis(
        &self,
        matrix: &Matrix2x2,
        qubit: PhysicalQubitId,
        target: &Target,
        basis: SingleQubitBasis,
    ) -> Result<SynthesizedOneQubitSequence, SynthesisError> {
        if qubit.index() >= target.num_qubits() {
            return Err(SynthesisError::QubitOutOfBounds {
                qubit,
                capacity: target.num_qubits(),
            });
        }

        if !basis.is_supported_by(target, qubit) {
            return Err(SynthesisError::UnsupportedBasis { basis, qubit });
        }

        let q_id = QubitId(qubit.index());

        let result = match basis {
            SingleQubitBasis::Zyz => {
                let euler = decompose_euler(matrix, EulerBasis::Zyz, self.tolerance)?;
                let (phi_raw, theta_val, lambda_raw) = euler.angles_rad();
                let alpha_raw = euler.global_phase.angle().radians();

                let (phi_norm, k_phi) = normalize_angle_with_spinor_shift(phi_raw);
                let (lambda_norm, k_lam) = normalize_angle_with_spinor_shift(lambda_raw);
                let total_k = k_phi + k_lam;
                let (alpha_norm, _) = normalize_angle_with_spinor_shift(
                    alpha_raw + ((total_k.rem_euclid(2)) as f64) * PI,
                );

                // Sequential execution order: Rz(lambda) -> Ry(theta) -> Rz(phi)
                let operations = vec![
                    Operation::custom_gate(
                        "rz",
                        vec![q_id],
                        vec![ParameterExpr::constant(lambda_norm)],
                    ),
                    Operation::custom_gate(
                        "ry",
                        vec![q_id],
                        vec![ParameterExpr::constant(theta_val)],
                    ),
                    Operation::custom_gate(
                        "rz",
                        vec![q_id],
                        vec![ParameterExpr::constant(phi_norm)],
                    ),
                ];

                SynthesizedOneQubitSequence::new(
                    operations,
                    qubit,
                    GlobalPhase::from_radians(alpha_norm),
                    basis,
                )
            }
            SingleQubitBasis::Zxz => {
                let euler = decompose_euler(matrix, EulerBasis::Zxz, self.tolerance)?;
                let (phi_raw, theta_val, lambda_raw) = euler.angles_rad();
                let alpha_raw = euler.global_phase.angle().radians();

                let (phi_norm, k_phi) = normalize_angle_with_spinor_shift(phi_raw);
                let (lambda_norm, k_lam) = normalize_angle_with_spinor_shift(lambda_raw);
                let total_k = k_phi + k_lam;
                let (alpha_norm, _) = normalize_angle_with_spinor_shift(
                    alpha_raw + ((total_k.rem_euclid(2)) as f64) * PI,
                );

                // Sequential execution order: Rz(lambda) -> Rx(theta) -> Rz(phi)
                let operations = vec![
                    Operation::custom_gate(
                        "rz",
                        vec![q_id],
                        vec![ParameterExpr::constant(lambda_norm)],
                    ),
                    Operation::custom_gate(
                        "rx",
                        vec![q_id],
                        vec![ParameterExpr::constant(theta_val)],
                    ),
                    Operation::custom_gate(
                        "rz",
                        vec![q_id],
                        vec![ParameterExpr::constant(phi_norm)],
                    ),
                ];

                SynthesizedOneQubitSequence::new(
                    operations,
                    qubit,
                    GlobalPhase::from_radians(alpha_norm),
                    basis,
                )
            }
            SingleQubitBasis::RzSx => {
                // Specialized Hadamard synthesis:
                // H = e^{i*pi/4} * (Rz(pi/2) * SX * Rz(pi/2))
                if matrix.approx_eq(&matrix_h(), self.tolerance) {
                    let operations = vec![
                        Operation::custom_gate(
                            "rz",
                            vec![q_id],
                            vec![ParameterExpr::constant(FRAC_PI_2)],
                        ),
                        Operation::custom_gate("sx", vec![q_id], vec![]),
                        Operation::custom_gate(
                            "rz",
                            vec![q_id],
                            vec![ParameterExpr::constant(FRAC_PI_2)],
                        ),
                    ];
                    return Ok(SynthesizedOneQubitSequence::new(
                        operations,
                        qubit,
                        GlobalPhase::from_radians(FRAC_PI_4),
                        basis,
                    ));
                }

                // Synthesize from canonical Z-Y-Z angles:
                // U = e^{i * alpha} * (Rz(phi) * Ry(theta) * Rz(lambda))
                // Mathematical identity:
                // Ry(theta) = exp(i*pi/2) * ( Rz(pi) * SX * Rz(theta + pi) * SX * I )
                // U = exp(i*(alpha + pi/2)) * ( Rz(phi + pi) * SX * Rz(theta + pi) * SX * Rz(lambda) )
                let euler = decompose_euler(matrix, EulerBasis::Zyz, self.tolerance)?;
                let (phi_raw, theta_val, lambda_raw) = euler.angles_rad();
                let alpha_raw = euler.global_phase.angle().radians();

                let th1_raw = lambda_raw;
                let th2_raw = theta_val + PI;
                let th3_raw = phi_raw + PI;
                let global_phase_raw = alpha_raw + (PI / 2.0);

                let (th1_norm, k1) = normalize_angle_with_spinor_shift(th1_raw);
                let (th2_norm, k2) = normalize_angle_with_spinor_shift(th2_raw);
                let (th3_norm, k3) = normalize_angle_with_spinor_shift(th3_raw);

                let total_k = k1 + k2 + k3;
                let (global_phase_norm, _) = normalize_angle_with_spinor_shift(
                    global_phase_raw + ((total_k.rem_euclid(2)) as f64) * PI,
                );

                // Sequential execution order: Rz(th1) -> SX -> Rz(th2) -> SX -> Rz(th3)
                let operations = vec![
                    Operation::custom_gate(
                        "rz",
                        vec![q_id],
                        vec![ParameterExpr::constant(th1_norm)],
                    ),
                    Operation::custom_gate("sx", vec![q_id], vec![]),
                    Operation::custom_gate(
                        "rz",
                        vec![q_id],
                        vec![ParameterExpr::constant(th2_norm)],
                    ),
                    Operation::custom_gate("sx", vec![q_id], vec![]),
                    Operation::custom_gate(
                        "rz",
                        vec![q_id],
                        vec![ParameterExpr::constant(th3_norm)],
                    ),
                ];

                SynthesizedOneQubitSequence::new(
                    operations,
                    qubit,
                    GlobalPhase::from_radians(global_phase_norm),
                    basis,
                )
            }
            SingleQubitBasis::U3 => {
                let euler = decompose_euler(matrix, EulerBasis::Zyz, self.tolerance)?;
                let (phi_raw, theta_val, lambda_raw) = euler.angles_rad();
                let alpha_raw = euler.global_phase.angle().radians();

                let (phi_norm, _) = normalize_angle_with_spinor_shift(phi_raw);
                let (lambda_norm, _) = normalize_angle_with_spinor_shift(lambda_raw);

                // In OpenQASM 3: U3(theta, phi, lambda) = exp(i * (phi + lambda)/2) * (Rz(phi) * Ry(theta) * Rz(lambda))
                // Remaining global phase: alpha - (phi_norm + lambda_norm) / 2
                let (global_phase_norm, _) =
                    normalize_angle_with_spinor_shift(alpha_raw - ((phi_norm + lambda_norm) / 2.0));

                let op_name = if target.operation("u3").is_some() {
                    "u3"
                } else {
                    "u"
                };

                let operations = vec![Operation::custom_gate(
                    op_name,
                    vec![q_id],
                    vec![
                        ParameterExpr::constant(theta_val),
                        ParameterExpr::constant(phi_norm),
                        ParameterExpr::constant(lambda_norm),
                    ],
                )];

                SynthesizedOneQubitSequence::new(
                    operations,
                    qubit,
                    GlobalPhase::from_radians(global_phase_norm),
                    basis,
                )
            }
        };

        // Requirement 6: Verify legality of every emitted target operation
        result.verify_legality(target)?;

        // Self-verification: guarantee equivalence against target matrix within tolerance
        result.verify_equivalence(matrix, self.tolerance)?;

        Ok(result)
    }
}
