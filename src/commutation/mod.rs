//! Commutation analysis rules and registry for quantum operations.
//!
//! Defines mathematically verified preconditions and rules for safe operation reordering
//! in QCC. This module implements a read-only analysis layer; it does not perform circuit or
//! DAG mutations.

pub mod cx;
pub mod pauli;

use std::collections::HashMap;

pub use crate::commutation::cx::{
    cx_cx_commutation, cx_cz_commutation, cz_cz_commutation, single_qubit_cx_commutation,
    single_qubit_cz_commutation,
};
pub use crate::commutation::pauli::{
    analyze_pauli_string_commutation, are_same_axis_rotations, Pauli, PauliCommutation,
    RotationAxis,
};
use crate::ir::operation::Operation;
use crate::quantum::angle::{normalize_rotation_angle, RotationNormalizationMode};
use crate::quantum::parameter::Angle;

/// Mathematically verified rule explaining why two operations commute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommutationRule {
    /// Operations act on completely disjoint quantum and classical wires.
    DisjointWires,
    /// At least one operation is mathematically proven to be an identity operator ($\pm I$).
    IdentityOperand,
    /// The two operations are identical gates with identical operands and parameters.
    IdenticalOperations,
    /// Single-qubit operations sharing the same coordinate rotation axis ($X$, $Y$, or $Z$) on the same wire.
    SameAxisRotations,
    /// $Z$-axis operation ($Z, S, S^\dagger, T, T^\dagger, R_z$) on the control qubit of $CX$.
    ZAxisOnCxControl,
    /// $X$-axis operation ($X, R_x$) on the target qubit of $CX$.
    XAxisOnCxTarget,
    /// $Z$-axis operation on either operand of symmetric $CZ$.
    ZAxisOnCz,
    /// Commutation between two diagonal two-qubit operators ($CZ$ with $CZ$).
    TwoQubitDiagonal,
    /// $CX$ and $CZ$ gates sharing the $CX$ control qubit with disjoint target wires.
    CxCzSharedControl,
    /// Two $CX$ gates with shared control and disjoint targets ($CX(c, t_1)$ and $CX(c, t_2)$).
    CxSharedControl,
    /// Two $CX$ gates with shared target and disjoint controls ($CX(c_1, t)$ and $CX(c_2, t)$).
    CxSharedTarget,
    /// Multi-qubit Pauli string commutation ($(-1)^k = +1$ where $k$ is even).
    PauliString,
}

impl CommutationRule {
    /// Returns a human-readable title for the rule.
    pub fn title(&self) -> &'static str {
        match self {
            Self::DisjointWires => "Disjoint Wires",
            Self::IdentityOperand => "Identity Operator",
            Self::IdenticalOperations => "Identical Operations",
            Self::SameAxisRotations => "Same-Axis Rotations",
            Self::ZAxisOnCxControl => "Z-Axis on CX Control",
            Self::XAxisOnCxTarget => "X-Axis on CX Target",
            Self::ZAxisOnCz => "Z-Axis on CZ Operand",
            Self::TwoQubitDiagonal => "Two-Qubit Diagonal Operators",
            Self::CxCzSharedControl => "CX and CZ with Shared Control",
            Self::CxSharedControl => "CX Gates with Shared Control",
            Self::CxSharedTarget => "CX Gates with Shared Target",
            Self::PauliString => "Pauli String Commutation",
        }
    }
}

/// Reason explaining why two operations do not commute or are excluded from commutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NonCommutationReason {
    /// Non-unitary operation (measure, reset, barrier, classical computation).
    NonUnitaryOperation,
    /// Classical wire dependency hazard (RAW, WAR, WAW).
    ClassicalHazard,
    /// Single-qubit operations with differing rotation axes on the same qubit.
    CrossAxisRotations,
    /// Non-$Z$-axis operation on the control qubit of $CX$.
    CxControlNonZAxis,
    /// Non-$X$-axis operation on the target qubit of $CX$.
    CxTargetNonXAxis,
    /// $CX$ gates forming a chained dependency ($CX(a, b)$ and $CX(b, c)$).
    CxChainedDependency,
    /// $CX$ gates with reversed operands ($CX(a, b)$ and $CX(b, a)$).
    CxReversedOperands,
    /// Non-$Z$-axis operation on an operand of $CZ$.
    CzNonZAxis,
    /// Pauli operators anticommute ($\{P_1, P_2\} = 0$).
    PauliAnticommuting,
    /// Operation semantics are unsupported, uncatalogued, or ambiguous.
    UnsupportedOperation,
}

impl NonCommutationReason {
    /// Returns a human-readable explanation of the non-commutation cause.
    pub fn description(&self) -> &'static str {
        match self {
            Self::NonUnitaryOperation => "non-unitary operations (measure, reset, barrier, classical) do not commute by default",
            Self::ClassicalHazard => "classical wire hazard (RAW, WAR, or WAW) prevents commutation",
            Self::CrossAxisRotations => "single-qubit operations on different coordinate axes do not commute",
            Self::CxControlNonZAxis => "only Z-axis operations commute with CX control",
            Self::CxTargetNonXAxis => "only X-axis operations commute with CX target",
            Self::CxChainedDependency => "CX gates with chained control-target dependencies do not commute",
            Self::CxReversedOperands => "CX gates with reversed control and target do not commute",
            Self::CzNonZAxis => "only Z-axis operations commute with CZ operands",
            Self::PauliAnticommuting => "Pauli operators anticommute with odd single-qubit overlap",
            Self::UnsupportedOperation => "uncatalogued or unsupported operation cannot be proven to commute",
        }
    }
}

/// Outcome of a commutation query between two operations.
#[derive(Clone, Debug, PartialEq)]
pub enum CommutationResult {
    /// Operations are mathematically proven to commute.
    Commutes {
        /// The exact rule that authorized commutation.
        rule: CommutationRule,
        /// Detailed mathematical explanation.
        explanation: String,
    },
    /// Operations are proven or declared not to commute.
    DoesNotCommute {
        /// Reason for non-commutation.
        reason: NonCommutationReason,
        /// Detailed explanation.
        explanation: String,
    },
    /// Commutation cannot be determined (e.g. unbound symbolic parameters).
    Unknown {
        /// Explanation of ambiguity.
        reason: String,
    },
}

impl CommutationResult {
    /// Returns `true` if the operations are proven to commute.
    #[inline]
    pub fn is_commuting(&self) -> bool {
        matches!(self, Self::Commutes { .. })
    }

    /// Returns the matched commutation rule, if any.
    #[inline]
    pub fn rule(&self) -> Option<CommutationRule> {
        match self {
            Self::Commutes { rule, .. } => Some(*rule),
            _ => None,
        }
    }

    /// Returns the non-commutation reason, if any.
    #[inline]
    pub fn non_commutation_reason(&self) -> Option<NonCommutationReason> {
        match self {
            Self::DoesNotCommute { reason, .. } => Some(*reason),
            _ => None,
        }
    }
}

/// Precondition descriptor documenting an enabled commutation rule.
#[derive(Clone, Debug, PartialEq)]
pub struct CommutationPrecondition {
    /// Rule identifier.
    pub rule: CommutationRule,
    /// Supported gate/operation types.
    pub supported_operations: &'static [&'static str],
    /// Required operand relations (e.g. "disjoint", "same wire", "control only").
    pub operand_constraint: &'static str,
    /// Required parameter domain (e.g. "any", "0 mod 2pi").
    pub parameter_constraint: &'static str,
    /// Whether the rule is unconditional / exact.
    pub is_exact: bool,
}

/// Central registry of mathematically verified commutation rules.
#[derive(Clone, Debug, Default)]
pub struct CommutationRegistry {
    preconditions: Vec<CommutationPrecondition>,
}

impl CommutationRegistry {
    /// Creates a new commutation registry pre-populated with all verified rules.
    pub fn new() -> Self {
        let preconditions = vec![
            CommutationPrecondition {
                rule: CommutationRule::DisjointWires,
                supported_operations: &["all unitary gates"],
                operand_constraint: "disjoint quantum wires and no classical hazards",
                parameter_constraint: "any",
                is_exact: true,
            },
            CommutationPrecondition {
                rule: CommutationRule::IdentityOperand,
                supported_operations: &["id", "i", "rx", "ry", "rz"],
                operand_constraint: "any overlapping wires",
                parameter_constraint: "angle = 0 mod 2pi for rotations",
                is_exact: true,
            },
            CommutationPrecondition {
                rule: CommutationRule::IdenticalOperations,
                supported_operations: &["all canonical gates"],
                operand_constraint: "identical ordered operands",
                parameter_constraint: "identical parameters",
                is_exact: true,
            },
            CommutationPrecondition {
                rule: CommutationRule::SameAxisRotations,
                supported_operations: &["x", "y", "z", "s", "sdg", "t", "tdg", "rx", "ry", "rz"],
                operand_constraint: "same single qubit",
                parameter_constraint: "any angle on matching axis",
                is_exact: true,
            },
            CommutationPrecondition {
                rule: CommutationRule::ZAxisOnCxControl,
                supported_operations: &["z", "s", "sdg", "t", "tdg", "rz", "id"],
                operand_constraint: "single-qubit operand equals CX control (qubits[0])",
                parameter_constraint: "any angle for rz",
                is_exact: true,
            },
            CommutationPrecondition {
                rule: CommutationRule::XAxisOnCxTarget,
                supported_operations: &["x", "rx", "id"],
                operand_constraint: "single-qubit operand equals CX target (qubits[1])",
                parameter_constraint: "any angle for rx",
                is_exact: true,
            },
            CommutationPrecondition {
                rule: CommutationRule::ZAxisOnCz,
                supported_operations: &["z", "s", "sdg", "t", "tdg", "rz", "id"],
                operand_constraint: "single-qubit operand equals either CZ operand",
                parameter_constraint: "any angle for rz",
                is_exact: true,
            },
            CommutationPrecondition {
                rule: CommutationRule::TwoQubitDiagonal,
                supported_operations: &["cz"],
                operand_constraint: "cz with cz on arbitrary overlapping wires",
                parameter_constraint: "none",
                is_exact: true,
            },
            CommutationPrecondition {
                rule: CommutationRule::CxCzSharedControl,
                supported_operations: &["cx", "cz"],
                operand_constraint:
                    "cx and cz sharing the cx control qubit, with disjoint target wires",
                parameter_constraint: "none",
                is_exact: true,
            },
            CommutationPrecondition {
                rule: CommutationRule::CxSharedControl,
                supported_operations: &["cx"],
                operand_constraint: "identical control, disjoint targets",
                parameter_constraint: "none",
                is_exact: true,
            },
            CommutationPrecondition {
                rule: CommutationRule::CxSharedTarget,
                supported_operations: &["cx"],
                operand_constraint: "identical target, disjoint controls",
                parameter_constraint: "none",
                is_exact: true,
            },
            CommutationPrecondition {
                rule: CommutationRule::PauliString,
                supported_operations: &["x", "y", "z", "id"],
                operand_constraint: "even number of single-qubit anticommuting overlaps",
                parameter_constraint: "none",
                is_exact: true,
            },
        ];

        Self { preconditions }
    }

    /// Returns the list of registered rule preconditions.
    pub fn preconditions(&self) -> &[CommutationPrecondition] {
        &self.preconditions
    }

    /// Evaluates commutation between two operations given a variable environment.
    pub fn check_commutation(
        &self,
        op1: &Operation,
        op2: &Operation,
        env: &HashMap<String, f64>,
    ) -> CommutationResult {
        operations_commute(op1, op2, env)
    }
}

/// Returns `true` if the gate mnemonic is a recognized canonical single-qubit gate in QCC.
fn is_known_single_qubit_gate(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "x" | "y" | "z" | "h" | "s" | "sdg" | "t" | "tdg" | "rx" | "ry" | "rz" | "id" | "i"
    )
}

/// Helper returning `true` if an operation is proven to be the identity operator ($\pm I$).
fn is_proven_identity(op: &Operation, env: &HashMap<String, f64>) -> Option<bool> {
    match op {
        Operation::Gate {
            gate_name,
            parameters,
            ..
        } => {
            let lower = gate_name.to_ascii_lowercase();
            if lower == "id" || lower == "i" {
                return Some(true);
            }
            if matches!(lower.as_str(), "rx" | "ry" | "rz") {
                if let Some(param) = parameters.first() {
                    match param.eval(env) {
                        Ok(val) => {
                            let norm = normalize_rotation_angle(
                                val,
                                1e-12,
                                RotationNormalizationMode::TwoPiWithPhase,
                            );
                            return Some(norm.is_identity());
                        }
                        Err(_) => return None, // Unbound symbolic variable
                    }
                }
            }
            Some(false)
        }
        _ => Some(false),
    }
}

/// Evaluates commutation between two [`Operation`] instances in deterministic rule order.
pub fn operations_commute(
    op1: &Operation,
    op2: &Operation,
    env: &HashMap<String, f64>,
) -> CommutationResult {
    let effects1 = op1.wire_effects();
    let effects2 = op2.wire_effects();

    // 1. Non-unitary exclusion
    if effects1.is_barrier
        || effects2.is_barrier
        || matches!(
            (op1, op2),
            (Operation::Measure { .. }, _)
                | (_, Operation::Measure { .. })
                | (Operation::Reset { .. }, _)
                | (_, Operation::Reset { .. })
                | (Operation::Classical { .. }, _)
                | (_, Operation::Classical { .. })
        )
    {
        return CommutationResult::DoesNotCommute {
            reason: NonCommutationReason::NonUnitaryOperation,
            explanation: NonCommutationReason::NonUnitaryOperation
                .description()
                .to_string(),
        };
    }

    // 2. Classical hazard check
    let has_classical_conflict = effects1
        .classical_writes
        .iter()
        .any(|w| effects2.classical_reads.contains(w) || effects2.classical_writes.contains(w))
        || effects2
            .classical_writes
            .iter()
            .any(|w| effects1.classical_reads.contains(w));

    if has_classical_conflict {
        return CommutationResult::DoesNotCommute {
            reason: NonCommutationReason::ClassicalHazard,
            explanation: NonCommutationReason::ClassicalHazard
                .description()
                .to_string(),
        };
    }

    // Extract quantum gate definitions
    let (g1, qubits1, params1) = match op1 {
        Operation::Gate {
            gate_name,
            qubits,
            parameters,
        } => (gate_name.as_str(), qubits.as_slice(), parameters.as_slice()),
        _ => {
            return CommutationResult::DoesNotCommute {
                reason: NonCommutationReason::UnsupportedOperation,
                explanation: "Only unitary Gate operations are eligible for commutation"
                    .to_string(),
            };
        }
    };

    let (g2, qubits2, params2) = match op2 {
        Operation::Gate {
            gate_name,
            qubits,
            parameters,
        } => (gate_name.as_str(), qubits.as_slice(), parameters.as_slice()),
        _ => {
            return CommutationResult::DoesNotCommute {
                reason: NonCommutationReason::UnsupportedOperation,
                explanation: "Only unitary Gate operations are eligible for commutation"
                    .to_string(),
            };
        }
    };

    // 3. Disjoint quantum wires
    let shares_qubit = qubits1.iter().any(|q| qubits2.contains(q));
    if !shares_qubit {
        return CommutationResult::Commutes {
            rule: CommutationRule::DisjointWires,
            explanation: format!(
                "Operations act on disjoint quantum wires: {:?} and {:?}",
                qubits1, qubits2
            ),
        };
    }

    // 4. Identity operand rule
    match (is_proven_identity(op1, env), is_proven_identity(op2, env)) {
        (Some(true), _) => {
            return CommutationResult::Commutes {
                rule: CommutationRule::IdentityOperand,
                explanation: format!("Operation '{g1}' is proven identity",),
            };
        }
        (_, Some(true)) => {
            return CommutationResult::Commutes {
                rule: CommutationRule::IdentityOperand,
                explanation: format!("Operation '{g2}' is proven identity",),
            };
        }
        (None, _) | (_, None) => {
            // Unbound symbolic variable could potentially be zero or non-zero
        }
        _ => {}
    }

    // 5. Identical operations rule
    if g1.eq_ignore_ascii_case(g2) && qubits1 == qubits2 {
        if params1 == params2 {
            return CommutationResult::Commutes {
                rule: CommutationRule::IdenticalOperations,
                explanation: format!(
                    "Identical gate '{g1}' with identical operands and parameters"
                ),
            };
        }
        // If parameters differ, check if both evaluate to the same value
        if params1.len() == 1 && params2.len() == 1 {
            if let (Ok(v1), Ok(v2)) = (params1[0].eval(env), params2[0].eval(env)) {
                let n1 = Angle::from_radians(v1).normalize().radians();
                let n2 = Angle::from_radians(v2).normalize().radians();
                if (n1 - n2).abs() < 1e-12 {
                    return CommutationResult::Commutes {
                        rule: CommutationRule::IdenticalOperations,
                        explanation: format!(
                            "Identical gate '{g1}' with equal evaluated parameters"
                        ),
                    };
                }
            }
        }
    }

    let lower1 = g1.to_ascii_lowercase();
    let lower2 = g2.to_ascii_lowercase();

    // 6. Single-qubit with single-qubit on the same wire
    if qubits1.len() == 1 && qubits2.len() == 1 && qubits1[0] == qubits2[0] {
        let q = qubits1[0];

        // Same axis rotations
        if are_same_axis_rotations(&lower1, &lower2) {
            let axis = RotationAxis::from_gate_name(&lower1).unwrap();
            return CommutationResult::Commutes {
                rule: CommutationRule::SameAxisRotations,
                explanation: format!(
                    "Single-qubit operations '{lower1}' and '{lower2}' share {:?} rotation axis on q[{}]",
                    axis, q.0
                ),
            };
        }

        // Pauli string / single-qubit Pauli comparison
        if let (Some(p1), Some(p2)) = (
            Pauli::from_gate_name(&lower1),
            Pauli::from_gate_name(&lower2),
        ) {
            if p1.commutes_with(&p2) {
                return CommutationResult::Commutes {
                    rule: CommutationRule::PauliString,
                    explanation: format!(
                        "Pauli operators '{lower1}' and '{lower2}' commute on q[{}]",
                        q.0
                    ),
                };
            } else {
                return CommutationResult::DoesNotCommute {
                    reason: NonCommutationReason::PauliAnticommuting,
                    explanation: format!(
                        "Pauli operators '{lower1}' and '{lower2}' anticommute on q[{}]",
                        q.0
                    ),
                };
            }
        }

        // Non-commuting single-qubit gates
        if !is_known_single_qubit_gate(&lower1) || !is_known_single_qubit_gate(&lower2) {
            return CommutationResult::DoesNotCommute {
                reason: NonCommutationReason::UnsupportedOperation,
                explanation: format!(
                    "Uncatalogued single-qubit gate '{lower1}' or '{lower2}' cannot be analyzed for commutation"
                ),
            };
        }

        return CommutationResult::DoesNotCommute {
            reason: NonCommutationReason::CrossAxisRotations,
            explanation: format!(
                "Single-qubit gates '{lower1}' and '{lower2}' act on differing axes on q[{}]",
                q.0
            ),
        };
    }

    // 7. Single-qubit with CX
    if qubits1.len() == 1 && lower2 == "cx" && qubits2.len() == 2 {
        let q = qubits1[0];
        let c = qubits2[0];
        let t = qubits2[1];
        match single_qubit_cx_commutation(&lower1, q, c, t) {
            Some(true) => {
                let rule = if q == c {
                    CommutationRule::ZAxisOnCxControl
                } else if q == t {
                    CommutationRule::XAxisOnCxTarget
                } else {
                    CommutationRule::DisjointWires
                };
                return CommutationResult::Commutes {
                    rule,
                    explanation: format!(
                        "Gate '{lower1}' on q[{}] satisfies CX commutation rule ({:?})",
                        q.0, rule
                    ),
                };
            }
            Some(false) => {
                let reason = if q == c {
                    NonCommutationReason::CxControlNonZAxis
                } else {
                    NonCommutationReason::CxTargetNonXAxis
                };
                return CommutationResult::DoesNotCommute {
                    reason,
                    explanation: format!(
                        "Gate '{lower1}' on q[{}] fails CX commutation rule ({:?})",
                        q.0, reason
                    ),
                };
            }
            None => {
                return CommutationResult::DoesNotCommute {
                    reason: NonCommutationReason::UnsupportedOperation,
                    explanation: format!(
                        "Unknown or ambiguous gate '{lower1}' in CX commutation check"
                    ),
                };
            }
        }
    }

    // Symmetric CX with single-qubit
    if lower1 == "cx" && qubits1.len() == 2 && qubits2.len() == 1 {
        let c = qubits1[0];
        let t = qubits1[1];
        let q = qubits2[0];
        match single_qubit_cx_commutation(&lower2, q, c, t) {
            Some(true) => {
                let rule = if q == c {
                    CommutationRule::ZAxisOnCxControl
                } else if q == t {
                    CommutationRule::XAxisOnCxTarget
                } else {
                    CommutationRule::DisjointWires
                };
                return CommutationResult::Commutes {
                    rule,
                    explanation: format!(
                        "Gate '{lower2}' on q[{}] satisfies CX commutation rule ({:?})",
                        q.0, rule
                    ),
                };
            }
            Some(false) => {
                let reason = if q == c {
                    NonCommutationReason::CxControlNonZAxis
                } else {
                    NonCommutationReason::CxTargetNonXAxis
                };
                return CommutationResult::DoesNotCommute {
                    reason,
                    explanation: format!(
                        "Gate '{lower2}' on q[{}] fails CX commutation rule ({:?})",
                        q.0, reason
                    ),
                };
            }
            None => {
                return CommutationResult::DoesNotCommute {
                    reason: NonCommutationReason::UnsupportedOperation,
                    explanation: format!(
                        "Unknown or ambiguous gate '{lower2}' in CX commutation check"
                    ),
                };
            }
        }
    }

    // 8. Single-qubit with CZ
    if qubits1.len() == 1 && lower2 == "cz" && qubits2.len() == 2 {
        let q = qubits1[0];
        let z0 = qubits2[0];
        let z1 = qubits2[1];
        match single_qubit_cz_commutation(&lower1, q, z0, z1) {
            Some(true) => {
                return CommutationResult::Commutes {
                    rule: CommutationRule::ZAxisOnCz,
                    explanation: format!("Z-axis gate '{lower1}' commutes with CZ on q[{}]", q.0),
                };
            }
            Some(false) => {
                return CommutationResult::DoesNotCommute {
                    reason: NonCommutationReason::CzNonZAxis,
                    explanation: format!(
                        "Non-Z-axis gate '{lower1}' on q[{}] does not commute with CZ",
                        q.0
                    ),
                };
            }
            None => {
                return CommutationResult::DoesNotCommute {
                    reason: NonCommutationReason::UnsupportedOperation,
                    explanation: format!("Unknown gate '{lower1}' in CZ commutation check"),
                };
            }
        }
    }

    if lower1 == "cz" && qubits1.len() == 2 && qubits2.len() == 1 {
        let z0 = qubits1[0];
        let z1 = qubits1[1];
        let q = qubits2[0];
        match single_qubit_cz_commutation(&lower2, q, z0, z1) {
            Some(true) => {
                return CommutationResult::Commutes {
                    rule: CommutationRule::ZAxisOnCz,
                    explanation: format!("Z-axis gate '{lower2}' commutes with CZ on q[{}]", q.0),
                };
            }
            Some(false) => {
                return CommutationResult::DoesNotCommute {
                    reason: NonCommutationReason::CzNonZAxis,
                    explanation: format!(
                        "Non-Z-axis gate '{lower2}' on q[{}] does not commute with CZ",
                        q.0
                    ),
                };
            }
            None => {
                return CommutationResult::DoesNotCommute {
                    reason: NonCommutationReason::UnsupportedOperation,
                    explanation: format!("Unknown gate '{lower2}' in CZ commutation check"),
                };
            }
        }
    }

    // 9. Two-qubit gate with two-qubit gate
    if lower1 == "cx" && lower2 == "cx" && qubits1.len() == 2 && qubits2.len() == 2 {
        let c1 = qubits1[0];
        let t1 = qubits1[1];
        let c2 = qubits2[0];
        let t2 = qubits2[1];
        if cx_cx_commutation(c1, t1, c2, t2) {
            let rule = if c1 == c2 && t1 == t2 {
                CommutationRule::IdenticalOperations
            } else if c1 == c2 {
                CommutationRule::CxSharedControl
            } else if t1 == t2 {
                CommutationRule::CxSharedTarget
            } else {
                CommutationRule::DisjointWires
            };
            return CommutationResult::Commutes {
                rule,
                explanation: format!("CX gates commute under rule {:?}", rule),
            };
        } else {
            let reason = if c1 == t2 && t1 == c2 {
                NonCommutationReason::CxReversedOperands
            } else {
                NonCommutationReason::CxChainedDependency
            };
            return CommutationResult::DoesNotCommute {
                reason,
                explanation: format!("CX gates do not commute due to {:?}", reason),
            };
        }
    }

    if lower1 == "cz" && lower2 == "cz" && qubits1.len() == 2 && qubits2.len() == 2 {
        let _ = cz_cz_commutation(qubits1[0], qubits1[1], qubits2[0], qubits2[1]);
        return CommutationResult::Commutes {
            rule: CommutationRule::TwoQubitDiagonal,
            explanation: "All CZ gates are diagonal in the computational basis and commute"
                .to_string(),
        };
    }

    if lower1 == "cx" && lower2 == "cz" && qubits1.len() == 2 && qubits2.len() == 2 {
        if cx_cz_commutation(qubits1[0], qubits1[1], qubits2[0], qubits2[1]) {
            return CommutationResult::Commutes {
                rule: CommutationRule::CxCzSharedControl,
                explanation: "CX and CZ share the CX control qubit (where both branch on computational basis projectors) and act on disjoint target wires"
                    .to_string(),
            };
        } else {
            return CommutationResult::DoesNotCommute {
                reason: NonCommutationReason::CxTargetNonXAxis,
                explanation: "CZ touches CX target, where X and Z anticommute".to_string(),
            };
        }
    }

    if lower1 == "cz" && lower2 == "cx" && qubits1.len() == 2 && qubits2.len() == 2 {
        if cx_cz_commutation(qubits2[0], qubits2[1], qubits1[0], qubits1[1]) {
            return CommutationResult::Commutes {
                rule: CommutationRule::CxCzSharedControl,
                explanation: "CZ and CX share the CX control qubit (where both branch on computational basis projectors) and act on disjoint target wires"
                    .to_string(),
            };
        } else {
            return CommutationResult::DoesNotCommute {
                reason: NonCommutationReason::CxTargetNonXAxis,
                explanation: "CZ touches CX target, where X and Z anticommute".to_string(),
            };
        }
    }

    // Default: uncatalogued or non-commuting combination
    CommutationResult::DoesNotCommute {
        reason: NonCommutationReason::UnsupportedOperation,
        explanation: format!("No verified commutation rule exists for '{lower1}' and '{lower2}'"),
    }
}
