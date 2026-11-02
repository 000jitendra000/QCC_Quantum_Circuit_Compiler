//! Operation statement lowering from AST to canonical QccIR.
//!
//! Converts gate applications, measurements, resets, barriers, and global phase directives
//! into validated QccIR [`Operation`] instances, enforcing semantic contracts:
//! - Little-endian qubit indexing
//! - For CX, operand 0 is control, operand 1 is target
//! - Non-unitary ordering semantics (RAW, WAR, WAW)
//! - Explicit circuit global phase preservation ($U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$)
//! - Source span provenance preservation

use std::collections::HashMap;

use crate::diagnostics::{qasm, Diagnostic};
use crate::frontend::ast::expression::{AstExprKind, AstExpression};
use crate::frontend::ast::statement::{
    AstBarrier, AstGateCall, AstGlobalPhase, AstMeasure, AstReset,
};
use crate::frontend::lowering::registers::{
    expand_measurement_operands, expand_qubits_for_reset_or_barrier, resolve_qubit_operand,
};
use crate::frontend::semantic::SymbolTable;
use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;
use crate::ir::parameter::ParameterExpr;
use crate::ir::wire::QubitId;
use crate::optimizer::algebra::identities::simplify_parameter_expr;
use crate::quantum::gate::Gate;

/// Lowers an AST parameter expression into a canonical QccIR [`ParameterExpr`].
///
/// Substitutes mathematical constants `pi` and `π` with $\pi$, resolves bound variables
/// from `env`, checks for division by zero (`E0030`) and non-finite results (`E0032`),
/// and simplifies constant subexpressions without losing symbolic structure.
///
/// # Errors
/// Returns [`Diagnostic`] with:
/// - `E0030` if division by zero occurs
/// - `E0031` if an unbound symbol identifier is encountered
/// - `E0032` if evaluation results in non-finite values (NaN / infinity)
pub fn lower_parameter_expr(
    expr: &AstExpression,
    env: &HashMap<String, f64>,
    file_path: &str,
) -> Result<ParameterExpr, Box<Diagnostic>> {
    let raw = lower_expr_recursive(expr, env, file_path)?;
    Ok(simplify_parameter_expr(&raw))
}

fn lower_expr_recursive(
    expr: &AstExpression,
    env: &HashMap<String, f64>,
    file_path: &str,
) -> Result<ParameterExpr, Box<Diagnostic>> {
    match &expr.kind {
        AstExprKind::Constant(val) => {
            if !val.is_finite() {
                return Err(Box::new(qasm::non_finite_parameter(
                    *val, expr.span, file_path,
                )));
            }
            Ok(ParameterExpr::Constant(*val))
        }
        AstExprKind::Identifier(name) => {
            if name == "pi" || name == "π" {
                Ok(ParameterExpr::Constant(std::f64::consts::PI))
            } else if let Some(&bound) = env.get(name) {
                if !bound.is_finite() {
                    return Err(Box::new(qasm::non_finite_parameter(
                        bound, expr.span, file_path,
                    )));
                }
                Ok(ParameterExpr::Constant(bound))
            } else {
                Err(Box::new(qasm::unresolved_symbol(
                    name,
                    "parameter expression",
                    expr.span,
                    file_path,
                )))
            }
        }
        AstExprKind::Add(a, b) => {
            let pa = lower_expr_recursive(a, env, file_path)?;
            let pb = lower_expr_recursive(b, env, file_path)?;
            Ok(pa + pb)
        }
        AstExprKind::Sub(a, b) => {
            let pa = lower_expr_recursive(a, env, file_path)?;
            let pb = lower_expr_recursive(b, env, file_path)?;
            Ok(pa - pb)
        }
        AstExprKind::Mul(a, b) => {
            let pa = lower_expr_recursive(a, env, file_path)?;
            let pb = lower_expr_recursive(b, env, file_path)?;
            Ok(pa * pb)
        }
        AstExprKind::Div(a, b) => {
            let pa = lower_expr_recursive(a, env, file_path)?;
            let pb = lower_expr_recursive(b, env, file_path)?;

            // Check exact division by zero
            if let Ok(denom_val) = pb.eval(env) {
                if denom_val == 0.0 || denom_val == -0.0 {
                    return Err(Box::new(qasm::division_by_zero(expr.span, file_path)));
                }
                if let Ok(num_val) = pa.eval(env) {
                    let result = num_val / denom_val;
                    if !result.is_finite() {
                        return Err(Box::new(qasm::non_finite_parameter(
                            result, expr.span, file_path,
                        )));
                    }
                }
            } else if pb == ParameterExpr::Constant(0.0) || pb == ParameterExpr::Constant(-0.0) {
                return Err(Box::new(qasm::division_by_zero(expr.span, file_path)));
            }

            Ok(ParameterExpr::Div(Box::new(pa), Box::new(pb)))
        }
        AstExprKind::Neg(a) => {
            let pa = lower_expr_recursive(a, env, file_path)?;
            Ok(-pa)
        }
        AstExprKind::Pow(base, exponent) => {
            let p_base = lower_expr_recursive(base, env, file_path)?;
            let p_exp = lower_expr_recursive(exponent, env, file_path)?;
            match (p_base.eval(env), p_exp.eval(env)) {
                (Ok(b), Ok(e)) => {
                    let res = b.powf(e);
                    if !res.is_finite() {
                        return Err(Box::new(qasm::non_finite_parameter(
                            res, expr.span, file_path,
                        )));
                    }
                    Ok(ParameterExpr::Constant(res))
                }
                _ => Err(Box::new(qasm::lowering_error(
                    "unsupported symbolic exponentiation in QccIR parameter expression",
                    expr.span,
                    file_path,
                ))),
            }
        }
        AstExprKind::Call(func, args) => {
            if args.len() != 1 {
                return Err(Box::new(qasm::lowering_error(
                    format!(
                        "function '{func}' expects 1 argument, but {} provided",
                        args.len()
                    ),
                    expr.span,
                    file_path,
                )));
            }
            let arg_param = lower_expr_recursive(&args[0], env, file_path)?;
            let arg_val = arg_param.eval(env).map_err(|_| {
                Box::new(qasm::lowering_error(
                    format!("symbolic argument to function '{func}' cannot be evaluated at lowering time"),
                    expr.span,
                    file_path,
                ))
            })?;
            let res = match func.as_str() {
                "sin" => arg_val.sin(),
                "cos" => arg_val.cos(),
                "tan" => arg_val.tan(),
                "sqrt" => {
                    if arg_val < 0.0 {
                        return Err(Box::new(qasm::non_finite_parameter(
                            f64::NAN,
                            expr.span,
                            file_path,
                        )));
                    }
                    arg_val.sqrt()
                }
                "exp" => arg_val.exp(),
                "ln" => {
                    if arg_val <= 0.0 {
                        return Err(Box::new(qasm::non_finite_parameter(
                            f64::NAN,
                            expr.span,
                            file_path,
                        )));
                    }
                    arg_val.ln()
                }
                _ => {
                    return Err(Box::new(qasm::lowering_error(
                        format!("unsupported function '{func}' in parameter expression"),
                        expr.span,
                        file_path,
                    )));
                }
            };
            if !res.is_finite() {
                return Err(Box::new(qasm::non_finite_parameter(
                    res, expr.span, file_path,
                )));
            }
            Ok(ParameterExpr::Constant(res))
        }
    }
}

/// Lowers an AST gate call statement into a canonical QccIR gate [`Operation`].
///
/// Enforces:
/// - Exact parameter and qubit arities (`E0021`, `E0022`)
/// - Distinct operands for multi-qubit gates (`E0023`)
/// - For CX: operand 0 is control, operand 1 is target
/// - Preservation of source span in `circuit.metadata`
pub fn lower_gate_statement(
    circuit: &mut Circuit,
    call: &AstGateCall,
    symbols: &SymbolTable,
    env: &HashMap<String, f64>,
    file_path: &str,
) -> Result<OperationId, Box<Diagnostic>> {
    let gate_name = call.name.to_ascii_lowercase();

    match gate_name.as_str() {
        // --- 1-Qubit Fixed Gates ---
        "x" | "y" | "z" | "h" | "s" | "sdg" | "t" | "tdg" => {
            if !call.parameters.is_empty() {
                return Err(Box::new(qasm::gate_param_arity(
                    &call.name,
                    0,
                    call.parameters.len(),
                    call.span,
                    file_path,
                )));
            }
            if call.qubits.len() != 1 {
                return Err(Box::new(qasm::gate_qubit_arity(
                    &call.name,
                    1,
                    call.qubits.len(),
                    call.span,
                    file_path,
                )));
            }
            let qid = resolve_qubit_operand(symbols, &call.qubits[0], file_path)?;
            let gate = match gate_name.as_str() {
                "x" => Gate::X,
                "y" => Gate::Y,
                "z" => Gate::Z,
                "h" => Gate::H,
                "s" => Gate::S,
                "sdg" => Gate::Sdg,
                "t" => Gate::T,
                "tdg" => Gate::Tdg,
                _ => unreachable!(),
            };
            let op = Operation::gate(&gate, vec![qid]);
            add_operation_with_span(circuit, op, call.span, file_path)
        }

        // --- 1-Qubit Explicit Identity Gates ---
        "id" | "i" => {
            if !call.parameters.is_empty() {
                return Err(Box::new(qasm::gate_param_arity(
                    &call.name,
                    0,
                    call.parameters.len(),
                    call.span,
                    file_path,
                )));
            }
            if call.qubits.len() != 1 {
                return Err(Box::new(qasm::gate_qubit_arity(
                    &call.name,
                    1,
                    call.qubits.len(),
                    call.span,
                    file_path,
                )));
            }
            let qid = resolve_qubit_operand(symbols, &call.qubits[0], file_path)?;
            let op = Operation::custom_gate("id", vec![qid], Vec::new());
            add_operation_with_span(circuit, op, call.span, file_path)
        }

        // --- 1-Qubit Parameterized Rotation Gates ---
        "rx" | "ry" | "rz" => {
            if call.parameters.len() != 1 {
                return Err(Box::new(qasm::gate_param_arity(
                    &call.name,
                    1,
                    call.parameters.len(),
                    call.span,
                    file_path,
                )));
            }
            if call.qubits.len() != 1 {
                return Err(Box::new(qasm::gate_qubit_arity(
                    &call.name,
                    1,
                    call.qubits.len(),
                    call.span,
                    file_path,
                )));
            }
            let qid = resolve_qubit_operand(symbols, &call.qubits[0], file_path)?;
            let param = lower_parameter_expr(&call.parameters[0], env, file_path)?;
            let op = Operation::custom_gate(gate_name, vec![qid], vec![param]);
            add_operation_with_span(circuit, op, call.span, file_path)
        }

        // --- 2-Qubit Entanglers ---
        "cx" => {
            if !call.parameters.is_empty() {
                return Err(Box::new(qasm::gate_param_arity(
                    &call.name,
                    0,
                    call.parameters.len(),
                    call.span,
                    file_path,
                )));
            }
            if call.qubits.len() != 2 {
                return Err(Box::new(qasm::gate_qubit_arity(
                    &call.name,
                    2,
                    call.qubits.len(),
                    call.span,
                    file_path,
                )));
            }
            let q0 = resolve_qubit_operand(symbols, &call.qubits[0], file_path)?;
            let q1 = resolve_qubit_operand(symbols, &call.qubits[1], file_path)?;
            if q0 == q1 {
                return Err(Box::new(qasm::duplicate_qubit_operand(
                    "cx",
                    &call.qubits[1].to_string(),
                    call.qubits[1].span,
                    file_path,
                )));
            }
            // Invariant: Operand 0 is control, Operand 1 is target
            let op = Operation::gate(&Gate::CX, vec![q0, q1]);
            add_operation_with_span(circuit, op, call.span, file_path)
        }

        "cz" => {
            if !call.parameters.is_empty() {
                return Err(Box::new(qasm::gate_param_arity(
                    &call.name,
                    0,
                    call.parameters.len(),
                    call.span,
                    file_path,
                )));
            }
            if call.qubits.len() != 2 {
                return Err(Box::new(qasm::gate_qubit_arity(
                    &call.name,
                    2,
                    call.qubits.len(),
                    call.span,
                    file_path,
                )));
            }
            let q0 = resolve_qubit_operand(symbols, &call.qubits[0], file_path)?;
            let q1 = resolve_qubit_operand(symbols, &call.qubits[1], file_path)?;
            if q0 == q1 {
                return Err(Box::new(qasm::duplicate_qubit_operand(
                    "cz",
                    &call.qubits[1].to_string(),
                    call.qubits[1].span,
                    file_path,
                )));
            }
            let op = Operation::gate(&Gate::CZ, vec![q0, q1]);
            add_operation_with_span(circuit, op, call.span, file_path)
        }

        "swap" => {
            if !call.parameters.is_empty() {
                return Err(Box::new(qasm::gate_param_arity(
                    &call.name,
                    0,
                    call.parameters.len(),
                    call.span,
                    file_path,
                )));
            }
            if call.qubits.len() != 2 {
                return Err(Box::new(qasm::gate_qubit_arity(
                    &call.name,
                    2,
                    call.qubits.len(),
                    call.span,
                    file_path,
                )));
            }
            let q0 = resolve_qubit_operand(symbols, &call.qubits[0], file_path)?;
            let q1 = resolve_qubit_operand(symbols, &call.qubits[1], file_path)?;
            if q0 == q1 {
                return Err(Box::new(qasm::duplicate_qubit_operand(
                    "swap",
                    &call.qubits[1].to_string(),
                    call.qubits[1].span,
                    file_path,
                )));
            }
            let op = Operation::gate(&Gate::SWAP, vec![q0, q1]);
            add_operation_with_span(circuit, op, call.span, file_path)
        }

        // --- Unsupported 3-Qubit Gates ---
        "ccx" | "cswap" => Err(Box::new(qasm::unsupported_3q_gate(
            &call.name, call.span, file_path,
        ))),

        // --- Unsupported Standard Gates Requiring Decomposition ---
        "u1" | "u2" | "u3" | "u" | "p" | "sx" | "sxdg" | "cy" | "ch" | "crx" | "cry" | "crz"
        | "cu" | "rxx" | "ryy" | "rzz" => Err(Box::new(qasm::unsupported_gate(
            &call.name, call.span, file_path,
        ))),

        // --- Unknown Gates ---
        _ => Err(Box::new(qasm::unknown_gate(
            &call.name, call.span, None, file_path,
        ))),
    }
}

/// Lowers a measurement statement into one or more QccIR [`Operation::Measure`] operations.
///
/// Preserves:
/// - Single-bit measurements: 1 operation
/// - Whole-register measurements: expanded to $N$ operations in sequential index order ($0 \dots N-1$)
/// - RAW/WAW dependency contracts on classical wires
pub fn lower_measure_statement(
    circuit: &mut Circuit,
    measure: &AstMeasure,
    symbols: &SymbolTable,
    file_path: &str,
) -> Result<Vec<OperationId>, Box<Diagnostic>> {
    let operand_pairs = expand_measurement_operands(symbols, measure, file_path)?;
    let mut op_ids = Vec::with_capacity(operand_pairs.len());

    for (qid, cid) in operand_pairs {
        let op = Operation::measure(qid, cid);
        let op_id = add_operation_with_span(circuit, op, measure.span, file_path)?;
        op_ids.push(op_id);
    }

    Ok(op_ids)
}

/// Lowers an active reset statement into one or more QccIR [`Operation::Reset`] operations.
///
/// Preserves:
/// - Single-qubit resets: 1 operation
/// - Whole-register resets: expanded to $N$ operations in sequential index order ($0 \dots N-1$)
pub fn lower_reset_statement(
    circuit: &mut Circuit,
    reset: &AstReset,
    symbols: &SymbolTable,
    file_path: &str,
) -> Result<Vec<OperationId>, Box<Diagnostic>> {
    let qids = expand_qubits_for_reset_or_barrier(symbols, &reset.qubit, file_path)?;
    let mut op_ids = Vec::with_capacity(qids.len());

    for qid in qids {
        let op = Operation::reset(qid);
        let op_id = add_operation_with_span(circuit, op, reset.span, file_path)?;
        op_ids.push(op_id);
    }

    Ok(op_ids)
}

/// Lowers a synchronization barrier statement into a QccIR [`Operation::Barrier`] operation.
///
/// Preserves:
/// - Global barrier (`barrier;`): synchronizes all qubits declared in the circuit
/// - Selective barrier (`barrier q0, q1;`): synchronizes explicitly designated qubits
/// - Rejects duplicate qubits within the barrier (`E0023`)
pub fn lower_barrier_statement(
    circuit: &mut Circuit,
    barrier: &AstBarrier,
    symbols: &SymbolTable,
    file_path: &str,
) -> Result<OperationId, Box<Diagnostic>> {
    let qids = if barrier.is_global() {
        let total = circuit.num_qubits();
        (0..total).map(QubitId).collect()
    } else {
        let mut collected: Vec<QubitId> = Vec::new();
        for qref in &barrier.qubits {
            let expanded = expand_qubits_for_reset_or_barrier(symbols, qref, file_path)?;
            for q in expanded {
                if collected.contains(&q) {
                    return Err(Box::new(qasm::duplicate_qubit_operand(
                        "barrier",
                        &qref.to_string(),
                        qref.span,
                        file_path,
                    )));
                }
                collected.push(q);
            }
        }
        collected
    };

    let op = Operation::barrier(qids);
    add_operation_with_span(circuit, op, barrier.span, file_path)
}

/// Lowers a global phase directive (`gphase(expr);`), accumulating into the circuit's global phase.
///
/// Preserves explicit circuit global phase: $U_{\text{circuit}} = e^{i\phi} U_{\text{ops}}$.
pub fn lower_global_phase_statement(
    circuit: &mut Circuit,
    gphase: &AstGlobalPhase,
    env: &HashMap<String, f64>,
    file_path: &str,
) -> Result<(), Box<Diagnostic>> {
    let phase = lower_parameter_expr(&gphase.angle, env, file_path)?;
    circuit.add_global_phase(phase);
    Ok(())
}

/// Helper inserting an operation into the circuit and recording its source span provenance.
fn add_operation_with_span(
    circuit: &mut Circuit,
    op: Operation,
    span: crate::diagnostics::SourceSpan,
    file_path: &str,
) -> Result<OperationId, Box<Diagnostic>> {
    let op_id = circuit.add_operation(op).map_err(|err| {
        Box::new(qasm::lowering_error(
            format!("failed to insert operation into circuit DAG: {err}"),
            span,
            file_path,
        ))
    })?;

    // Record source span provenance in circuit metadata
    circuit.metadata.insert(
        format!("op.{}.span", op_id.0),
        format!(
            "{}:{}-{}:{}",
            span.start_line(),
            span.start_col(),
            span.end_line(),
            span.end_col()
        ),
    );

    Ok(op_id)
}
