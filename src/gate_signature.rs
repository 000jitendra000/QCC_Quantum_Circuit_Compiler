//! Canonical logical gate identifiers, signatures, parameter domains, and inverse metadata.
//!
//! Provides the foundational type definitions for QCC's canonical logical gate catalog,
//! separating target-independent mathematical operators from hardware-native instruction sets.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::quantum::semantics::{OperandRole, OperationCategory};

/// Canonical enumeration of target-independent logical quantum gates supported by QCC.
///
/// QCC's logical gate set contains:
/// - **Fixed 1-qubit gates**: [`Id`](GateId::Id), [`X`](GateId::X), [`Y`](GateId::Y),
///   [`Z`](GateId::Z), [`H`](GateId::H), [`S`](GateId::S), [`Sdg`](GateId::Sdg),
///   [`T`](GateId::T), [`Tdg`](GateId::Tdg).
/// - **Parameterized 1-qubit rotations**: [`Rx`](GateId::Rx), [`Ry`](GateId::Ry),
///   [`Rz`](GateId::Rz), [`P`](GateId::P).
/// - **Fixed 2-qubit entanglers**: [`CX`](GateId::CX), [`CZ`](GateId::CZ), [`SWAP`](GateId::SWAP).
///
/// Target-native instructions (e.g. `sx`, `sy`, `ecr`, `iswap`, `rzz`, `fsim`) are
/// strictly excluded from this logical catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GateId {
    /// Single-qubit identity gate $I = \begin{pmatrix} 1 & 0 \\ 0 & 1 \end{pmatrix}$.
    Id,
    /// Pauli-X gate $\begin{pmatrix} 0 & 1 \\ 1 & 0 \end{pmatrix}$.
    X,
    /// Pauli-Y gate $\begin{pmatrix} 0 & -i \\ i & 0 \end{pmatrix}$.
    Y,
    /// Pauli-Z gate $\begin{pmatrix} 1 & 0 \\ 0 & -1 \end{pmatrix}$.
    Z,
    /// Hadamard gate $\frac{1}{\sqrt{2}} \begin{pmatrix} 1 & 1 \\ 1 & -1 \end{pmatrix}$.
    H,
    /// Phase gate $S = \sqrt{Z} = \begin{pmatrix} 1 & 0 \\ 0 & i \end{pmatrix}$.
    S,
    /// Phase gate adjoint $S^\dagger = \begin{pmatrix} 1 & 0 \\ 0 & -i \end{pmatrix}$.
    Sdg,
    /// Phase gate $T = \sqrt{S} = \begin{pmatrix} 1 & 0 \\ 0 & e^{i\pi/4} \end{pmatrix}$.
    T,
    /// Phase gate adjoint $T^\dagger = \begin{pmatrix} 1 & 0 \\ 0 & e^{-i\pi/4} \end{pmatrix}$.
    Tdg,
    /// Rotation about X-axis $R_x(\theta) = \exp(-i \frac{\theta}{2} X)$.
    Rx,
    /// Rotation about Y-axis $R_y(\theta) = \exp(-i \frac{\theta}{2} Y)$.
    Ry,
    /// Rotation about Z-axis $R_z(\theta) = \exp(-i \frac{\theta}{2} Z)$.
    Rz,
    /// Arbitrary phase rotation $P(\lambda) = \begin{pmatrix} 1 & 0 \\ 0 & e^{i\lambda} \end{pmatrix}$.
    P,
    /// Controlled-NOT (CX) entangling gate with operand 0 as control and operand 1 as target.
    CX,
    /// Controlled-Z (CZ) symmetric entangling gate $\text{diag}(1, 1, 1, -1)$.
    CZ,
    /// Two-qubit state exchange gate (SWAP).
    SWAP,
}

impl GateId {
    /// Returns the canonical lowercase mnemonic name for the gate.
    #[inline]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
            Self::H => "h",
            Self::S => "s",
            Self::Sdg => "sdg",
            Self::T => "t",
            Self::Tdg => "tdg",
            Self::Rx => "rx",
            Self::Ry => "ry",
            Self::Rz => "rz",
            Self::P => "p",
            Self::CX => "cx",
            Self::CZ => "cz",
            Self::SWAP => "swap",
        }
    }
}

impl fmt::Display for GateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Domain constraints on continuous quantum gate parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParameterDomain {
    /// Spinor angle with $4\pi$ periodicity: $R(\theta + 4\pi) = R(\theta)$ and $R(\theta + 2\pi) = -R(\theta)$.
    Angle4Pi,
    /// Phase angle with $2\pi$ periodicity: $P(\lambda + 2\pi) = P(\lambda)$ with zero global phase delta.
    Angle2Pi,
    /// Unconstrained real scalar domain $(-\infty, +\infty)$.
    UnconstrainedReal,
}

impl fmt::Display for ParameterDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Angle4Pi => write!(f, "Angle4Pi"),
            Self::Angle2Pi => write!(f, "Angle2Pi"),
            Self::UnconstrainedReal => write!(f, "UnconstrainedReal"),
        }
    }
}

/// Mathematical inverse metadata classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InverseKind {
    /// Operation is Hermitian and self-inverse ($U = U^\dagger$, $U^2 = I$).
    SelfInverse,
    /// Distinct paired discrete gate in the catalog ($U^\dagger = V$ and $V^\dagger = U$).
    Paired(GateId),
    /// Parameter negation rule: $U(\vec{\theta})^\dagger = U(-\vec{\theta})$.
    ParameterNegation,
    /// Non-unitary operation or gate with no catalogued unitary inverse.
    None,
}

impl fmt::Display for InverseKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SelfInverse => write!(f, "SelfInverse"),
            Self::Paired(id) => write!(f, "Paired({id})"),
            Self::ParameterNegation => write!(f, "ParameterNegation"),
            Self::None => write!(f, "None"),
        }
    }
}

/// Canonical metadata specification for a target-independent logical quantum gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GateSignature {
    /// Canonical logical gate identifier.
    pub id: GateId,
    /// Primary lowercase canonical name.
    pub name: &'static str,
    /// Recognized lowercase aliases.
    pub aliases: &'static [&'static str],
    /// Number of qubit operands required (quantum arity).
    pub quantum_arity: usize,
    /// Number of continuous or discrete parameters required.
    pub parameter_count: usize,
    /// Domain specifications for each parameter (length equals `parameter_count`).
    pub parameter_domains: &'static [ParameterDomain],
    /// Operational category (unitary transformation versus non-unitary side effect).
    pub category: OperationCategory,
    /// Exact mathematical inverse behavior.
    pub inverse_kind: InverseKind,
    /// Whether the gate action is symmetric under permutation of its qubit operands.
    pub is_symmetric: bool,
    /// Semantic role of each qubit operand (length equals `quantum_arity`).
    pub operand_roles: &'static [OperandRole],
    /// Human-readable mathematical description and formula.
    pub description: &'static str,
}

impl GateSignature {
    /// Returns `true` if this is a single-qubit gate.
    #[inline]
    pub const fn is_single_qubit(&self) -> bool {
        self.quantum_arity == 1
    }

    /// Returns `true` if this is a two-qubit gate.
    #[inline]
    pub const fn is_two_qubit(&self) -> bool {
        self.quantum_arity == 2
    }

    /// Returns `true` if the gate takes continuous parameters.
    #[inline]
    pub const fn is_parameterized(&self) -> bool {
        self.parameter_count > 0
    }

    /// Returns `true` if this is a unitary operation.
    #[inline]
    pub const fn is_unitary(&self) -> bool {
        matches!(self.category, OperationCategory::Unitary)
    }

    /// Returns `true` if the gate is mathematically self-inverse ($U^2 = I$).
    #[inline]
    pub const fn is_self_inverse(&self) -> bool {
        matches!(self.inverse_kind, InverseKind::SelfInverse)
    }

    /// Tests if a given string matches this gate's canonical name or any alias (case-insensitive).
    pub fn matches_name(&self, name: &str) -> bool {
        if self.name.eq_ignore_ascii_case(name) {
            return true;
        }
        for &alias in self.aliases {
            if alias.eq_ignore_ascii_case(name) {
                return true;
            }
        }
        false
    }
}

impl fmt::Display for GateSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}(arity={}, params={}, category={:?})",
            self.name, self.quantum_arity, self.parameter_count, self.category
        )
    }
}
