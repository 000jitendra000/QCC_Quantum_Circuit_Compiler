//! Operation dispatch connecting QccIR gate AST nodes to statevector kernels.

use std::collections::HashMap;

use crate::ir::parameter::ParameterExpr;
use crate::quantum::parameter::Angle;
use crate::simulator::error::ExecutionError;
use crate::simulator::statevector::StateVector;

/// Evaluates a parameter expression to an `f64` value using an optional variable environment.
pub fn eval_parameter(
    param: &ParameterExpr,
    env: Option<&HashMap<String, f64>>,
) -> Result<f64, ExecutionError> {
    let empty_env = HashMap::new();
    let env_ref = env.unwrap_or(&empty_env);
    param.eval(env_ref).map_err(|msg| {
        if let Some(stripped) = msg.strip_prefix("unbound parameter symbol: '") {
            if let Some(sym) = stripped.strip_suffix('\'') {
                return ExecutionError::UnevaluatedParameter {
                    symbol: sym.to_string(),
                };
            }
        }
        ExecutionError::UnevaluatedParameter { symbol: msg }
    })
}

/// Dispatches a unitary gate operation to the appropriate in-place kernel on `state`.
///
/// Preserves little-endian qubit indexing and exact operand ordering:
/// - Single-qubit gates act on `qubits[0]`.
/// - CX acts with `qubits[0]` as control and `qubits[1]` as target.
/// - CZ and SWAP act symmetrically on `qubits[0]` and `qubits[1]`.
pub fn dispatch_gate(
    state: &mut StateVector,
    gate_name: &str,
    qubits: &[usize],
    parameters: &[ParameterExpr],
    env: Option<&HashMap<String, f64>>,
) -> Result<(), ExecutionError> {
    match gate_name.to_ascii_lowercase().as_str() {
        "id" | "i" => {
            if qubits.is_empty() || qubits[0] >= state.num_qubits() {
                return Err(ExecutionError::Simulator(
                    crate::simulator::error::SimulatorError::TargetOutOfBounds {
                        target: qubits.first().copied().unwrap_or(0),
                        num_qubits: state.num_qubits(),
                    },
                ));
            }
            // Explicit identity gate: no modification to statevector amplitudes
        }
        "x" => {
            state.apply_x(qubits[0])?;
        }
        "y" => {
            state.apply_y(qubits[0])?;
        }
        "z" => {
            state.apply_z(qubits[0])?;
        }
        "h" => {
            state.apply_h(qubits[0])?;
        }
        "s" => {
            state.apply_s(qubits[0])?;
        }
        "sdg" => {
            state.apply_sdg(qubits[0])?;
        }
        "t" => {
            state.apply_t(qubits[0])?;
        }
        "tdg" => {
            state.apply_tdg(qubits[0])?;
        }
        "rx" => {
            let theta = eval_parameter(&parameters[0], env)?;
            state.apply_rx(qubits[0], Angle::from_radians(theta))?;
        }
        "ry" => {
            let theta = eval_parameter(&parameters[0], env)?;
            state.apply_ry(qubits[0], Angle::from_radians(theta))?;
        }
        "rz" => {
            let theta = eval_parameter(&parameters[0], env)?;
            state.apply_rz(qubits[0], Angle::from_radians(theta))?;
        }
        "sx" => {
            state.apply_matrix2x2(qubits[0], &crate::quantum::gate_matrix::matrix_sx())?;
        }
        "u3" | "u" => {
            let theta = eval_parameter(&parameters[0], env)?;
            let phi = eval_parameter(&parameters[1], env)?;
            let lam = eval_parameter(&parameters[2], env)?;
            let mat = crate::quantum::gate_matrix::matrix_u3(
                Angle::from_radians(theta),
                Angle::from_radians(phi),
                Angle::from_radians(lam),
            );
            state.apply_matrix2x2(qubits[0], &mat)?;
        }
        "cx" => {
            state.apply_cx(qubits[0], qubits[1])?;
        }
        "cz" => {
            state.apply_cz(qubits[0], qubits[1])?;
        }
        "swap" => {
            state.apply_swap(qubits[0], qubits[1])?;
        }
        _ => {
            return Err(ExecutionError::UnsupportedGate {
                gate: gate_name.to_string(),
                reason: format!(
                    "gate '{}' has no matching statevector simulation kernel",
                    gate_name
                ),
            });
        }
    }
    Ok(())
}
