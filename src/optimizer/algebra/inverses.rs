//! Inverses registry and gate inverse algebra.
//!
//! Categorizes self-inverse gates, discrete inverse pairs, and parameterized rotation
//! inverses, maintaining exact mathematical rules and rejecting unsupported or non-unitary
//! inversion requests.

use std::f64::consts::PI;

use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::optimizer::algebra::AlgebraError;
use crate::quantum::gate::Gate;

/// Returns `true` if the gate with the given canonical name is mathematically self-inverse ($U^2 = I$).
///
/// Self-inverse gates in QCC's canonical basis are:
/// - Single-qubit: `X`, `Y`, `Z`, `H`.
/// - Two-qubit: `CX`, `CZ`, `SWAP`.
#[inline]
pub fn is_self_inverse(gate_name: &str) -> bool {
    matches!(
        gate_name.to_ascii_lowercase().as_str(),
        "x" | "y" | "z" | "h" | "cx" | "cz" | "swap"
    )
}

/// Returns the canonical inverse gate name if known.
///
/// For rotation gates, the inverted gate shares the same name (with negated parameter).
pub fn invert_gate_name(gate_name: &str) -> Option<&'static str> {
    match gate_name.to_ascii_lowercase().as_str() {
        "x" => Some("x"),
        "y" => Some("y"),
        "z" => Some("z"),
        "h" => Some("h"),
        "s" => Some("sdg"),
        "sdg" => Some("s"),
        "t" => Some("tdg"),
        "tdg" => Some("t"),
        "rx" => Some("rx"),
        "ry" => Some("ry"),
        "rz" => Some("rz"),
        "cx" => Some("cx"),
        "cz" => Some("cz"),
        "swap" => Some("swap"),
        _ => None,
    }
}

/// Computes the exact mathematical inverse of a typed [`Gate`].
#[inline]
pub fn invert_gate(gate: &Gate) -> Gate {
    gate.inverse()
}

/// Inverts a concrete [`Operation`].
///
/// # Errors
/// Returns [`AlgebraError::NonUnitaryOperation`] for measurements, resets, barriers, and classical ops.
/// Returns [`AlgebraError::NoInverseRegistered`] for uncatalogued custom gates.
pub fn invert_operation(op: &Operation) -> Result<Operation, AlgebraError> {
    match op {
        Operation::Gate {
            gate_name,
            qubits,
            parameters,
        } => {
            let lower = gate_name.to_ascii_lowercase();
            match lower.as_str() {
                "x" | "y" | "z" | "h" | "cx" | "cz" | "swap" => Ok(Operation::Gate {
                    gate_name: gate_name.clone(),
                    qubits: qubits.clone(),
                    parameters: parameters.clone(),
                }),
                "s" => Ok(Operation::Gate {
                    gate_name: "sdg".to_string(),
                    qubits: qubits.clone(),
                    parameters: Vec::new(),
                }),
                "sdg" => Ok(Operation::Gate {
                    gate_name: "s".to_string(),
                    qubits: qubits.clone(),
                    parameters: Vec::new(),
                }),
                "t" => Ok(Operation::Gate {
                    gate_name: "tdg".to_string(),
                    qubits: qubits.clone(),
                    parameters: Vec::new(),
                }),
                "tdg" => Ok(Operation::Gate {
                    gate_name: "t".to_string(),
                    qubits: qubits.clone(),
                    parameters: Vec::new(),
                }),
                "rx" | "ry" | "rz" => {
                    let inverted_params = parameters
                        .iter()
                        .map(|p| match p {
                            ParameterExpr::Constant(val) => ParameterExpr::Constant(-val),
                            ParameterExpr::Neg(inner) => (**inner).clone(),
                            other => ParameterExpr::Neg(Box::new(other.clone())),
                        })
                        .collect();
                    Ok(Operation::Gate {
                        gate_name: gate_name.clone(),
                        qubits: qubits.clone(),
                        parameters: inverted_params,
                    })
                }
                _ => Err(AlgebraError::NoInverseRegistered {
                    name: gate_name.clone(),
                }),
            }
        }
        Operation::Measure { .. }
        | Operation::Reset { .. }
        | Operation::Barrier { .. }
        | Operation::Classical { .. } => Err(AlgebraError::NonUnitaryOperation),
    }
}

/// Checks whether two operations are exact mathematical inverses of each other ($op_2 \cdot op_1 = I$).
///
/// Both operations must act on the exact same qubit operands and cancel to identity without
/// global phase discrepancy.
pub fn are_inverses(op1: &Operation, op2: &Operation) -> bool {
    let (g1, q1, p1) = match op1 {
        Operation::Gate {
            gate_name,
            qubits,
            parameters,
        } => (gate_name.to_ascii_lowercase(), qubits, parameters),
        _ => return false,
    };
    let (g2, q2, p2) = match op2 {
        Operation::Gate {
            gate_name,
            qubits,
            parameters,
        } => (gate_name.to_ascii_lowercase(), qubits, parameters),
        _ => return false,
    };

    // Qubit operands check
    if q1.len() != q2.len() {
        return false;
    }

    let is_symmetric = matches!(g1.as_str(), "cz" | "swap") && g1 == g2;
    if is_symmetric {
        if q1.len() == 2 && q2.len() == 2 {
            let same = (q1[0] == q2[0] && q1[1] == q2[1]) || (q1[0] == q2[1] && q1[1] == q2[0]);
            if !same {
                return false;
            }
        } else if q1 != q2 {
            return false;
        }
    } else if q1 != q2 {
        return false;
    }

    // Gate relationship check
    match (g1.as_str(), g2.as_str()) {
        ("x", "x")
        | ("y", "y")
        | ("z", "z")
        | ("h", "h")
        | ("cx", "cx")
        | ("cz", "cz")
        | ("swap", "swap") => true,
        ("s", "sdg") | ("sdg", "s") => true,
        ("t", "tdg") | ("tdg", "t") => true,
        ("rx", "rx") | ("ry", "ry") | ("rz", "rz") => {
            if p1.len() != 1 || p2.len() != 1 {
                return false;
            }
            are_rotation_angles_inverse(&p1[0], &p2[0])
        }
        _ => false,
    }
}

/// Helper to verify if two parameter expressions evaluate to inverse rotation angles ($(\theta_1 + \theta_2) = 0 \pmod{4\pi}$).
///
/// Note: exact matrix cancellation requires modulo $4\pi$, because $R_k(2\pi) = -I \neq I$.
fn are_rotation_angles_inverse(p1: &ParameterExpr, p2: &ParameterExpr) -> bool {
    // 1. Direct symbolic negation check: p1 == -p2 or -p1 == p2
    match (p1, p2) {
        (ParameterExpr::Constant(c1), ParameterExpr::Constant(c2)) => {
            let sum = c1 + c2;
            let four_pi = 4.0 * PI;
            let rem = (sum % four_pi).abs();
            rem < 1e-12 || (four_pi - rem).abs() < 1e-12
        }
        (ParameterExpr::Neg(inner), other) | (other, ParameterExpr::Neg(inner))
            if **inner == *other =>
        {
            true
        }
        (ParameterExpr::Symbolic(s1), ParameterExpr::Symbolic(s2)) => {
            // Distinct positive symbols are not inverses
            s1 == s2 && false
        }
        _ => false,
    }
}
