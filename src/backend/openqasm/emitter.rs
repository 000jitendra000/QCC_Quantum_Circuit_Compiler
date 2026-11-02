//! Deterministic OpenQASM 3 emitter for QccIR circuits.

use crate::backend::openqasm::errors::OpenQasmError;
use crate::backend::openqasm::format::{
    format_bit_ref, format_parameter_expr, format_qubit_ref, is_valid_identifier, is_zero_parameter,
};
use crate::ir::circuit::Circuit;
use crate::ir::node::OperationId;
use crate::ir::operation::Operation;

/// Policy controlling how circuit-level global phase is exported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GlobalPhasePolicy {
    /// Emit `gphase(<expr>);` when global phase is non-zero; emit nothing when phase is zero.
    #[default]
    EmitGPhase,
    /// Strictly reject any non-zero global phase with [`OpenQasmError::NonZeroGlobalPhase`].
    RejectNonZero,
    /// Always emit `gphase(<expr>);`, including `gphase(0);` for zero phase.
    AlwaysEmit,
}

/// Syntax format for quantum measurement statements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MeasurementSyntax {
    /// Standard OpenQASM 3 assignment syntax: `c[0] = measure q[0];`
    #[default]
    Assignment,
    /// OpenQASM 2 compatibility arrow syntax: `measure q[0] -> c[0];`
    Arrow,
}

/// Configuration options for the OpenQASM 3 emitter.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EmitterOptions {
    /// Whether to emit `include "stdgates.inc";`.
    /// - `None` (default): automatically include when standard gates are used.
    /// - `Some(true)`: always include.
    /// - `Some(false)`: never include.
    pub include_stdgates: Option<bool>,
    /// Policy for exporting circuit-level global phase.
    pub global_phase_policy: GlobalPhasePolicy,
    /// Syntax style for measurement operations.
    pub measurement_syntax: MeasurementSyntax,
    /// Whether to permit custom/non-standard gate names without returning [`OpenQasmError::UnsupportedGate`].
    pub allow_custom_gates: bool,
}

/// Deterministic OpenQASM 3 exporter.
#[derive(Clone, Debug, Default)]
pub struct OpenQasmEmitter {
    options: EmitterOptions,
}

impl OpenQasmEmitter {
    /// Creates a new emitter with default options.
    pub fn new() -> Self {
        Self {
            options: EmitterOptions::default(),
        }
    }

    /// Creates a new emitter with the provided configuration options.
    pub fn with_options(options: EmitterOptions) -> Self {
        Self { options }
    }

    /// Returns a reference to the active emitter options.
    pub fn options(&self) -> &EmitterOptions {
        &self.options
    }

    /// Returns a mutable reference to the active emitter options.
    pub fn options_mut(&mut self) -> &mut EmitterOptions {
        &mut self.options
    }

    /// Exports the provided QccIR [`Circuit`] to a deterministic OpenQASM 3 program string.
    ///
    /// # Process
    /// 1. Validates the circuit graph structure and semantics (`circuit.validate()?`).
    /// 2. Ensures the circuit contains at least one declared qubit.
    /// 3. Validates register identifiers.
    /// 4. Computes deterministic topological ordering via Kahn's algorithm.
    /// 5. Emits `OPENQASM 3.0;` header and conditional `include "stdgates.inc";`.
    /// 6. Emits quantum and classical register declarations.
    /// 7. Emits circuit-level global phase according to the configured [`GlobalPhasePolicy`].
    /// 8. Emits operations in topological order with lowercase gate names, correct operand order,
    ///    and standard measurement/reset/barrier syntax.
    ///
    /// # Errors
    /// Returns [`OpenQasmError`] if circuit validation fails, unsupported operations or gates are
    /// encountered, non-finite parameters exist, or an active policy is violated.
    pub fn emit(&self, circuit: &Circuit) -> Result<String, OpenQasmError> {
        // 1. Pre-export validation
        circuit.validate()?;

        // 2. Non-empty circuit check
        if circuit.num_qubits() == 0 {
            return Err(OpenQasmError::EmptyCircuit);
        }

        // 3. Register identifier validation
        for reg in &circuit.quantum_registers {
            if !is_valid_identifier(&reg.name) {
                return Err(OpenQasmError::InvalidIdentifier {
                    identifier: reg.name.clone(),
                });
            }
        }
        for reg in &circuit.classical_registers {
            if !is_valid_identifier(&reg.name) {
                return Err(OpenQasmError::InvalidIdentifier {
                    identifier: reg.name.clone(),
                });
            }
        }

        // 4. Deterministic topological traversal order
        let order = circuit.graph.deterministic_topological_order()?;

        // 5. Scan for standard gate usage
        let mut has_gates = false;
        for &op_id in &order {
            if let Some(node) = circuit.graph.get_node(op_id) {
                if matches!(node.operation, Operation::Gate { .. }) {
                    has_gates = true;
                    break;
                }
            }
        }

        let emit_stdgates = match self.options.include_stdgates {
            Some(explicit) => explicit,
            None => has_gates,
        };

        let mut out = String::new();

        // Header
        out.push_str("OPENQASM 3.0;\n");
        if emit_stdgates {
            out.push_str("include \"stdgates.inc\";\n");
        }
        out.push('\n');

        // Declarations
        for reg in &circuit.quantum_registers {
            out.push_str(&format!("qubit[{}] {};\n", reg.size, reg.name));
        }
        for reg in &circuit.classical_registers {
            out.push_str(&format!("bit[{}] {};\n", reg.size, reg.name));
        }

        let has_declarations =
            !circuit.quantum_registers.is_empty() || !circuit.classical_registers.is_empty();

        // Global Phase Handling
        let is_zero_phase = is_zero_parameter(&circuit.global_phase);
        let mut emitted_gphase = false;

        match self.options.global_phase_policy {
            GlobalPhasePolicy::RejectNonZero => {
                if !is_zero_phase {
                    let formatted = format_parameter_expr(&circuit.global_phase)?;
                    return Err(OpenQasmError::NonZeroGlobalPhase { phase: formatted });
                }
            }
            GlobalPhasePolicy::AlwaysEmit => {
                if has_declarations {
                    out.push('\n');
                }
                let formatted = format_parameter_expr(&circuit.global_phase)?;
                out.push_str(&format!("gphase({formatted});\n"));
                emitted_gphase = true;
            }
            GlobalPhasePolicy::EmitGPhase => {
                if !is_zero_phase {
                    if has_declarations {
                        out.push('\n');
                    }
                    let formatted = format_parameter_expr(&circuit.global_phase)?;
                    out.push_str(&format!("gphase({formatted});\n"));
                    emitted_gphase = true;
                }
            }
        }

        if has_declarations && !emitted_gphase && !order.is_empty() {
            out.push('\n');
        }

        // Operation Emission
        for &op_id in &order {
            let node = circuit.graph.get_node(op_id).ok_or(OpenQasmError::Ir(
                crate::ir::error::IrError::UnknownOperation { id: op_id },
            ))?;

            match &node.operation {
                Operation::Gate {
                    gate_name,
                    qubits,
                    parameters,
                } => {
                    let lower_name = gate_name.to_lowercase();
                    self.validate_gate(
                        &lower_name,
                        qubits.len(),
                        parameters.len(),
                        op_id,
                        self.options.allow_custom_gates,
                    )?;

                    let q_refs = qubits
                        .iter()
                        .map(|q| format_qubit_ref(*q, &circuit.quantum_registers))
                        .collect::<Result<Vec<_>, _>>()?;
                    let q_str = q_refs.join(", ");

                    if parameters.is_empty() {
                        out.push_str(&format!("{lower_name} {q_str};\n"));
                    } else {
                        let p_strs = parameters
                            .iter()
                            .map(format_parameter_expr)
                            .collect::<Result<Vec<_>, _>>()?;
                        let p_str = p_strs.join(", ");
                        out.push_str(&format!("{lower_name}({p_str}) {q_str};\n"));
                    }
                }
                Operation::Measure { qubit, destination } => {
                    let q_ref = format_qubit_ref(*qubit, &circuit.quantum_registers)?;
                    let c_ref = format_bit_ref(*destination, &circuit.classical_registers)?;
                    match self.options.measurement_syntax {
                        MeasurementSyntax::Assignment => {
                            out.push_str(&format!("{c_ref} = measure {q_ref};\n"));
                        }
                        MeasurementSyntax::Arrow => {
                            out.push_str(&format!("measure {q_ref} -> {c_ref};\n"));
                        }
                    }
                }
                Operation::Reset { qubit } => {
                    let q_ref = format_qubit_ref(*qubit, &circuit.quantum_registers)?;
                    out.push_str(&format!("reset {q_ref};\n"));
                }
                Operation::Barrier { qubits } => {
                    if qubits.is_empty() {
                        out.push_str("barrier;\n");
                    } else {
                        let q_refs = qubits
                            .iter()
                            .map(|q| format_qubit_ref(*q, &circuit.quantum_registers))
                            .collect::<Result<Vec<_>, _>>()?;
                        let q_str = q_refs.join(", ");
                        out.push_str(&format!("barrier {q_str};\n"));
                    }
                }
                Operation::Classical { name, .. } => {
                    return Err(OpenQasmError::UnsupportedOperation {
                        op_id: Some(op_id),
                        operation: "classical".to_string(),
                        reason: format!(
                            "classical operation '{name}' cannot be emitted in OpenQASM 3 at this phase"
                        ),
                    });
                }
            }
        }

        Ok(out)
    }

    fn validate_gate(
        &self,
        gate_name: &str,
        qubits: usize,
        params: usize,
        op_id: OperationId,
        allow_custom: bool,
    ) -> Result<(), OpenQasmError> {
        let (expected_qubits, expected_params) = match gate_name {
            "x" | "y" | "z" | "h" | "s" | "sdg" | "t" | "tdg" | "sx" | "sxdg" | "id" | "i" => {
                (1, 0)
            }
            "rx" | "ry" | "rz" | "p" | "u1" => (1, 1),
            "u2" => (1, 2),
            "u" | "u3" => (1, 3),
            "cx" | "cz" | "swap" | "cy" | "ch" => (2, 0),
            "cp" | "crx" | "cry" | "crz" => (2, 1),
            "ccx" | "cswap" => (3, 0),
            _ => {
                if allow_custom {
                    return Ok(());
                }
                return Err(OpenQasmError::UnsupportedGate {
                    op_id: Some(op_id),
                    gate: gate_name.to_string(),
                    reason: format!(
                        "gate '{gate_name}' is not recognized in standard OpenQASM 3 gate set"
                    ),
                });
            }
        };

        if qubits != expected_qubits {
            return Err(OpenQasmError::GateArityMismatch {
                gate: gate_name.to_string(),
                expected: expected_qubits,
                found: qubits,
            });
        }

        if params != expected_params {
            return Err(OpenQasmError::GateParameterMismatch {
                gate: gate_name.to_string(),
                expected: expected_params,
                found: params,
            });
        }

        Ok(())
    }
}

/// Convenience function exporting a [`Circuit`] with default [`EmitterOptions`].
pub fn emit_openqasm(circuit: &Circuit) -> Result<String, OpenQasmError> {
    OpenQasmEmitter::new().emit(circuit)
}
