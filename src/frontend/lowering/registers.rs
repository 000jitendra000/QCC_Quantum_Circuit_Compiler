//! Register allocation and operand resolution for AST-to-QccIR lowering.
//!
//! Maps frontend register declarations to QccIR [`Circuit`] quantum and classical registers,
//! preserving declaration order and mapping source operand references to typed
//! [`QubitId`] and [`ClassicalBitId`] identities.

use std::ops::Range;

use crate::diagnostics::{qasm, Diagnostic};
use crate::frontend::ast::{AstBitRef, AstQubitRef, AstRegisterDecl};
use crate::frontend::parser::registers::RegisterError;
use crate::frontend::semantic::SymbolTable;
use crate::ir::circuit::Circuit;
use crate::ir::wire::{ClassicalBitId, QubitId};

/// Allocates a quantum register in `circuit` from an AST register declaration node.
///
/// # Errors
/// Returns a diagnostic if the register has size 0 (`E0011`) or duplicate name (`E0010`).
pub fn lower_quantum_register_decl(
    circuit: &mut Circuit,
    decl: &AstRegisterDecl,
    file_path: &str,
) -> Result<Range<usize>, Box<Diagnostic>> {
    let size = decl.size.unwrap_or(1);
    if size == 0 || size > crate::frontend::parser::declarations::MAX_REGISTER_SIZE {
        return Err(Box::new(qasm::invalid_register_size(
            &decl.name, decl.span, file_path,
        )));
    }
    circuit
        .add_quantum_register(&decl.name, size)
        .map_err(|err| match err {
            crate::ir::error::IrError::DuplicateRegister { name } => Box::new(
                qasm::duplicate_declaration(&name, decl.span, None, file_path),
            ),
            other => Box::new(qasm::lowering_error(
                format!(
                    "failed to declare quantum register '{}': {other}",
                    decl.name
                ),
                decl.span,
                file_path,
            )),
        })
}

/// Allocates a classical register in `circuit` from an AST register declaration node.
///
/// # Errors
/// Returns a diagnostic if the register has size 0 (`E0011`) or duplicate name (`E0010`).
pub fn lower_classical_register_decl(
    circuit: &mut Circuit,
    decl: &AstRegisterDecl,
    file_path: &str,
) -> Result<Range<usize>, Box<Diagnostic>> {
    let size = decl.size.unwrap_or(1);
    if size == 0 || size > crate::frontend::parser::declarations::MAX_REGISTER_SIZE {
        return Err(Box::new(qasm::invalid_register_size(
            &decl.name, decl.span, file_path,
        )));
    }
    circuit
        .add_classical_register(&decl.name, size)
        .map_err(|err| match err {
            crate::ir::error::IrError::DuplicateRegister { name } => Box::new(
                qasm::duplicate_declaration(&name, decl.span, None, file_path),
            ),
            other => Box::new(qasm::lowering_error(
                format!(
                    "failed to declare classical register '{}': {other}",
                    decl.name
                ),
                decl.span,
                file_path,
            )),
        })
}

/// Resolves an AST qubit reference to a single typed QccIR [`QubitId`].
///
/// Accepts indexed references (e.g. `q[0]`) or scalar references (e.g. `q` for `qubit q;`).
///
/// # Errors
/// Returns structured diagnostics for undeclared register, type mismatch, bounds errors,
/// or unindexed arrays.
pub fn resolve_qubit_operand(
    symbols: &SymbolTable,
    qubit_ref: &AstQubitRef,
    file_path: &str,
) -> Result<QubitId, Box<Diagnostic>> {
    symbols
        .resolve_qubit(qubit_ref)
        .map(|res| QubitId(res.flat_qubit_id.0 as usize))
        .map_err(|err| Box::new(convert_register_error(&err, file_path)))
}

/// Resolves an AST classical bit reference to a single typed QccIR [`ClassicalBitId`].
///
/// Accepts indexed references (e.g. `c[0]`) or scalar references (e.g. `c` for `bit c;`).
///
/// # Errors
/// Returns structured diagnostics for undeclared register, type mismatch, bounds errors,
/// or unindexed arrays.
pub fn resolve_bit_operand(
    symbols: &SymbolTable,
    bit_ref: &AstBitRef,
    file_path: &str,
) -> Result<ClassicalBitId, Box<Diagnostic>> {
    symbols
        .resolve_bit(bit_ref)
        .map(|res| ClassicalBitId(res.flat_bit_id.0 as usize))
        .map_err(|err| Box::new(convert_register_error(&err, file_path)))
}

/// Expands a qubit reference into a list of [`QubitId`]s for non-unitary operations
/// (such as reset and barrier) that support whole-register application.
///
/// - If `qubit_ref` is indexed (e.g. `q[0]`): returns `[QubitId(q0)]`.
/// - If `qubit_ref` is a scalar declaration (`qubit q;`): returns `[QubitId(q)]`.
/// - If `qubit_ref` is an unindexed array (`qubit[n] q;`): returns `[QubitId(0), ..., QubitId(n-1)]`.
pub fn expand_qubits_for_reset_or_barrier(
    symbols: &SymbolTable,
    qubit_ref: &AstQubitRef,
    file_path: &str,
) -> Result<Vec<QubitId>, Box<Diagnostic>> {
    // First validate qubit reference allowing unindexed array
    symbols
        .validate_qubit_ref(qubit_ref, true)
        .map_err(|err| Box::new(convert_register_error(&err, file_path)))?;

    let reg = symbols
        .registers()
        .get_quantum(&qubit_ref.name)
        .ok_or_else(|| {
            Box::new(qasm::undeclared_register(
                &qubit_ref.name,
                qubit_ref.span,
                file_path,
            ))
        })?;

    match qubit_ref.index {
        Some(idx) => {
            if idx >= reg.size {
                Err(Box::new(qasm::index_out_of_bounds(
                    &reg.name,
                    idx,
                    reg.size,
                    qubit_ref.span,
                    file_path,
                )))
            } else {
                Ok(vec![QubitId(reg.base_index + idx)])
            }
        }
        None => {
            // Whole register expansion (single element for scalar, all elements for array)
            Ok((0..reg.size).map(|i| QubitId(reg.base_index + i)).collect())
        }
    }
}

/// Expands a measurement statement into pairs of `(QubitId, ClassicalBitId)` operands.
///
/// Handles:
/// - Single-bit measurement: `c[i] = measure q[j];` -> 1 pair
/// - Scalar measurement: `c = measure q;` where both are scalar -> 1 pair
/// - Whole-register measurement: `c = measure q;` where both are arrays of size $N$ -> $N$ pairs
///
/// # Errors
/// Returns diagnostics for register mismatch, bounds errors, or size mismatch (`E0025`).
pub fn expand_measurement_operands(
    symbols: &SymbolTable,
    measure: &crate::frontend::ast::AstMeasure,
    file_path: &str,
) -> Result<Vec<(QubitId, ClassicalBitId)>, Box<Diagnostic>> {
    // Validate measurement pairing via SymbolTable
    symbols
        .validate_measurement(&measure.qubit, &measure.bit, measure.span)
        .map_err(|err| Box::new(convert_register_error(&err, file_path)))?;

    let q_entry = symbols
        .registers()
        .get_quantum(&measure.qubit.name)
        .ok_or_else(|| {
            Box::new(qasm::undeclared_register(
                &measure.qubit.name,
                measure.qubit.span,
                file_path,
            ))
        })?;

    let b_entry = symbols
        .registers()
        .get_classical(&measure.bit.name)
        .ok_or_else(|| {
            Box::new(qasm::undeclared_register(
                &measure.bit.name,
                measure.bit.span,
                file_path,
            ))
        })?;

    match (measure.qubit.index, measure.bit.index) {
        (Some(q_idx), Some(b_idx)) => {
            let qid = QubitId(q_entry.base_index + q_idx);
            let cid = ClassicalBitId(b_entry.base_index + b_idx);
            Ok(vec![(qid, cid)])
        }
        (None, None) => {
            if q_entry.size != b_entry.size {
                return Err(Box::new(qasm::measurement_size_mismatch(
                    &q_entry.name,
                    q_entry.size,
                    &b_entry.name,
                    b_entry.size,
                    measure.span,
                    file_path,
                )));
            }
            let pairs = (0..q_entry.size)
                .map(|i| {
                    (
                        QubitId(q_entry.base_index + i),
                        ClassicalBitId(b_entry.base_index + i),
                    )
                })
                .collect();
            Ok(pairs)
        }
        _ => Err(Box::new(qasm::lowering_error(
            "unsupported measurement operand alignment",
            measure.span,
            file_path,
        ))),
    }
}

/// Converts an internal [`RegisterError`] into a source-located standardized [`Diagnostic`].
pub fn convert_register_error(err: &RegisterError, file_path: &str) -> Diagnostic {
    match err {
        RegisterError::DuplicateName {
            name,
            span,
            previous_span,
        } => {
            let prev = previous_span.as_ref().map(|s| **s);
            qasm::duplicate_declaration(name, *span, prev, file_path)
        }
        RegisterError::ZeroSize { name, span } => {
            qasm::invalid_register_size(name, *span, file_path)
        }
        RegisterError::UndeclaredRegister { name, span } => {
            qasm::undeclared_register(name, *span, file_path)
        }
        RegisterError::TypeMismatch {
            name,
            expected,
            actual,
            span,
        } => qasm::register_type_mismatch(name, expected, actual, *span, file_path),
        RegisterError::IndexOutOfBounds {
            name,
            index,
            size,
            span,
        } => qasm::index_out_of_bounds(name, *index, *size, *span, file_path),
        RegisterError::IndexedScalar { name, span } => qasm::indexed_scalar(name, *span, file_path),
        RegisterError::UnindexedArray { name, size, span } => {
            qasm::unindexed_array(name, *size, *span, file_path)
        }
        RegisterError::MeasurementSizeMismatch {
            qubit_name,
            qubit_size,
            bit_name,
            bit_size,
            span,
        } => qasm::measurement_size_mismatch(
            qubit_name,
            *qubit_size,
            bit_name,
            *bit_size,
            *span,
            file_path,
        ),
    }
}
