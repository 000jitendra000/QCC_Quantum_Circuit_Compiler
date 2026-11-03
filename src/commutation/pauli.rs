//! Pauli algebra and axis-based commutation rules.
//!
//! Provides single-qubit and multi-qubit Pauli commutation and anticommutation analysis,
//! rotation axis classification, and exact commutation checks for same-axis rotations.

use std::collections::HashMap;

use crate::gate_catalog::GateCatalog;
use crate::gate_signature::GateId;
use crate::ir::wire::QubitId;

/// Coordinate rotation axis for single-qubit gates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RotationAxis {
    /// Pauli X axis ($X, R_x$).
    X,
    /// Pauli Y axis ($Y, R_y$).
    Y,
    /// Pauli Z axis ($Z, S, S^\dagger, T, T^\dagger, R_z$).
    Z,
}

impl RotationAxis {
    /// Returns the coordinate rotation axis for canonical single-qubit gate mnemonics.
    ///
    /// Gates with non-standard axes (e.g. Hadamard $H$, which rotates around $(X+Z)/\sqrt{2}$)
    /// return `None`.
    pub fn from_gate_name(gate_name: &str) -> Option<Self> {
        let sig = GateCatalog::lookup(gate_name)?;
        match sig.id {
            GateId::X | GateId::Rx => Some(Self::X),
            GateId::Y | GateId::Ry => Some(Self::Y),
            GateId::Z
            | GateId::S
            | GateId::Sdg
            | GateId::T
            | GateId::Tdg
            | GateId::Rz
            | GateId::P => Some(Self::Z),
            _ => None,
        }
    }
}

/// Single-qubit Pauli operators ($I, X, Y, Z$).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pauli {
    /// Identity operator $I$.
    I,
    /// Pauli X operator $\sigma_x$.
    X,
    /// Pauli Y operator $\sigma_y$.
    Y,
    /// Pauli Z operator $\sigma_z$.
    Z,
}

impl Pauli {
    /// Classifies a canonical gate mnemonic into a Pauli operator if applicable.
    pub fn from_gate_name(gate_name: &str) -> Option<Self> {
        let sig = GateCatalog::lookup(gate_name)?;
        match sig.id {
            GateId::X => Some(Self::X),
            GateId::Y => Some(Self::Y),
            GateId::Z => Some(Self::Z),
            GateId::Id => Some(Self::I),
            _ => None,
        }
    }

    /// Returns `true` if two single-qubit Pauli operators commute ($[P_1, P_2] = 0$).
    ///
    /// - Identity $I$ commutes with all Pauli operators.
    /// - Every Pauli operator commutes with itself: $[X, X] = [Y, Y] = [Z, Z] = 0$.
    /// - Distinct non-identity Paulis anticommute and do NOT commute: $[X, Y] \neq 0$, $[X, Z] \neq 0$, $[Y, Z] \neq 0$.
    #[inline]
    pub fn commutes_with(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::I, _) | (_, Self::I) => true,
            (a, b) => a == b,
        }
    }

    /// Returns `true` if two single-qubit Pauli operators anticommute ($\{P_1, P_2\} = 0$).
    ///
    /// - Distinct non-identity Paulis anticommute: $\{X, Y\} = \{X, Z\} = \{Y, Z\} = 0$.
    /// - Identity and identical Paulis do NOT anticommute.
    #[inline]
    pub fn anticommutes_with(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::I, _) | (_, Self::I) => false,
            (a, b) => a != b,
        }
    }
}

/// Multi-qubit Pauli string commutation result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauliCommutation {
    /// The Pauli strings commute: $P \cdot Q = Q \cdot P$.
    Commutes,
    /// The Pauli strings anticommute: $P \cdot Q = - Q \cdot P$.
    Anticommutes,
}

/// Analyzes commutation between two multi-qubit Pauli strings.
///
/// For two Pauli strings $P = \bigotimes_q P_q$ and $Q = \bigotimes_q Q_q$:
/// $$P \cdot Q = (-1)^k Q \cdot P$$
/// where $k$ is the number of qubits on which $P_q$ and $Q_q$ anticommute.
/// - If $k$ is even ($k \equiv 0 \pmod 2$), $P$ and $Q$ commute.
/// - If $k$ is odd ($k \equiv 1 \pmod 2$), $P$ and $Q$ anticommute.
pub fn analyze_pauli_string_commutation(
    string1: &[(QubitId, Pauli)],
    string2: &[(QubitId, Pauli)],
) -> PauliCommutation {
    let mut map1: HashMap<QubitId, Pauli> = HashMap::new();
    for &(q, p) in string1 {
        map1.insert(q, p);
    }

    let mut anticommuting_qubits = 0usize;
    for &(q, p2) in string2 {
        if let Some(&p1) = map1.get(&q) {
            if p1.anticommutes_with(&p2) {
                anticommuting_qubits += 1;
            }
        }
    }

    if anticommuting_qubits % 2 == 0 {
        PauliCommutation::Commutes
    } else {
        PauliCommutation::Anticommutes
    }
}

/// Returns `true` if two single-qubit gate operations share the same rotation axis on the same qubit.
///
/// This covers:
/// - $X$-axis: $X$ and $R_x(\theta)$, or two $R_x$ rotations.
/// - $Y$-axis: $Y$ and $R_y(\theta)$, or two $R_y$ rotations.
/// - $Z$-axis: any combination of $Z, S, S^\dagger, T, T^\dagger, R_z(\theta)$.
pub fn are_same_axis_rotations(gate1: &str, gate2: &str) -> bool {
    let axis1 = RotationAxis::from_gate_name(gate1);
    let axis2 = RotationAxis::from_gate_name(gate2);
    match (axis1, axis2) {
        (Some(a1), Some(a2)) => a1 == a2,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pauli_self_commutation() {
        assert!(Pauli::X.commutes_with(&Pauli::X));
        assert!(Pauli::Y.commutes_with(&Pauli::Y));
        assert!(Pauli::Z.commutes_with(&Pauli::Z));
        assert!(Pauli::I.commutes_with(&Pauli::X));
        assert!(Pauli::I.commutes_with(&Pauli::I));
    }

    #[test]
    fn test_pauli_anticommutation() {
        assert!(Pauli::X.anticommutes_with(&Pauli::Y));
        assert!(Pauli::Y.anticommutes_with(&Pauli::X));
        assert!(Pauli::X.anticommutes_with(&Pauli::Z));
        assert!(Pauli::Z.anticommutes_with(&Pauli::X));
        assert!(Pauli::Y.anticommutes_with(&Pauli::Z));
        assert!(Pauli::Z.anticommutes_with(&Pauli::Y));

        // Distinct Paulis must NOT commute
        assert!(!Pauli::X.commutes_with(&Pauli::Y));
        assert!(!Pauli::X.commutes_with(&Pauli::Z));
        assert!(!Pauli::Y.commutes_with(&Pauli::Z));
    }

    #[test]
    fn test_multi_qubit_pauli_string_commutation() {
        let q0 = QubitId(0);
        let q1 = QubitId(1);

        // X(0) X(1) and Z(0) Z(1): 2 anticommutations -> commutes!
        let s1 = [(q0, Pauli::X), (q1, Pauli::X)];
        let s2 = [(q0, Pauli::Z), (q1, Pauli::Z)];
        assert_eq!(
            analyze_pauli_string_commutation(&s1, &s2),
            PauliCommutation::Commutes
        );

        // X(0) I(1) and Z(0) Z(1): 1 anticommutation -> anticommutes!
        let s3 = [(q0, Pauli::X)];
        assert_eq!(
            analyze_pauli_string_commutation(&s3, &s2),
            PauliCommutation::Anticommutes
        );
    }

    #[test]
    fn test_same_axis_rotations() {
        assert!(are_same_axis_rotations("z", "s"));
        assert!(are_same_axis_rotations("s", "t"));
        assert!(are_same_axis_rotations("t", "rz"));
        assert!(are_same_axis_rotations("sdg", "tdg"));
        assert!(are_same_axis_rotations("x", "rx"));
        assert!(are_same_axis_rotations("y", "ry"));

        assert!(!are_same_axis_rotations("x", "z"));
        assert!(!are_same_axis_rotations("rx", "rz"));
        assert!(!are_same_axis_rotations("h", "x"));
        assert!(!are_same_axis_rotations("h", "z"));
    }
}
