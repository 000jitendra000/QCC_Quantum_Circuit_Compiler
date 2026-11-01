//! Deterministic formatting helpers for OpenQASM 3 identifiers, registers, and expressions.

use crate::backend::openqasm::errors::OpenQasmError;
use crate::ir::circuit::{ClassicalRegister, QuantumRegister};
use crate::ir::parameter::ParameterExpr;
use crate::ir::wire::{ClassicalBitId, QubitId};

/// Reserved keywords in OpenQASM 3.0 that cannot be used as register or symbol identifiers.
pub const OPENQASM_KEYWORDS: &[&str] = &[
    "OPENQASM", "include", "qubit", "bit", "qreg", "creg", "gate", "measure", "reset", "barrier",
    "gphase", "if", "else", "while", "for", "in", "return", "def", "defcal", "cal", "let", "const",
    "input", "output", "angle", "float", "int", "uint", "bool", "complex", "duration", "stretch",
    "box", "delay", "inv", "ctrl", "negctrl", "extern",
];

/// Validates whether an identifier adheres to OpenQASM 3 syntax:
/// `[a-zA-Z_][a-zA-Z0-9_]*` and is not a reserved keyword.
pub fn is_valid_identifier(ident: &str) -> bool {
    if ident.is_empty() {
        return false;
    }
    let mut chars = ident.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return false;
        }
    }
    !OPENQASM_KEYWORDS.contains(&ident)
}

/// Formats a flat [`QubitId`] into its scoped OpenQASM 3 register reference, e.g. `"q[0]"`.
///
/// # Errors
/// Returns [`OpenQasmError::UnresolvedQubit`] if the qubit does not belong to any declared register.
/// Returns [`OpenQasmError::InvalidIdentifier`] if the register name is not a legal OpenQASM identifier.
pub fn format_qubit_ref(
    qubit: QubitId,
    registers: &[QuantumRegister],
) -> Result<String, OpenQasmError> {
    for reg in registers {
        if qubit.0 >= reg.start_qubit && qubit.0 < reg.start_qubit + reg.size {
            if !is_valid_identifier(&reg.name) {
                return Err(OpenQasmError::InvalidIdentifier {
                    identifier: reg.name.clone(),
                });
            }
            let index = qubit.0 - reg.start_qubit;
            return Ok(format!("{}[{}]", reg.name, index));
        }
    }

    let total = registers.iter().map(|r| r.size).sum();
    Err(OpenQasmError::UnresolvedQubit {
        qubit: qubit.0,
        total,
    })
}

/// Formats a flat [`ClassicalBitId`] into its scoped OpenQASM 3 register reference, e.g. `"c[0]"`.
///
/// # Errors
/// Returns [`OpenQasmError::UnresolvedClassicalBit`] if the bit does not belong to any declared register.
/// Returns [`OpenQasmError::InvalidIdentifier`] if the register name is not a legal OpenQASM identifier.
pub fn format_bit_ref(
    bit: ClassicalBitId,
    registers: &[ClassicalRegister],
) -> Result<String, OpenQasmError> {
    for reg in registers {
        if bit.0 >= reg.start_bit && bit.0 < reg.start_bit + reg.size {
            if !is_valid_identifier(&reg.name) {
                return Err(OpenQasmError::InvalidIdentifier {
                    identifier: reg.name.clone(),
                });
            }
            let index = bit.0 - reg.start_bit;
            return Ok(format!("{}[{}]", reg.name, index));
        }
    }

    let total = registers.iter().map(|r| r.size).sum();
    Err(OpenQasmError::UnresolvedClassicalBit { bit: bit.0, total })
}

/// Formats a floating-point number into a deterministic, locale-independent string representation.
///
/// # Errors
/// Returns [`OpenQasmError::NonFiniteParameter`] if the value is NaN or infinite.
pub fn format_float(val: f64) -> Result<String, OpenQasmError> {
    if !val.is_finite() {
        return Err(OpenQasmError::NonFiniteParameter { value: val });
    }
    // Normalize -0.0 to 0.0
    if val == 0.0 {
        return Ok("0".to_string());
    }

    // Rust default format for f64 is locale-independent (uses '.' decimal separator)
    let s = format!("{val}");
    Ok(s)
}

/// Operator precedence levels for parenthesizing arithmetic subexpressions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Precedence {
    /// Addition, Subtraction (`+`, `-`)
    AddSub = 1,
    /// Multiplication, Division (`*`, `/`)
    MulDiv = 2,
    /// Unary negation (`-`)
    Unary = 3,
    /// Atoms (Constants, Symbols)
    Atom = 4,
}

fn expr_precedence(expr: &ParameterExpr) -> Precedence {
    match expr {
        ParameterExpr::Constant(_) | ParameterExpr::Symbolic(_) => Precedence::Atom,
        ParameterExpr::Neg(_) => Precedence::Unary,
        ParameterExpr::Mul(_, _) | ParameterExpr::Div(_, _) => Precedence::MulDiv,
        ParameterExpr::Add(_, _) | ParameterExpr::Sub(_, _) => Precedence::AddSub,
    }
}

fn format_subexpr(expr: &ParameterExpr, min_prec: Precedence) -> Result<String, OpenQasmError> {
    let prec = expr_precedence(expr);
    let formatted = match expr {
        ParameterExpr::Constant(val) => format_float(*val)?,
        ParameterExpr::Symbolic(name) => {
            if !is_valid_identifier(name) {
                return Err(OpenQasmError::InvalidIdentifier {
                    identifier: name.clone(),
                });
            }
            name.clone()
        }
        ParameterExpr::Add(left, right) => {
            let left_str = format_subexpr(left, Precedence::AddSub)?;
            let right_str = format_subexpr(right, Precedence::AddSub)?;
            format!("{left_str} + {right_str}")
        }
        ParameterExpr::Sub(left, right) => {
            let left_str = format_subexpr(left, Precedence::AddSub)?;
            // Right operand of Sub requires higher precedence to avoid associativity ambiguity: a - (b + c)
            let right_str = format_subexpr(right, Precedence::MulDiv)?;
            format!("{left_str} - {right_str}")
        }
        ParameterExpr::Mul(left, right) => {
            let left_str = format_subexpr(left, Precedence::MulDiv)?;
            let right_str = format_subexpr(right, Precedence::MulDiv)?;
            format!("{left_str} * {right_str}")
        }
        ParameterExpr::Div(left, right) => {
            let left_str = format_subexpr(left, Precedence::MulDiv)?;
            // Right operand of Div requires higher precedence to avoid: a / (b * c)
            let right_str = format_subexpr(right, Precedence::Unary)?;
            format!("{left_str} / {right_str}")
        }
        ParameterExpr::Neg(inner) => {
            let inner_str = format_subexpr(inner, Precedence::Unary)?;
            format!("-{inner_str}")
        }
    };

    if prec < min_prec {
        Ok(format!("({formatted})"))
    } else {
        Ok(formatted)
    }
}

/// Formats a [`ParameterExpr`] into valid OpenQASM 3 expression syntax with correct operator precedence.
///
/// # Errors
/// Returns [`OpenQasmError::NonFiniteParameter`] if any constant within the expression is NaN or infinite.
/// Returns [`OpenQasmError::InvalidIdentifier`] if any symbol within the expression is an invalid identifier.
pub fn format_parameter_expr(expr: &ParameterExpr) -> Result<String, OpenQasmError> {
    format_subexpr(expr, Precedence::AddSub)
}

/// Returns `true` if the parameter expression evaluates/normalizes structurally to zero.
pub fn is_zero_parameter(expr: &ParameterExpr) -> bool {
    match expr {
        ParameterExpr::Constant(val) => val.abs() < 1e-15,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_identifiers() {
        assert!(is_valid_identifier("q"));
        assert!(is_valid_identifier("ancilla_0"));
        assert!(is_valid_identifier("_hidden"));
        assert!(is_valid_identifier("theta1"));

        // Invalid: starts with digit, empty, contains spaces, or reserved keyword
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("0q"));
        assert!(!is_valid_identifier("q[0]"));
        assert!(!is_valid_identifier("my qubit"));
        assert!(!is_valid_identifier("qubit"));
        assert!(!is_valid_identifier("OPENQASM"));
        assert!(!is_valid_identifier("measure"));
    }

    #[test]
    fn test_format_qubit_and_bit_refs() {
        let q_regs = vec![
            QuantumRegister {
                name: "q".to_string(),
                size: 2,
                start_qubit: 0,
            },
            QuantumRegister {
                name: "anc".to_string(),
                size: 1,
                start_qubit: 2,
            },
        ];

        assert_eq!(format_qubit_ref(QubitId(0), &q_regs).unwrap(), "q[0]");
        assert_eq!(format_qubit_ref(QubitId(1), &q_regs).unwrap(), "q[1]");
        assert_eq!(format_qubit_ref(QubitId(2), &q_regs).unwrap(), "anc[0]");
        assert!(format_qubit_ref(QubitId(3), &q_regs).is_err());

        let c_regs = vec![ClassicalRegister {
            name: "c".to_string(),
            size: 2,
            start_bit: 0,
        }];
        assert_eq!(format_bit_ref(ClassicalBitId(0), &c_regs).unwrap(), "c[0]");
        assert_eq!(format_bit_ref(ClassicalBitId(1), &c_regs).unwrap(), "c[1]");
        assert!(format_bit_ref(ClassicalBitId(2), &c_regs).is_err());
    }

    #[test]
    fn test_format_parameter_expr_precedence() {
        let a = ParameterExpr::symbol("a");
        let b = ParameterExpr::symbol("b");
        let c = ParameterExpr::symbol("c");

        // a + b
        let expr = a.clone() + b.clone();
        assert_eq!(format_parameter_expr(&expr).unwrap(), "a + b");

        // (a + b) * c
        let expr = (a.clone() + b.clone()) * c.clone();
        assert_eq!(format_parameter_expr(&expr).unwrap(), "(a + b) * c");

        // a + b * c
        let expr = a.clone() + (b.clone() * c.clone());
        assert_eq!(format_parameter_expr(&expr).unwrap(), "a + b * c");

        // a - (b + c)
        let expr = ParameterExpr::Sub(Box::new(a.clone()), Box::new(b.clone() + c.clone()));
        assert_eq!(format_parameter_expr(&expr).unwrap(), "a - (b + c)");

        // -(a + b)
        let expr = -(a.clone() + b.clone());
        assert_eq!(format_parameter_expr(&expr).unwrap(), "-(a + b)");

        // Non-finite rejection
        let nan_expr = ParameterExpr::constant(f64::NAN);
        assert!(format_parameter_expr(&nan_expr).is_err());
    }
}
