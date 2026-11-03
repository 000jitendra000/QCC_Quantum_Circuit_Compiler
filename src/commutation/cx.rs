//! Commutation rules for controlled operations ($CX$, $CZ$) and multi-qubit gates.
//!
//! Enforces mathematically verified preconditions for:
//! - $Z$-axis operations on the control qubit of $CX$.
//! - $X$-axis operations on the target qubit of $CX$.
//! - $Z$-axis operations on either operand of symmetric $CZ$.
//! - Commuting pairs of two-qubit controlled gates ($CX$ with shared control or target, $CZ$ with $CZ$).
//! - Explicit non-commutation for reversed, cross-axis, and chained configurations.

use crate::gate_catalog::GateCatalog;
use crate::gate_signature::GateId;
use crate::ir::wire::QubitId;

/// Checks whether a single-qubit gate commutes with a $CX$ gate.
///
/// In QCC, for $CX(q_{\text{ctrl}}, q_{\text{targ}})$:
/// - **Operand 0 is control**: Commutes with $Z$-axis operations ($Z, S, S^\dagger, T, T^\dagger, R_z$).
/// - **Operand 1 is target**: Commutes with $X$-axis operations ($X, R_x$).
///
/// Returns `Some(true)` if proven to commute, `Some(false)` if proven not to commute,
/// or `None` if the gate mnemonic is uncatalogued.
pub fn single_qubit_cx_commutation(
    single_gate: &str,
    single_qubit: QubitId,
    cx_control: QubitId,
    cx_target: QubitId,
) -> Option<bool> {
    if single_qubit == cx_control {
        // Must be a Z-axis operation or identity to commute with CX control
        let sig = GateCatalog::lookup(single_gate)?;
        match sig.id {
            GateId::Id
            | GateId::Z
            | GateId::S
            | GateId::Sdg
            | GateId::T
            | GateId::Tdg
            | GateId::Rz
            | GateId::P => Some(true),
            GateId::X | GateId::Y | GateId::H | GateId::Rx | GateId::Ry => Some(false),
            _ => None,
        }
    } else if single_qubit == cx_target {
        // Must be an X-axis operation or identity to commute with CX target
        let sig = GateCatalog::lookup(single_gate)?;
        match sig.id {
            GateId::Id | GateId::X | GateId::Rx => Some(true),
            GateId::Z
            | GateId::Y
            | GateId::H
            | GateId::S
            | GateId::Sdg
            | GateId::T
            | GateId::Tdg
            | GateId::Ry
            | GateId::Rz
            | GateId::P => Some(false),
            _ => None,
        }
    } else {
        // Disjoint wire: single_qubit is neither control nor target
        Some(true)
    }
}

/// Checks whether a single-qubit gate commutes with a symmetric $CZ(q_0, q_1)$ gate.
///
/// Because $CZ$ is diagonal in the computational basis ($\text{diag}(1, 1, 1, -1)$),
/// any $Z$-axis operation ($Z, S, S^\dagger, T, T^\dagger, R_z$) on *either* operand commutes.
pub fn single_qubit_cz_commutation(
    single_gate: &str,
    single_qubit: QubitId,
    cz_q0: QubitId,
    cz_q1: QubitId,
) -> Option<bool> {
    if single_qubit == cz_q0 || single_qubit == cz_q1 {
        let sig = GateCatalog::lookup(single_gate)?;
        match sig.id {
            GateId::Id
            | GateId::Z
            | GateId::S
            | GateId::Sdg
            | GateId::T
            | GateId::Tdg
            | GateId::Rz
            | GateId::P => Some(true),
            GateId::X | GateId::Y | GateId::H | GateId::Rx | GateId::Ry => Some(false),
            _ => None,
        }
    } else {
        // Disjoint wire
        Some(true)
    }
}

/// Checks commutation between two $CX$ gates.
///
/// Preconditions:
/// 1. Identical: $CX(c, t)$ and $CX(c, t)$ commute.
/// 2. Disjoint: $\{c_1, t_1\} \cap \{c_2, t_2\} = \emptyset$ commutes.
/// 3. Shared control, distinct targets: $CX(c, t_1)$ and $CX(c, t_2)$ commute ($t_1 \neq t_2$).
/// 4. Shared target, distinct controls: $CX(c_1, t)$ and $CX(c_2, t)$ commute ($c_1 \neq c_2$).
/// 5. Chained control-target ($c_1 = t_2$ or $t_1 = c_2$): do NOT commute.
/// 6. Reversed operands ($c_1 = t_2$ and $t_1 = c_2$): do NOT commute.
pub fn cx_cx_commutation(c1: QubitId, t1: QubitId, c2: QubitId, t2: QubitId) -> bool {
    if c1 == c2 && t1 == t2 {
        // Identical operations
        true
    } else if c1 != c2 && c1 != t2 && t1 != c2 && t1 != t2 {
        // Disjoint qubits
        true
    } else if c1 == c2 && t1 != t2 {
        // Shared control: both project onto |0><0| and |1><1| on c, targets act on disjoint wires
        true
    } else if t1 == t2 && c1 != c2 {
        // Shared target: both apply X on target, [X, X] = 0, controls act on disjoint wires
        true
    } else {
        // Chained (target is control of the other) or reversed
        false
    }
}

/// Checks commutation between two $CZ$ gates.
///
/// Because all $CZ$ operations are diagonal in the computational basis, any pair of $CZ$ gates
/// commutes regardless of qubit overlap.
pub fn cz_cz_commutation(_q0_a: QubitId, _q1_a: QubitId, _q0_b: QubitId, _q1_b: QubitId) -> bool {
    true
}

/// Checks commutation between a $CX(c, t)$ gate and a $CZ(z_0, z_1)$ gate.
///
/// - If disjoint: commutes.
/// - If $c \in \{z_0, z_1\}$ and $t \notin \{z_0, z_1\}$: $c$ is control of $CX$ and operand of $CZ$.
///   Since both act diagonally in $Z$ on $c$ and $t$ is untouched by $CZ$, they commute!
/// - If $t \in \{z_0, z_1\}$: $CZ$ applies $Z$ to target when other qubit is 1. Since $Z$ and $X$
///   anticommute on the target qubit, they do NOT commute.
pub fn cx_cz_commutation(cx_c: QubitId, cx_t: QubitId, cz_0: QubitId, cz_1: QubitId) -> bool {
    let cz_qubits = [cz_0, cz_1];
    let touches_control = cz_qubits.contains(&cx_c);
    let touches_target = cz_qubits.contains(&cx_t);

    match (touches_control, touches_target) {
        (false, false) => true, // Disjoint
        (true, false) => true,  // Overlap only on CX control (both diagonal on control)
        _ => false,             // Overlap on CX target (X and Z anticommute on target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cx_control_z_axis_commutes() {
        let c = QubitId(0);
        let t = QubitId(1);

        assert_eq!(single_qubit_cx_commutation("z", c, c, t), Some(true));
        assert_eq!(single_qubit_cx_commutation("s", c, c, t), Some(true));
        assert_eq!(single_qubit_cx_commutation("sdg", c, c, t), Some(true));
        assert_eq!(single_qubit_cx_commutation("t", c, c, t), Some(true));
        assert_eq!(single_qubit_cx_commutation("tdg", c, c, t), Some(true));
        assert_eq!(single_qubit_cx_commutation("rz", c, c, t), Some(true));

        // Non-Z axis on control must NOT commute
        assert_eq!(single_qubit_cx_commutation("x", c, c, t), Some(false));
        assert_eq!(single_qubit_cx_commutation("y", c, c, t), Some(false));
        assert_eq!(single_qubit_cx_commutation("h", c, c, t), Some(false));
        assert_eq!(single_qubit_cx_commutation("rx", c, c, t), Some(false));
        assert_eq!(single_qubit_cx_commutation("ry", c, c, t), Some(false));
    }

    #[test]
    fn test_cx_target_x_axis_commutes() {
        let c = QubitId(0);
        let t = QubitId(1);

        assert_eq!(single_qubit_cx_commutation("x", t, c, t), Some(true));
        assert_eq!(single_qubit_cx_commutation("rx", t, c, t), Some(true));

        // Non-X axis on target must NOT commute
        assert_eq!(single_qubit_cx_commutation("z", t, c, t), Some(false));
        assert_eq!(single_qubit_cx_commutation("y", t, c, t), Some(false));
        assert_eq!(single_qubit_cx_commutation("h", t, c, t), Some(false));
        assert_eq!(single_qubit_cx_commutation("s", t, c, t), Some(false));
        assert_eq!(single_qubit_cx_commutation("t", t, c, t), Some(false));
        assert_eq!(single_qubit_cx_commutation("ry", t, c, t), Some(false));
        assert_eq!(single_qubit_cx_commutation("rz", t, c, t), Some(false));
    }

    #[test]
    fn test_cz_commutation_rules() {
        let q0 = QubitId(0);
        let q1 = QubitId(1);

        assert_eq!(single_qubit_cz_commutation("z", q0, q0, q1), Some(true));
        assert_eq!(single_qubit_cz_commutation("rz", q1, q0, q1), Some(true));
        assert_eq!(single_qubit_cz_commutation("s", q0, q0, q1), Some(true));

        assert_eq!(single_qubit_cz_commutation("x", q0, q0, q1), Some(false));
        assert_eq!(single_qubit_cz_commutation("y", q1, q0, q1), Some(false));
    }

    #[test]
    fn test_cx_cx_shared_control_and_target() {
        let q0 = QubitId(0);
        let q1 = QubitId(1);
        let q2 = QubitId(2);

        // Shared control, distinct targets: commutes
        assert!(cx_cx_commutation(q0, q1, q0, q2));
        // Shared target, distinct controls: commutes
        assert!(cx_cx_commutation(q0, q2, q1, q2));
        // Chained: q1 is target of first, control of second -> does NOT commute
        assert!(!cx_cx_commutation(q0, q1, q1, q2));
        // Reversed: CX(0,1) and CX(1,0) -> does NOT commute
        assert!(!cx_cx_commutation(q0, q1, q1, q0));
    }
}
