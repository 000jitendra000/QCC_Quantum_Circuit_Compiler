//! Canonical logical gate catalog for the Quantum Circuit Compiler (QCC).
//!
//! Provides the authoritative, target-independent registry of supported logical quantum gates.
//! Separates mathematical operator identity and semantic contracts from hardware-native
//! instruction sets and vendor-specific coupling topologies.

use crate::gate_signature::{GateId, GateSignature, InverseKind, ParameterDomain};
use crate::quantum::gate_matrix::{
    matrix_cx, matrix_cz, matrix_h, matrix_id, matrix_p, matrix_rx, matrix_ry, matrix_rz, matrix_s,
    matrix_sdg, matrix_swap, matrix_t, matrix_tdg, matrix_x, matrix_y, matrix_z, Matrix2x2,
    Matrix4x4,
};
use crate::quantum::parameter::Angle;
use crate::quantum::semantics::{OperandRole, OperationCategory};

/// Canonical static slice of all 16 supported logical quantum gate signatures.
pub static CANONICAL_GATES: &[GateSignature] = &[
    // ------------------------------------------------------------------------
    // Single-qubit fixed gates (9 gates)
    // ------------------------------------------------------------------------
    GateSignature {
        id: GateId::Id,
        name: "id",
        aliases: &["i"],
        quantum_arity: 1,
        parameter_count: 0,
        parameter_domains: &[],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::SelfInverse,
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "Single-qubit identity gate: I = [[1, 0], [0, 1]]",
    },
    GateSignature {
        id: GateId::X,
        name: "x",
        aliases: &[],
        quantum_arity: 1,
        parameter_count: 0,
        parameter_domains: &[],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::SelfInverse,
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "Pauli-X bit-flip gate: [[0, 1], [1, 0]]",
    },
    GateSignature {
        id: GateId::Y,
        name: "y",
        aliases: &[],
        quantum_arity: 1,
        parameter_count: 0,
        parameter_domains: &[],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::SelfInverse,
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "Pauli-Y gate: [[0, -i], [i, 0]]",
    },
    GateSignature {
        id: GateId::Z,
        name: "z",
        aliases: &[],
        quantum_arity: 1,
        parameter_count: 0,
        parameter_domains: &[],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::SelfInverse,
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "Pauli-Z phase-flip gate: [[1, 0], [0, -1]]",
    },
    GateSignature {
        id: GateId::H,
        name: "h",
        aliases: &[],
        quantum_arity: 1,
        parameter_count: 0,
        parameter_domains: &[],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::SelfInverse,
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "Hadamard basis change gate: 1/sqrt(2) * [[1, 1], [1, -1]]",
    },
    GateSignature {
        id: GateId::S,
        name: "s",
        aliases: &[],
        quantum_arity: 1,
        parameter_count: 0,
        parameter_domains: &[],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::Paired(GateId::Sdg),
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "Phase S gate (sqrt(Z)): [[1, 0], [0, i]]",
    },
    GateSignature {
        id: GateId::Sdg,
        name: "sdg",
        aliases: &["s_dag", "sdag"],
        quantum_arity: 1,
        parameter_count: 0,
        parameter_domains: &[],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::Paired(GateId::S),
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "Adjoint phase Sdg gate (S^dagger): [[1, 0], [0, -i]]",
    },
    GateSignature {
        id: GateId::T,
        name: "t",
        aliases: &[],
        quantum_arity: 1,
        parameter_count: 0,
        parameter_domains: &[],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::Paired(GateId::Tdg),
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "T gate (sqrt(S) = Z^(1/4)): [[1, 0], [0, e^(i*pi/4)]]",
    },
    GateSignature {
        id: GateId::Tdg,
        name: "tdg",
        aliases: &["t_dag", "tdag"],
        quantum_arity: 1,
        parameter_count: 0,
        parameter_domains: &[],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::Paired(GateId::T),
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "Adjoint Tdg gate (T^dagger): [[1, 0], [0, e^(-i*pi/4)]]",
    },
    // ------------------------------------------------------------------------
    // Single-qubit continuous parameterized gates (4 gates)
    // ------------------------------------------------------------------------
    GateSignature {
        id: GateId::Rx,
        name: "rx",
        aliases: &[],
        quantum_arity: 1,
        parameter_count: 1,
        parameter_domains: &[ParameterDomain::Angle4Pi],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::ParameterNegation,
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "Rotation about X-axis: exp(-i*theta/2 * X)",
    },
    GateSignature {
        id: GateId::Ry,
        name: "ry",
        aliases: &[],
        quantum_arity: 1,
        parameter_count: 1,
        parameter_domains: &[ParameterDomain::Angle4Pi],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::ParameterNegation,
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "Rotation about Y-axis: exp(-i*theta/2 * Y)",
    },
    GateSignature {
        id: GateId::Rz,
        name: "rz",
        aliases: &[],
        quantum_arity: 1,
        parameter_count: 1,
        parameter_domains: &[ParameterDomain::Angle4Pi],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::ParameterNegation,
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "Rotation about Z-axis: exp(-i*theta/2 * Z)",
    },
    GateSignature {
        id: GateId::P,
        name: "p",
        aliases: &["phase", "u1"],
        quantum_arity: 1,
        parameter_count: 1,
        parameter_domains: &[ParameterDomain::Angle2Pi],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::ParameterNegation,
        is_symmetric: true,
        operand_roles: &[OperandRole::Target],
        description: "Arbitrary phase gate: diag(1, exp(i*lambda))",
    },
    // ------------------------------------------------------------------------
    // Two-qubit fixed entanglers (3 gates)
    // ------------------------------------------------------------------------
    GateSignature {
        id: GateId::CX,
        name: "cx",
        aliases: &["cnot"],
        quantum_arity: 2,
        parameter_count: 0,
        parameter_domains: &[],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::SelfInverse,
        is_symmetric: false,
        operand_roles: &[OperandRole::Control, OperandRole::Target],
        description: "Controlled-NOT: operand 0 is control, operand 1 is target",
    },
    GateSignature {
        id: GateId::CZ,
        name: "cz",
        aliases: &[],
        quantum_arity: 2,
        parameter_count: 0,
        parameter_domains: &[],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::SelfInverse,
        is_symmetric: true,
        operand_roles: &[OperandRole::Target, OperandRole::Target],
        description: "Controlled-Z: symmetric diag(1, 1, 1, -1)",
    },
    GateSignature {
        id: GateId::SWAP,
        name: "swap",
        aliases: &[],
        quantum_arity: 2,
        parameter_count: 0,
        parameter_domains: &[],
        category: OperationCategory::Unitary,
        inverse_kind: InverseKind::SelfInverse,
        is_symmetric: true,
        operand_roles: &[OperandRole::Target, OperandRole::Target],
        description: "Two-qubit state exchange (SWAP)",
    },
];

/// Known target-native or vendor-specific instruction mnemonics.
///
/// These operations represent hardware-native pulses, cross-resonance entanglers,
/// or vendor-specific decompositions, and are **strictly excluded** from the
/// universal logical gate catalog.
pub static KNOWN_TARGET_NATIVE_CANDIDATES: &[&str] = &[
    "sx", "sy", "sxdg", "sydg", "rzx", "rzz", "rxx", "ryy", "ecr", "iswap", "fsim", "cz_pulse",
    "xy", "cr",
];

/// Authoritative catalog providing lookup, query, and mathematical validation APIs for logical gates.
pub struct GateCatalog;

impl GateCatalog {
    /// Returns a slice of all canonical logical gate signatures.
    #[inline]
    pub fn all_gates() -> &'static [GateSignature] {
        CANONICAL_GATES
    }

    /// Looks up a logical gate signature by name or alias (case-insensitive).
    ///
    /// Returns `Some(&GateSignature)` if the name matches a canonical logical gate,
    /// or `None` if the gate is unknown or is a target-native instruction.
    pub fn lookup(name: &str) -> Option<&'static GateSignature> {
        CANONICAL_GATES.iter().find(|sig| sig.matches_name(name))
    }

    /// Looks up a logical gate signature strictly by its exact primary canonical name.
    pub fn lookup_exact(name: &str) -> Option<&'static GateSignature> {
        CANONICAL_GATES.iter().find(|sig| sig.name == name)
    }

    /// Returns the gate signature for a typed [`GateId`].
    pub fn get(id: GateId) -> &'static GateSignature {
        CANONICAL_GATES
            .iter()
            .find(|sig| sig.id == id)
            .expect("every GateId variant must have an entry in CANONICAL_GATES")
    }

    /// Returns `true` if `name` matches any supported canonical logical gate or alias.
    #[inline]
    pub fn is_supported(name: &str) -> bool {
        Self::lookup(name).is_some()
    }

    /// Returns `true` if `name` is an exact primary canonical name (case-insensitive).
    pub fn is_canonical_name(name: &str) -> bool {
        CANONICAL_GATES
            .iter()
            .any(|sig| sig.name.eq_ignore_ascii_case(name))
    }

    /// Returns `true` if the gate with the given name is self-inverse ($U^2 = I$).
    #[inline]
    pub fn is_self_inverse(name: &str) -> bool {
        Self::lookup(name)
            .map(|sig| sig.is_self_inverse())
            .unwrap_or(false)
    }

    /// Returns the canonical inverse gate name if known.
    ///
    /// For self-inverse and parameterized gates, returns the canonical primary name.
    /// For discrete paired gates ($S \leftrightarrow S^\dagger$, $T \leftrightarrow T^\dagger$),
    /// returns the counterpart's canonical name.
    pub fn invert_gate_name(name: &str) -> Option<&'static str> {
        let sig = Self::lookup(name)?;
        match sig.inverse_kind {
            InverseKind::SelfInverse | InverseKind::ParameterNegation => Some(sig.name),
            InverseKind::Paired(paired_id) => Some(Self::get(paired_id).name),
            InverseKind::None => None,
        }
    }

    /// Returns `true` if `name` represents a known target-native hardware instruction.
    ///
    /// Target-native instructions (e.g. `sx`, `sy`, `ecr`, `iswap`, `rzz`) are excluded
    /// from the logical gate catalog and must be resolved by target-aware basis translation.
    pub fn is_target_native_candidate(name: &str) -> bool {
        KNOWN_TARGET_NATIVE_CANDIDATES
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(name))
    }

    /// Evaluates the $2 \times 2$ unitary matrix for a single-qubit [`GateId`].
    ///
    /// Continuous parameters are read from `params` (defaulting to 0.0 if not provided).
    /// Returns `None` if `id` is a two-qubit gate.
    pub fn matrix_for_gate_1q(id: GateId, params: &[f64]) -> Option<Matrix2x2> {
        match id {
            GateId::Id => Some(matrix_id()),
            GateId::X => Some(matrix_x()),
            GateId::Y => Some(matrix_y()),
            GateId::Z => Some(matrix_z()),
            GateId::H => Some(matrix_h()),
            GateId::S => Some(matrix_s()),
            GateId::Sdg => Some(matrix_sdg()),
            GateId::T => Some(matrix_t()),
            GateId::Tdg => Some(matrix_tdg()),
            GateId::Rx => {
                let theta = params.first().copied().unwrap_or(0.0);
                Some(matrix_rx(Angle::from_radians(theta)))
            }
            GateId::Ry => {
                let theta = params.first().copied().unwrap_or(0.0);
                Some(matrix_ry(Angle::from_radians(theta)))
            }
            GateId::Rz => {
                let theta = params.first().copied().unwrap_or(0.0);
                Some(matrix_rz(Angle::from_radians(theta)))
            }
            GateId::P => {
                let lambda = params.first().copied().unwrap_or(0.0);
                Some(matrix_p(Angle::from_radians(lambda)))
            }
            GateId::CX | GateId::CZ | GateId::SWAP => None,
        }
    }

    /// Evaluates the $4 \times 4$ unitary matrix for a two-qubit [`GateId`].
    ///
    /// Basis states are ordered little-endian $\{|00\rangle, |01\rangle, |10\rangle, |11\rangle\}$.
    /// Returns `None` if `id` is a single-qubit gate.
    pub fn matrix_for_gate_2q(id: GateId) -> Option<Matrix4x4> {
        match id {
            GateId::CX => Some(matrix_cx()),
            GateId::CZ => Some(matrix_cz()),
            GateId::SWAP => Some(matrix_swap()),
            _ => None,
        }
    }

    /// Returns an iterator over all single-qubit logical gates.
    pub fn single_qubit_gates() -> impl Iterator<Item = &'static GateSignature> {
        CANONICAL_GATES.iter().filter(|sig| sig.is_single_qubit())
    }

    /// Returns an iterator over all two-qubit logical gates.
    pub fn two_qubit_gates() -> impl Iterator<Item = &'static GateSignature> {
        CANONICAL_GATES.iter().filter(|sig| sig.is_two_qubit())
    }

    /// Returns an iterator over all parameterized logical gates.
    pub fn parameterized_gates() -> impl Iterator<Item = &'static GateSignature> {
        CANONICAL_GATES.iter().filter(|sig| sig.is_parameterized())
    }

    /// Returns an iterator over all fixed (non-parameterized) logical gates.
    pub fn fixed_gates() -> impl Iterator<Item = &'static GateSignature> {
        CANONICAL_GATES.iter().filter(|sig| !sig.is_parameterized())
    }
}
